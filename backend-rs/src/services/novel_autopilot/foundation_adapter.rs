use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::{
        cooperative_cancellation_service::CooperativeCancellationToken,
        foundation_generation_service::{
            generate_foundation_for_project_with_guidance, FoundationGenerationError,
            GenerateFoundationForProject, GeneratedFoundation,
        },
        project_service::ProjectService,
    },
    tasks::types::TaskRecord,
};

use super::{
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotFoundationCommit,
        NovelAutopilotFoundationSnapshot, NovelAutopilotRepository, NovelAutopilotRepositoryError,
        NovelAutopilotStepTerminalPatch,
    },
    router::AutopilotStepPlan,
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus},
};

const FOUNDATION_MANUAL_CONTENT_PRESENT: &str = "foundation_manual_content_present";
const FOUNDATION_GENERATION_INCOMPLETE: &str = "foundation_generation_incomplete";
const FOUNDATION_BUSINESS_DATA_CHANGED: &str = "foundation_business_data_changed";

#[derive(Debug)]
pub(crate) enum FoundationAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
    ProjectRead,
}

impl FoundationAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "foundation_generation_cancelled",
            Self::Repository(error) => error.code(),
            Self::ProjectRead => "project_read_failed",
        }
    }
}

#[derive(Debug)]
pub(crate) enum FoundationAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_foundation_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    additional_guidance: Option<&str>,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<FoundationAdapterOutcome, FoundationAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let project = ProjectService::get(db, &claimed.run.project_id, &record.user_id)
        .await
        .map_err(|_| FoundationAdapterError::ProjectRead)?
        .ok_or(FoundationAdapterError::ProjectRead)?;
    let expected_foundation = NovelAutopilotFoundationSnapshot::from_project(&project);

    if expected_foundation.is_complete() {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Skipped,
            FOUNDATION_MANUAL_CONTENT_PRESENT,
            None,
        )
        .await;
    }

    let generated = match generate_foundation_for_project_with_guidance(
        db,
        GenerateFoundationForProject {
            user_id: &record.user_id,
            project_id: &claimed.run.project_id,
            provider_override: None,
            model_override: None,
        },
        additional_guidance,
        Some(cancellation_token),
        |progress| async move {
            tracing::debug!(
                event = "novel_book_autopilot_foundation_progress",
                progress = progress.progress,
                status = progress.status,
                message = %progress.message,
                "durable foundation generation progress updated"
            );
            Ok(())
        },
        {
            let output_observer = output_observer.clone();
            move |content| {
                let output_observer = output_observer.clone();
                async move {
                    output_observer.content(content).await;
                    Ok(())
                }
            }
        },
        {
            let output_observer = output_observer.clone();
            move |reasoning| {
                let output_observer = output_observer.clone();
                async move {
                    output_observer.reasoning(reasoning).await;
                    Ok(())
                }
            }
        },
    )
    .await
    {
        Ok(generated) => generated,
        Err(FoundationGenerationError::Cancelled) => return Err(FoundationAdapterError::Cancelled),
        Err(error) => {
            let error_code = error.code();
            tracing::warn!(
                event = "novel_book_autopilot_foundation_generation_failed",
                error_code,
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                "durable foundation generation failed before business commit"
            );
            return finish_waiting_human(
                db,
                record,
                claimed,
                step,
                NovelAutopilotStepStatus::Failed,
                error_code,
                None,
            )
            .await;
        }
    };

    ensure_not_cancelled(cancellation_token)?;
    let Some(foundation_commit) = foundation_commit(&generated) else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            FOUNDATION_GENERATION_INCOMPLETE,
            Some(generated.content_digest),
        )
        .await;
    };

    let provider = generated.provider;
    let model = generated.model;
    let attempts = generated.attempts;
    let committed = match NovelAutopilotRepository::commit_foundation_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_foundation,
        foundation_commit,
    )
    .await
    {
        Ok(committed) => committed,
        Err(NovelAutopilotRepositoryError::BusinessDataChanged) => {
            return finish_waiting_human(
                db,
                record,
                claimed,
                step,
                NovelAutopilotStepStatus::Skipped,
                FOUNDATION_BUSINESS_DATA_CHANGED,
                None,
            )
            .await;
        }
        Err(error) => return Err(FoundationAdapterError::Repository(error)),
    };

    let result = json!({
        "run_id": committed.run.id,
        "run_status": committed.run.status,
        "run_epoch": committed.run.epoch,
        "run_version": committed.run.version,
        "dispatch_status": "step_completed",
        "step_id": committed.step.id,
        "step_type": step.step_type,
        "step_status": committed.step.status,
        "provider": provider,
        "model": model,
        "attempts": attempts,
        "result_digest": committed.step.result_digest,
    });
    Ok(FoundationAdapterOutcome::StepCompleted {
        result,
        run: committed.run,
    })
}

