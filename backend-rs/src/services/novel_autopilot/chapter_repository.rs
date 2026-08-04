use std::fmt;

use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    QueryFilter, QuerySelect, Set, TransactionTrait,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    models::{chapter, chapter_draft_attempt, novel_autopilot_run, novel_autopilot_step_run},
    services::{
        chapter_content_digest_service::chapter_content_digest,
        chapter_draft_history_service::{
            candidate_draft_apply_history_model, candidate_draft_generated_content_payload,
        },
        chapter_draft_source_service::extract_candidate_draft_full_content,
        chapter_narrative_cleaner_service::{
            contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
        },
    },
};

use super::{
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    types::{
        NovelAutopilotQualityDecision, NovelAutopilotRunStatus, NovelAutopilotStepStatus,
        NovelAutopilotStepType,
    },
};

/// ChapterGenerate 的业务 CAS 快照。
///
/// `chapters` 尚无原生 revision，因此提交时会将此处完整业务字段转换为
/// SQL 条件。生成期间的任意人工编辑都会使更新影响行数为零，防止自动结果
/// 覆盖人工内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterBusinessSnapshot {
    pub(crate) project_id: String,
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) title: String,
    pub(crate) content: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) word_count: i32,
    pub(crate) status: String,
    pub(crate) outline_id: Option<String>,
    pub(crate) sub_index: i32,
    pub(crate) expansion_plan: Option<String>,
    pub(crate) updated_at: Option<NaiveDateTime>,
}

impl ChapterBusinessSnapshot {
    pub(crate) async fn load(
        db: &DatabaseConnection,
        project_id: &str,
        chapter_id: &str,
    ) -> Result<Self, NovelAutopilotRepositoryError> {
        let chapter = chapter::Entity::find_by_id(chapter_id)
            .filter(chapter::Column::ProjectId.eq(project_id))
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        Ok(Self::from_model(&chapter))
    }

    pub(crate) fn content_digest(&self) -> Option<String> {
        self.content.as_deref().map(chapter_content_digest)
    }

    pub(crate) fn from_model(chapter: &chapter::Model) -> Self {
        Self {
            project_id: chapter.project_id.clone(),
            chapter_id: chapter.id.clone(),
            chapter_number: chapter.chapter_number,
            title: chapter.title.clone(),
            content: chapter.content.clone(),
            summary: chapter.summary.clone(),
            word_count: chapter.word_count,
            status: chapter.status.clone(),
            outline_id: chapter.outline_id.clone(),
            sub_index: chapter.sub_index,
            expansion_plan: chapter.expansion_plan.clone(),
            updated_at: chapter.updated_at,
        }
    }
}

/// 允许 ChapterGenerate 写入的最小结果集合。
///
/// 质量决策、重试和人工复核由 durable router 负责；该提交入口仅接受已通过
/// 质量门的正文，因而不会把 `manual_review` / `retry` 误标为完成章节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotChapterGenerateCommit {
    pub(crate) content: String,
    pub(crate) word_count: i32,
    pub(crate) status: String,
    pub(crate) result_digest: String,
    pub(crate) quality_decision: String,
}

const NOVEL_AUTOPILOT_CANDIDATE_SOURCE: &str = "novel_book_autopilot";
const NOVEL_AUTOPILOT_CANDIDATE_WAITING: &str = "waiting_human";
const NOVEL_AUTOPILOT_CANDIDATE_ACCEPTED: &str = "accepted";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NovelAutopilotManualReviewCandidate {
    pub(crate) content: String,
    pub(crate) word_count: i32,
    pub(crate) chapter_status: String,
    pub(crate) result_digest: String,
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) quality_gate_action: Option<String>,
    pub(crate) quality_gate_message: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AcceptedNovelAutopilotChapterCandidate {
    pub(crate) run: novel_autopilot_run::Model,
    pub(crate) step: novel_autopilot_step_run::Model,
    pub(crate) candidate_id: String,
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) word_count: i32,
}

