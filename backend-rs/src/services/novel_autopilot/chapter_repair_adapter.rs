use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::{
        chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig,
        chapter_generation_execution_contract_service::prepare_role_aware_generation_execution_config,
        chapter_repair_generation_service::{
            generate_chapter_repair_candidate_for_autopilot, ChapterRepairCandidate,
            ChapterRepairGenerationError,
        },
        cooperative_cancellation_service::CooperativeCancellationToken,
        generation_contract_service::GenerationIntentKind,
    },
    tasks::types::TaskRecord,
};

use super::{
    chapter_repair_repository::{
        NovelAutopilotChapterRepairCommit, NovelAutopilotChapterRepairFailureEvidence,
    },
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ChapterBusinessSnapshot, ClaimedNovelAutopilotStep, NovelAutopilotManualReviewCandidate,
        NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    router::AutopilotStepPlan,
    types::{
        NovelAutopilotQualityDecision, NovelAutopilotRunConfig, NovelAutopilotStepStatus,
        NovelAutopilotStepType,
    },
};

const CHAPTER_REPAIR_FACTS_INVALID: &str = "chapter_repair_step_facts_invalid";
const CHAPTER_REPAIR_EXECUTION_CONFIG_FAILED: &str = "chapter_repair_execution_config_failed";
const CHAPTER_REPAIR_PROVIDER_FAILED: &str = "chapter_repair_provider_failed";
const CHAPTER_REPAIR_QUALITY_RETRY: &str = "chapter_repair_quality_retry";
const CHAPTER_REPAIR_QUALITY_AUTO_REPAIR: &str = "chapter_repair_quality_auto_repair";
const CHAPTER_REPAIR_MANUAL_REVIEW: &str = "chapter_repair_manual_review";
const CHAPTER_REPAIR_BUSINESS_DATA_CHANGED: &str = "chapter_repair_business_data_changed";

#[derive(Debug)]
pub(crate) enum ChapterRepairAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
}

