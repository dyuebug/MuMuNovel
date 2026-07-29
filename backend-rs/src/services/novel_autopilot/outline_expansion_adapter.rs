use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::{
        cooperative_cancellation_service::CooperativeCancellationToken,
        outline_expansion_generation_service::{
            generate_outline_expansion_for_autopilot, GenerateOutlineExpansion,
            GeneratedOutlineExpansion, OutlineExpansionGenerationError,
        },
    },
    tasks::types::TaskRecord,
};

use super::{
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotExpandedChapterCommit,
        NovelAutopilotOutlineExpansionCommit, NovelAutopilotOutlineSnapshot,
        NovelAutopilotRepository, NovelAutopilotRepositoryError, NovelAutopilotStepTerminalPatch,
    },
    router::AutopilotStepPlan,
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus},
};

const OUTLINE_ALREADY_EXPANDED: &str = "outline_already_expanded";
const OUTLINE_EXPANSION_STEP_INVALID: &str = "outline_expansion_step_invalid";
const OUTLINE_EXPANSION_INCOMPLETE: &str = "outline_expansion_incomplete";
const OUTLINE_EXPANSION_BUSINESS_DATA_CHANGED: &str = "outline_expansion_business_data_changed";
const DEFAULT_EXPANSION_STRATEGY: &str = "balanced";

#[derive(Debug)]
pub(crate) enum OutlineExpansionAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
}

impl OutlineExpansionAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "outline_expansion_cancelled",
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum OutlineExpansionAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_outline_expansion_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    additional_guidance: Option<&str>,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<OutlineExpansionAdapterOutcome, OutlineExpansionAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let Some(outline_id) = step.outline_id.as_deref() else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            OUTLINE_EXPANSION_STEP_INVALID,
            None,
        )
        .await;
    };
    let Some(target_chapter_count) = step
        .target_chapter_count
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
    else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            OUTLINE_EXPANSION_STEP_INVALID,
            None,
        )
        .await;
    };

    let expected_outline = NovelAutopilotOutlineSnapshot::load(db, &claimed.run.project_id)
        .await
        .map_err(OutlineExpansionAdapterError::Repository)?;
    if !expected_outline.contains_outline(outline_id) {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            OUTLINE_EXPANSION_STEP_INVALID,
            None,
        )
        .await;
    }
    if expected_outline.has_chapters_for_outline(outline_id) {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Skipped,
            OUTLINE_ALREADY_EXPANDED,
            None,
        )
        .await;
    }

    let generated = match generate_outline_expansion_for_autopilot(
        db,
        GenerateOutlineExpansion {
            user_id: &record.user_id,
            outline_id,
            target_chapter_count,
            expansion_strategy: DEFAULT_EXPANSION_STRATEGY,
            enable_scene_analysis: true,
            provider_override: None,
            model_override: None,
        },
        additional_guidance,
        Some(cancellation_token),
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
        Err(OutlineExpansionGenerationError::Cancelled) => {
            return Err(OutlineExpansionAdapterError::Cancelled);
        }
        Err(error) => {
            let error_code = error.code();
            tracing::warn!(
                event = "novel_book_autopilot_outline_expansion_failed",
                error_code,
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                outline_id,
                "durable outline expansion failed before business commit"
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
    if generated.project_id != claimed.run.project_id
        || generated.outline_id != outline_id
        || generated.chapter_plans.len() != target_chapter_count
    {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            OUTLINE_EXPANSION_INCOMPLETE,
            Some(generated.result_digest),
        )
        .await;
    }

    let provider = generated.provider.clone();
    let model = generated.model.clone();
    let chapter_count = generated.chapter_plans.len();
    let Some(expansion_commit) = expansion_commit(&generated) else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            OUTLINE_EXPANSION_INCOMPLETE,
            Some(generated.result_digest),
        )
        .await;
    };
    let committed = match NovelAutopilotRepository::commit_outline_expansion_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_outline,
        expansion_commit,
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
                OUTLINE_EXPANSION_BUSINESS_DATA_CHANGED,
                None,
            )
            .await;
        }
        Err(error) => return Err(OutlineExpansionAdapterError::Repository(error)),
    };

    Ok(OutlineExpansionAdapterOutcome::StepCompleted {
        result: json!({
            "run_id": committed.run.id,
            "run_status": committed.run.status,
            "run_epoch": committed.run.epoch,
            "run_version": committed.run.version,
            "dispatch_status": "step_completed",
            "step_id": committed.step.id,
            "step_type": step.step_type,
            "step_status": committed.step.status,
            "outline_id": outline_id,
            "chapter_count": chapter_count,
            "provider": provider,
            "model": model,
            "result_digest": committed.step.result_digest,
        }),
        run: committed.run,
    })
}