fn foundation_commit(generated: &GeneratedFoundation) -> Option<NovelAutopilotFoundationCommit> {
    if !generated.is_complete() {
        return None;
    }
    Some(NovelAutopilotFoundationCommit {
        title: generated.title.clone(),
        description: generated.description.clone(),
        theme: generated.theme.clone(),
        genre: generated.genre.join(","),
        narrative_perspective: generated.narrative_perspective.clone(),
        result_digest: generated.content_digest.clone(),
    })
}

async fn finish_waiting_human(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    terminal_status: NovelAutopilotStepStatus,
    reason_code: &str,
    result_digest: Option<String>,
) -> Result<FoundationAdapterOutcome, FoundationAdapterError> {
    let terminal = NovelAutopilotRepository::complete_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        terminal_status,
        NovelAutopilotStepTerminalPatch {
            result_digest,
            quality_decision: None,
            error_code: Some(reason_code.to_string()),
        },
    )
    .await
    .map_err(FoundationAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(FoundationAdapterError::Repository)?;

    Ok(FoundationAdapterOutcome::WaitingHuman {
        result: waiting_human_view(&waiting, &terminal, step, reason_code),
    })
}

fn waiting_human_view(
    run: &novel_autopilot_run::Model,
    terminal: &ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    reason_code: &str,
) -> Value {
    json!({
        "run_id": run.id,
        "run_status": run.status,
        "run_epoch": run.epoch,
        "run_version": run.version,
        "dispatch_status": "waiting_human",
        "reason_code": reason_code,
        "step_id": terminal.step.id,
        "step_type": step.step_type,
        "step_status": terminal.step.status,
    })
}

fn ensure_not_cancelled(
    cancellation_token: &CooperativeCancellationToken,
) -> Result<(), FoundationAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(FoundationAdapterError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::foundation_commit;
    use crate::services::foundation_generation_service::GeneratedFoundation;

    #[test]
    fn complete_generated_foundation_maps_to_repository_commit() {
        let generated = GeneratedFoundation {
            title: "雾钟封港".to_string(),
            description: "少女必须在天亮前查清父亲失踪的真相。".to_string(),
            theme: "真相与守护的代价".to_string(),
            genre: vec!["悬疑".to_string(), "都市".to_string()],
            narrative_perspective: "第三人称".to_string(),
            provider: "provider".to_string(),
            model: "model".to_string(),
            attempts: 1,
            content_digest: "digest".to_string(),
        };

        let commit = foundation_commit(&generated).expect("complete foundation maps to commit");
        assert_eq!(commit.title, "雾钟封港");
        assert_eq!(commit.genre, "悬疑,都市");
        assert_eq!(commit.result_digest, "digest");
    }

    #[test]
    fn incomplete_generated_foundation_never_maps_to_repository_commit() {
        let generated = GeneratedFoundation {
            title: "".to_string(),
            description: "简介".to_string(),
            theme: "主题".to_string(),
            genre: vec!["悬疑".to_string()],
            narrative_perspective: "第三人称".to_string(),
            provider: "provider".to_string(),
            model: "model".to_string(),
            attempts: 3,
            content_digest: "digest".to_string(),
        };

        assert!(foundation_commit(&generated).is_none());
    }
}
