use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    services::{
        book_polish_generation_service::{
            generate_book_polish_candidate_with_guidance, BookPolishGenerationError,
        },
        chapter_generation_execution_contract_service::prepare_role_aware_generation_execution_config,
        cooperative_cancellation_service::CooperativeCancellationToken,
        generation_contract_service::GenerationIntentKind,
    },
    tasks::types::TaskRecord,
};

use super::{
    book_polish_repository::NovelAutopilotBookPolishCommit,
    book_review_service::BookReviewRewriteReference,
    chapter_repository::ChapterBusinessSnapshot,
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    router::AutopilotStepPlan,
    types::{NovelAutopilotQualityDecision, NovelAutopilotRunConfig},
};

const BOOK_POLISH_FACTS_INVALID: &str = "book_polish_step_facts_invalid";
const BOOK_POLISH_EXECUTION_CONFIG_FAILED: &str = "book_polish_execution_config_failed";
const BOOK_POLISH_PROVIDER_FAILED: &str = "book_polish_provider_failed";
const BOOK_POLISH_CONTENT_UNCHANGED: &str = "book_polish_content_unchanged";
const BOOK_POLISH_BUSINESS_DATA_CHANGED: &str = "book_polish_business_data_changed";

#[derive(Debug)]
pub(crate) enum BookPolishAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
}

impl BookPolishAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum BookPolishAdapterOutcome {
    StepCompleted {
        result: Value,
        run: crate::models::novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_book_polish_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    additional_guidance: Option<&str>,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<BookPolishAdapterOutcome, BookPolishAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let (Some(chapter_id), Some(chapter_number)) =
        (step.chapter_id.as_deref(), step.chapter_number)
    else {
        return finish_failure(
            db,
            record,
            claimed,
            step,
            BOOK_POLISH_FACTS_INVALID,
            false,
            true,
            NovelAutopilotQualityDecision::ManualReview,
        )
        .await;
    };

    let expected_rewrite = match load_expected_rewrite(&claimed, chapter_id, chapter_number) {
        Some(rewrite) => rewrite,
        None => {
            return finish_failure(
                db,
                record,
                claimed,
                step,
                BOOK_POLISH_FACTS_INVALID,
                false,
                true,
                NovelAutopilotQualityDecision::ManualReview,
            )
            .await;
        }
    };
    let expected_chapter = ChapterBusinessSnapshot::load(db, &claimed.run.project_id, chapter_id)
        .await
        .map_err(BookPolishAdapterError::Repository)?;
    if expected_chapter.chapter_number != i32::try_from(chapter_number).unwrap_or_default()
        || expected_chapter
            .content
            .as_deref()
            .is_none_or(|content| content.trim().is_empty())
        || expected_chapter.content_digest().as_deref()
            != Some(expected_rewrite.source_content_digest.as_str())
        || claimed.step.chapter_id.as_deref() != Some(chapter_id)
        || claimed.step.chapter_number != Some(expected_chapter.chapter_number)
    {
        return finish_failure(
            db,
            record,
            claimed,
            step,
            BOOK_POLISH_FACTS_INVALID,
            false,
            true,
            NovelAutopilotQualityDecision::ManualReview,
        )
        .await;
    }

    let execution_config = match prepare_role_aware_generation_execution_config(
        db,
        &record.user_id,
        GenerationIntentKind::BookPolish,
        None,
    )
    .await
    {
        Ok(config) => config,
        Err(_) => {
            return finish_provider_failure(
                db,
                record,
                claimed,
                step,
                config,
                BOOK_POLISH_EXECUTION_CONFIG_FAILED,
            )
            .await;
        }
    };
    ensure_not_cancelled(cancellation_token)?;

    let original_content = expected_chapter.content.as_deref().unwrap_or_default();
    let generated = match generate_book_polish_candidate_with_guidance(
        original_content,
        None,
        "balanced",
        true,
        true,
        execution_config.ai_config,
        additional_guidance,
        Some(cancellation_token),
    )
    .await
    {
        Ok(candidate) => candidate,
        Err(BookPolishGenerationError::Cancelled) => return Err(BookPolishAdapterError::Cancelled),
        Err(error) => {
            tracing::warn!(
                event = "novel_book_autopilot_book_polish_generation_failed",
                error_code = error.code(),
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                chapter_id,
                chapter_number,
                "durable book polish candidate generation failed"
            );
            return finish_provider_failure(
                db,
                record,
                claimed,
                step,
                config,
                BOOK_POLISH_PROVIDER_FAILED,
            )
            .await;
        }
    };
    ensure_not_cancelled(cancellation_token)?;
    output_observer.content(generated.content.clone()).await;

    if generated.content_digest == expected_rewrite.source_content_digest {
        return finish_quality_failure(
            db,
            record,
            claimed,
            step,
            config,
            BOOK_POLISH_CONTENT_UNCHANGED,
        )
        .await;
    }

    let committed = match NovelAutopilotRepository::commit_book_polish_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_chapter,
        &expected_rewrite,
        NovelAutopilotBookPolishCommit {
            content: generated.content,
            word_count: generated.word_count_after,
            content_digest: generated.content_digest.clone(),
            result_digest: generated.content_digest,
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
                BOOK_POLISH_BUSINESS_DATA_CHANGED,
                false,
                true,
                NovelAutopilotQualityDecision::ManualReview,
            )
            .await;
        }
        Err(error) => return Err(BookPolishAdapterError::Repository(error)),
    };

    Ok(BookPolishAdapterOutcome::StepCompleted {
        result: json!({
            "run_id": committed.run.id,
            "run_status": committed.run.status,
            "run_epoch": committed.run.epoch,
            "run_version": committed.run.version,
            "dispatch_status": "schedule_next",
            "step_id": committed.step.id,
            "step_type": step.step_type,
            "step_status": committed.step.status,
            "chapter_id": chapter_id,
            "chapter_number": chapter_number,
            "attempt": committed.step.attempt,
            "word_count_before": generated.word_count_before,
            "word_count_after": generated.word_count_after,
            "quality_decision": committed.step.quality_decision,
            "result_digest": committed.step.result_digest,
        }),
        run: committed.run,
    })
}

