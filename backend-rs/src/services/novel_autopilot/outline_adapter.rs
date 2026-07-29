use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::{novel_autopilot_run, project},
    services::{
        cooperative_cancellation_service::CooperativeCancellationToken,
        project_service::ProjectService,
        wizard_outline_generation_service::{
            generate_outline_plan_for_project_with_guidance, GenerateOutlinePlanForProject,
            GeneratedOutlineItem, GeneratedOutlinePlan, WizardOutlineGenerationError,
        },
    },
    tasks::types::TaskRecord,
};

use super::{
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotOutlineCommit, NovelAutopilotOutlineItemCommit,
        NovelAutopilotOutlineSnapshot, NovelAutopilotPendingChapterCommit,
        NovelAutopilotRepository, NovelAutopilotRepositoryError, NovelAutopilotStepTerminalPatch,
    },
    router::AutopilotStepPlan,
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus},
};

const MAX_OUTLINES_PER_STEP: usize = 10;
const OUTLINE_MANUAL_CONTENT_PRESENT: &str = "outline_manual_content_present";
const OUTLINE_GENERATION_INCOMPLETE: &str = "outline_generation_incomplete";
const OUTLINE_BUSINESS_DATA_CHANGED: &str = "outline_business_data_changed";

#[derive(Debug)]
pub(crate) enum OutlineAdapterError {
    Cancelled,
    ProjectRead,
    Repository(NovelAutopilotRepositoryError),
}

impl OutlineAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "outline_generation_cancelled",
            Self::ProjectRead => "project_read_failed",
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum OutlineAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_outline_design_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    additional_guidance: Option<&str>,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<OutlineAdapterOutcome, OutlineAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let project = ProjectService::get(db, &claimed.run.project_id, &record.user_id)
        .await
        .map_err(|_| OutlineAdapterError::ProjectRead)?
        .ok_or(OutlineAdapterError::ProjectRead)?;
    let expected_outline = NovelAutopilotOutlineSnapshot::load(db, &claimed.run.project_id)
        .await
        .map_err(OutlineAdapterError::Repository)?;

    if !expected_outline.is_blank() {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Skipped,
            OUTLINE_MANUAL_CONTENT_PRESENT,
            None,
        )
        .await;
    }

    let generated = match generate_outline_plan_for_project_with_guidance(
        db,
        outline_generation_request(record, &claimed, &project),
        additional_guidance,
        Some(cancellation_token),
        |progress| async move {
            tracing::debug!(
                event = "novel_book_autopilot_outline_progress",
                progress = progress.progress,
                status = progress.status,
                message = %progress.message,
                "durable outline generation progress updated"
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
        Err(WizardOutlineGenerationError::Cancelled) => {
            return Err(OutlineAdapterError::Cancelled);
        }
        Err(error) => {
            let error_code = error.code();
            tracing::warn!(
                event = "novel_book_autopilot_outline_generation_failed",
                error_code,
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                "durable outline generation failed before business commit"
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
    let Some(outline_commit) = outline_commit(&generated, &project) else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            OUTLINE_GENERATION_INCOMPLETE,
            Some(generated.content_digest),
        )
        .await;
    };

    let provider = generated.provider;
    let model = generated.model;
    let attempts = generated.attempts;
    let outline_count = generated.outlines.len();
    let pending_chapter_count = generated.suggested_pending_chapters.len();
    let committed = match NovelAutopilotRepository::commit_outline_design_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_outline,
        outline_commit,
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
                OUTLINE_BUSINESS_DATA_CHANGED,
                None,
            )
            .await;
        }
        Err(error) => return Err(OutlineAdapterError::Repository(error)),
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
        "outline_count": outline_count,
        "pending_chapter_count": pending_chapter_count,
        "result_digest": committed.step.result_digest,
    });
    Ok(OutlineAdapterOutcome::StepCompleted {
        result,
        run: committed.run,
    })
}

fn outline_generation_request<'a>(
    record: &'a TaskRecord,
    claimed: &'a ClaimedNovelAutopilotStep,
    project: &'a project::Model,
) -> GenerateOutlinePlanForProject<'a> {
    let chapter_count = usize::try_from(claimed.run.total_chapters)
        .unwrap_or(1)
        .clamp(1, MAX_OUTLINES_PER_STEP);

    GenerateOutlinePlanForProject {
        user_id: &record.user_id,
        project_id: &claimed.run.project_id,
        chapter_count,
        narrative_perspective: project.narrative_perspective.as_deref(),
        target_words: project.target_words,
        requirements: None,
        creative_mode: project.default_creative_mode.as_deref(),
        story_focus: project.default_story_focus.as_deref(),
        plot_stage: project.default_plot_stage.as_deref(),
        story_creation_brief: project.default_story_creation_brief.as_deref(),
        quality_preset: project.default_quality_preset.as_deref(),
        quality_notes: project.default_quality_notes.as_deref(),
        compact_mode: false,
        provider_override: None,
        model_override: None,
    }
}

fn outline_commit(
    generated: &GeneratedOutlinePlan,
    project: &project::Model,
) -> Option<NovelAutopilotOutlineCommit> {
    if generated.outlines.is_empty() {
        return None;
    }

    let outlines = generated
        .outlines
        .iter()
        .map(outline_item_commit)
        .collect::<Option<Vec<_>>>()?;
    let pending_chapters = generated
        .suggested_pending_chapters
        .iter()
        .map(|chapter| NovelAutopilotPendingChapterCommit {
            chapter_number: chapter.chapter_number,
            title: chapter.title.clone(),
            summary: chapter.summary.clone(),
            outline_index: chapter.outline_index,
        })
        .collect();

    Some(NovelAutopilotOutlineCommit {
        outlines,
        pending_chapters,
        outline_mode: generated.outline_mode.clone(),
        narrative_perspective: project.narrative_perspective.clone(),
        target_words: project.target_words,
        result_digest: generated.content_digest.clone(),
    })
}

