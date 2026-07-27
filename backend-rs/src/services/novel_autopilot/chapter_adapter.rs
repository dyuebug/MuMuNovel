use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::{
        chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig,
        chapter_generation_execution_contract_service::{
            prepare_role_aware_generation_execution_config, SingleChapterGenerationCompatOptions,
            DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT,
        },
        chapter_generation_service::{
            generate_chapter_candidate_for_autopilot, ChapterGeneratedDraft, ChapterGenerationError,
        },
        cooperative_cancellation_service::CooperativeCancellationToken,
        generation_contract_service::GenerationIntentKind,
    },
    tasks::types::TaskRecord,
};

use super::{
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ChapterBusinessSnapshot, ClaimedNovelAutopilotStep, NovelAutopilotChapterGenerateCommit,
        NovelAutopilotManualReviewCandidate, NovelAutopilotRepository,
        NovelAutopilotRepositoryError, NovelAutopilotStepTerminalPatch,
    },
    router::AutopilotStepPlan,
    types::{
        NovelAutopilotHumanGateMode, NovelAutopilotQualityDecision, NovelAutopilotRunConfig,
        NovelAutopilotRunStatus, NovelAutopilotStepStatus, NovelAutopilotStepType,
    },
};

const CHAPTER_STEP_FACTS_INVALID: &str = "chapter_step_facts_invalid";
const CHAPTER_BUSINESS_DATA_CHANGED: &str = "chapter_business_data_changed";
const CHAPTER_GENERATION_ATTEMPTS_EXHAUSTED: &str = "chapter_generation_attempts_exhausted";
const CHAPTER_QUALITY_MANUAL_REVIEW: &str = "chapter_quality_manual_review";
const CHAPTER_QUALITY_RETRY: &str = "chapter_quality_retry";
const CHAPTER_QUALITY_AUTO_REPAIR: &str = "chapter_quality_auto_repair";
const CHAPTER_EXECUTION_CONFIG_FAILED: &str = "chapter_execution_config_failed";
const CHAPTER_HUMAN_GATE: &str = "chapter_human_gate";
const HUMAN_GATE_EVERY_VOLUME_BOUNDARY_UNAVAILABLE: &str =
    "human_gate_every_volume_boundary_unavailable";

#[derive(Debug)]
pub(crate) enum ChapterAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
}

impl ChapterAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "chapter_generation_cancelled",
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ChapterAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChapterQualityRoute {
    Accept,
    Retry(NovelAutopilotQualityDecision),
    ManualReview,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_chapter_generate_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    additional_guidance: Option<&str>,
    gateway_config: &ChapterCandidateRouteGatewayConfig,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<ChapterAdapterOutcome, ChapterAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let Some(chapter_id) = step.chapter_id.as_deref() else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            CHAPTER_STEP_FACTS_INVALID,
            None,
            Some(NovelAutopilotQualityDecision::ManualReview),
        )
        .await;
    };
    let Some(chapter_number) = step.chapter_number else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            CHAPTER_STEP_FACTS_INVALID,
            None,
            Some(NovelAutopilotQualityDecision::ManualReview),
        )
        .await;
    };

    let expected_chapter = ChapterBusinessSnapshot::load(db, &claimed.run.project_id, chapter_id)
        .await
        .map_err(ChapterAdapterError::Repository)?;
    if expected_chapter.chapter_number != i32::try_from(chapter_number).unwrap_or_default()
        || claimed.step.chapter_id.as_deref() != Some(chapter_id)
        || claimed.step.chapter_number != Some(expected_chapter.chapter_number)
    {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            CHAPTER_STEP_FACTS_INVALID,
            None,
            Some(NovelAutopilotQualityDecision::ManualReview),
        )
        .await;
    }

    let execution_config = match prepare_role_aware_generation_execution_config(
        db,
        &record.user_id,
        GenerationIntentKind::ChapterGenerate,
        None,
    )
    .await
    {
        Ok(execution_config) => execution_config,
        Err(_) => {
            return finish_generation_failure(
                db,
                record,
                claimed,
                step,
                config,
                CHAPTER_EXECUTION_CONFIG_FAILED,
            )
            .await;
        }
    };
    ensure_not_cancelled(cancellation_token)?;

    let generated = match generate_chapter_candidate_for_autopilot(
        db,
        &record.user_id,
        chapter_id,
        DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT,
        &SingleChapterGenerationCompatOptions::default(),
        execution_config,
        additional_guidance,
        gateway_config.clone(),
        Some(cancellation_token),
    )
    .await
    {
        Ok(generated) => generated,
        Err(ChapterGenerationError::Cancelled) => return Err(ChapterAdapterError::Cancelled),
        Err(error) => {
            tracing::warn!(
                event = "novel_book_autopilot_chapter_generation_failed",
                error_code = error.code(),
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                chapter_id,
                chapter_number,
                "durable chapter generation failed before business commit"
            );
            return finish_generation_failure(db, record, claimed, step, config, error.code())
                .await;
        }
    };
    ensure_not_cancelled(cancellation_token)?;
    output_observer.content(generated.content.clone()).await;

    match route_quality_action(generated.quality_gate_action.as_deref()) {
        ChapterQualityRoute::Accept => {
            commit_accepted_chapter(
                db,
                record,
                claimed,
                step,
                config,
                expected_chapter,
                generated,
            )
            .await
        }
        ChapterQualityRoute::Retry(decision) => {
            finish_quality_retry(db, record, claimed, step, config, generated, decision).await
        }
        ChapterQualityRoute::ManualReview => {
            persist_manual_review_candidate(db, record, claimed, step, &expected_chapter, generated)
                .await
        }
    }
}

