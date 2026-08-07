use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

use crate::{
    models::{chapter, novel_autopilot_run},
    services::{
        chapter_analysis_generation_service::{
            generate_chapter_analysis_candidate_for_autopilot, ChapterAnalysisCandidate,
            ChapterAnalysisGenerationError,
        },
        chapter_analysis_runtime_service::persistence_owner::synchronize_chapter_analysis_derivatives,
        cooperative_cancellation_service::CooperativeCancellationToken,
    },
    tasks::types::TaskRecord,
};

use super::{
    chapter_analysis_repository::NovelAutopilotChapterAnalysisCommit,
    chapter_repository::ChapterBusinessSnapshot,
    failure_diagnostic::{NovelAutopilotFailureDiagnostic, NovelAutopilotFailureDomain},
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    router::AutopilotStepPlan,
    types::{
        NovelAutopilotFailureCounterKind, NovelAutopilotQualityDecision, NovelAutopilotRunConfig,
    },
};

const CHAPTER_ANALYSIS_FACTS_INVALID: &str = "chapter_analysis_context_invalid";

#[derive(Debug)]
pub(crate) enum ChapterAnalysisAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
}

impl ChapterAnalysisAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "chapter_analysis_cancelled",
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ChapterAnalysisAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_chapter_analysis_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    additional_guidance: Option<&str>,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<ChapterAnalysisAdapterOutcome, ChapterAnalysisAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let (Some(chapter_id), Some(chapter_number)) =
        (step.chapter_id.as_deref(), step.chapter_number)
    else {
        return finish_failure(
            db,
            record,
            claimed,
            step,
            CHAPTER_ANALYSIS_FACTS_INVALID,
            Some(NovelAutopilotFailureDiagnostic::context_invalid(
                "invalid_input",
            )),
            NovelAutopilotFailureCounterKind::None,
            true,
        )
        .await;
    };
    let expected_chapter = ChapterBusinessSnapshot::load(db, &claimed.run.project_id, chapter_id)
        .await
        .map_err(ChapterAnalysisAdapterError::Repository)?;
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
            CHAPTER_ANALYSIS_FACTS_INVALID,
            Some(NovelAutopilotFailureDiagnostic::context_invalid(
                "invalid_input",
            )),
            NovelAutopilotFailureCounterKind::None,
            true,
        )
        .await;
    }

    let generated = match generate_chapter_analysis_candidate_for_autopilot(
        db,
        &record.user_id,
        chapter_id,
        additional_guidance,
        Some(cancellation_token),
    )
    .await
    {
        Ok(generated) => generated,
        Err(ChapterAnalysisGenerationError::Cancelled) => {
            return Err(ChapterAnalysisAdapterError::Cancelled)
        }
        Err(error) => {
            let diagnostic = error.failure_diagnostic();
            let reason_code = diagnostic.reason_code(NovelAutopilotFailureDomain::ChapterAnalysis);
            let counts_as_provider_failure = diagnostic.counts_as_provider_failure();
            let next_failures = claimed.run.consecutive_provider_failures.saturating_add(1);
            let waiting_human = !diagnostic.retryable
                || claimed.step.attempt >= i32_from_u32(config.max_step_attempts)
                || (counts_as_provider_failure
                    && next_failures >= i32_from_u32(config.max_consecutive_provider_failures));
            tracing::warn!(
                event = "novel_book_autopilot_chapter_analysis_generation_failed",
                error_code = error.code(),
                reason_code = %reason_code,
                failure_category = diagnostic.category.as_str(),
                provider = diagnostic.provider.as_deref(),
                model = diagnostic.model.as_deref(),
                http_status = diagnostic.http_status,
                retryable = diagnostic.retryable,
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                attempt = claimed.step.attempt,
                background_task_id = %record.task_id,
                chapter_id,
                "durable chapter analysis candidate generation failed"
            );
            return finish_failure(
                db,
                record,
                claimed,
                step,
                &reason_code,
                Some(diagnostic),
                if counts_as_provider_failure {
                    NovelAutopilotFailureCounterKind::Provider
                } else {
                    NovelAutopilotFailureCounterKind::None
                },
                waiting_human,
            )
            .await;
        }
    };
    ensure_not_cancelled(cancellation_token)?;
    output_observer.content(generated.payload.to_string()).await;
    if generated.chapter_id != chapter_id
        || generated.chapter_number != expected_chapter.chapter_number
    {
        return finish_failure(
            db,
            record,
            claimed,
            step,
            CHAPTER_ANALYSIS_FACTS_INVALID,
            Some(NovelAutopilotFailureDiagnostic::context_invalid(
                "invalid_result",
            )),
            NovelAutopilotFailureCounterKind::None,
            true,
        )
        .await;
    }
    commit_generated_analysis(
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

#[allow(clippy::too_many_arguments)]
async fn commit_generated_analysis(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    expected_chapter: ChapterBusinessSnapshot,
    generated: ChapterAnalysisCandidate,
) -> Result<ChapterAnalysisAdapterOutcome, ChapterAnalysisAdapterError> {
    let next_quality_failures = claimed.run.consecutive_quality_failures.saturating_add(1);
    let budget_exhausted = generated.quality_decision != NovelAutopilotQualityDecision::Accept
        && (claimed.step.attempt >= i32_from_u32(config.max_step_attempts)
            || next_quality_failures >= i32_from_u32(config.max_consecutive_quality_failures));
    let waiting_human = generated.quality_decision == NovelAutopilotQualityDecision::ManualReview
        || budget_exhausted;
    let persisted_decision = if waiting_human {
        NovelAutopilotQualityDecision::ManualReview
    } else {
        generated.quality_decision
    };
    let result_digest = generated.result_digest.clone();
    let overall_score = generated.overall_score;
    let payload = generated.payload.clone();
    let committed = NovelAutopilotRepository::commit_chapter_analysis_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_chapter,
        NovelAutopilotChapterAnalysisCommit {
            payload: generated.payload,
            result_digest: generated.result_digest,
            quality_decision: persisted_decision,
            waiting_human,
        },
    )
    .await
    .map_err(ChapterAnalysisAdapterError::Repository)?;

    if let Ok(Some(chapter_model)) = chapter::Entity::find_by_id(&expected_chapter.chapter_id)
        .one(db)
        .await
    {
        if let Err(error) =
            synchronize_chapter_analysis_derivatives(db, &record.user_id, &chapter_model, &payload)
                .await
        {
            tracing::warn!(
                event = "novel_book_autopilot_chapter_analysis_derivative_sync_failed",
                run_id = %committed.run.id,
                step_id = %committed.step.id,
                chapter_id = %expected_chapter.chapter_id,
                error = %error,
                "chapter analysis core commit succeeded but derivative synchronization failed"
            );
        }
    }

    let dispatch_status = if waiting_human {
        "waiting_human"
    } else {
        "step_completed"
    };
    let reason_code = committed.step.error_code.clone();
    let result = json!({
        "run_id": committed.run.id,
        "run_status": committed.run.status,
        "run_epoch": committed.run.epoch,
        "run_version": committed.run.version,
        "dispatch_status": dispatch_status,
        "reason_code": reason_code,
        "step_id": committed.step.id,
        "step_key": committed.step.step_key,
        "step_type": committed.step.step_type,
        "chapter_id": expected_chapter.chapter_id,
        "chapter_number": expected_chapter.chapter_number,
        "overall_score": overall_score,
        "quality_decision": persisted_decision.as_str(),
        "result_digest": result_digest,
        "status": committed.run.status,
        "step_status": committed.step.status,
        "attempt": committed.step.attempt,
        "waiting_human": waiting_human,
    });
    if waiting_human {
        Ok(ChapterAnalysisAdapterOutcome::WaitingHuman { result })
    } else {
        Ok(ChapterAnalysisAdapterOutcome::StepCompleted {
            result,
            run: committed.run,
        })
    }
}