impl NovelAutopilotRepository {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn persist_chapter_manual_review_candidate(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_step_type: NovelAutopilotStepType,
        expected_background_task_id: Option<&str>,
        expected_chapter: &ChapterBusinessSnapshot,
        terminal_status: NovelAutopilotStepStatus,
        error_code: &str,
        candidate: NovelAutopilotManualReviewCandidate,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        validate_manual_review_candidate(&candidate)?;
        if !terminal_status.is_terminal()
            || !matches!(
                expected_step_type,
                NovelAutopilotStepType::ChapterGenerate | NovelAutopilotStepType::ChapterRepair
            )
            || error_code.trim().is_empty()
            || error_code.len() > 160
        {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

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
            || step.step_type != expected_step_type.as_str()
            || step.chapter_id.as_deref() != Some(expected_chapter.chapter_id.as_str())
            || step.chapter_number != Some(expected_chapter.chapter_number)
            || run.active_background_task_id.as_deref() != expected_background_task_id
            || step.background_task_id.as_deref() != expected_background_task_id
        {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

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

        let mut repair_payload = json!({
            "candidate_full_content": candidate.content,
            "candidate_content_digest": candidate.result_digest,
            "content_complete": true,
            "candidate_chapter_status": candidate.chapter_status,
            "autopilot_chapter_snapshot_digest": chapter_business_snapshot_digest(expected_chapter),
        });
        if let Some(message) = candidate
            .quality_gate_message
            .as_deref()
            .map(str::trim)
            .filter(|message| !message.is_empty())
        {
            repair_payload["quality_gate_message"] =
                Value::String(message.chars().take(1000).collect());
        }
        chapter_draft_attempt::ActiveModel {
            id: Set(step_id.to_string()),
            project_id: Set(expected_chapter.project_id.clone()),
            chapter_id: Set(Some(expected_chapter.chapter_id.clone())),
            batch_task_id: Set(None),
            source: Set(NOVEL_AUTOPILOT_CANDIDATE_SOURCE.to_string()),
            attempt_state: Set(NOVEL_AUTOPILOT_CANDIDATE_WAITING.to_string()),
            quality_gate_action: Set(candidate.quality_gate_action),
            quality_gate_decision: Set(Some(
                NovelAutopilotQualityDecision::ManualReview
                    .as_str()
                    .to_string(),
            )),
            word_count: Set(candidate.word_count),
            summary_preview: Set(Some(
                repair_payload["candidate_full_content"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(220)
                    .collect(),
            )),
            content_preview: Set(Some(
                repair_payload["candidate_full_content"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(4000)
                    .collect(),
            )),
            quality_metrics: Set(candidate.quality_metrics),
            repair_payload: Set(Some(repair_payload)),
            created_at: Set(Some(now)),
        }
        .insert(&txn)
        .await
        .map_err(database_error)?;

        let mut run_update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::Status,
                Expr::value(NovelAutopilotRunStatus::WaitingHuman.as_str()),
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
            // Preserve the previous complete_step + transition_owned version shape.
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(2),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now));
        if expected_step_type == NovelAutopilotStepType::ChapterRepair {
            run_update = run_update.col_expr(
                novel_autopilot_run::Column::ConsecutiveQualityFailures,
                Expr::col(novel_autopilot_run::Column::ConsecutiveQualityFailures).add(1),
            );
        }
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
                Expr::value(terminal_status.as_str()),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ResultDigest,
                Expr::value(Some(candidate.result_digest)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::QualityDecision,
                Expr::value(Some(
                    NovelAutopilotQualityDecision::ManualReview
                        .as_str()
                        .to_string(),
                )),
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
            .filter(novel_autopilot_step_run::Column::StepType.eq(expected_step_type.as_str()))
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
        Ok(ClaimedNovelAutopilotStep {
            run: find_owned_run(db, &run.id, user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }

    pub(crate) async fn find_waiting_chapter_candidate_id(
        db: &DatabaseConnection,
        project_id: &str,
        step_id: &str,
    ) -> Result<Option<String>, NovelAutopilotRepositoryError> {
        chapter_draft_attempt::Entity::find()
            .select_only()
            .column(chapter_draft_attempt::Column::Id)
            .filter(chapter_draft_attempt::Column::Id.eq(step_id))
            .filter(chapter_draft_attempt::Column::ProjectId.eq(project_id))
            .filter(chapter_draft_attempt::Column::Source.eq(NOVEL_AUTOPILOT_CANDIDATE_SOURCE))
            .filter(
                chapter_draft_attempt::Column::AttemptState.eq(NOVEL_AUTOPILOT_CANDIDATE_WAITING),
            )
            .into_tuple::<String>()
            .one(db)
            .await
            .map_err(database_error)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn accept_chapter_manual_review_candidate(
        db: &DatabaseConnection,
        run_id: &str,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_background_task_id: Option<&str>,
    ) -> Result<AcceptedNovelAutopilotChapterCandidate, NovelAutopilotRepositoryError> {
        let run = find_owned_run(db, run_id, user_id).await?;
        if run.version != expected_run_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if run.epoch != expected_run_epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        if run.status != NovelAutopilotRunStatus::Running.as_str()
            || run.current_step.is_some()
            || run.active_background_task_id.as_deref() != expected_background_task_id
        {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .filter(novel_autopilot_step_run::Column::RunId.eq(run_id))
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let step_type = step
            .step_type
            .parse::<NovelAutopilotStepType>()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidTransition)?;
        if !matches!(
            step_type,
            NovelAutopilotStepType::ChapterGenerate | NovelAutopilotStepType::ChapterRepair
        ) || !step
            .status
            .parse::<NovelAutopilotStepStatus>()
            .is_ok_and(NovelAutopilotStepStatus::is_terminal)
            || !matches!(
                step.error_code.as_deref(),
                Some(
                    "chapter_quality_manual_review"
                        | "chapter_generation_attempts_exhausted"
                        | "chapter_repair_manual_review"
                )
            )
            || step.quality_decision.as_deref()
                != Some(NovelAutopilotQualityDecision::ManualReview.as_str())
        {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }
        let chapter_id = step
            .chapter_id
            .clone()
            .ok_or(NovelAutopilotRepositoryError::InvalidTransition)?;
        let chapter_number = step
            .chapter_number
            .ok_or(NovelAutopilotRepositoryError::InvalidTransition)?;

        let candidate = chapter_draft_attempt::Entity::find_by_id(step_id)
            .filter(chapter_draft_attempt::Column::ProjectId.eq(&run.project_id))
            .filter(chapter_draft_attempt::Column::ChapterId.eq(Some(chapter_id.clone())))
            .filter(chapter_draft_attempt::Column::Source.eq(NOVEL_AUTOPILOT_CANDIDATE_SOURCE))
            .filter(
                chapter_draft_attempt::Column::AttemptState.eq(NOVEL_AUTOPILOT_CANDIDATE_WAITING),
            )
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let (candidate_content, content_complete) =
            extract_candidate_draft_full_content(&candidate);
        if !content_complete {
            return Err(invalid_config("candidate_content"));
        }
        let stored_result_digest = candidate
            .repair_payload
            .as_ref()
            .and_then(|payload| payload.get("candidate_content_digest"))
            .and_then(Value::as_str)
            .filter(|digest| !digest.trim().is_empty())
            .ok_or_else(|| invalid_config("candidate_result_digest"))?;
        let computed_result_digest = chapter_content_digest(&candidate_content);
        if stored_result_digest != computed_result_digest
            || step.result_digest.as_deref() != Some(stored_result_digest)
        {
            return Err(invalid_config("candidate_result_digest"));
        }
        let (candidate_content, _) = sanitize_generated_narrative_text(&candidate_content);
        if candidate_content.trim().is_empty()
            || contains_chapter_workflow_meta_text(&candidate_content)
        {
            return Err(invalid_config("candidate_content"));
        }
        let candidate_status = candidate
            .repair_payload
            .as_ref()
            .and_then(|payload| payload.get("candidate_chapter_status"))
            .and_then(Value::as_str)
            .filter(|status| !status.trim().is_empty())
            .unwrap_or(NovelAutopilotStepStatus::Completed.as_str())
            .to_string();
        if candidate_status != NovelAutopilotStepStatus::Completed.as_str() {
            return Err(invalid_config("candidate_chapter_status"));
        }
        let expected_snapshot_digest = candidate
            .repair_payload
            .as_ref()
            .and_then(|payload| payload.get("autopilot_chapter_snapshot_digest"))
            .and_then(Value::as_str)
            .filter(|digest| !digest.trim().is_empty())
            .ok_or_else(|| invalid_config("candidate_snapshot_digest"))?;
        let current_chapter = chapter::Entity::find_by_id(&chapter_id)
            .filter(chapter::Column::ProjectId.eq(&run.project_id))
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let current_snapshot = ChapterBusinessSnapshot::from_model(&current_chapter);
        if current_snapshot.chapter_number != chapter_number
            || chapter_business_snapshot_digest(&current_snapshot) != expected_snapshot_digest
        {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }

        let now = Utc::now().naive_utc();
        let word_count = i32::try_from(candidate_content.chars().count()).unwrap_or(i32::MAX);
        if word_count <= 0 {
            return Err(invalid_config("candidate_word_count"));
        }
        let txn = db.begin().await.map_err(database_error)?;
        let chapter_update = chapter::Entity::update_many()
            .col_expr(
                chapter::Column::Content,
                Expr::value(Some(candidate_content.clone())),
            )
            .col_expr(chapter::Column::WordCount, Expr::value(word_count))
            .col_expr(chapter::Column::Status, Expr::value(candidate_status))
            .col_expr(chapter::Column::UpdatedAt, Expr::value(Some(now)))
            .filter(chapter::Column::Id.eq(&chapter_id))
            .filter(chapter::Column::ProjectId.eq(&run.project_id))
            .filter(chapter_snapshot_condition(&current_snapshot))
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if chapter_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }

        let generated_content = candidate_draft_generated_content_payload(
            &candidate_content,
            candidate.quality_metrics.clone(),
        );
        candidate_draft_apply_history_model(
            Uuid::new_v4().to_string(),
            &current_chapter,
            generated_content,
            now,
        )
        .insert(&txn)
        .await
        .map_err(database_error)?;

        let candidate_update = chapter_draft_attempt::Entity::update_many()
            .col_expr(
                chapter_draft_attempt::Column::AttemptState,
                Expr::value(NOVEL_AUTOPILOT_CANDIDATE_ACCEPTED),
            )
            .col_expr(
                chapter_draft_attempt::Column::QualityGateDecision,
                Expr::value(Some(
                    NovelAutopilotQualityDecision::Accept.as_str().to_string(),
                )),
            )
            .filter(chapter_draft_attempt::Column::Id.eq(step_id))
            .filter(
                chapter_draft_attempt::Column::AttemptState.eq(NOVEL_AUTOPILOT_CANDIDATE_WAITING),
            )
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if candidate_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let word_delta = i64::from(word_count) - i64::from(current_snapshot.word_count);
        let mut run_update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::CurrentChapterId,
                Expr::value(Some(chapter_id.clone())),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentChapterNumber,
                Expr::value(Some(chapter_number)),
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
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now));
        run_update = match step_type {
            NovelAutopilotStepType::ChapterGenerate => run_update
                .col_expr(
                    novel_autopilot_run::Column::CompletedChapters,
                    Expr::col(novel_autopilot_run::Column::CompletedChapters).add(1),
                )
                .col_expr(
                    novel_autopilot_run::Column::TotalWordCount,
                    Expr::col(novel_autopilot_run::Column::TotalWordCount)
                        .add(i64::from(word_count)),
                ),
            NovelAutopilotStepType::ChapterRepair => run_update.col_expr(
                novel_autopilot_run::Column::TotalWordCount,
                Expr::col(novel_autopilot_run::Column::TotalWordCount).add(word_delta),
            ),
            _ => unreachable!("validated chapter candidate step type"),
        };
        let run_update = run_update
            .filter(novel_autopilot_run::Column::Id.eq(run_id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_run_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(expected_run_epoch))
            .filter(novel_autopilot_run::Column::CurrentStep.is_null())
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

        txn.commit().await.map_err(database_error)?;
        Ok(AcceptedNovelAutopilotChapterCandidate {
            run: find_owned_run(db, run_id, user_id).await?,
            step,
            candidate_id: candidate.id,
            chapter_id,
            chapter_number,
            word_count,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_chapter_generate_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_chapter: &ChapterBusinessSnapshot,
        target_run_status: NovelAutopilotRunStatus,
        chapter_commit: NovelAutopilotChapterGenerateCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        validate_chapter_generate_commit(&chapter_commit)?;
        if !matches!(
            target_run_status,
            NovelAutopilotRunStatus::Running | NovelAutopilotRunStatus::WaitingHuman
        ) {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "target_run_status",
                code: "unsupported",
            });
        }

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
            || step.step_type != NovelAutopilotStepType::ChapterGenerate.as_str()
            || step.chapter_id.as_deref() != Some(expected_chapter.chapter_id.as_str())
            || step.chapter_number != Some(expected_chapter.chapter_number)
            || run.active_background_task_id.as_deref() != expected_background_task_id
            || step.background_task_id.as_deref() != expected_background_task_id
        {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;

        let chapter_update = chapter::Entity::update_many()
            .col_expr(
                chapter::Column::Content,
                Expr::value(Some(chapter_commit.content)),
            )
            .col_expr(
                chapter::Column::WordCount,
                Expr::value(chapter_commit.word_count),
            )
            .col_expr(chapter::Column::Status, Expr::value(chapter_commit.status))
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
                novel_autopilot_run::Column::Status,
                Expr::value(target_run_status.as_str()),
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
                novel_autopilot_run::Column::CurrentChapterId,
                Expr::value(Some(expected_chapter.chapter_id.clone())),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentChapterNumber,
                Expr::value(Some(expected_chapter.chapter_number)),
            )
            .col_expr(
                novel_autopilot_run::Column::CompletedChapters,
                Expr::col(novel_autopilot_run::Column::CompletedChapters).add(1),
            )
            .col_expr(
                novel_autopilot_run::Column::TotalWordCount,
                Expr::col(novel_autopilot_run::Column::TotalWordCount)
                    .add(i64::from(chapter_commit.word_count)),
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
                Expr::value(Some(chapter_commit.result_digest)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::QualityDecision,
                Expr::value(Some(chapter_commit.quality_decision)),
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
                    .eq(NovelAutopilotStepType::ChapterGenerate.as_str()),
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

        Ok(ClaimedNovelAutopilotStep {
            run: find_owned_run(db, &run.id, user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }
}

fn chapter_business_snapshot_digest(snapshot: &ChapterBusinessSnapshot) -> String {
    chapter_content_digest(
        &json!({
            "project_id": snapshot.project_id,
            "chapter_id": snapshot.chapter_id,
            "chapter_number": snapshot.chapter_number,
            "title": snapshot.title,
            "content": snapshot.content,
            "summary": snapshot.summary,
            "word_count": snapshot.word_count,
            "status": snapshot.status,
            "outline_id": snapshot.outline_id,
            "sub_index": snapshot.sub_index,
            "expansion_plan": snapshot.expansion_plan,
            "updated_at": snapshot.updated_at.map(|value| value.format("%Y-%m-%dT%H:%M:%S%.f").to_string()),
        })
        .to_string(),
    )
}

fn validate_manual_review_candidate(
    candidate: &NovelAutopilotManualReviewCandidate,
) -> Result<(), NovelAutopilotRepositoryError> {
    if candidate.content.trim().is_empty() {
        return Err(invalid_config("candidate_content"));
    }
    if candidate.word_count <= 0 {
        return Err(invalid_config("candidate_word_count"));
    }
    if candidate.chapter_status != NovelAutopilotStepStatus::Completed.as_str() {
        return Err(invalid_config("candidate_chapter_status"));
    }
    if candidate.result_digest.trim().is_empty() {
        return Err(invalid_config("candidate_result_digest"));
    }
    if candidate.result_digest != chapter_content_digest(&candidate.content) {
        return Err(invalid_config("candidate_result_digest"));
    }
    Ok(())
}

pub(crate) fn chapter_snapshot_condition(snapshot: &ChapterBusinessSnapshot) -> Condition {
    let mut condition = Condition::all()
        .add(chapter::Column::ChapterNumber.eq(snapshot.chapter_number))
        .add(chapter::Column::Title.eq(&snapshot.title))
        .add(chapter::Column::WordCount.eq(snapshot.word_count))
        .add(chapter::Column::Status.eq(&snapshot.status))
        .add(chapter::Column::SubIndex.eq(snapshot.sub_index));
    condition =
        add_optional_string_condition(condition, chapter::Column::Content, &snapshot.content);
    condition =
        add_optional_string_condition(condition, chapter::Column::Summary, &snapshot.summary);
    condition =
        add_optional_string_condition(condition, chapter::Column::OutlineId, &snapshot.outline_id);
    condition = add_optional_string_condition(
        condition,
        chapter::Column::ExpansionPlan,
        &snapshot.expansion_plan,
    );
    condition.add(match snapshot.updated_at {
        Some(value) => chapter::Column::UpdatedAt.eq(value),
        None => chapter::Column::UpdatedAt.is_null(),
    })
}

fn add_optional_string_condition(
    condition: Condition,
    column: chapter::Column,
    value: &Option<String>,
) -> Condition {
    condition.add(match value.as_deref() {
        Some(value) => column.eq(value),
        None => column.is_null(),
    })
}

fn validate_chapter_generate_commit(
    chapter_commit: &NovelAutopilotChapterGenerateCommit,
) -> Result<(), NovelAutopilotRepositoryError> {
    if chapter_commit.content.trim().is_empty() {
        return Err(invalid_config("content"));
    }
    if chapter_commit.word_count <= 0 {
        return Err(invalid_config("word_count"));
    }
    if chapter_commit.status != NovelAutopilotStepStatus::Completed.as_str() {
        return Err(invalid_config("status"));
    }
    if chapter_commit.result_digest.trim().is_empty() {
        return Err(invalid_config("result_digest"));
    }
    if chapter_commit.quality_decision != NovelAutopilotQualityDecision::Accept.as_str() {
        return Err(invalid_config("quality_decision"));
    }
    Ok(())
}

const fn invalid_config(field: &'static str) -> NovelAutopilotRepositoryError {
    NovelAutopilotRepositoryError::InvalidConfig {
        field,
        code: "invalid",
    }
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

fn database_error(error: impl fmt::Display) -> NovelAutopilotRepositoryError {
    NovelAutopilotRepositoryError::Database(error.to_string())
}
