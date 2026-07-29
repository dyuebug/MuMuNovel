use std::fmt;

use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
};

use crate::{
    models::{chapter, novel_autopilot_run, novel_autopilot_step_run, plot_analysis},
    services::chapter_content_digest_service::chapter_content_digest,
};

use super::{
    book_review_service::BookReviewRewriteReference,
    chapter_repository::{chapter_snapshot_condition, ChapterBusinessSnapshot},
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    types::{
        NovelAutopilotQualityDecision, NovelAutopilotRunStatus, NovelAutopilotStepStatus,
        NovelAutopilotStepType,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotBookPolishCommit {
    pub(crate) content: String,
    pub(crate) word_count: i32,
    pub(crate) content_digest: String,
    pub(crate) result_digest: String,
}

impl NovelAutopilotRepository {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_book_polish_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_chapter: &ChapterBusinessSnapshot,
        expected_rewrite: &BookReviewRewriteReference,
        commit: NovelAutopilotBookPolishCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        validate_polish_commit(&commit, expected_rewrite)?;
        let (run, step) = load_and_validate_polish_fence(
            db,
            step_id,
            user_id,
            expected_run_version,
            expected_run_epoch,
            expected_step_key,
            expected_background_task_id,
            expected_chapter,
            expected_rewrite,
        )
        .await?;
        let mut pending_rewrites =
            serde_json::from_value::<Vec<BookReviewRewriteReference>>(run.pending_rewrites.clone())
                .map_err(|_| invalid_config("pending_rewrites"))?;
        if pending_rewrites.first() != Some(expected_rewrite) {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }
        pending_rewrites.remove(0);
        let next_pending_rewrites = serde_json::to_value(pending_rewrites)
            .map_err(|_| invalid_config("pending_rewrites"))?;

        let now = Utc::now().naive_utc();
        let word_count_delta =
            i64::from(commit.word_count) - i64::from(expected_chapter.word_count);
        let txn = db.begin().await.map_err(database_error)?;

        let chapter_update = chapter::Entity::update_many()
            .col_expr(chapter::Column::Content, Expr::value(Some(commit.content)))
            .col_expr(chapter::Column::WordCount, Expr::value(commit.word_count))
            .col_expr(chapter::Column::UpdatedAt, Expr::value(Some(now)))
            .filter(chapter::Column::Id.eq(&expected_chapter.chapter_id))
            .filter(chapter::Column::ProjectId.eq(&expected_chapter.project_id))
            .filter(chapter_snapshot_condition(expected_chapter))
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if chapter_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }

        let run_update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::CurrentStep,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_run::Column::ActiveBackgroundTaskId,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentChapterId,
                Expr::value(Some(expected_chapter.chapter_id.clone())),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentChapterNumber,
                Expr::value(Some(expected_chapter.chapter_number)),
            )
            .col_expr(
                novel_autopilot_run::Column::TotalWordCount,
                Expr::col(novel_autopilot_run::Column::TotalWordCount).add(word_count_delta),
            )
            .col_expr(
                novel_autopilot_run::Column::PendingRewrites,
                Expr::value(next_pending_rewrites),
            )
            .col_expr(
                novel_autopilot_run::Column::ConsecutiveProviderFailures,
                Expr::value(0),
            )
            .col_expr(
                novel_autopilot_run::Column::ConsecutiveQualityFailures,
                Expr::value(0),
            )
            .col_expr(
                novel_autopilot_run::Column::LastErrorCode,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
            .filter(novel_autopilot_run::Column::Id.eq(&run.id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_run_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(expected_run_epoch))
            .filter(novel_autopilot_run::Column::CurrentStep.eq(expected_step_key))
            // PostgreSQL 的 `json` 类型没有等号运算符；队首重写项已在事务前校验，
            // 并且任何合法队列更新都必须推进 version，因此由 version/epoch CAS 防止并发覆盖。
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
            .filter(novel_autopilot_step_run::Column::Id.eq(step_id))
            .filter(novel_autopilot_step_run::Column::RunEpoch.eq(expected_run_epoch))
            .filter(novel_autopilot_step_run::Column::StepKey.eq(expected_step_key))
            .filter(novel_autopilot_step_run::Column::Attempt.eq(step.attempt))
            .filter(
                novel_autopilot_step_run::Column::StepType
                    .eq(NovelAutopilotStepType::BookPolish.as_str()),
            )
            .filter(
                novel_autopilot_step_run::Column::BackgroundTaskId
                    .eq(expected_background_task_id.map(str::to_string)),
            )
            .filter(
                novel_autopilot_step_run::Column::Status
                    .eq(NovelAutopilotStepStatus::Running.as_str()),
            )
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if step_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        txn.commit().await.map_err(database_error)?;
        reload_claimed(db, step_id, &run.id, user_id).await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn finish_book_polish_failure(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        error_code: &str,
        provider_failure: bool,
        waiting_human: bool,
        quality_decision: NovelAutopilotQualityDecision,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = find_owned_run(db, &step.run_id, user_id).await?;
        if run.version != expected_run_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if run.epoch != expected_run_epoch || step.run_epoch != expected_run_epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        if run.status != NovelAutopilotRunStatus::Running.as_str()
            || step.status != NovelAutopilotStepStatus::Running.as_str()
            || run.current_step.as_deref() != Some(expected_step_key)
            || step.step_key != expected_step_key
            || step.step_type != NovelAutopilotStepType::BookPolish.as_str()
            || run.active_background_task_id.as_deref() != expected_background_task_id
            || step.background_task_id.as_deref() != expected_background_task_id
        {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let now = Utc::now().naive_utc();
        let target_status = if waiting_human {
            NovelAutopilotRunStatus::WaitingHuman
        } else {
            NovelAutopilotRunStatus::Running
        };
        let txn = db.begin().await.map_err(database_error)?;
        let mut run_update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::Status,
                Expr::value(target_status.as_str()),
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
                novel_autopilot_run::Column::LastErrorCode,
                Expr::value(Some(error_code.to_string())),
            )
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now));
        run_update = if provider_failure {
            run_update
                .col_expr(
                    novel_autopilot_run::Column::ConsecutiveProviderFailures,
                    Expr::col(novel_autopilot_run::Column::ConsecutiveProviderFailures).add(1),
                )
                .col_expr(
                    novel_autopilot_run::Column::ConsecutiveQualityFailures,
                    Expr::value(0),
                )
        } else {
            run_update
                .col_expr(
                    novel_autopilot_run::Column::ConsecutiveProviderFailures,
                    Expr::value(0),
                )
                .col_expr(
                    novel_autopilot_run::Column::ConsecutiveQualityFailures,
                    Expr::col(novel_autopilot_run::Column::ConsecutiveQualityFailures).add(1),
                )
        };
        let run_update = run_update
            .filter(novel_autopilot_run::Column::Id.eq(&run.id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_run_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(expected_run_epoch))
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
                Expr::value(NovelAutopilotStepStatus::Failed.as_str()),
            )
            .col_expr(
                novel_autopilot_step_run::Column::QualityDecision,
                Expr::value(Some(quality_decision.as_str().to_string())),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ErrorCode,
                Expr::value(Some(error_code.to_string())),
            )
            .col_expr(
                novel_autopilot_step_run::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(novel_autopilot_step_run::Column::Id.eq(step_id))
            .filter(novel_autopilot_step_run::Column::RunEpoch.eq(expected_run_epoch))
            .filter(novel_autopilot_step_run::Column::StepKey.eq(expected_step_key))
            .filter(novel_autopilot_step_run::Column::Attempt.eq(step.attempt))
            .filter(
                novel_autopilot_step_run::Column::StepType
                    .eq(NovelAutopilotStepType::BookPolish.as_str()),
            )
            .filter(
                novel_autopilot_step_run::Column::BackgroundTaskId
                    .eq(expected_background_task_id.map(str::to_string)),
            )
            .filter(
                novel_autopilot_step_run::Column::Status
                    .eq(NovelAutopilotStepStatus::Running.as_str()),
            )
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if step_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }
        txn.commit().await.map_err(database_error)?;
        reload_claimed(db, step_id, &run.id, user_id).await
    }
}