async fn persist_manual_review_candidate(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    expected_chapter: &ChapterBusinessSnapshot,
    generated: ChapterGeneratedDraft,
) -> Result<ChapterAdapterOutcome, ChapterAdapterError> {
    let word_count = generated.word_count;
    let quality_metrics = generated.quality_metrics.clone();
    let quality_gate_action = generated.quality_gate_action.clone();
    let quality_gate_message = generated.quality_gate_message.clone();
    let terminal = NovelAutopilotRepository::persist_chapter_manual_review_candidate(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        NovelAutopilotStepType::ChapterGenerate,
        Some(&record.task_id),
        expected_chapter,
        NovelAutopilotStepStatus::Skipped,
        CHAPTER_QUALITY_MANUAL_REVIEW,
        NovelAutopilotManualReviewCandidate {
            content: generated.content,
            word_count,
            chapter_status: generated.chapter_status,
            result_digest: generated.content_digest,
            quality_metrics: quality_metrics.clone(),
            quality_gate_action: quality_gate_action.clone(),
            quality_gate_message: quality_gate_message.clone(),
        },
    )
    .await
    .map_err(ChapterAdapterError::Repository)?;

    Ok(ChapterAdapterOutcome::WaitingHuman {
        result: json!({
            "run_id": terminal.run.id,
            "run_status": terminal.run.status,
            "run_epoch": terminal.run.epoch,
            "run_version": terminal.run.version,
            "dispatch_status": "waiting_human",
            "reason_code": CHAPTER_QUALITY_MANUAL_REVIEW,
            "step_id": terminal.step.id,
            "candidate_id": terminal.step.id,
            "step_type": step.step_type,
            "step_status": terminal.step.status,
            "chapter_id": expected_chapter.chapter_id,
            "chapter_number": expected_chapter.chapter_number,
            "word_count": word_count,
            "quality_decision": NovelAutopilotQualityDecision::ManualReview,
            "quality_gate_action": quality_gate_action,
            "quality_gate_message": quality_gate_message,
            "quality_metrics": quality_metrics,
            "result_digest": terminal.step.result_digest,
        }),
    })
}

async fn commit_accepted_chapter(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    expected_chapter: ChapterBusinessSnapshot,
    generated: ChapterGeneratedDraft,
) -> Result<ChapterAdapterOutcome, ChapterAdapterError> {
    let word_count = generated.word_count;
    let quality_metrics = generated.quality_metrics.clone();
    let quality_gate_action = generated.quality_gate_action.clone();
    let human_gate_reason =
        accepted_chapter_human_gate_reason(config, claimed.run.completed_chapters);
    let target_run_status = if human_gate_reason.is_some() {
        NovelAutopilotRunStatus::WaitingHuman
    } else {
        NovelAutopilotRunStatus::Running
    };
    let committed = match NovelAutopilotRepository::commit_chapter_generate_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_chapter,
        target_run_status,
        NovelAutopilotChapterGenerateCommit {
            content: generated.content,
            word_count,
            status: generated.chapter_status,
            result_digest: generated.content_digest,
            quality_decision: NovelAutopilotQualityDecision::Accept.as_str().to_string(),
        },
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
                CHAPTER_BUSINESS_DATA_CHANGED,
                None,
                Some(NovelAutopilotQualityDecision::ManualReview),
            )
            .await;
        }
        Err(error) => return Err(ChapterAdapterError::Repository(error)),
    };

    let waiting_human = target_run_status == NovelAutopilotRunStatus::WaitingHuman;
    let result = json!({
        "run_id": committed.run.id,
        "run_status": committed.run.status,
        "run_epoch": committed.run.epoch,
        "run_version": committed.run.version,
        "dispatch_status": if waiting_human { "waiting_human" } else { "step_completed" },
        "reason_code": human_gate_reason,
        "step_id": committed.step.id,
        "step_type": step.step_type,
        "step_status": committed.step.status,
        "chapter_id": expected_chapter.chapter_id,
        "chapter_number": expected_chapter.chapter_number,
        "word_count": word_count,
        "quality_decision": NovelAutopilotQualityDecision::Accept,
        "quality_gate_action": quality_gate_action,
        "quality_metrics": quality_metrics,
        "result_digest": committed.step.result_digest,
    });
    if waiting_human {
        Ok(ChapterAdapterOutcome::WaitingHuman { result })
    } else {
        Ok(ChapterAdapterOutcome::StepCompleted {
            result,
            run: committed.run,
        })
    }
}