async fn finish_failure(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    error_code: &str,
    failure_diagnostic: Option<NovelAutopilotFailureDiagnostic>,
    failure_counter_kind: NovelAutopilotFailureCounterKind,
    waiting_human: bool,
) -> Result<ChapterAnalysisAdapterOutcome, ChapterAnalysisAdapterError> {
    let retry_after_seconds = failure_diagnostic
        .as_ref()
        .and_then(NovelAutopilotFailureDiagnostic::retry_after_seconds);
    let terminal = NovelAutopilotRepository::finish_chapter_analysis_failure(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        error_code,
        failure_counter_kind,
        retry_after_seconds,
        waiting_human,
    )
    .await
    .map_err(ChapterAnalysisAdapterError::Repository)?;
    let mut result = json!({
        "run_id": terminal.run.id,
        "run_status": terminal.run.status,
        "run_epoch": terminal.run.epoch,
        "run_version": terminal.run.version,
        "dispatch_status": if waiting_human { "waiting_human" } else { "retry_scheduled" },
        "reason_code": error_code,
        "step_id": terminal.step.id,
        "step_key": terminal.step.step_key,
        "step_type": terminal.step.step_type,
        "chapter_id": terminal.step.chapter_id,
        "chapter_number": terminal.step.chapter_number,
        "status": terminal.run.status,
        "step_status": terminal.step.status,
        "attempt": terminal.step.attempt,
        "error_code": error_code,
        "waiting_human": waiting_human,
    });
    if let Some(failure_diagnostic) = failure_diagnostic {
        result["failure_diagnostic"] = failure_diagnostic.to_value();
    }
    if waiting_human {
        Ok(ChapterAnalysisAdapterOutcome::WaitingHuman { result })
    } else {
        Ok(ChapterAnalysisAdapterOutcome::StepCompleted {
            result,
            run: terminal.run,
        })
    }
}

fn ensure_not_cancelled(
    cancellation_token: &CooperativeCancellationToken,
) -> Result<(), ChapterAnalysisAdapterError> {
    if cancellation_token.is_cancelled() {
        return Err(ChapterAnalysisAdapterError::Cancelled);
    }
    Ok(())
}

fn i32_from_u32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