#[allow(clippy::too_many_arguments)]
async fn load_and_validate_polish_fence(
    db: &DatabaseConnection,
    step_id: &str,
    user_id: &str,
    expected_run_version: i64,
    expected_run_epoch: i64,
    expected_step_key: &str,
    expected_background_task_id: Option<&str>,
    expected_chapter: &ChapterBusinessSnapshot,
    expected_rewrite: &BookReviewRewriteReference,
) -> Result<
    (novel_autopilot_run::Model, novel_autopilot_step_run::Model),
    NovelAutopilotRepositoryError,
> {
    let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
        .one(db)
        .await
        .map_err(database_error)?
        .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
    let run = find_owned_run(db, &step.run_id, user_id).await?;
    if run.version != expected_run_version {
        return Err(NovelAutopilotRepositoryError::StaleVersion);
    }
    if run.epoch != expected_run_epoch || step.run_epoch != expected_run_epoch {
        return Err(NovelAutopilotRepositoryError::StaleEpoch);
    }
    if run.project_id != expected_chapter.project_id
        || run.status != NovelAutopilotRunStatus::Running.as_str()
        || step.status != NovelAutopilotStepStatus::Running.as_str()
        || run.current_step.as_deref() != Some(expected_step_key)
        || step.step_key != expected_step_key
        || step.step_type != NovelAutopilotStepType::BookPolish.as_str()
        || step.chapter_id.as_deref() != Some(expected_chapter.chapter_id.as_str())
        || step.chapter_number != Some(expected_chapter.chapter_number)
        || run.active_background_task_id.as_deref() != expected_background_task_id
        || step.background_task_id.as_deref() != expected_background_task_id
        || expected_rewrite.chapter_id != expected_chapter.chapter_id
        || expected_rewrite.chapter_number != expected_chapter.chapter_number
        || expected_chapter.content_digest().as_deref()
            != Some(expected_rewrite.source_content_digest.as_str())
    {
        return Err(NovelAutopilotRepositoryError::InvalidTransition);
    }

    let analysis = plot_analysis::Entity::find_by_id(&expected_rewrite.analysis_id)
        .one(db)
        .await
        .map_err(database_error)?
        .ok_or(NovelAutopilotRepositoryError::BusinessDataChanged)?;
    if analysis.chapter_id != expected_chapter.chapter_id
        || analysis.source_content_digest.as_deref()
            != Some(expected_rewrite.source_content_digest.as_str())
    {
        return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
    }
    Ok((run, step))
}