pub(crate) fn accepted_chapter_target_status(
    config: &NovelAutopilotRunConfig,
    completed_chapters_before_commit: i32,
) -> NovelAutopilotRunStatus {
    if accepted_chapter_human_gate_reason(config, completed_chapters_before_commit).is_some() {
        NovelAutopilotRunStatus::WaitingHuman
    } else {
        NovelAutopilotRunStatus::Running
    }
}

fn accepted_chapter_human_gate_reason(
    config: &NovelAutopilotRunConfig,
    completed_chapters_before_commit: i32,
) -> Option<&'static str> {
    let completed_after_commit = u32::try_from(completed_chapters_before_commit)
        .unwrap_or_default()
        .saturating_add(1);
    match config.human_gate_mode {
        NovelAutopilotHumanGateMode::EveryChapter => Some(CHAPTER_HUMAN_GATE),
        NovelAutopilotHumanGateMode::EveryNChapters
            if config.gate_interval > 0 && completed_after_commit % config.gate_interval == 0 =>
        {
            Some(CHAPTER_HUMAN_GATE)
        }
        NovelAutopilotHumanGateMode::EveryVolume => {
            Some(HUMAN_GATE_EVERY_VOLUME_BOUNDARY_UNAVAILABLE)
        }
        NovelAutopilotHumanGateMode::FullyAutomatic
        | NovelAutopilotHumanGateMode::HighRiskOnly
        | NovelAutopilotHumanGateMode::EveryNChapters => None,
    }
}

async fn finish_quality_retry(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    generated: ChapterGeneratedDraft,
    decision: NovelAutopilotQualityDecision,
) -> Result<ChapterAdapterOutcome, ChapterAdapterError> {
    let reason_code = match decision {
        NovelAutopilotQualityDecision::AutoRepair => CHAPTER_QUALITY_AUTO_REPAIR,
        _ => CHAPTER_QUALITY_RETRY,
    };
    if attempt_available(&claimed, config) {
        let terminal = NovelAutopilotRepository::complete_step(
            db,
            &claimed.step.id,
            &record.user_id,
            claimed.run.version,
            claimed.run.epoch,
            &step.step_key,
            Some(&record.task_id),
            NovelAutopilotStepStatus::Failed,
            NovelAutopilotStepTerminalPatch {
                result_digest: Some(generated.content_digest),
                quality_decision: Some(decision.as_str().to_string()),
                error_code: Some(reason_code.to_string()),
            },
        )
        .await
        .map_err(ChapterAdapterError::Repository)?;
        return Ok(ChapterAdapterOutcome::StepCompleted {
            result: json!({
                "run_id": terminal.run.id,
                "run_status": terminal.run.status,
                "run_epoch": terminal.run.epoch,
                "run_version": terminal.run.version,
                "dispatch_status": "retry_scheduled",
                "reason_code": reason_code,
                "step_id": terminal.step.id,
                "step_type": step.step_type,
                "step_status": terminal.step.status,
                "chapter_id": step.chapter_id,
                "chapter_number": step.chapter_number,
                "attempt": terminal.step.attempt,
                "max_step_attempts": config.max_step_attempts,
                "quality_decision": decision,
                "quality_gate_action": generated.quality_gate_action,
                "quality_metrics": generated.quality_metrics,
                "result_digest": terminal.step.result_digest,
            }),
            run: terminal.run,
        });
    }

    finish_waiting_human(
        db,
        record,
        claimed,
        step,
        NovelAutopilotStepStatus::Failed,
        CHAPTER_GENERATION_ATTEMPTS_EXHAUSTED,
        Some(generated.content_digest),
        Some(decision),
    )
    .await
}