impl ChapterRepairAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "chapter_repair_cancelled",
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ChapterRepairAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChapterRepairQualityRoute {
    Accept,
    Retry(NovelAutopilotQualityDecision),
    ManualReview,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_chapter_repair_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    additional_guidance: Option<&str>,
    gateway_config: &ChapterCandidateRouteGatewayConfig,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<ChapterRepairAdapterOutcome, ChapterRepairAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let (Some(chapter_id), Some(chapter_number)) =
        (step.chapter_id.as_deref(), step.chapter_number)
    else {
        return finish_failure(
            db,
            record,
            claimed,
            step,
            CHAPTER_REPAIR_FACTS_INVALID,
            true,
            true,
            NovelAutopilotQualityDecision::ManualReview,
            None,
        )
        .await;
    };

    let expected_chapter = ChapterBusinessSnapshot::load(db, &claimed.run.project_id, chapter_id)
        .await
        .map_err(ChapterRepairAdapterError::Repository)?;
    if expected_chapter.chapter_number != i32::try_from(chapter_number).unwrap_or_default()
        || expected_chapter
            .content
            .as_deref()
            .is_none_or(|content| content.trim().is_empty())
        || claimed.step.chapter_id.as_deref() != Some(chapter_id)
        || claimed.step.chapter_number != Some(expected_chapter.chapter_number)
    {
        return finish_failure(
            db,
            record,
            claimed,
            step,
            CHAPTER_REPAIR_FACTS_INVALID,
            true,
            true,
            NovelAutopilotQualityDecision::ManualReview,
            None,
        )
        .await;
    }

    let execution_config = match prepare_role_aware_generation_execution_config(
        db,
        &record.user_id,
        GenerationIntentKind::ChapterRepair,
        None,
    )
    .await
    {
        Ok(execution_config) => execution_config,
        Err(_) => {
            return finish_provider_failure(
                db,
                record,
                claimed,
                step,
                config,
                CHAPTER_REPAIR_EXECUTION_CONFIG_FAILED,
            )
            .await;
        }
    };
    ensure_not_cancelled(cancellation_token)?;

    let generated = match generate_chapter_repair_candidate_for_autopilot(
        db,
        &record.user_id,
        chapter_id,
        &claimed.run.id,
        claimed.run.epoch,
        execution_config,
        additional_guidance,
        gateway_config.clone(),
        Some(cancellation_token),
    )
    .await
    {
        Ok(generated) => generated,
        Err(ChapterRepairGenerationError::Cancelled) => {
            return Err(ChapterRepairAdapterError::Cancelled)
        }
        Err(error) => {
            tracing::warn!(
                event = "novel_book_autopilot_chapter_repair_generation_failed",
                error_code = error.code(),
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                chapter_id,
                chapter_number,
                "durable chapter repair candidate generation failed"
            );
            return finish_provider_failure(
                db,
                record,
                claimed,
                step,
                config,
                CHAPTER_REPAIR_PROVIDER_FAILED,
            )
            .await;
        }
    };
    ensure_not_cancelled(cancellation_token)?;
    output_observer.content(generated.content.clone()).await;

    if generated.chapter_id != chapter_id
        || generated.chapter_number != expected_chapter.chapter_number
    {
        return finish_failure(
            db,
            record,
            claimed,
            step,
            CHAPTER_REPAIR_FACTS_INVALID,
            true,
            true,
            NovelAutopilotQualityDecision::ManualReview,
            None,
        )
        .await;
    }

    match route_quality_action(generated.quality_gate_action.as_deref()) {
        ChapterRepairQualityRoute::Accept => {
            commit_accepted_repair(db, record, claimed, step, expected_chapter, generated).await
        }
        ChapterRepairQualityRoute::Retry(decision) => {
            finish_quality_failure(
                db,
                record,
                claimed,
                step,
                config,
                &expected_chapter,
                generated,
                decision,
            )
            .await
        }
        ChapterRepairQualityRoute::ManualReview => {
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
    generated: ChapterRepairCandidate,
) -> Result<ChapterRepairAdapterOutcome, ChapterRepairAdapterError> {
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
        NovelAutopilotStepType::ChapterRepair,
        Some(&record.task_id),
        expected_chapter,
        NovelAutopilotStepStatus::Failed,
        CHAPTER_REPAIR_MANUAL_REVIEW,
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
    .map_err(ChapterRepairAdapterError::Repository)?;

    Ok(ChapterRepairAdapterOutcome::WaitingHuman {
        result: json!({
            "run_id": terminal.run.id,
            "run_status": terminal.run.status,
            "run_epoch": terminal.run.epoch,
            "run_version": terminal.run.version,
            "dispatch_status": "waiting_human",
            "reason_code": CHAPTER_REPAIR_MANUAL_REVIEW,
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

async fn commit_accepted_repair(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    expected_chapter: ChapterBusinessSnapshot,
    generated: ChapterRepairCandidate,
) -> Result<ChapterRepairAdapterOutcome, ChapterRepairAdapterError> {
    let word_count = generated.word_count;
    let result_digest = generated.content_digest.clone();
    let committed = match NovelAutopilotRepository::commit_chapter_repair_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_chapter,
        NovelAutopilotChapterRepairCommit {
            content: generated.content,
            word_count,
            status: generated.chapter_status,
            result_digest,
        },
    )
    .await
    {
        Ok(committed) => committed,
        Err(NovelAutopilotRepositoryError::BusinessDataChanged) => {
            return finish_failure(
                db,
                record,
                claimed,
                step,
                CHAPTER_REPAIR_BUSINESS_DATA_CHANGED,
                false,
                true,
                NovelAutopilotQualityDecision::ManualReview,
                None,
            )
            .await;
        }
        Err(error) => return Err(ChapterRepairAdapterError::Repository(error)),
    };

    Ok(ChapterRepairAdapterOutcome::StepCompleted {
        result: json!({
            "run_id": committed.run.id,
            "run_status": committed.run.status,
            "run_epoch": committed.run.epoch,
            "run_version": committed.run.version,
            "dispatch_status": "step_completed",
            "step_id": committed.step.id,
            "step_type": step.step_type,
            "step_status": committed.step.status,
            "chapter_id": expected_chapter.chapter_id,
            "chapter_number": expected_chapter.chapter_number,
            "word_count": word_count,
            "quality_decision": NovelAutopilotQualityDecision::Accept,
            "result_digest": committed.step.result_digest,
        }),
        run: committed.run,
    })
}

async fn finish_provider_failure(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    reason_code: &str,
) -> Result<ChapterRepairAdapterOutcome, ChapterRepairAdapterError> {
    let next_failures = claimed.run.consecutive_provider_failures.saturating_add(1);
    let waiting_human = claimed.step.attempt >= i32_from_u32(config.max_step_attempts)
        || next_failures >= i32_from_u32(config.max_consecutive_provider_failures);
    finish_failure(
        db,
        record,
        claimed,
        step,
        reason_code,
        true,
        waiting_human,
        NovelAutopilotQualityDecision::Retry,
        None,
    )
    .await
}

async fn finish_quality_failure(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    expected_chapter: &ChapterBusinessSnapshot,
    generated: ChapterRepairCandidate,
    decision: NovelAutopilotQualityDecision,
) -> Result<ChapterRepairAdapterOutcome, ChapterRepairAdapterError> {
    let next_failures = claimed.run.consecutive_quality_failures.saturating_add(1);
    let waiting_human = quality_retry_budget_exhausted(
        claimed.step.attempt,
        next_failures,
        config.max_step_attempts,
        config.max_consecutive_quality_failures,
    );
    let reason_code = match decision {
        NovelAutopilotQualityDecision::AutoRepair => CHAPTER_REPAIR_QUALITY_AUTO_REPAIR,
        _ => CHAPTER_REPAIR_QUALITY_RETRY,
    };
    let persisted_decision = if waiting_human {
        NovelAutopilotQualityDecision::ManualReview
    } else {
        decision
    };
    if waiting_human {
        return persist_manual_review_candidate(
            db,
            record,
            claimed,
            step,
            expected_chapter,
            generated,
        )
        .await;
    }
    let draft_attempt = generated.build_retry_draft_attempt(
        &expected_chapter.project_id,
        &claimed.step.id,
        &claimed.run.id,
        claimed.run.epoch,
        claimed.step.attempt,
        persisted_decision.as_str(),
    );
    let result_digest = generated.content_digest.clone();
    finish_failure(
        db,
        record,
        claimed,
        step,
        reason_code,
        false,
        false,
        persisted_decision,
        Some(NovelAutopilotChapterRepairFailureEvidence {
            expected_chapter: expected_chapter.clone(),
            draft_attempt,
            result_digest,
        }),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn finish_failure(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    reason_code: &str,
    provider_failure: bool,
    waiting_human: bool,
    quality_decision: NovelAutopilotQualityDecision,
    candidate_evidence: Option<NovelAutopilotChapterRepairFailureEvidence>,
) -> Result<ChapterRepairAdapterOutcome, ChapterRepairAdapterError> {
    let quality_diagnostics = candidate_evidence.as_ref().map(|evidence| {
        let payload = evidence.draft_attempt.repair_payload.as_ref();
        (
            evidence.draft_attempt.quality_gate_action.clone(),
            evidence.draft_attempt.quality_metrics.clone(),
            payload
                .and_then(|payload| payload.get("quality_gate_message"))
                .cloned(),
        )
    });
    let terminal = NovelAutopilotRepository::finish_chapter_repair_failure(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        reason_code,
        provider_failure,
        waiting_human,
        quality_decision,
        candidate_evidence,
    )
    .await
    .map_err(ChapterRepairAdapterError::Repository)?;

    let mut result = json!({
        "run_id": terminal.run.id,
        "run_status": terminal.run.status,
        "run_epoch": terminal.run.epoch,
        "run_version": terminal.run.version,
        "dispatch_status": if waiting_human { "waiting_human" } else { "retry_scheduled" },
        "reason_code": reason_code,
        "step_id": terminal.step.id,
        "step_type": step.step_type,
        "step_status": terminal.step.status,
        "chapter_id": step.chapter_id,
        "chapter_number": step.chapter_number,
        "attempt": terminal.step.attempt,
        "quality_decision": terminal.step.quality_decision,
        "result_digest": terminal.step.result_digest,
    });
    if let Some((quality_gate_action, quality_metrics, quality_gate_message)) = quality_diagnostics
    {
        result["quality_gate_action"] = json!(quality_gate_action);
        result["quality_metrics"] = json!(quality_metrics);
        result["quality_gate_message"] = quality_gate_message.unwrap_or(Value::Null);
    }
    if waiting_human {
        Ok(ChapterRepairAdapterOutcome::WaitingHuman { result })
    } else {
        Ok(ChapterRepairAdapterOutcome::StepCompleted {
            result,
            run: terminal.run,
        })
    }
}

fn route_quality_action(action: Option<&str>) -> ChapterRepairQualityRoute {
    match action.map(str::trim).filter(|action| !action.is_empty()) {
        None | Some("continue" | "allow_save" | "accept" | "pass" | "passed") => {
            ChapterRepairQualityRoute::Accept
        }
        Some("auto_repair" | "repair") => {
            ChapterRepairQualityRoute::Retry(NovelAutopilotQualityDecision::AutoRepair)
        }
        Some("retry") => ChapterRepairQualityRoute::Retry(NovelAutopilotQualityDecision::Retry),
        Some("manual_review" | "reject") | Some(_) => ChapterRepairQualityRoute::ManualReview,
    }
}

fn ensure_not_cancelled(
    cancellation_token: &CooperativeCancellationToken,
) -> Result<(), ChapterRepairAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(ChapterRepairAdapterError::Cancelled)
    } else {
        Ok(())
    }
}

fn i32_from_u32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn quality_retry_budget_exhausted(
    attempt: i32,
    next_quality_failures: i32,
    max_step_attempts: u32,
    max_consecutive_quality_failures: u32,
) -> bool {
    attempt >= i32_from_u32(max_step_attempts)
        || next_quality_failures >= i32_from_u32(max_consecutive_quality_failures)
}

#[cfg(test)]
mod tests {
    use super::{quality_retry_budget_exhausted, route_quality_action, ChapterRepairQualityRoute};
    use crate::services::novel_autopilot::types::NovelAutopilotQualityDecision;

    #[test]
    fn repair_quality_actions_are_exhaustive_and_conservative() {
        assert_eq!(
            route_quality_action(None),
            ChapterRepairQualityRoute::Accept
        );
        assert_eq!(
            route_quality_action(Some("continue")),
            ChapterRepairQualityRoute::Accept
        );
        assert_eq!(
            route_quality_action(Some("allow_save")),
            ChapterRepairQualityRoute::Accept
        );
        assert_eq!(
            route_quality_action(Some("auto_repair")),
            ChapterRepairQualityRoute::Retry(NovelAutopilotQualityDecision::AutoRepair)
        );
        assert_eq!(
            route_quality_action(Some("retry")),
            ChapterRepairQualityRoute::Retry(NovelAutopilotQualityDecision::Retry)
        );
        assert_eq!(
            route_quality_action(Some("reject")),
            ChapterRepairQualityRoute::ManualReview
        );
        assert_eq!(
            route_quality_action(Some("unknown")),
            ChapterRepairQualityRoute::ManualReview
        );
    }

    #[test]
    fn third_quality_failure_exhausts_budget_without_scheduling_a_fourth_retry() {
        assert!(!quality_retry_budget_exhausted(1, 1, 3, 3));
        assert!(!quality_retry_budget_exhausted(2, 2, 3, 3));
        assert!(quality_retry_budget_exhausted(3, 3, 3, 3));
        assert!(quality_retry_budget_exhausted(2, 3, 5, 3));
    }
}