fn validate_polish_commit(
    commit: &NovelAutopilotBookPolishCommit,
    expected_rewrite: &BookReviewRewriteReference,
) -> Result<(), NovelAutopilotRepositoryError> {
    if commit.content.trim().is_empty() {
        return Err(invalid_config("content"));
    }
    if commit.word_count <= 0 {
        return Err(invalid_config("word_count"));
    }
    if commit.content_digest.trim().is_empty() {
        return Err(invalid_config("content_digest"));
    }
    if chapter_content_digest(&commit.content) != commit.content_digest {
        return Err(invalid_config("content_digest"));
    }
    if commit.content_digest == expected_rewrite.source_content_digest {
        return Err(invalid_config("content_unchanged"));
    }
    if commit.result_digest.trim().is_empty() {
        return Err(invalid_config("result_digest"));
    }
    Ok(())
}

async fn reload_claimed(
    db: &DatabaseConnection,
    step_id: &str,
    run_id: &str,
    user_id: &str,
) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
    Ok(ClaimedNovelAutopilotStep {
        run: find_owned_run(db, run_id, user_id).await?,
        step: novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
    })
}

async fn find_owned_run(
    db: &DatabaseConnection,
    run_id: &str,
    user_id: &str,
) -> Result<novel_autopilot_run::Model, NovelAutopilotRepositoryError> {
    novel_autopilot_run::Entity::find_by_id(run_id)
        .filter(novel_autopilot_run::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(database_error)?
        .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)
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