async fn finish_generation_failure(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    reason_code: &str,
) -> Result<ChapterAdapterOutcome, ChapterAdapterError> {
    if attempt_available(&claimed, config) {
        let terminal = NovelAutopilotRepository::complete_step(
            db,
            &claimed.step.id,
            &record.user_id,
            claimed.run.version,
            claimed.run.epoch,
            &step.step_key,
            Some(&record.task_id),
            NovelAutopilotStepStatus::Failed,
            NovelAutopilotStepTerminalPatch {
                result_digest: None,
                quality_decision: Some(NovelAutopilotQualityDecision::Retry.as_str().to_string()),
                error_code: Some(reason_code.to_string()),
            },
        )
        .await
        .map_err(ChapterAdapterError::Repository)?;
        return Ok(ChapterAdapterOutcome::StepCompleted {
            result: json!({
                "run_id": terminal.run.id,
                "run_status": terminal.run.status,
                "run_epoch": terminal.run.epoch,
                "run_version": terminal.run.version,
                "dispatch_status": "retry_scheduled",
                "reason_code": reason_code,
                "step_id": terminal.step.id,
                "step_type": step.step_type,
                "step_status": terminal.step.status,
                "chapter_id": step.chapter_id,
                "chapter_number": step.chapter_number,
                "attempt": terminal.step.attempt,
                "max_step_attempts": config.max_step_attempts,
                "quality_decision": NovelAutopilotQualityDecision::Retry,
            }),
            run: terminal.run,
        });
    }

    finish_waiting_human(
        db,
        record,
        claimed,
        step,
        NovelAutopilotStepStatus::Failed,
        CHAPTER_GENERATION_ATTEMPTS_EXHAUSTED,
        None,
        Some(NovelAutopilotQualityDecision::Retry),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn finish_waiting_human(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    terminal_status: NovelAutopilotStepStatus,
    reason_code: &str,
    result_digest: Option<String>,
    quality_decision: Option<NovelAutopilotQualityDecision>,
) -> Result<ChapterAdapterOutcome, ChapterAdapterError> {
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
            quality_decision: quality_decision.map(|decision| decision.as_str().to_string()),
            error_code: Some(reason_code.to_string()),
        },
    )
    .await
    .map_err(ChapterAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(ChapterAdapterError::Repository)?;

    Ok(ChapterAdapterOutcome::WaitingHuman {
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
            "chapter_id": step.chapter_id,
            "chapter_number": step.chapter_number,
            "quality_decision": terminal.step.quality_decision,
            "result_digest": terminal.step.result_digest,
        }),
    })
}

fn attempt_available(
    claimed: &ClaimedNovelAutopilotStep,
    config: &NovelAutopilotRunConfig,
) -> bool {
    u32::try_from(claimed.step.attempt).is_ok_and(|attempt| attempt < config.max_step_attempts)
}

fn route_quality_action(action: Option<&str>) -> ChapterQualityRoute {
    match action.map(str::trim).filter(|action| !action.is_empty()) {
        None | Some("continue" | "allow_save" | "accept" | "pass" | "passed") => {
            ChapterQualityRoute::Accept
        }
        Some("auto_repair" | "repair") => {
            ChapterQualityRoute::Retry(NovelAutopilotQualityDecision::AutoRepair)
        }
        Some("retry") => ChapterQualityRoute::Retry(NovelAutopilotQualityDecision::Retry),
        Some("manual_review" | "reject") | Some(_) => ChapterQualityRoute::ManualReview,
    }
}

