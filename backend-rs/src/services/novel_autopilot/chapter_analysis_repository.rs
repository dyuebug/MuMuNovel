use std::fmt;

use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use serde_json::Value;

use crate::{
    models::{chapter, novel_autopilot_run, novel_autopilot_step_run, plot_analysis},
    services::chapter_analysis_runtime_service::persistence_owner::build_plot_analysis_active_model,
};

use super::{
    chapter_repository::{chapter_snapshot_condition, ChapterBusinessSnapshot},
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    types::{
        NovelAutopilotFailureCounterKind, NovelAutopilotQualityDecision, NovelAutopilotRunStatus,
        NovelAutopilotStepStatus, NovelAutopilotStepType,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct NovelAutopilotChapterAnalysisCommit {
    pub(crate) payload: Value,
    pub(crate) result_digest: String,
    pub(crate) quality_decision: NovelAutopilotQualityDecision,
    pub(crate) waiting_human: bool,
}

impl NovelAutopilotRepository {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_chapter_analysis_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_chapter: &ChapterBusinessSnapshot,
        commit: NovelAutopilotChapterAnalysisCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        validate_analysis_commit(&commit)?;
        let (run, step) = load_and_validate_analysis_fence(
            db,
            step_id,
            user_id,
            expected_run_version,
            expected_run_epoch,
            expected_step_key,
            expected_background_task_id,
            expected_chapter,
        )
        .await?;
        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        let chapter_fence = chapter::Entity::update_many()
            .col_expr(
                chapter::Column::UpdatedAt,
                Expr::col(chapter::Column::UpdatedAt).into(),
            )
            .filter(chapter::Column::Id.eq(&expected_chapter.chapter_id))
            .filter(chapter::Column::ProjectId.eq(&expected_chapter.project_id))
            .filter(chapter_snapshot_condition(expected_chapter))
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if chapter_fence.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }

        let chapter_model = chapter::Entity::find_by_id(&expected_chapter.chapter_id)
            .one(&txn)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::BusinessDataChanged)?;
        let current_content_digest = expected_chapter.content_digest();
        if let Some(existing_analysis) = plot_analysis::Entity::find()
            .filter(plot_analysis::Column::ChapterId.eq(&expected_chapter.chapter_id))
            .one(&txn)
            .await
            .map_err(database_error)?
        {
            if existing_analysis.source_content_digest == current_content_digest {
                return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
            }
            plot_analysis::Entity::delete_by_id(existing_analysis.id)
                .exec(&txn)
                .await
                .map_err(database_error)?;
        }
        build_plot_analysis_active_model(&chapter_model, &commit.payload, now)
            .insert(&txn)
            .await
            .map_err(database_error)?;

        let target_status = if commit.waiting_human {
            NovelAutopilotRunStatus::WaitingHuman
        } else {
            NovelAutopilotRunStatus::Running
        };
        let quality_failed = commit.quality_decision != NovelAutopilotQualityDecision::Accept;
        let error_code = match commit.quality_decision {
            NovelAutopilotQualityDecision::Accept => None,
            NovelAutopilotQualityDecision::AutoRepair => Some("chapter_analysis_auto_repair"),
            NovelAutopilotQualityDecision::ManualReview => Some("chapter_analysis_manual_review"),
            NovelAutopilotQualityDecision::Retry => Some("chapter_analysis_retry"),
            NovelAutopilotQualityDecision::Reject => Some("chapter_analysis_reject"),
        };
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
                novel_autopilot_run::Column::ConsecutiveProviderFailures,
                Expr::value(0),
            )
            .col_expr(
                novel_autopilot_run::Column::LastErrorCode,
                Expr::value(error_code.map(str::to_string)),
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
            .filter(
                novel_autopilot_run::Column::Status.eq(NovelAutopilotRunStatus::Running.as_str()),
            )
            .filter(novel_autopilot_run::Column::CurrentStep.eq(expected_step_key))
            .filter(
                novel_autopilot_run::Column::ActiveBackgroundTaskId
                    .eq(expected_background_task_id.map(str::to_string)),
            );
        run_update = if quality_failed {
            run_update.col_expr(
                novel_autopilot_run::Column::ConsecutiveQualityFailures,
                Expr::col(novel_autopilot_run::Column::ConsecutiveQualityFailures).add(1),
            )
        } else {
            run_update.col_expr(
                novel_autopilot_run::Column::ConsecutiveQualityFailures,
                Expr::value(0),
            )
        };
        if run_update
            .exec(&txn)
            .await
            .map_err(database_error)?
            .rows_affected
            != 1
        {
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
                Expr::value(Some(commit.quality_decision.as_str().to_string())),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ErrorCode,
                Expr::value(error_code.map(str::to_string)),
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
                    .eq(NovelAutopilotStepType::ChapterAnalyze.as_str()),
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
    pub(crate) async fn finish_chapter_analysis_failure(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        error_code: &str,
        failure_counter_kind: NovelAutopilotFailureCounterKind,
        waiting_human: bool,
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
            || step.step_type != NovelAutopilotStepType::ChapterAnalyze.as_str()
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
        run_update = match failure_counter_kind {
            NovelAutopilotFailureCounterKind::Provider => run_update
                .col_expr(
                    novel_autopilot_run::Column::ConsecutiveProviderFailures,
                    Expr::col(novel_autopilot_run::Column::ConsecutiveProviderFailures).add(1),
                )
                .col_expr(
                    novel_autopilot_run::Column::ConsecutiveQualityFailures,
                    Expr::value(0),
                ),
            NovelAutopilotFailureCounterKind::Quality => run_update
                .col_expr(
                    novel_autopilot_run::Column::ConsecutiveProviderFailures,
                    Expr::value(0),
                )
                .col_expr(
                    novel_autopilot_run::Column::ConsecutiveQualityFailures,
                    Expr::col(novel_autopilot_run::Column::ConsecutiveQualityFailures).add(1),
                ),
            NovelAutopilotFailureCounterKind::None => run_update,
        };
        let run_update = run_update
            .filter(novel_autopilot_run::Column::Id.eq(&run.id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_run_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(expected_run_epoch))
            .filter(
                novel_autopilot_run::Column::Status.eq(NovelAutopilotRunStatus::Running.as_str()),
            )
            .filter(novel_autopilot_run::Column::CurrentStep.eq(expected_step_key))
            .filter(
                novel_autopilot_run::Column::ActiveBackgroundTaskId
                    .eq(expected_background_task_id.map(str::to_string)),
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
                    .eq(NovelAutopilotStepType::ChapterAnalyze.as_str()),
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
async fn load_and_validate_analysis_fence(
    db: &DatabaseConnection,
    step_id: &str,
    user_id: &str,
    expected_run_version: i64,
    expected_run_epoch: i64,
    expected_step_key: &str,
    expected_background_task_id: Option<&str>,
    expected_chapter: &ChapterBusinessSnapshot,
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
        || step.step_type != NovelAutopilotStepType::ChapterAnalyze.as_str()
        || step.chapter_id.as_deref() != Some(expected_chapter.chapter_id.as_str())
        || step.chapter_number != Some(expected_chapter.chapter_number)
        || run.active_background_task_id.as_deref() != expected_background_task_id
        || step.background_task_id.as_deref() != expected_background_task_id
    {
        return Err(NovelAutopilotRepositoryError::InvalidTransition);
    }
    Ok((run, step))
}

fn validate_analysis_commit(
    commit: &NovelAutopilotChapterAnalysisCommit,
) -> Result<(), NovelAutopilotRepositoryError> {
    if !commit.payload.is_object() {
        return Err(invalid_config("payload"));
    }
    if commit.result_digest.trim().is_empty() {
        return Err(invalid_config("result_digest"));
    }
    if matches!(
        commit.quality_decision,
        NovelAutopilotQualityDecision::Retry | NovelAutopilotQualityDecision::Reject
    ) {
        return Err(invalid_config("quality_decision"));
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