fn load_expected_rewrite(
    claimed: &ClaimedNovelAutopilotStep,
    chapter_id: &str,
    chapter_number: u32,
) -> Option<BookReviewRewriteReference> {
    let rewrites = serde_json::from_value::<Vec<BookReviewRewriteReference>>(
        claimed.run.pending_rewrites.clone(),
    )
    .ok()?;
    let rewrite = rewrites.into_iter().next()?;
    (rewrite.chapter_id == chapter_id
        && u32::try_from(rewrite.chapter_number).ok() == Some(chapter_number))
    .then_some(rewrite)
}

async fn finish_provider_failure(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    reason_code: &str,
) -> Result<BookPolishAdapterOutcome, BookPolishAdapterError> {
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
        if waiting_human {
            NovelAutopilotQualityDecision::ManualReview
        } else {
            NovelAutopilotQualityDecision::Retry
        },
    )
    .await
}

async fn finish_quality_failure(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    reason_code: &str,
) -> Result<BookPolishAdapterOutcome, BookPolishAdapterError> {
    let next_failures = claimed.run.consecutive_quality_failures.saturating_add(1);
    let waiting_human = claimed.step.attempt >= i32_from_u32(config.max_step_attempts)
        || next_failures >= i32_from_u32(config.max_consecutive_quality_failures);
    finish_failure(
        db,
        record,
        claimed,
        step,
        reason_code,
        false,
        waiting_human,
        if waiting_human {
            NovelAutopilotQualityDecision::ManualReview
        } else {
            NovelAutopilotQualityDecision::Retry
        },
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
) -> Result<BookPolishAdapterOutcome, BookPolishAdapterError> {
    let terminal = NovelAutopilotRepository::finish_book_polish_failure(
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
    )
    .await
    .map_err(BookPolishAdapterError::Repository)?;

    let result = json!({
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
    if waiting_human {
        Ok(BookPolishAdapterOutcome::WaitingHuman { result })
    } else {
        Ok(BookPolishAdapterOutcome::StepCompleted {
            result,
            run: terminal.run,
        })
    }
}

fn ensure_not_cancelled(
    cancellation_token: &CooperativeCancellationToken,
) -> Result<(), BookPolishAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(BookPolishAdapterError::Cancelled)
    } else {
        Ok(())
    }
}

fn i32_from_u32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