fn ensure_not_cancelled(
    cancellation_token: &CooperativeCancellationToken,
) -> Result<(), ChapterAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(ChapterAdapterError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{attempt_available, route_quality_action, ChapterQualityRoute};
    use crate::{
        models::{novel_autopilot_run, novel_autopilot_step_run},
        services::novel_autopilot::{
            repository::ClaimedNovelAutopilotStep,
            types::{NovelAutopilotQualityDecision, NovelAutopilotRunConfig},
        },
    };
    use chrono::NaiveDate;

    #[test]
    fn quality_actions_route_without_inventing_provider_reasoning() {
        assert_eq!(route_quality_action(None), ChapterQualityRoute::Accept);
        assert_eq!(
            route_quality_action(Some("allow_save")),
            ChapterQualityRoute::Accept
        );
        assert_eq!(
            route_quality_action(Some("repair")),
            ChapterQualityRoute::Retry(NovelAutopilotQualityDecision::AutoRepair)
        );
        assert_eq!(
            route_quality_action(Some("retry")),
            ChapterQualityRoute::Retry(NovelAutopilotQualityDecision::Retry)
        );
        assert_eq!(
            route_quality_action(Some("unknown")),
            ChapterQualityRoute::ManualReview
        );
    }

    #[test]
    fn retry_budget_uses_one_based_step_attempt() {
        let mut config = NovelAutopilotRunConfig::default();
        config.max_step_attempts = 3;
        let claimed = claimed_step(2);
        assert!(attempt_available(&claimed, &config));
        let claimed = claimed_step(3);
        assert!(!attempt_available(&claimed, &config));
    }

    fn claimed_step(attempt: i32) -> ClaimedNovelAutopilotStep {
        let now = NaiveDate::from_ymd_opt(2026, 7, 19)
            .expect("date")
            .and_hms_opt(0, 0, 0)
            .expect("time");
        ClaimedNovelAutopilotStep {
            run: novel_autopilot_run::Model {
                id: "run-1".to_string(),
                project_id: "project-1".to_string(),
                user_id: "owner-1".to_string(),
                schema_version: "novel-autopilot/v1".to_string(),
                status: "running".to_string(),
                current_phase: "writing".to_string(),
                current_step: Some("chapter:1:generate".to_string()),
                active_scope_key: Some("project-1".to_string()),
                current_chapter_id: Some("chapter-1".to_string()),
                current_chapter_number: Some(1),
                total_chapters: 1,
                completed_chapters: 0,
                failed_chapters: serde_json::json!([]),
                pending_rewrites: serde_json::json!([]),
                total_word_count: 0,
                execution_scope: "complete_book".to_string(),
                human_gate_mode: "high_risk_only".to_string(),
                gate_interval: Some(5),
                config_snapshot: serde_json::json!({}),
                max_chapters: Some(1),
                max_tokens: Some(1_000),
                max_estimated_cost: None,
                max_runtime_seconds: Some(3_600),
                used_tokens: 0,
                estimated_cost: 0.0,
                epoch: 1,
                version: 1,
                consecutive_provider_failures: 0,
                consecutive_quality_failures: 0,
                last_error_code: None,
                guidance_digest: None,
                active_background_task_id: Some("task-1".to_string()),
                final_export_ref: None,
                created_at: now,
                updated_at: now,
                started_at: Some(now),
                paused_at: None,
                completed_at: None,
            },
            step: novel_autopilot_step_run::Model {
                id: "step-1".to_string(),
                run_id: "run-1".to_string(),
                step_key: "chapter:1:generate".to_string(),
                step_type: "chapter_generate".to_string(),
                phase: "writing".to_string(),
                chapter_id: Some("chapter-1".to_string()),
                chapter_number: Some(1),
                attempt,
                run_epoch: 1,
                status: "running".to_string(),
                background_task_id: Some("task-1".to_string()),
                input_digest: "digest".to_string(),
                result_digest: None,
                quality_decision: None,
                error_code: None,
                started_at: Some(now),
                completed_at: None,
                created_at: now,
                updated_at: now,
            },
        }
    }
}

#[cfg(test)]
mod human_gate_tests {
    use super::accepted_chapter_target_status;
    use crate::services::novel_autopilot::types::{
        NovelAutopilotHumanGateMode, NovelAutopilotRunConfig, NovelAutopilotRunStatus,
    };

    #[test]
    fn accepted_chapter_gate_respects_configured_interval() {
        let mut config = NovelAutopilotRunConfig {
            human_gate_mode: NovelAutopilotHumanGateMode::EveryNChapters,
            gate_interval: 3,
            ..NovelAutopilotRunConfig::default()
        };
        assert_eq!(
            accepted_chapter_target_status(&config, 1),
            NovelAutopilotRunStatus::Running
        );
        assert_eq!(
            accepted_chapter_target_status(&config, 2),
            NovelAutopilotRunStatus::WaitingHuman
        );

        config.human_gate_mode = NovelAutopilotHumanGateMode::EveryChapter;
        assert_eq!(
            accepted_chapter_target_status(&config, 0),
            NovelAutopilotRunStatus::WaitingHuman
        );

        config.human_gate_mode = NovelAutopilotHumanGateMode::HighRiskOnly;
        assert_eq!(
            accepted_chapter_target_status(&config, 2),
            NovelAutopilotRunStatus::Running
        );

        config.human_gate_mode = NovelAutopilotHumanGateMode::EveryVolume;
        assert_eq!(
            accepted_chapter_target_status(&config, 2),
            NovelAutopilotRunStatus::WaitingHuman
        );
    }
}
