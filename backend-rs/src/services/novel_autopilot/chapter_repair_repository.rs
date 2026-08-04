use std::fmt;

use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};

use crate::{
    models::{chapter, chapter_draft_attempt, novel_autopilot_run, novel_autopilot_step_run},
    services::{
        chapter_content_digest_service::chapter_content_digest,
        chapter_draft_source_service::extract_candidate_draft_full_content,
        chapter_generation_history_persistence_service::build_single_generation_candidate_draft_attempt_active_model,
        chapter_repair_generation_service::{
            CHAPTER_REPAIR_RETRY_SOURCE, CHAPTER_REPAIR_RETRY_STATE,
        },
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotChapterRepairCommit {
    pub(crate) content: String,
    pub(crate) word_count: i32,
    pub(crate) status: String,
    pub(crate) result_digest: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NovelAutopilotChapterRepairFailureEvidence {
    pub(crate) expected_chapter: ChapterBusinessSnapshot,
    pub(crate) draft_attempt: chapter_draft_attempt::Model,
    pub(crate) result_digest: String,
}

impl NovelAutopilotRepository {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_chapter_repair_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_chapter: &ChapterBusinessSnapshot,
        commit: NovelAutopilotChapterRepairCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        validate_repair_commit(&commit)?;
        let (run, step) = load_and_validate_repair_fence(
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
        let word_count_delta =
            i64::from(commit.word_count) - i64::from(expected_chapter.word_count);
        let txn = db.begin().await.map_err(database_error)?;

        let chapter_update = chapter::Entity::update_many()
            .col_expr(chapter::Column::Content, Expr::value(Some(commit.content)))
            .col_expr(chapter::Column::WordCount, Expr::value(commit.word_count))
            .col_expr(chapter::Column::Status, Expr::value(commit.status))
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
                    .eq(NovelAutopilotStepType::ChapterRepair.as_str()),
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
    pub(crate) async fn finish_chapter_repair_failure(
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
        quality_decision: NovelAutopilotQualityDecision,
        candidate_evidence: Option<NovelAutopilotChapterRepairFailureEvidence>,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        if let Some(evidence) = candidate_evidence.as_ref() {
            validate_repair_failure_evidence(
                evidence,
                step_id,
                failure_counter_kind,
                waiting_human,
                quality_decision,
            )?;
        }
        let (run, step) = if let Some(evidence) = candidate_evidence.as_ref() {
            let fence = load_and_validate_repair_fence(
                db,
                step_id,
                user_id,
                expected_run_version,
                expected_run_epoch,
                expected_step_key,
                expected_background_task_id,
                &evidence.expected_chapter,
            )
            .await?;
            validate_repair_failure_scope(evidence, &fence.0, expected_run_epoch)?;
            fence
        } else {
            load_and_validate_failure_fence(
                db,
                step_id,
                user_id,
                expected_run_version,
                expected_run_epoch,
                expected_step_key,
                expected_background_task_id,
            )
            .await?
        };

        let now = Utc::now().naive_utc();
        let target_status = if waiting_human {
            NovelAutopilotRunStatus::WaitingHuman
        } else {
            NovelAutopilotRunStatus::Running
        };
        let txn = db.begin().await.map_err(database_error)?;
        if let Some(evidence) = candidate_evidence.as_ref() {
            let chapter_fence = chapter::Entity::update_many()
                .col_expr(
                    chapter::Column::UpdatedAt,
                    Expr::col(chapter::Column::UpdatedAt).into(),
                )
                .filter(chapter::Column::Id.eq(&evidence.expected_chapter.chapter_id))
                .filter(chapter::Column::ProjectId.eq(&evidence.expected_chapter.project_id))
                .filter(chapter_snapshot_condition(&evidence.expected_chapter))
                .exec(&txn)
                .await
                .map_err(database_error)?;
            if chapter_fence.rows_affected != 1 {
                return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
            }
            build_single_generation_candidate_draft_attempt_active_model(&evidence.draft_attempt)
                .insert(&txn)
                .await
                .map_err(database_error)?;
        }
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

        let mut step_update = novel_autopilot_step_run::Entity::update_many()
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
            );
        if let Some(evidence) = candidate_evidence.as_ref() {
            step_update = step_update.col_expr(
                novel_autopilot_step_run::Column::ResultDigest,
                Expr::value(Some(evidence.result_digest.clone())),
            );
        }
        let step_update = step_update
            .filter(novel_autopilot_step_run::Column::Id.eq(step_id))
            .filter(novel_autopilot_step_run::Column::RunEpoch.eq(expected_run_epoch))
            .filter(novel_autopilot_step_run::Column::StepKey.eq(expected_step_key))
            .filter(novel_autopilot_step_run::Column::Attempt.eq(step.attempt))
            .filter(
                novel_autopilot_step_run::Column::StepType
                    .eq(NovelAutopilotStepType::ChapterRepair.as_str()),
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
async fn load_and_validate_failure_fence(
    db: &DatabaseConnection,
    step_id: &str,
    user_id: &str,
    expected_run_version: i64,
    expected_run_epoch: i64,
    expected_step_key: &str,
    expected_background_task_id: Option<&str>,
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
    if run.status != NovelAutopilotRunStatus::Running.as_str()
        || step.status != NovelAutopilotStepStatus::Running.as_str()
        || run.current_step.as_deref() != Some(expected_step_key)
        || step.step_key != expected_step_key
        || step.step_type != NovelAutopilotStepType::ChapterRepair.as_str()
        || run.active_background_task_id.as_deref() != expected_background_task_id
        || step.background_task_id.as_deref() != expected_background_task_id
    {
        return Err(NovelAutopilotRepositoryError::InvalidTransition);
    }
    Ok((run, step))
}

#[allow(clippy::too_many_arguments)]
async fn load_and_validate_repair_fence(
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
        || step.step_type != NovelAutopilotStepType::ChapterRepair.as_str()
        || step.chapter_id.as_deref() != Some(expected_chapter.chapter_id.as_str())
        || step.chapter_number != Some(expected_chapter.chapter_number)
        || run.active_background_task_id.as_deref() != expected_background_task_id
        || step.background_task_id.as_deref() != expected_background_task_id
    {
        return Err(NovelAutopilotRepositoryError::InvalidTransition);
    }
    Ok((run, step))
}

fn validate_repair_commit(
    commit: &NovelAutopilotChapterRepairCommit,
) -> Result<(), NovelAutopilotRepositoryError> {
    if commit.content.trim().is_empty() {
        return Err(invalid_config("content"));
    }
    if commit.word_count <= 0 {
        return Err(invalid_config("word_count"));
    }
    if commit.status != NovelAutopilotStepStatus::Completed.as_str() {
        return Err(invalid_config("status"));
    }
    if commit.result_digest.trim().is_empty() {
        return Err(invalid_config("result_digest"));
    }
    Ok(())
}

fn validate_repair_failure_evidence(
    evidence: &NovelAutopilotChapterRepairFailureEvidence,
    step_id: &str,
    failure_counter_kind: NovelAutopilotFailureCounterKind,
    waiting_human: bool,
    quality_decision: NovelAutopilotQualityDecision,
) -> Result<(), NovelAutopilotRepositoryError> {
    let draft = &evidence.draft_attempt;
    if failure_counter_kind != NovelAutopilotFailureCounterKind::Quality
        || waiting_human
        || !matches!(
            quality_decision,
            NovelAutopilotQualityDecision::Retry | NovelAutopilotQualityDecision::AutoRepair
        )
        || evidence.result_digest.trim().is_empty()
        || draft.id != step_id
        || draft.project_id != evidence.expected_chapter.project_id
        || draft.chapter_id.as_deref() != Some(evidence.expected_chapter.chapter_id.as_str())
        || draft.batch_task_id.is_some()
        || draft.source != CHAPTER_REPAIR_RETRY_SOURCE
        || draft.attempt_state != CHAPTER_REPAIR_RETRY_STATE
        || draft.word_count <= 0
        || draft.repair_payload.is_none()
    {
        return Err(NovelAutopilotRepositoryError::InvalidTransition);
    }
    Ok(())
}

fn validate_repair_failure_scope(
    evidence: &NovelAutopilotChapterRepairFailureEvidence,
    run: &novel_autopilot_run::Model,
    expected_run_epoch: i64,
) -> Result<(), NovelAutopilotRepositoryError> {
    let payload = evidence
        .draft_attempt
        .repair_payload
        .as_ref()
        .ok_or(NovelAutopilotRepositoryError::InvalidTransition)?;
    let (candidate_content, content_complete) =
        extract_candidate_draft_full_content(&evidence.draft_attempt);
    let source_digest = evidence
        .expected_chapter
        .content_digest()
        .ok_or_else(|| invalid_config("source_content_digest"))?;
    let analysis_id_is_valid = payload
        .get("analysis_id")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    if payload.get("run_id").and_then(serde_json::Value::as_str) != Some(run.id.as_str())
        || payload.get("run_epoch").and_then(serde_json::Value::as_i64) != Some(expected_run_epoch)
        || payload
            .get("source_content_digest")
            .and_then(serde_json::Value::as_str)
            != Some(source_digest.as_str())
        || !analysis_id_is_valid
        || payload
            .get("candidate_content_digest")
            .and_then(serde_json::Value::as_str)
            != Some(evidence.result_digest.as_str())
        || !content_complete
        || chapter_content_digest(&candidate_content) != evidence.result_digest
        || i32::try_from(candidate_content.chars().count()).unwrap_or(i32::MAX)
            != evidence.draft_attempt.word_count
    {
        return Err(NovelAutopilotRepositoryError::InvalidTransition);
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
