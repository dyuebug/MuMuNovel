use std::fmt;

use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set, TransactionTrait,
};
use uuid::Uuid;

use crate::models::{chapter, novel_autopilot_run, novel_autopilot_step_run, outline, project};

use super::{
    outline_repository::NovelAutopilotOutlineSnapshot,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus, NovelAutopilotStepType},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotExpandedChapterCommit {
    pub title: String,
    pub summary: String,
    pub sub_index: i32,
    pub expansion_plan: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotOutlineExpansionCommit {
    pub outline_id: String,
    pub chapters: Vec<NovelAutopilotExpandedChapterCommit>,
    pub result_digest: String,
}

impl NovelAutopilotRepository {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_outline_expansion_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_outline: &NovelAutopilotOutlineSnapshot,
        commit: NovelAutopilotOutlineExpansionCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        validate_commit(&commit)?;

        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = find_owned_run(db, &step.run_id, user_id).await?;
        validate_execution_fence(
            &run,
            &step,
            expected_run_version,
            expected_run_epoch,
            expected_step_key,
            expected_background_task_id,
        )?;

        let txn = db.begin().await.map_err(database_error)?;
        let current_project = project::Entity::find_by_id(&run.project_id)
            .filter(project::Column::UserId.eq(user_id))
            .one(&txn)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::BusinessDataChanged)?;
        let current_outlines = outline::Entity::find()
            .filter(outline::Column::ProjectId.eq(&run.project_id))
            .all(&txn)
            .await
            .map_err(database_error)?;
        let current_chapters = chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(&run.project_id))
            .all(&txn)
            .await
            .map_err(database_error)?;
        let next_chapter_number = current_chapters
            .iter()
            .map(|chapter| chapter.chapter_number)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(NovelAutopilotRepositoryError::InvalidConfig {
                field: "chapter_number",
                code: "overflow",
            })?;
        let current_snapshot = NovelAutopilotOutlineSnapshot::from_models(
            current_project,
            current_outlines,
            current_chapters,
        );
        if &current_snapshot != expected_outline
            || !current_snapshot.contains_outline(&commit.outline_id)
            || current_snapshot.has_chapters_for_outline(&commit.outline_id)
        {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }

        let now = Utc::now().naive_utc();
        for (offset, item) in commit.chapters.iter().enumerate() {
            let offset = i32::try_from(offset).map_err(|_| {
                NovelAutopilotRepositoryError::InvalidConfig {
                    field: "chapters",
                    code: "too_many",
                }
            })?;
            let chapter_number = next_chapter_number.checked_add(offset).ok_or(
                NovelAutopilotRepositoryError::InvalidConfig {
                    field: "chapter_number",
                    code: "overflow",
                },
            )?;
            chapter::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                project_id: Set(run.project_id.clone()),
                chapter_number: Set(chapter_number),
                title: Set(item.title.clone()),
                content: Set(Some(String::new())),
                summary: Set(Some(item.summary.clone())),
                word_count: Set(0),
                status: Set("pending".to_string()),
                outline_id: Set(Some(commit.outline_id.clone())),
                sub_index: Set(item.sub_index),
                expansion_plan: Set(Some(item.expansion_plan.clone())),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            }
            .insert(&txn)
            .await
            .map_err(database_error)?;
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
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
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
                Expr::value(NovelAutopilotStepStatus::Completed.as_str()),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ResultDigest,
                Expr::value(Some(commit.result_digest)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::QualityDecision,
                Expr::value(None::<String>),
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
            .filter(
                novel_autopilot_step_run::Column::StepType
                    .eq(NovelAutopilotStepType::OutlineExpand.as_str()),
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

fn validate_commit(
    commit: &NovelAutopilotOutlineExpansionCommit,
) -> Result<(), NovelAutopilotRepositoryError> {
    if commit.outline_id.trim().is_empty() || commit.chapters.is_empty() {
        return Err(NovelAutopilotRepositoryError::InvalidConfig {
            field: "chapters",
            code: "required",
        });
    }
    for (index, item) in commit.chapters.iter().enumerate() {
        let expected_sub_index =
            i32::try_from(index + 1).map_err(|_| NovelAutopilotRepositoryError::InvalidConfig {
                field: "sub_index",
                code: "overflow",
            })?;
        if item.title.trim().is_empty()
            || item.sub_index != expected_sub_index
            || serde_json::from_str::<serde_json::Value>(&item.expansion_plan).is_err()
        {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "chapters",
                code: "invalid",
            });
        }
    }
    Ok(())
}

fn validate_execution_fence(
    run: &novel_autopilot_run::Model,
    step: &novel_autopilot_step_run::Model,
    expected_run_version: i64,
    expected_run_epoch: i64,
    expected_step_key: &str,
    expected_background_task_id: Option<&str>,
) -> Result<(), NovelAutopilotRepositoryError> {
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
        || step.step_type != NovelAutopilotStepType::OutlineExpand.as_str()
        || run.active_background_task_id.as_deref() != expected_background_task_id
        || step.background_task_id.as_deref() != expected_background_task_id
    {
        return Err(NovelAutopilotRepositoryError::InvalidTransition);
    }
    Ok(())
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

fn database_error(error: impl fmt::Display) -> NovelAutopilotRepositoryError {
    NovelAutopilotRepositoryError::Database(error.to_string())
}