fn expansion_commit(
    generated: &GeneratedOutlineExpansion,
) -> Option<NovelAutopilotOutlineExpansionCommit> {
    let chapters = generated
        .chapter_plans
        .iter()
        .map(|plan| {
            let title = plan.get("title")?.as_str()?.trim().to_string();
            let summary = plan.get("plot_summary")?.as_str()?.trim().to_string();
            let sub_index = plan
                .get("sub_index")?
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())?;
            if title.is_empty() || summary.is_empty() || sub_index <= 0 {
                return None;
            }
            Some(NovelAutopilotExpandedChapterCommit {
                title,
                summary,
                sub_index,
                expansion_plan: serde_json::to_string(plan).ok()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if chapters.is_empty() {
        return None;
    }
    Some(NovelAutopilotOutlineExpansionCommit {
        outline_id: generated.outline_id.clone(),
        chapters,
        result_digest: generated.result_digest.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_commit_preserves_normalized_plan() {
        let plan = json!({
            "outline_id": "outline-1",
            "sub_index": 1,
            "title": "第一章",
            "plot_summary": "主角进入遗迹。",
            "narrative_goal": "建立冲突",
            "key_events": ["进入遗迹"],
            "character_focus": ["主角"],
            "estimated_words": 3000,
        });
        let generated = GeneratedOutlineExpansion {
            project_id: "project-1".to_string(),
            outline_id: "outline-1".to_string(),
            chapter_plans: vec![plan.clone()],
            provider: "mock".to_string(),
            model: "mock-model".to_string(),
            result_digest: "sha256:test".to_string(),
        };

        let commit = expansion_commit(&generated).expect("normalized plan should commit");

        assert_eq!(commit.outline_id, "outline-1");
        assert_eq!(commit.result_digest, "sha256:test");
        assert_eq!(commit.chapters.len(), 1);
        assert_eq!(commit.chapters[0].title, "第一章");
        assert_eq!(commit.chapters[0].summary, "主角进入遗迹。");
        assert_eq!(commit.chapters[0].sub_index, 1);
        assert_eq!(
            serde_json::from_str::<Value>(&commit.chapters[0].expansion_plan)
                .expect("serialized expansion plan"),
            plan
        );
    }
}

async fn finish_waiting_human(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    terminal_status: NovelAutopilotStepStatus,
    reason_code: &str,
    result_digest: Option<String>,
) -> Result<OutlineExpansionAdapterOutcome, OutlineExpansionAdapterError> {
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
    .map_err(OutlineExpansionAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(OutlineExpansionAdapterError::Repository)?;

    Ok(OutlineExpansionAdapterOutcome::WaitingHuman {
        result: json!({
            "run_id": waiting.id,
            "run_status": waiting.status,
            "run_epoch": waiting.epoch,
            "run_version": waiting.version,
            "dispatch_status": "waiting_human",
            "reason_code": reason_code,
            "step_id": terminal.step.id,
            "step_type": step.step_type,
            "step_status": terminal.step.status,
        }),
    })
}

fn ensure_not_cancelled(
    cancellation_token: &CooperativeCancellationToken,
) -> Result<(), OutlineExpansionAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(OutlineExpansionAdapterError::Cancelled)
    } else {
        Ok(())
    }
}
