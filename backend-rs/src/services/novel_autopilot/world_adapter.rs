use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::{
        cooperative_cancellation_service::CooperativeCancellationToken,
        project_service::ProjectService,
        wizard_world_generation_service::{
            generate_world_building_for_project_with_guidance, GenerateWorldBuildingForProject,
            GeneratedWorldBuilding, WizardWorldGenerationError, WorldGenerationFailurePolicy,
        },
    },
    tasks::types::TaskRecord,
};

use super::{
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
        NovelAutopilotStepTerminalPatch, NovelAutopilotWorldCommit, NovelAutopilotWorldSnapshot,
    },
    router::AutopilotStepPlan,
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus},
};

const WORLD_MANUAL_CONTENT_PRESENT: &str = "world_building_manual_content_present";
const WORLD_GENERATION_INCOMPLETE: &str = "world_generation_incomplete";
const WORLD_BUSINESS_DATA_CHANGED: &str = "world_building_business_data_changed";

#[derive(Debug)]
pub(crate) enum WorldAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
    ProjectRead,
}

impl WorldAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "world_generation_cancelled",
            Self::Repository(error) => error.code(),
            Self::ProjectRead => "project_read_failed",
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorldAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_world_building_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    additional_guidance: Option<&str>,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<WorldAdapterOutcome, WorldAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let project = ProjectService::get(db, &claimed.run.project_id, &record.user_id)
        .await
        .map_err(|_| WorldAdapterError::ProjectRead)?
        .ok_or(WorldAdapterError::ProjectRead)?;
    let expected_world = NovelAutopilotWorldSnapshot::from_project(&project);

    if !expected_world.is_blank() {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Skipped,
            WORLD_MANUAL_CONTENT_PRESENT,
            None,
        )
        .await;
    }

    let generated = match generate_world_building_for_project_with_guidance(
        db,
        GenerateWorldBuildingForProject {
            user_id: &record.user_id,
            project_id: &claimed.run.project_id,
            provider_override: None,
            model_override: None,
            failure_policy: WorldGenerationFailurePolicy::ReturnError,
        },
        additional_guidance,
        Some(cancellation_token),
        |progress| async move {
            tracing::debug!(
                event = "novel_book_autopilot_world_progress",
                progress = progress.progress,
                status = progress.status,
                message = %progress.message,
                "durable world generation progress updated"
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
        Err(WizardWorldGenerationError::Cancelled) => return Err(WorldAdapterError::Cancelled),
        Err(error) => {
            let error_code = error.code();
            tracing::warn!(
                event = "novel_book_autopilot_world_generation_failed",
                error_code,
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                "durable world generation failed before business commit"
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
    let Some(world_commit) = world_commit(&generated) else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            WORLD_GENERATION_INCOMPLETE,
            Some(generated.content_digest),
        )
        .await;
    };

    let provider = generated.provider;
    let model = generated.model;
    let attempts = generated.attempts;
    let committed = match NovelAutopilotRepository::commit_world_building_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_world,
        world_commit,
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
                WORLD_BUSINESS_DATA_CHANGED,
                None,
            )
            .await;
        }
        Err(error) => return Err(WorldAdapterError::Repository(error)),
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
    Ok(WorldAdapterOutcome::StepCompleted {
        result,
        run: committed.run,
    })
}

fn world_commit(generated: &GeneratedWorldBuilding) -> Option<NovelAutopilotWorldCommit> {
    if !generated.is_complete() {
        return None;
    }
    Some(NovelAutopilotWorldCommit {
        time_period: generated.time_period.clone()?,
        location: generated.location.clone()?,
        atmosphere: generated.atmosphere.clone()?,
        rules: generated.rules.clone()?,
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
) -> Result<WorldAdapterOutcome, WorldAdapterError> {
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
    .map_err(WorldAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(WorldAdapterError::Repository)?;

    Ok(WorldAdapterOutcome::WaitingHuman {
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
) -> Result<(), WorldAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(WorldAdapterError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::world_commit;
    use crate::services::wizard_world_generation_service::GeneratedWorldBuilding;

    #[test]
    fn complete_generated_world_maps_to_repository_commit() {
        let generated = GeneratedWorldBuilding {
            time_period: Some("蒸汽纪元".to_string()),
            location: Some("浮空群岛".to_string()),
            atmosphere: Some("工业与秘术并存".to_string()),
            rules: Some("记忆可以作为燃料".to_string()),
            provider: "provider".to_string(),
            model: "model".to_string(),
            attempts: 1,
            used_compatibility_placeholder: false,
            content_digest: "digest".to_string(),
        };

        let commit = world_commit(&generated).expect("complete world maps to commit");
        assert_eq!(commit.time_period, "蒸汽纪元");
        assert_eq!(commit.result_digest, "digest");
    }

    #[test]
    fn placeholder_world_never_maps_to_repository_commit() {
        let generated = GeneratedWorldBuilding {
            time_period: Some("生成失败".to_string()),
            location: Some("生成失败".to_string()),
            atmosphere: Some("生成失败".to_string()),
            rules: Some("生成失败".to_string()),
            provider: "provider".to_string(),
            model: "model".to_string(),
            attempts: 3,
            used_compatibility_placeholder: true,
            content_digest: "digest".to_string(),
        };

        assert!(world_commit(&generated).is_none());
    }
}
