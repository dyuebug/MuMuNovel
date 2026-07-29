use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::cooperative_cancellation_service::CooperativeCancellationToken,
    tasks::types::TaskRecord,
};

use super::{
    book_review_repository::NovelAutopilotBookReviewCommit,
    book_review_service::{load_book_review_summary, BookReviewServiceError, BookReviewSummary},
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
        NovelAutopilotStepTerminalPatch,
    },
    router::AutopilotStepPlan,
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus, NovelAutopilotStepType},
};

const BOOK_REVIEW_FACTS_INVALID: &str = "book_review_step_facts_invalid";
const BOOK_REVIEW_NOT_READY: &str = "book_review_not_ready";

#[derive(Debug)]
pub(crate) enum BookReviewAdapterError {
    Cancelled,
    Service(BookReviewServiceError),
    Repository(NovelAutopilotRepositoryError),
}

impl BookReviewAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "book_review_cancelled",
            Self::Service(error) => error.code(),
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum BookReviewAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_book_review_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<BookReviewAdapterOutcome, BookReviewAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    if step.step_type != NovelAutopilotStepType::BookReview
        || step.chapter_id.is_some()
        || step.chapter_number.is_some()
        || claimed.step.step_type != NovelAutopilotStepType::BookReview.as_str()
    {
        return finish_waiting_human(db, record, claimed, step, None, BOOK_REVIEW_FACTS_INVALID)
            .await;
    }

    let expected_chapter_count = u32::try_from(claimed.run.total_chapters).unwrap_or_default();
    let summary = load_book_review_summary(
        db,
        &claimed.run.project_id,
        &record.user_id,
        expected_chapter_count,
    )
    .await
    .map_err(BookReviewAdapterError::Service)?;
    ensure_not_cancelled(cancellation_token)?;
    if !summary.ready {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            Some(&summary),
            BOOK_REVIEW_NOT_READY,
        )
        .await;
    }

    let committed = NovelAutopilotRepository::commit_book_review_step(
        db,
        &claimed,
        &record.user_id,
        &step.step_key,
        Some(&record.task_id),
        NovelAutopilotBookReviewCommit {
            pending_rewrites: summary.pending_rewrites.clone(),
            result_digest: summary.result_digest.clone(),
        },
    )
    .await
    .map_err(BookReviewAdapterError::Repository)?;

    Ok(BookReviewAdapterOutcome::StepCompleted {
        result: summary_result(&committed.run, &summary, "completed", None),
        run: committed.run,
    })
}

async fn finish_waiting_human(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    summary: Option<&BookReviewSummary>,
    reason_code: &str,
) -> Result<BookReviewAdapterOutcome, BookReviewAdapterError> {
    let terminal = NovelAutopilotRepository::complete_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        NovelAutopilotStepStatus::Skipped,
        NovelAutopilotStepTerminalPatch {
            result_digest: summary.map(|summary| summary.result_digest.clone()),
            quality_decision: None,
            error_code: Some(reason_code.to_string()),
        },
    )
    .await
    .map_err(BookReviewAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(BookReviewAdapterError::Repository)?;

    Ok(BookReviewAdapterOutcome::WaitingHuman {
        result: match summary {
            Some(summary) => summary_result(&waiting, summary, "waiting_human", Some(reason_code)),
            None => json!({
                "run_id": waiting.id,
                "run_status": waiting.status,
                "run_epoch": waiting.epoch,
                "run_version": waiting.version,
                "dispatch_status": "waiting_human",
                "reason_code": reason_code,
            }),
        },
    })
}

fn summary_result(
    run: &novel_autopilot_run::Model,
    summary: &BookReviewSummary,
    dispatch_status: &str,
    reason_code: Option<&str>,
) -> Value {
    json!({
        "run_id": run.id,
        "run_status": run.status,
        "run_epoch": run.epoch,
        "run_version": run.version,
        "dispatch_status": dispatch_status,
        "reason_code": reason_code,
        "book_review": {
            "ready": summary.ready,
            "consistency_ready": summary.consistency.ready,
            "expected_analysis_count": summary.expected_analysis_count,
            "analyzed_chapter_count": summary.analyzed_chapter_count,
            "below_target_chapter_count": summary.below_target_chapter_count,
            "suggestion_chapter_count": summary.suggestion_chapter_count,
            "pending_rewrite_count": summary.pending_rewrites.len(),
            "result_digest": summary.result_digest,
        }
    })
}

fn ensure_not_cancelled(
    cancellation_token: &CooperativeCancellationToken,
) -> Result<(), BookReviewAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(BookReviewAdapterError::Cancelled)
    } else {
        Ok(())
    }
}
