use std::fmt;

use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
};

use crate::models::{novel_autopilot_run, novel_autopilot_step_run};

use super::{
    book_review_service::BookReviewRewriteReference,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    types::{
        NovelAutopilotQualityDecision, NovelAutopilotRunStatus, NovelAutopilotStepStatus,
        NovelAutopilotStepType,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct NovelAutopilotBookReviewCommit {
    pub pending_rewrites: Vec<BookReviewRewriteReference>,
    pub result_digest: String,
}

impl NovelAutopilotRepository {
    pub(crate) async fn commit_book_review_step(
        db: &DatabaseConnection,
        claimed: &ClaimedNovelAutopilotStep,
        user_id: &str,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        commit: NovelAutopilotBookReviewCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        if commit.result_digest.trim().is_empty() {
            return Err(invalid_config("result_digest"));
        }
        let pending_rewrites = serde_json::to_value(&commit.pending_rewrites)
            .map_err(|_| invalid_config("pending_rewrites"))?;
        let step = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = Self::find_owned(db, &step.run_id, user_id).await?;
        if run.version != claimed.run.version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if run.epoch != claimed.run.epoch || step.run_epoch != claimed.run.epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        if run.status != NovelAutopilotRunStatus::Running.as_str()
            || step.status != NovelAutopilotStepStatus::Running.as_str()
            || run.current_step.as_deref() != Some(expected_step_key)
            || step.step_key != expected_step_key
            || step.step_type != NovelAutopilotStepType::BookReview.as_str()
            || step.chapter_id.is_some()
            || step.chapter_number.is_some()
            || run.active_background_task_id.as_deref() != expected_background_task_id
            || step.background_task_id.as_deref() != expected_background_task_id
        {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        let run_update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::PendingRewrites,
                Expr::value(pending_rewrites),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentStep,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_run::Column::ActiveBackgroundTaskId,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
            .filter(novel_autopilot_run::Column::Id.eq(&run.id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(claimed.run.version))
            .filter(novel_autopilot_run::Column::Epoch.eq(claimed.run.epoch))
            .filter(novel_autopilot_run::Column::CurrentStep.eq(expected_step_key))
            .filter(
                novel_autopilot_run::Column::ActiveBackgroundTaskId
                    .eq(expected_background_task_id.map(str::to_string)),
            )
            .filter(
                novel_autopilot_run::Column::Status.eq(NovelAutopilotRunStatus::Running.as_str()),
            )
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if run_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }

        let step_update = novel_autopilot_step_run::Entity::update_many()
            .col_expr(
                novel_autopilot_step_run::Column::Status,
                Expr::value(NovelAutopilotStepStatus::Completed.as_str()),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ResultDigest,
                Expr::value(Some(commit.result_digest)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::QualityDecision,
                Expr::value(Some(
                    NovelAutopilotQualityDecision::Accept.as_str().to_string(),
                )),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ErrorCode,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_step_run::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(novel_autopilot_step_run::Column::Id.eq(&step.id))
            .filter(novel_autopilot_step_run::Column::RunId.eq(&run.id))
            .filter(novel_autopilot_step_run::Column::RunEpoch.eq(claimed.run.epoch))
            .filter(novel_autopilot_step_run::Column::StepKey.eq(expected_step_key))
            .filter(
                novel_autopilot_step_run::Column::StepType
                    .eq(NovelAutopilotStepType::BookReview.as_str()),
            )
            .filter(
                novel_autopilot_step_run::Column::Status
                    .eq(NovelAutopilotStepStatus::Running.as_str()),
            )
            .filter(
                novel_autopilot_step_run::Column::BackgroundTaskId
                    .eq(expected_background_task_id.map(str::to_string)),
            )
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if step_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        txn.commit().await.map_err(database_error)?;

        reload_claimed(db, &step.id, &run.id, user_id).await
    }
}

async fn reload_claimed(
    db: &DatabaseConnection,
    step_id: &str,
    run_id: &str,
    user_id: &str,
) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
    Ok(ClaimedNovelAutopilotStep {
        run: NovelAutopilotRepository::find_owned(db, run_id, user_id).await?,
        step: novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
    })
}

const fn invalid_config(field: &'static str) -> NovelAutopilotRepositoryError {
    NovelAutopilotRepositoryError::InvalidConfig {
        field,
        code: "invalid",
    }
}

fn database_error(error: impl fmt::Display) -> NovelAutopilotRepositoryError {
    NovelAutopilotRepositoryError::Database(error.to_string())
}