fn outline_item_commit(
    generated: &GeneratedOutlineItem,
) -> Option<NovelAutopilotOutlineItemCommit> {
    let structure = json!({
        "chapter_number": generated.chapter_number,
        "title": generated.title,
        "summary": generated.summary,
        "content": generated.content,
        "scenes": generated.scenes,
        "characters": generated.characters.iter().map(|character| json!({
            "name": character.name,
            "type": character.character_type,
        })).collect::<Vec<_>>(),
        "key_points": generated.key_points,
        "emotion": generated.emotion,
        "narrative_goal": generated.narrative_goal,
        "conflict_line": generated.conflict_line,
        "decision": generated.decision,
        "cost": generated.cost,
        "rule_impact": generated.rule_impact,
        "dialogue_hook": generated.dialogue_hook,
        "character_turns": generated.character_turns,
        "suggested_target_words": generated.suggested_target_words,
    });

    Some(NovelAutopilotOutlineItemCommit {
        title: generated.title.clone(),
        content: generated.content.clone(),
        structure: serde_json::to_string(&structure).ok()?,
        order_index: generated.chapter_number,
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
) -> Result<OutlineAdapterOutcome, OutlineAdapterError> {
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
    .map_err(OutlineAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(OutlineAdapterError::Repository)?;

    Ok(OutlineAdapterOutcome::WaitingHuman {
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
) -> Result<(), OutlineAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(OutlineAdapterError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{outline_commit, outline_item_commit};
    use crate::{
        models::project,
        services::wizard_outline_generation_service::{
            GeneratedOutlineCharacterRef, GeneratedOutlineItem, GeneratedOutlinePlan,
            GeneratedPendingChapter,
        },
    };

    fn project() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "owner-1".to_string(),
            title: "星门".to_string(),
            description: Some("远征故事".to_string()),
            theme: Some("选择".to_string()),
            genre: Some("奇幻".to_string()),
            target_words: 3000,
            current_words: 0,
            status: "draft".to_string(),
            wizard_status: "in_progress".to_string(),
            wizard_step: 6,
            outline_mode: "one-to-one".to_string(),
            world_time_period: Some("新历".to_string()),
            world_location: Some("浮空城".to_string()),
            world_atmosphere: Some("紧张".to_string()),
            world_rules: Some("星门需要代价".to_string()),
            chapter_count: Some(1),
            narrative_perspective: Some("第三人称".to_string()),
            character_count: 1,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            updated_at: None,
        }
    }

    fn generated_item() -> GeneratedOutlineItem {
        GeneratedOutlineItem {
            chapter_number: 1,
            title: "星门异动".to_string(),
            summary: "主角发现星门异常".to_string(),
            content: "主角发现星门异常，并被迫作出选择。".to_string(),
            scenes: vec!["城墙".to_string()],
            characters: vec![GeneratedOutlineCharacterRef {
                name: "林舟".to_string(),
                character_type: Some("character".to_string()),
            }],
            key_points: vec!["发现异常".to_string()],
            emotion: Some("紧张".to_string()),
            narrative_goal: Some("查明来源".to_string()),
            conflict_line: None,
            decision: Some("进入星门".to_string()),
            cost: None,
            rule_impact: None,
            dialogue_hook: None,
            character_turns: Vec::new(),
            suggested_target_words: Some(3000),
        }
    }

    #[test]
    fn generated_outline_maps_to_repository_commit() {
        let generated = GeneratedOutlinePlan {
            outlines: vec![generated_item()],
            outline_mode: "one-to-one".to_string(),
            suggested_pending_chapters: vec![GeneratedPendingChapter {
                chapter_number: 1,
                title: "星门异动".to_string(),
                summary: "主角发现星门异常".to_string(),
                outline_index: 0,
            }],
            provider: "provider".to_string(),
            model: "model".to_string(),
            attempts: 1,
            content_digest: "digest".to_string(),
        };

        let commit = outline_commit(&generated, &project()).expect("complete outline commit");
        assert_eq!(commit.outlines.len(), 1);
        assert_eq!(commit.pending_chapters.len(), 1);
        assert_eq!(commit.result_digest, "digest");
        assert_eq!(commit.outline_mode, "one-to-one");
    }

    #[test]
    fn outline_structure_preserves_legacy_character_type_key() {
        let commit = outline_item_commit(&generated_item()).expect("outline item commit");
        let structure: serde_json::Value =
            serde_json::from_str(&commit.structure).expect("valid structure json");

        assert_eq!(structure["characters"][0]["type"], "character");
        assert!(structure["characters"][0].get("character_type").is_none());
    }

    #[test]
    fn empty_generation_is_rejected_before_repository_commit() {
        let generated = GeneratedOutlinePlan {
            outlines: Vec::new(),
            outline_mode: "detail".to_string(),
            suggested_pending_chapters: Vec::new(),
            provider: "provider".to_string(),
            model: "model".to_string(),
            attempts: 1,
            content_digest: "digest".to_string(),
        };

        assert!(outline_commit(&generated, &project()).is_none());
    }
}
