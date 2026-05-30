use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

use crate::ai::{service::AIService, AIConfig};
use crate::models::{batch_generation_task, chapter, plot_analysis};
use crate::services::chapter_analysis_runtime_service::{
    analyze_generated_chapter_follow_up, execute_prepared_chapter_analysis_trigger,
    prepare_chapter_analysis_trigger,
};
use crate::services::chapter_batch_generation_runtime_checkpoint_service::{
    build_batch_generation_runtime_checkpoint_for_stage, BatchGenerationFailureKind,
    BatchGenerationSnapshotStage,
};
use crate::services::chapter_batch_generation_quality_status_service::{
    resolve_failed_terminal_semantics_from_sources, BatchGenerationFailedTerminalKind,
    BatchGenerationQualityStatusContext,
};
use crate::services::chapter_batch_generation_snapshot_service::{
    load_batch_generation_snapshot, replace_batch_generation_runtime_snapshot_for_resume,
    upsert_batch_generation_runtime_snapshot,
};
use crate::services::chapter_batch_generation_write_workflow_service::{
    active_story_repair_payload_from_runtime_state,
    build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload,
    load_recent_batch_story_repair_quality_summary,
    parse_batch_generation_request_runtime_state,
};
use crate::services::chapter_generation_research_payload_service::build_single_chapter_research_provider_payload;
use crate::services::chapter_generation_runtime_service::{
    generate_and_persist_chapter_content_with_provider_payload, GeneratedChapterResult,
};
use crate::services::chapter_single_generation_prepare_service::{
    SingleChapterGenerationCompatOptions, SingleChapterGenerationTarget,
};
use crate::services::chapter_single_generation_runtime_state_service::build_prompt_overrides_from_compat_options;
use crate::services::chapter_story_repair_quality_context_service::{
    advance_quality_metrics_summary_state, aggregate_story_repair_quality_summaries,
    build_quality_metrics_summary_from_state, extract_quality_history_context,
    merge_active_story_repair_payloads,
    restore_active_story_repair_payload_from_quality_context,
    restore_story_repair_compat_options_from_active_snapshot,
    build_quality_metrics_summary_state_from_history,
};

use super::chapter_batch_generation_resume_semantics_service::ResumeBatchGenerationCommandState;

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationExecutionInput {
    pub(crate) user_id: String,
    pub(crate) chapter_ids: Vec<String>,
    pub(crate) target_word_count: i32,
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) ai_config: AIConfig,
}

pub(crate) fn dispatch_batch_generation_runtime(
    db: DatabaseConnection,
    task_id: String,
    execution_input: BatchGenerationExecutionInput,
) {
    tokio::spawn(async move {
        execute_batch_generation_runtime(&db, &task_id, execution_input).await;
    });
}

struct BatchGenerationRuntimeSession {
    ai_service: AIService,
    user_id: String,
    target_word_count: i32,
    compat_options: SingleChapterGenerationCompatOptions,
    total_chapters: i32,
}

impl BatchGenerationRuntimeSession {
    fn from_execution_input(execution_input: BatchGenerationExecutionInput) -> (Self, Vec<String>) {
        let BatchGenerationExecutionInput {
            user_id,
            chapter_ids,
            target_word_count,
            compat_options,
            ai_config,
        } = execution_input;

        (
            Self {
                ai_service: AIService::new(ai_config),
                user_id,
                target_word_count,
                compat_options,
                total_chapters: chapter_ids.len() as i32,
            },
            chapter_ids,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelFieldUpdate<T> {
    Keep,
    Set(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskTimestampUpdate {
    Keep,
    Clear,
    Now,
}

pub(crate) async fn reset_batch_generation_task_for_resume(
    db: &DatabaseConnection,
    task: &ResumeBatchGenerationCommandState,
    runtime_state_seed: Option<Value>,
) -> Result<(), String> {
    let reset_semantics = task.resolve_reset_semantics();
    let mut active = batch_generation_task::ActiveModel {
        id: Set(task.batch_id.clone()),
        ..Default::default()
    };
    active.failed_chapters = Set(reset_semantics.failed_chapters.clone());
    BatchGenerationTaskStage::ResumeReset.apply_to_active_model(
        &mut active,
        reset_semantics.current_chapter_id.as_deref(),
        reset_semantics.current_chapter_number,
        reset_semantics.completed_chapters,
        task.total_chapters,
        None,
        Utc::now().naive_utc(),
    );

    active.update(db).await.map_err(|error| error.to_string())?;
    let resume_checkpoint =
        build_batch_generation_resume_runtime_checkpoint(task, runtime_state_seed);

    replace_batch_generation_runtime_snapshot_for_resume(db, &task.batch_id, resume_checkpoint).await
}

pub(crate) fn build_batch_generation_resume_runtime_checkpoint(
    task: &ResumeBatchGenerationCommandState,
    runtime_state_seed: Option<Value>,
) -> Value {
    let reset_semantics = task.resolve_reset_semantics();
    let mut checkpoint = reset_semantics.build_resume_checkpoint(task.total_chapters);
    if let (Some(checkpoint_object), Some(Value::Object(seed_object))) =
        (checkpoint.as_object_mut(), runtime_state_seed)
    {
        checkpoint_object.extend(seed_object);
    }
    checkpoint
}

fn append_failed_chapter_entry(
    failed_chapters: &Value,
    failed_entry: Option<&Value>,
) -> Value {
    let mut items = failed_chapters.as_array().cloned().unwrap_or_default();
    if let Some(entry) = failed_entry.filter(|entry| entry.is_object()) {
        items.push(entry.clone());
    }
    Value::Array(items)
}

fn build_batch_generation_failed_chapter_entry(
    chapter_id: Option<&str>,
    chapter_number: Option<i32>,
    chapter_title: Option<&str>,
    task_error_message: &str,
    retry_count: i32,
) -> Value {
    json!({
        "chapter_id": chapter_id,
        "chapter_number": chapter_number,
        "title": chapter_title,
        "error": task_error_message,
        "retry_count": retry_count.max(0),
    })
}

fn build_quality_gate_blocked_failed_chapter_entry(
    chapter_id: Option<&str>,
    chapter_number: Option<i32>,
    chapter_title: Option<&str>,
    task_error_message: &str,
    retry_count: i32,
    quality_gate_label: &str,
) -> Value {
    let mut entry = build_batch_generation_failed_chapter_entry(
        chapter_id,
        chapter_number,
        chapter_title,
        task_error_message,
        retry_count,
    );
    if let Some(object) = entry.as_object_mut() {
        apply_manual_review_terminal_fields(object, quality_gate_label);
    }
    entry
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchGenerationTaskStage {
    ResumeReset,
    Preparing,
    ChapterStarted,
    ChapterSucceeded,
    Cancelled,
    Failed,
}

impl BatchGenerationTaskStage {
    fn status(self, completed_chapters: i32, total_chapters: i32) -> &'static str {
        match self {
            Self::ResumeReset => "pending",
            Self::Preparing | Self::ChapterStarted => "running",
            Self::ChapterSucceeded => {
                if completed_chapters >= total_chapters {
                    "completed"
                } else {
                    "running"
                }
            }
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    fn started_at_update(self) -> TaskTimestampUpdate {
        match self {
            Self::ResumeReset => TaskTimestampUpdate::Clear,
            Self::Preparing => TaskTimestampUpdate::Now,
            Self::ChapterStarted | Self::ChapterSucceeded | Self::Cancelled | Self::Failed => {
                TaskTimestampUpdate::Keep
            }
        }
    }

    fn completed_at_update(self, completed_chapters: i32, total_chapters: i32) -> TaskTimestampUpdate {
        match self {
            Self::ResumeReset | Self::Preparing => TaskTimestampUpdate::Clear,
            Self::ChapterStarted => TaskTimestampUpdate::Keep,
            Self::ChapterSucceeded => {
                if completed_chapters >= total_chapters {
                    TaskTimestampUpdate::Now
                } else {
                    TaskTimestampUpdate::Keep
                }
            }
            Self::Cancelled | Self::Failed => TaskTimestampUpdate::Now,
        }
    }

    fn error_message_update(self, error_message: Option<String>) -> ModelFieldUpdate<Option<String>> {
        match self {
            Self::ResumeReset | Self::Preparing | Self::ChapterStarted | Self::ChapterSucceeded => {
                ModelFieldUpdate::Set(None)
            }
            Self::Cancelled => ModelFieldUpdate::Keep,
            Self::Failed => ModelFieldUpdate::Set(error_message),
        }
    }

    fn completed_chapters_update(self, completed_chapters: i32) -> ModelFieldUpdate<i32> {
        match self {
            Self::ResumeReset | Self::ChapterStarted | Self::ChapterSucceeded | Self::Failed => {
                ModelFieldUpdate::Set(completed_chapters)
            }
            Self::Preparing | Self::Cancelled => ModelFieldUpdate::Keep,
        }
    }

    fn current_retry_count_update(self) -> ModelFieldUpdate<i32> {
        match self {
            Self::ResumeReset | Self::Preparing => ModelFieldUpdate::Set(0),
            Self::ChapterStarted | Self::ChapterSucceeded | Self::Cancelled | Self::Failed => {
                ModelFieldUpdate::Keep
            }
        }
    }

    fn current_chapter_id_update(self, current_chapter_id: Option<&str>) -> ModelFieldUpdate<Option<String>> {
        match self {
            Self::ResumeReset | Self::ChapterStarted | Self::ChapterSucceeded | Self::Failed => {
                ModelFieldUpdate::Set(current_chapter_id.map(str::to_string))
            }
            Self::Preparing | Self::Cancelled => ModelFieldUpdate::Keep,
        }
    }

    fn current_chapter_number_update(self, current_chapter_number: Option<i32>) -> ModelFieldUpdate<Option<i32>> {
        match self {
            Self::ResumeReset | Self::ChapterStarted | Self::ChapterSucceeded | Self::Failed => {
                ModelFieldUpdate::Set(current_chapter_number)
            }
            Self::Preparing | Self::Cancelled => ModelFieldUpdate::Keep,
        }
    }

    fn total_chapters_update(self, total_chapters: i32) -> ModelFieldUpdate<i32> {
        match self {
            Self::ChapterStarted => ModelFieldUpdate::Set(total_chapters),
            Self::ResumeReset
            | Self::Preparing
            | Self::ChapterSucceeded
            | Self::Cancelled
            | Self::Failed => ModelFieldUpdate::Keep,
        }
    }

    async fn persist_for_task(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        current_chapter_id: Option<&str>,
        current_chapter_number: Option<i32>,
        completed_chapters: i32,
        total_chapters: i32,
        error_message: Option<String>,
        now: chrono::NaiveDateTime,
    ) -> Result<(), String> {
        if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
        {
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            self.apply_to_active_model(
                &mut active,
                current_chapter_id,
                current_chapter_number,
                completed_chapters,
                total_chapters,
                error_message,
                now,
            );
            active.update(db).await.map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    fn apply_to_active_model(
        self,
        active: &mut batch_generation_task::ActiveModel,
        current_chapter_id: Option<&str>,
        current_chapter_number: Option<i32>,
        completed_chapters: i32,
        total_chapters: i32,
        error_message: Option<String>,
        now: chrono::NaiveDateTime,
    ) {
        active.status = Set(self.status(completed_chapters, total_chapters).to_string());

        match self.started_at_update() {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.started_at = Set(None),
            TaskTimestampUpdate::Now => active.started_at = Set(Some(now)),
        }

        match self.completed_at_update(completed_chapters, total_chapters) {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.completed_at = Set(None),
            TaskTimestampUpdate::Now => active.completed_at = Set(Some(now)),
        }

        match self.error_message_update(error_message) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.error_message = Set(value),
        }

        match self.completed_chapters_update(completed_chapters) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.completed_chapters = Set(value),
        }

        match self.current_retry_count_update() {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_retry_count = Set(value),
        }

        match self.current_chapter_id_update(current_chapter_id) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_id = Set(value),
        }

        match self.current_chapter_number_update(current_chapter_number) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_number = Set(value),
        }

        match self.total_chapters_update(total_chapters) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.total_chapters = Set(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationRuntimePersistencePlan {
    task_stage: BatchGenerationTaskStage,
    checkpoint_stage: BatchGenerationSnapshotStage,
    current_chapter_id: Option<String>,
    current_chapter_number: Option<i32>,
    completed_chapters: i32,
    total_chapters: i32,
    current_retry_count: Option<i32>,
    error_message: Option<String>,
    failed_chapter_entry: Option<Value>,
}

impl BatchGenerationRuntimePersistencePlan {
    fn preparing(total_chapters: i32) -> Self {
        Self {
            task_stage: BatchGenerationTaskStage::Preparing,
            checkpoint_stage: BatchGenerationSnapshotStage::Preparing,
            current_chapter_id: None,
            current_chapter_number: None,
            completed_chapters: 0,
            total_chapters,
            current_retry_count: Some(0),
            error_message: None,
            failed_chapter_entry: None,
        }
    }

    pub(crate) fn cancelled(completed_chapters: i32, total_chapters: i32) -> Self {
        Self {
            task_stage: BatchGenerationTaskStage::Cancelled,
            checkpoint_stage: BatchGenerationSnapshotStage::Cancelled,
            current_chapter_id: None,
            current_chapter_number: None,
            completed_chapters,
            total_chapters,
            current_retry_count: None,
            error_message: None,
            failed_chapter_entry: None,
        }
    }

    fn chapter_started(
        chapter_model: &chapter::Model,
        completed_chapters: i32,
        total_chapters: i32,
        retry_count: i32,
    ) -> Self {
        Self {
            task_stage: BatchGenerationTaskStage::ChapterStarted,
            checkpoint_stage: BatchGenerationSnapshotStage::ChapterStarted,
            current_chapter_id: Some(chapter_model.id.clone()),
            current_chapter_number: Some(chapter_model.chapter_number),
            completed_chapters,
            total_chapters,
            current_retry_count: Some(retry_count.max(0)),
            error_message: None,
            failed_chapter_entry: None,
        }
    }

    fn chapter_succeeded(
        chapter_model: &chapter::Model,
        completed_chapters: i32,
        total_chapters: i32,
    ) -> Self {
        Self {
            task_stage: BatchGenerationTaskStage::ChapterSucceeded,
            checkpoint_stage: BatchGenerationSnapshotStage::ChapterSucceeded,
            current_chapter_id: Some(chapter_model.id.clone()),
            current_chapter_number: Some(chapter_model.chapter_number),
            completed_chapters,
            total_chapters,
            current_retry_count: Some(0),
            error_message: None,
            failed_chapter_entry: None,
        }
    }

    fn failed(
        chapter_id: Option<&str>,
        chapter_number: Option<i32>,
        chapter_title: Option<&str>,
        completed_chapters: i32,
        total_chapters: i32,
        failure_kind: BatchGenerationFailureKind,
        retry_count: i32,
        task_error_message: String,
    ) -> Self {
        let failed_chapter_entry = Some(build_batch_generation_failed_chapter_entry(
            chapter_id,
            chapter_number,
            chapter_title,
            &task_error_message,
            retry_count,
        ));
        Self {
            task_stage: BatchGenerationTaskStage::Failed,
            checkpoint_stage: BatchGenerationSnapshotStage::Failed(failure_kind),
            current_chapter_id: chapter_id.map(str::to_string),
            current_chapter_number: chapter_number,
            completed_chapters,
            total_chapters,
            current_retry_count: Some(retry_count.max(0)),
            error_message: Some(task_error_message),
            failed_chapter_entry,
        }
    }

    fn failed_quality_gate_blocked(
        chapter_id: Option<&str>,
        chapter_number: Option<i32>,
        chapter_title: Option<&str>,
        completed_chapters: i32,
        total_chapters: i32,
        retry_count: i32,
        quality_gate_label: &str,
        task_error_message: String,
    ) -> Self {
        let failed_chapter_entry = Some(build_quality_gate_blocked_failed_chapter_entry(
            chapter_id,
            chapter_number,
            chapter_title,
            &task_error_message,
            retry_count,
            quality_gate_label,
        ));
        Self {
            task_stage: BatchGenerationTaskStage::Failed,
            checkpoint_stage: BatchGenerationSnapshotStage::Failed(
                BatchGenerationFailureKind::QualityGateBlocked,
            ),
            current_chapter_id: chapter_id.map(str::to_string),
            current_chapter_number: chapter_number,
            completed_chapters,
            total_chapters,
            current_retry_count: Some(retry_count.max(0)),
            error_message: Some(task_error_message),
            failed_chapter_entry,
        }
    }

    pub(crate) async fn persist(self, db: &DatabaseConnection, task_id: &str) -> Result<(), String> {
        let now = Utc::now().naive_utc();
        if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
        {
            let existing_failed_chapters = task_model.failed_chapters.clone();
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            self.task_stage.apply_to_active_model(
                &mut active,
                self.current_chapter_id.as_deref(),
                self.current_chapter_number,
                self.completed_chapters,
                self.total_chapters,
                self.error_message.clone(),
                now,
            );
            if let Some(retry_count) = self.current_retry_count {
                active.current_retry_count = Set(retry_count.max(0));
            }
            if matches!(self.task_stage, BatchGenerationTaskStage::Failed) {
                active.failed_chapters = Set(append_failed_chapter_entry(
                    &existing_failed_chapters,
                    self.failed_chapter_entry.as_ref(),
                ));
            }
            active.update(db).await.map_err(|error| error.to_string())?;
        }

        upsert_batch_generation_runtime_snapshot(
            db,
            task_id,
            build_batch_generation_runtime_checkpoint_for_stage(
                self.checkpoint_stage,
                self.current_chapter_id.as_deref(),
                self.current_chapter_number,
                self.completed_chapters,
                self.total_chapters,
            ),
        )
        .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationStepProgress {
    completed: i32,
    total_chapters: i32,
}

impl BatchGenerationStepProgress {
    fn new(completed: i32, total_chapters: i32) -> Self {
        Self {
            completed,
            total_chapters,
        }
    }

    fn advance(&self) -> Self {
        Self {
            completed: self.completed + 1,
            total_chapters: self.total_chapters,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchGenerationStepOutcome {
    Continue { progress: BatchGenerationStepProgress },
    RetryCurrentChapter { next_retry_count: i32 },
    Stop,
}

fn should_retry_batch_generation_attempt(next_retry_count: i32, max_retries: i32) -> bool {
    next_retry_count >= 0 && next_retry_count <= max_retries.max(0)
}

fn batch_generation_retry_backoff_seconds(next_retry_count: i32) -> u64 {
    let exponent = next_retry_count.clamp(0, 4) as u32;
    2_u64.pow(exponent).min(10)
}

fn build_batch_generation_retry_waiting_snapshot(
    chapter_model: &chapter::Model,
    progress: &BatchGenerationStepProgress,
    next_retry_count: i32,
    max_retries: i32,
    wait_seconds: u64,
    error_message: &str,
) -> Value {
    let mut checkpoint = build_batch_generation_runtime_checkpoint_for_stage(
        BatchGenerationSnapshotStage::ChapterStarted,
        Some(&chapter_model.id),
        Some(chapter_model.chapter_number),
        progress.completed,
        progress.total_chapters,
    );
    if let Some(checkpoint_object) = checkpoint.as_object_mut() {
        checkpoint_object.insert(
            "last_event".to_string(),
            Value::String("chapter_retry".to_string()),
        );
        checkpoint_object.insert(
            "last_message".to_string(),
            Value::String(format!(
                "第 {} 章生成失败，{} 秒后进行第 {} 次重试",
                chapter_model.chapter_number, wait_seconds, next_retry_count
            )),
        );
        checkpoint_object.insert(
            "current_retry_count".to_string(),
            Value::Number(next_retry_count.into()),
        );
        checkpoint_object.insert("max_retries".to_string(), Value::Number(max_retries.into()));
        checkpoint_object.insert(
            "retry_backoff_seconds".to_string(),
            Value::Number((wait_seconds as i64).into()),
        );
        checkpoint_object.insert(
            "last_error".to_string(),
            Value::String(error_message.to_string()),
        );
    }
    checkpoint
}

async fn persist_batch_generation_retry_attempt(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_model: &chapter::Model,
    progress: &BatchGenerationStepProgress,
    next_retry_count: i32,
    max_retries: i32,
    error_message: &str,
) {
    if let Ok(Some(task_model)) = batch_generation_task::Entity::find_by_id(task_id).one(db).await {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.status = Set("running".to_string());
        active.error_message = Set(None);
        active.current_chapter_id = Set(Some(chapter_model.id.clone()));
        active.current_chapter_number = Set(Some(chapter_model.chapter_number));
        active.current_retry_count = Set(next_retry_count.max(0));
        let _ = active.update(db).await;
    }

    let wait_seconds = batch_generation_retry_backoff_seconds(next_retry_count);
    let _ = upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_batch_generation_retry_waiting_snapshot(
            chapter_model,
            progress,
            next_retry_count,
            max_retries,
            wait_seconds,
            error_message,
        ),
    )
    .await;
}

fn restore_batch_generation_runtime_compat_options_from_runtime_state(
    base_compat_options: &SingleChapterGenerationCompatOptions,
    runtime_state_payload: Option<&Value>,
) -> SingleChapterGenerationCompatOptions {
    if let Some(runtime_state_payload) = runtime_state_payload {
        return restore_story_repair_compat_options_from_active_snapshot(
            base_compat_options,
            active_story_repair_payload_from_runtime_state(Some(runtime_state_payload)).as_ref(),
            runtime_state_payload.get("quality_metrics_summary"),
            None,
        );
    }

    base_compat_options.clone()
}

async fn resolve_runtime_compat_options_for_batch_generation_step(
    db: &DatabaseConnection,
    task_id: &str,
    base_compat_options: &SingleChapterGenerationCompatOptions,
) -> SingleChapterGenerationCompatOptions {
    let runtime_state_payload = load_batch_generation_snapshot(db, task_id)
        .await
        .ok()
        .flatten()
        .and_then(|snapshot| snapshot.workflow_runtime_state);

    restore_batch_generation_runtime_compat_options_from_runtime_state(
        base_compat_options,
        runtime_state_payload.as_ref(),
    )
}

async fn build_runtime_provider_payload_for_batch_generation_step(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_model: &chapter::Model,
    compat_options: &SingleChapterGenerationCompatOptions,
) -> Result<
    crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload,
    String,
> {
    build_single_chapter_research_provider_payload(
        db,
        user_id,
        &SingleChapterGenerationTarget {
            project_id: chapter_model.project_id.clone(),
            chapter_id: chapter_model.id.clone(),
            chapter_number: chapter_model.chapter_number,
            title: chapter_model.title.clone(),
        },
        compat_options,
    )
    .await
}

async fn refresh_batch_generation_runtime_story_repair_state(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_model: &chapter::Model,
) {
    let existing_runtime_state = load_batch_generation_snapshot(db, task_id)
        .await
        .ok()
        .flatten()
        .and_then(|snapshot| snapshot.workflow_runtime_state);
    let Some(existing_runtime_state) = existing_runtime_state else {
        return;
    };

    let request_runtime_state =
        parse_batch_generation_request_runtime_state(Some(&existing_runtime_state));
    let explicit_story_repair_payload =
        active_story_repair_payload_from_runtime_state(Some(&existing_runtime_state));
    let quality_summary = load_recent_batch_story_repair_quality_summary(
        db,
        &chapter_model.project_id,
        chapter_model.chapter_number + 1,
    )
    .await
    .ok()
    .flatten();
    let refreshed_runtime_state =
        build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
            &request_runtime_state,
            explicit_story_repair_payload.as_ref(),
            quality_summary.as_ref(),
        );
    let _ = upsert_batch_generation_runtime_snapshot(db, task_id, refreshed_runtime_state).await;
}

fn normalized_quality_guidance_items(value: Option<&Value>, limit: usize) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .take(limit)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn build_current_chapter_quality_summary_from_plot_analysis(
    analysis: &plot_analysis::Model,
) -> Option<Value> {
    let overall_score = analysis.overall_quality_score?;
    let pacing_score = analysis.pacing_score;
    let engagement_score = analysis.engagement_score;
    let coherence_score = analysis.coherence_score;
    let suggestions = normalized_quality_guidance_items(analysis.suggestions.as_ref(), 4);

    let metric_pairs = [
        ("pacing", "节奏", pacing_score),
        ("engagement", "吸引力", engagement_score),
        ("coherence", "连贯性", coherence_score),
    ];
    let weakest_metric = metric_pairs
        .into_iter()
        .filter_map(|(key, label, value)| value.map(|score| (key, label, score)))
        .min_by(|left, right| left.2.total_cmp(&right.2));
    let weakest_metric_key = weakest_metric.map(|item| item.0.to_string());
    let weakest_metric_label = weakest_metric.map(|item| item.1.to_string());
    let weakest_metric_value = weakest_metric.map(|item| item.2);

    let mut focus_areas = Vec::new();
    if pacing_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("节奏".to_string());
    }
    if engagement_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("吸引力".to_string());
    }
    if coherence_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("连贯性".to_string());
    }

    let mut preserve_strengths = Vec::new();
    if pacing_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("节奏稳定".to_string());
    }
    if engagement_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("追读牵引".to_string());
    }
    if coherence_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("逻辑连贯".to_string());
    }
    if preserve_strengths.is_empty() && analysis.hooks_count > 0 {
        preserve_strengths.push("钩子密度".to_string());
    }

    let repair_summary = suggestions
        .first()
        .cloned()
        .or_else(|| analysis.analysis_report.clone())
        .unwrap_or_else(|| "当前章节质量分析已完成，建议继续按分析结果微调正文。".to_string());

    let (quality_gate_status, quality_gate_decision, quality_gate_label) = if overall_score < 6.0 {
        ("failed", "manual_review", "需要人工复核")
    } else if overall_score < 8.0 {
        ("warning", "auto_repair", "建议继续修复")
    } else {
        ("passed", "passed", "当前章节通过")
    };

    let failed_metrics = weakest_metric_label
        .as_ref()
        .map(|label| vec![json!({"label": label})])
        .unwrap_or_default();

    Some(json!({
        "overall_score": overall_score,
        "chapter_count": 1,
        "repair_guidance": {
            "summary": repair_summary,
            "repair_targets": suggestions,
            "preserve_strengths": preserve_strengths,
            "focus_areas": focus_areas,
            "weakest_metric_key": weakest_metric_key,
            "weakest_metric_label": weakest_metric_label,
            "weakest_metric_value": weakest_metric_value,
        },
        "quality_gate": {
            "status": quality_gate_status,
            "decision": quality_gate_decision,
            "label": quality_gate_label,
            "summary": repair_summary,
            "failed_metrics": failed_metrics,
        },
        "quality_runtime_context": {
            "scope": "batch",
            "recent_metrics": [{
                "history_index": 0,
                "overall_score": overall_score,
                "repair_guidance": {
                    "summary": repair_summary,
                    "repair_targets": suggestions,
                    "preserve_strengths": preserve_strengths,
                    "focus_areas": focus_areas,
                },
                "quality_gate": {
                    "status": quality_gate_status,
                    "decision": quality_gate_decision,
                    "label": quality_gate_label,
                    "summary": repair_summary,
                    "failed_metrics": failed_metrics,
                }
            }]
        }
    }))
}

fn build_current_chapter_latest_quality_metrics_from_plot_analysis(
    analysis: &plot_analysis::Model,
) -> Option<Value> {
    let overall_score = analysis.overall_quality_score?;
    let pacing_score = analysis.pacing_score;
    let engagement_score = analysis.engagement_score;
    let coherence_score = analysis.coherence_score;
    let suggestions = normalized_quality_guidance_items(analysis.suggestions.as_ref(), 4);

    let metric_pairs = [
        ("pacing", "节奏", pacing_score),
        ("engagement", "吸引力", engagement_score),
        ("coherence", "连贯性", coherence_score),
    ];
    let weakest_metric = metric_pairs
        .into_iter()
        .filter_map(|(key, label, value)| value.map(|score| (key, label, score)))
        .min_by(|left, right| left.2.total_cmp(&right.2));
    let weakest_metric_key = weakest_metric.map(|item| item.0.to_string());
    let weakest_metric_label = weakest_metric.map(|item| item.1.to_string());
    let weakest_metric_value = weakest_metric.map(|item| item.2);

    let mut focus_areas = Vec::new();
    if pacing_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("节奏".to_string());
    }
    if engagement_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("吸引力".to_string());
    }
    if coherence_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("连贯性".to_string());
    }

    let mut preserve_strengths = Vec::new();
    if pacing_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("节奏稳定".to_string());
    }
    if engagement_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("追读牵引".to_string());
    }
    if coherence_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("逻辑连贯".to_string());
    }
    if preserve_strengths.is_empty() && analysis.hooks_count > 0 {
        preserve_strengths.push("钩子密度".to_string());
    }

    let repair_summary = suggestions
        .first()
        .cloned()
        .or_else(|| analysis.analysis_report.clone())
        .unwrap_or_else(|| "当前章节质量分析已完成，建议继续按分析结果微调正文。".to_string());

    let (quality_gate_status, quality_gate_decision, quality_gate_label) = if overall_score < 6.0 {
        ("failed", "manual_review", "需要人工复核")
    } else if overall_score < 8.0 {
        ("warning", "auto_repair", "建议继续修复")
    } else {
        ("passed", "passed", "当前章节通过")
    };

    let failed_metrics = weakest_metric_label
        .as_ref()
        .map(|label| vec![json!({"label": label})])
        .unwrap_or_default();

    Some(json!({
        "overall_score": overall_score,
        "pacing_score": pacing_score,
        "engagement_score": engagement_score,
        "coherence_score": coherence_score,
        "repair_guidance": {
            "summary": repair_summary,
            "repair_targets": suggestions,
            "preserve_strengths": preserve_strengths,
            "focus_areas": focus_areas,
            "weakest_metric_key": weakest_metric_key,
            "weakest_metric_label": weakest_metric_label,
            "weakest_metric_value": weakest_metric_value,
        },
        "quality_gate": {
            "status": quality_gate_status,
            "decision": quality_gate_decision,
            "label": quality_gate_label,
            "summary": repair_summary,
            "failed_metrics": failed_metrics,
        },
        "quality_runtime_context": {
            "scope": "batch",
            "source": "plot_analysis",
        }
    }))
}

const MAX_BATCH_QUALITY_METRICS_HISTORY: usize = 20;

fn append_quality_metrics_history_event(
    existing_history: Option<&Value>,
    latest_quality_metrics: &Value,
) -> (Value, Option<Value>) {
    let mut history = existing_history
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let dropped_event = if history.len() >= MAX_BATCH_QUALITY_METRICS_HISTORY {
        history.first().cloned()
    } else {
        None
    };
    history.push(latest_quality_metrics.clone());
    if history.len() > MAX_BATCH_QUALITY_METRICS_HISTORY {
        history = history.split_off(history.len() - MAX_BATCH_QUALITY_METRICS_HISTORY);
    }
    (Value::Array(history), dropped_event)
}

fn build_batch_quality_summary_from_state_or_history(
    quality_metrics_summary_state: Option<&Value>,
    quality_metrics_history: &Value,
    fallback_quality_summary: &Value,
) -> Value {
    let history = quality_metrics_history
        .as_array()
        .map(|items| items.iter().filter(|item| item.is_object()).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    build_quality_metrics_summary_from_state(quality_metrics_summary_state, &history, "batch")
        .unwrap_or_else(|| fallback_quality_summary.clone())
}

fn build_batch_generation_runtime_state_payload_from_current_quality(
    request_runtime_state: &crate::services::chapter_batch_generation_write_workflow_service::BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<&Value>,
    existing_quality_metrics_summary_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    quality_summary: &Value,
    latest_quality_metrics: Option<&Value>,
) -> Value {
    let quality_metrics_history_with_drop = latest_quality_metrics.map(|latest_quality_metrics| {
        append_quality_metrics_history_event(existing_quality_metrics_history, latest_quality_metrics)
    });
    let quality_metrics_summary_state = latest_quality_metrics.and_then(|latest_quality_metrics| {
        let history = quality_metrics_history_with_drop
            .as_ref()
            .and_then(|(history, _)| history.as_array())
            .cloned()
            .unwrap_or_default();
        let dropped_event = quality_metrics_history_with_drop
            .as_ref()
            .and_then(|(_, dropped_event)| dropped_event.as_ref());
        advance_quality_metrics_summary_state(
            existing_quality_metrics_summary_state,
            latest_quality_metrics,
            &history,
            dropped_event,
            "batch",
        )
        .or_else(|| build_quality_metrics_summary_state_from_history(&history, "batch"))
    });
    let resolved_quality_summary = quality_metrics_history_with_drop
        .as_ref()
        .map(|(history, _)| {
            build_batch_quality_summary_from_state_or_history(
                quality_metrics_summary_state.as_ref(),
                history,
                quality_summary,
            )
        })
        .unwrap_or_else(|| quality_summary.clone());
    let mut payload = serde_json::Map::from_iter([(
        "batch_request_runtime_state".to_string(),
        json!(request_runtime_state),
    )]);

    let derived_story_repair_payload = restore_active_story_repair_payload_from_quality_context(
        Some(&resolved_quality_summary),
        None,
        "batch",
        "current_chapter_quality",
        "Current chapter quality",
    );
    let active_story_repair_payload = merge_active_story_repair_payloads(
        explicit_story_repair_payload,
        derived_story_repair_payload.as_ref(),
        "batch",
        "current_chapter_quality",
        "Current chapter quality",
    );

    if let Some(active_story_repair_payload) = active_story_repair_payload {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }
    payload.insert(
        "quality_metrics_summary".to_string(),
        resolved_quality_summary.clone(),
    );
    if let Some(quality_metrics_summary_state) = quality_metrics_summary_state {
        payload.insert(
            "quality_metrics_summary_state".to_string(),
            quality_metrics_summary_state,
        );
    }
    if let Some(latest_quality_metrics) = latest_quality_metrics {
        payload.insert(
            "latest_quality_metrics".to_string(),
            latest_quality_metrics.clone(),
        );
    }
    if let Some((quality_metrics_history, _)) = quality_metrics_history_with_drop {
        payload.insert("quality_metrics_history".to_string(), quality_metrics_history);
    }
    payload.insert(
        "quality_history_context".to_string(),
        extract_quality_history_context(Some(&resolved_quality_summary)).unwrap_or(Value::Null),
    );

    Value::Object(payload)
}

async fn build_batch_generation_current_chapter_quality_runtime_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_id: &str,
) -> Option<Value> {
    let latest_analysis = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ChapterId.eq(chapter_id))
        .order_by_desc(plot_analysis::Column::CreatedAt)
        .one(db)
        .await
        .ok()
        .flatten()?;
    let quality_summary = build_current_chapter_quality_summary_from_plot_analysis(&latest_analysis)?;
    let latest_quality_metrics =
        build_current_chapter_latest_quality_metrics_from_plot_analysis(&latest_analysis);
    let existing_snapshot = load_batch_generation_snapshot(db, task_id).await.ok().flatten();
    let existing_runtime_state = existing_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.workflow_runtime_state.clone());
    let request_runtime_state =
        parse_batch_generation_request_runtime_state(existing_runtime_state.as_ref());
    let explicit_story_repair_payload =
        active_story_repair_payload_from_runtime_state(existing_runtime_state.as_ref());
    let existing_quality_metrics_history = existing_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.quality_metrics_history.as_ref())
        .or_else(|| {
            existing_runtime_state
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|state| state.get("quality_metrics_history"))
        });
    let existing_quality_metrics_summary_state = existing_runtime_state
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|state| state.get("quality_metrics_summary_state"));

    Some(build_batch_generation_runtime_state_payload_from_current_quality(
        &request_runtime_state,
        explicit_story_repair_payload.as_ref(),
        existing_quality_metrics_summary_state,
        existing_quality_metrics_history,
        &quality_summary,
        latest_quality_metrics.as_ref(),
    ))
}

async fn persist_batch_generation_step_outcome(
    db: &DatabaseConnection,
    task_id: &str,
    persistence_plan: BatchGenerationRuntimePersistencePlan,
    outcome: BatchGenerationStepOutcome,
) -> BatchGenerationStepOutcome {
    let _ = persistence_plan.persist(db, task_id).await;
    outcome
}

async fn execute_batch_generation_step(
    db: &DatabaseConnection,
    task_id: &str,
    session: &BatchGenerationRuntimeSession,
    chapter_id: &str,
    progress: BatchGenerationStepProgress,
) -> BatchGenerationStepOutcome {
    let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .ok()
        .flatten()
    else {
        return BatchGenerationStepOutcome::Stop;
    };
    if task_model.status == "cancelled" {
        return persist_batch_generation_step_outcome(
            db,
            task_id,
            BatchGenerationRuntimePersistencePlan::cancelled(
                progress.completed,
                progress.total_chapters,
            ),
            BatchGenerationStepOutcome::Stop,
        )
        .await;
    }

    let chapter_model = match chapter::Entity::find_by_id(chapter_id).one(db).await {
        Ok(Some(chapter_model)) => chapter_model,
        Ok(None) => {
            return persist_batch_generation_step_outcome(
                db,
                task_id,
                BatchGenerationRuntimePersistencePlan::failed(
                    Some(chapter_id),
                    None,
                    None,
                    progress.completed,
                    progress.total_chapters,
                    BatchGenerationFailureKind::MissingChapter,
                    task_model.current_retry_count,
                    format!("Chapter not found: {}", chapter_id),
                ),
                BatchGenerationStepOutcome::Stop,
            )
            .await;
        }
        Err(error) => {
            return persist_batch_generation_step_outcome(
                db,
                task_id,
                BatchGenerationRuntimePersistencePlan::failed(
                    Some(chapter_id),
                    None,
                    None,
                    progress.completed,
                    progress.total_chapters,
                    BatchGenerationFailureKind::LoadChapterError,
                    task_model.current_retry_count,
                    error.to_string(),
                ),
                BatchGenerationStepOutcome::Stop,
            )
            .await;
        }
    };

    let mut retry_count = task_model.current_retry_count.max(0);
    let max_retries = task_model.max_retries.max(0);

    loop {
        let resolved_compat_options = resolve_runtime_compat_options_for_batch_generation_step(
            db,
            task_id,
            &session.compat_options,
        )
        .await;
        let prompt_overrides =
            build_prompt_overrides_from_compat_options(&resolved_compat_options);
        let _ = BatchGenerationRuntimePersistencePlan::chapter_started(
            &chapter_model,
            progress.completed,
            progress.total_chapters,
            retry_count,
        )
        .persist(db, task_id)
        .await;

        let provider_payload = match build_runtime_provider_payload_for_batch_generation_step(
            db,
            &session.user_id,
            &chapter_model,
            &resolved_compat_options,
        )
        .await {
            Ok(provider_payload) => provider_payload,
            Err(error) => {
                let next_retry_count = retry_count + 1;
                if should_retry_batch_generation_attempt(next_retry_count, max_retries) {
                    persist_batch_generation_retry_attempt(
                        db,
                        task_id,
                        &chapter_model,
                        &progress,
                        next_retry_count,
                        max_retries,
                        &error,
                    )
                    .await;
                    sleep(Duration::from_secs(batch_generation_retry_backoff_seconds(
                        next_retry_count,
                    )))
                    .await;
                    retry_count = next_retry_count;
                    continue;
                }

                return persist_batch_generation_step_outcome(
                    db,
                    task_id,
                    BatchGenerationRuntimePersistencePlan::failed(
                        Some(&chapter_model.id),
                        Some(chapter_model.chapter_number),
                        Some(&chapter_model.title),
                        progress.completed,
                        progress.total_chapters,
                        BatchGenerationFailureKind::GenerationError,
                        next_retry_count,
                        error,
                    ),
                    BatchGenerationStepOutcome::Stop,
                )
                .await;
            }
        };

        match generate_and_persist_chapter_content_with_provider_payload(
            db,
            &session.ai_service,
            &session.user_id,
            &chapter_model.id,
            session.target_word_count,
            provider_payload,
            &prompt_overrides,
        )
        .await
        {
            Ok(generated_result) => {
                match run_batch_generation_follow_up_analysis_with_failure_contract(
                    db,
                    task_id,
                    session,
                    &chapter_model,
                    &progress,
                    &generated_result,
                )
                .await
                {
                    BatchGenerationStepOutcome::Continue { progress: next_progress } => {
                        refresh_batch_generation_runtime_story_repair_state(
                            db,
                            task_id,
                            &chapter_model,
                        )
                        .await;
                        return persist_batch_generation_step_outcome(
                            db,
                            task_id,
                            BatchGenerationRuntimePersistencePlan::chapter_succeeded(
                                &chapter_model,
                                next_progress.completed,
                                next_progress.total_chapters,
                            ),
                            BatchGenerationStepOutcome::Continue {
                                progress: next_progress,
                            },
                        )
                        .await;
                    }
                    BatchGenerationStepOutcome::RetryCurrentChapter { next_retry_count } => {
                        sleep(Duration::from_secs(batch_generation_retry_backoff_seconds(
                            next_retry_count,
                        )))
                        .await;
                        retry_count = next_retry_count;
                        continue;
                    }
                    BatchGenerationStepOutcome::Stop => return BatchGenerationStepOutcome::Stop,
                }
            }
            Err(task_error_message) => {
                let next_retry_count = retry_count + 1;
                if should_retry_batch_generation_attempt(next_retry_count, max_retries) {
                    persist_batch_generation_retry_attempt(
                        db,
                        task_id,
                        &chapter_model,
                        &progress,
                        next_retry_count,
                        max_retries,
                        &task_error_message,
                    )
                    .await;
                    sleep(Duration::from_secs(batch_generation_retry_backoff_seconds(
                        next_retry_count,
                    )))
                    .await;
                    retry_count = next_retry_count;
                    continue;
                }

                return persist_batch_generation_step_outcome(
                    db,
                    task_id,
                    BatchGenerationRuntimePersistencePlan::failed(
                        Some(&chapter_model.id),
                        Some(chapter_model.chapter_number),
                        Some(&chapter_model.title),
                        progress.completed,
                        progress.total_chapters,
                        BatchGenerationFailureKind::GenerationError,
                        next_retry_count,
                        task_error_message,
                    ),
                    BatchGenerationStepOutcome::Stop,
                )
                .await;
            }
        }
    }
}

async fn run_batch_generation_follow_up_analysis(
    db: &DatabaseConnection,
    batch_task_id: &str,
    session: &BatchGenerationRuntimeSession,
    generated: &GeneratedChapterResult,
) -> Result<(), String> {
    if !session.compat_options.enable_analysis() {
        return Ok(());
    }

    for analysis_retry_count in 0..3 {
        if let Ok(create_state) =
            prepare_chapter_analysis_trigger(db, &generated.chapter_id, &session.user_id).await
        {
            let analysis_task_id = create_state.task_id.clone();
            let _ = upsert_batch_generation_runtime_snapshot(
                db,
                batch_task_id,
                json!({
                    "analysis_task_id": analysis_task_id,
                    "analysis_task_message": format!("第 {} 章分析任务已启动", generated.chapter_number),
                    "analysis_task_progress": 85,
                    "analysis_started_chapter_id": generated.chapter_id,
                    "analysis_started_chapter_number": generated.chapter_number,
                    "analysis_started_at": chrono::Utc::now().to_rfc3339(),
                    "analysis_retry_count": analysis_retry_count,
                    "analysis_max_retries": 3,
                }),
            )
            .await;

            match execute_prepared_chapter_analysis_trigger(db, &session.user_id, &create_state).await {
                Ok(_) => {
                    if let Some(current_quality_snapshot) =
                        build_batch_generation_current_chapter_quality_runtime_snapshot(
                            db,
                            batch_task_id,
                            &generated.chapter_id,
                        )
                        .await
                    {
                        let _ = upsert_batch_generation_runtime_snapshot(
                            db,
                            batch_task_id,
                            current_quality_snapshot,
                        )
                        .await;
                    }
                    let _ = upsert_batch_generation_runtime_snapshot(
                        db,
                        batch_task_id,
                        build_batch_generation_analysis_completed_snapshot(generated, analysis_retry_count),
                    )
                    .await;
                    return Ok(());
                }
                Err(error_message) => {
                    let _ = upsert_batch_generation_runtime_snapshot(
                        db,
                        batch_task_id,
                        json!({
                            "analysis_task_message": format!(
                                "第 {} 章分析失败，准备重试",
                                generated.chapter_number
                            ),
                            "analysis_task_progress": 85,
                            "analysis_last_error": error_message,
                            "analysis_retry_count": analysis_retry_count + 1,
                            "analysis_max_retries": 3,
                        }),
                    )
                    .await;

                    if analysis_retry_count < 2 {
                        let wait_time = 2_i32.pow((analysis_retry_count + 1) as u32).min(10) as u64;
                        sleep(Duration::from_secs(wait_time)).await;
                        continue;
                    }

                    return Err(error_message);
                }
            }
        }

        match analyze_generated_chapter_follow_up(db, &session.user_id, generated).await {
            Ok(_) => {
                if let Some(current_quality_snapshot) =
                    build_batch_generation_current_chapter_quality_runtime_snapshot(
                        db,
                        batch_task_id,
                        &generated.chapter_id,
                    )
                    .await
                {
                    let _ = upsert_batch_generation_runtime_snapshot(
                        db,
                        batch_task_id,
                        current_quality_snapshot,
                    )
                    .await;
                }
                let _ = upsert_batch_generation_runtime_snapshot(
                    db,
                    batch_task_id,
                    build_batch_generation_analysis_completed_snapshot(generated, analysis_retry_count),
                )
                .await;
                return Ok(());
            }
            Err(error) => {
                if analysis_retry_count < 2 {
                    let wait_time = 2_i32.pow((analysis_retry_count + 1) as u32).min(10) as u64;
                    sleep(Duration::from_secs(wait_time)).await;
                    let _ = upsert_batch_generation_runtime_snapshot(
                        db,
                        batch_task_id,
                        json!({
                            "analysis_task_message": format!("第 {} 章分析失败，准备重试", generated.chapter_number),
                            "analysis_task_progress": 85,
                            "analysis_last_error": format_analysis_error_message(&error),
                            "analysis_retry_count": analysis_retry_count + 1,
                            "analysis_max_retries": 3,
                        }),
                    )
                    .await;
                    continue;
                }

                return Err(format_analysis_error_message(&error));
            }
        }
    }

    Err("章节分析失败".to_string())
}

fn build_batch_generation_analysis_completed_snapshot(
    generated: &GeneratedChapterResult,
    analysis_retry_count: i32,
) -> serde_json::Value {
    json!({
        "analysis_task_message": format!("第 {} 章分析完成", generated.chapter_number),
        "analysis_task_progress": 100,
        "analysis_last_error": Value::Null,
        "analysis_retry_count": analysis_retry_count,
        "analysis_max_retries": 3,
        "quality_gate_decision": Value::Null,
        "quality_gate_label": Value::Null,
        "phase": Value::Null,
    })
}

fn build_quality_gate_blocked_runtime_state_patch(
    workflow_runtime_state: Option<&Value>,
    chapter_number: i32,
    manual_review_label: &str,
) -> Value {
    let mut payload = build_manual_review_terminal_runtime_patch_contract(
        chapter_number,
        manual_review_label,
    );
    apply_terminal_quality_runtime_patch_contract(
        &mut payload,
        workflow_runtime_state,
        manual_review_label,
    );

    Value::Object(payload)
}

fn increment_quality_gate_terminal_counts(
    quality_gate_counts: &mut serde_json::Map<String, Value>,
    recent_manual_review_count: &mut i64,
    recent_auto_repair_count: &mut i64,
    decision: Option<&str>,
) {
    let Some(decision) = decision.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };

    let current = quality_gate_counts
        .get(decision)
        .and_then(Value::as_i64)
        .unwrap_or(0);
    quality_gate_counts.insert(decision.to_string(), json!(current + 1));

    match decision {
        "manual_review" => *recent_manual_review_count += 1,
        "auto_repair" | "repair" => *recent_auto_repair_count += 1,
        _ => {}
    }
}

fn normalize_terminal_quality_gate_payload(
    payload: &mut Value,
    manual_review_label: &str,
) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };

    let quality_gate = object
        .entry("quality_gate".to_string())
        .or_insert_with(|| json!({}));
    if let Some(gate_object) = quality_gate.as_object_mut() {
        apply_manual_review_terminal_quality_gate(gate_object, manual_review_label);
    }
}

fn apply_manual_review_terminal_fields(
    object: &mut serde_json::Map<String, Value>,
    manual_review_label: &str,
) {
    object.insert("quality_gate_decision".to_string(), json!("manual_review"));
    object.insert(
        "quality_gate_label".to_string(),
        json!(manual_review_label),
    );
    object.insert("phase".to_string(), json!("quality_blocked"));
}

fn apply_manual_review_terminal_quality_gate(
    gate_object: &mut serde_json::Map<String, Value>,
    manual_review_label: &str,
) {
    gate_object.insert("status".to_string(), json!("failed"));
    gate_object.insert("decision".to_string(), json!("manual_review"));
    gate_object.insert("label".to_string(), json!(manual_review_label));
}

fn normalize_terminal_quality_summary_state(
    summary_state: &mut Value,
    manual_review_label: &str,
) {
    let Some(object) = summary_state.as_object_mut() else {
        return;
    };
    let Some(recent_history) = object
        .get_mut("recent_history")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let Some(last_metric) = recent_history.last_mut() else {
        return;
    };
    normalize_terminal_quality_gate_payload(last_metric, manual_review_label);
}

fn normalize_terminal_quality_history(
    quality_metrics_history: &mut Value,
    manual_review_label: &str,
) {
    let Some(last_metric) = quality_metrics_history
        .as_array_mut()
        .and_then(|history| history.last_mut())
    else {
        return;
    };
    normalize_terminal_quality_gate_payload(last_metric, manual_review_label);
}

fn normalize_terminal_quality_history_context(
    quality_history_context: &mut Value,
    manual_review_label: &str,
) {
    let mut quality_gate_counts = serde_json::Map::new();
    let mut recent_manual_review_count = 0_i64;
    let mut recent_auto_repair_count = 0_i64;
    if let Some(recent_metrics) = quality_history_context
        .get_mut("recent_metrics")
        .and_then(Value::as_array_mut)
    {
        for metric in recent_metrics {
            normalize_terminal_quality_gate_payload(metric, manual_review_label);
            increment_quality_gate_terminal_counts(
                &mut quality_gate_counts,
                &mut recent_manual_review_count,
                &mut recent_auto_repair_count,
                metric
                    .get("quality_gate")
                    .and_then(Value::as_object)
                    .and_then(|gate| gate.get("decision"))
                    .and_then(Value::as_str),
            );
        }
    }
    if let Some(object) = quality_history_context.as_object_mut() {
        object.insert(
            "quality_gate_counts".to_string(),
            Value::Object(quality_gate_counts),
        );
        object.insert(
            "recent_manual_review_count".to_string(),
            json!(recent_manual_review_count),
        );
        object.insert(
            "recent_auto_repair_count".to_string(),
            json!(recent_auto_repair_count),
        );
    }
}

fn apply_terminal_quality_runtime_patch_sections(
    payload: &mut serde_json::Map<String, Value>,
    workflow_runtime_state: Option<&Value>,
    manual_review_label: &str,
) {
    insert_normalized_terminal_quality_payload_field(
        payload,
        workflow_runtime_state,
        "quality_metrics_summary",
        normalize_terminal_quality_gate_payload,
        manual_review_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        workflow_runtime_state,
        "latest_quality_metrics",
        normalize_terminal_quality_gate_payload,
        manual_review_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        workflow_runtime_state,
        "quality_metrics_history",
        normalize_terminal_quality_history,
        manual_review_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        workflow_runtime_state,
        "quality_metrics_summary_state",
        normalize_terminal_quality_summary_state,
        manual_review_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        workflow_runtime_state,
        "quality_history_context",
        normalize_terminal_quality_history_context,
        manual_review_label,
    );
}

fn apply_terminal_quality_runtime_patch_contract(
    payload: &mut serde_json::Map<String, Value>,
    workflow_runtime_state: Option<&Value>,
    manual_review_label: &str,
) {
    apply_terminal_quality_runtime_patch_sections(
        payload,
        workflow_runtime_state,
        manual_review_label,
    );
    insert_terminal_active_story_repair_payload(
        payload,
        workflow_runtime_state,
        manual_review_label,
    );
}

fn build_manual_review_terminal_runtime_patch_contract(
    chapter_number: i32,
    manual_review_label: &str,
) -> serde_json::Map<String, Value> {
    serde_json::Map::from_iter([
        (
            "analysis_task_message".to_string(),
            json!(format!("第 {} 章触发质量门禁，需人工复核", chapter_number)),
        ),
        ("analysis_task_progress".to_string(), json!(100)),
        ("analysis_last_error".to_string(), Value::Null),
        ("quality_gate_decision".to_string(), json!("manual_review")),
        (
            "quality_gate_label".to_string(),
            json!(manual_review_label),
        ),
        ("phase".to_string(), json!("quality_blocked")),
    ])
}

fn build_retry_quality_runtime_patch_contract(
    chapter_number: i32,
    retry_label: &str,
) -> serde_json::Map<String, Value> {
    let mut payload = serde_json::Map::from_iter([
        (
            "analysis_task_message".to_string(),
            json!(format!("第 {} 章触发质量修复，等待重试", chapter_number)),
        ),
        ("analysis_task_progress".to_string(), json!(100)),
        ("analysis_last_error".to_string(), Value::Null),
        ("quality_gate_decision".to_string(), json!("auto_repair")),
        ("quality_gate_label".to_string(), json!(retry_label)),
        ("phase".to_string(), json!("repair_pending")),
    ]);
    apply_retry_quality_runtime_patch_contract(&mut payload, retry_label);
    payload
}

fn apply_retry_quality_runtime_patch_contract(
    payload: &mut serde_json::Map<String, Value>,
    retry_label: &str,
) {
    insert_retry_active_story_repair_payload(payload, retry_label);
}

fn insert_terminal_active_story_repair_payload(
    payload: &mut serde_json::Map<String, Value>,
    workflow_runtime_state: Option<&Value>,
    manual_review_label: &str,
) {
    let Some(mut active_story_repair_payload) =
        active_story_repair_payload_from_runtime_state(workflow_runtime_state)
    else {
        return;
    };
    if let Some(object) = active_story_repair_payload.as_object_mut() {
        apply_manual_review_terminal_fields(object, manual_review_label);
    }
    payload.insert(
        "active_story_repair_payload".to_string(),
        active_story_repair_payload,
    );
}

fn insert_retry_active_story_repair_payload(
    payload: &mut serde_json::Map<String, Value>,
    retry_label: &str,
) {
    let Some(active_story_repair_payload) = payload
        .get("active_story_repair_payload")
        .and_then(Value::as_object)
        .cloned()
    else {
        return;
    };

    let mut next_active_story_repair_payload = active_story_repair_payload;
    next_active_story_repair_payload.insert(
        "quality_gate_decision".to_string(),
        json!("auto_repair"),
    );
    next_active_story_repair_payload.insert(
        "quality_gate_label".to_string(),
        json!(retry_label),
    );
    next_active_story_repair_payload.insert("phase".to_string(), json!("repair_pending"));

    payload.insert(
        "active_story_repair_payload".to_string(),
        Value::Object(next_active_story_repair_payload),
    );
}

fn insert_normalized_terminal_quality_payload_field(
    payload: &mut serde_json::Map<String, Value>,
    workflow_runtime_state: Option<&Value>,
    field_name: &str,
    normalize: fn(&mut Value, &str),
    manual_review_label: &str,
) {
    let Some(mut value) = workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get(field_name))
        .cloned()
    else {
        return;
    };
    normalize(&mut value, manual_review_label);
    payload.insert(field_name.to_string(), value);
}

async fn resolve_batch_generation_quality_gate_manual_review_label(
    db: &DatabaseConnection,
    task_id: &str,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    let snapshot = load_batch_generation_snapshot(db, task_id).await.ok().flatten()?;
    let workflow_runtime_state = snapshot.workflow_runtime_state.as_ref();
    let quality_metrics_summary = snapshot
        .quality_metrics_summary
        .as_ref()
        .or_else(|| workflow_runtime_state.and_then(|state| state.get("quality_metrics_summary")));
    let latest_quality_metrics = snapshot
        .latest_quality_metrics
        .as_ref()
        .or_else(|| workflow_runtime_state.and_then(|state| state.get("latest_quality_metrics")));
    resolve_failed_terminal_semantics_from_sources(
        Some(&json!([])),
        Some(&BatchGenerationQualityStatusContext {
            active_story_repair_payload: workflow_runtime_state
                .and_then(Value::as_object)
                .and_then(|state| state.get("active_story_repair_payload"))
                .cloned(),
            quality_metrics_summary: quality_metrics_summary.cloned(),
            latest_quality_metrics: latest_quality_metrics.cloned(),
        }),
        current_retry_count,
        max_retries,
    )
    .filter(|semantics| semantics.kind == BatchGenerationFailedTerminalKind::ManualReview)
    .map(|semantics| semantics.label)
}

fn format_analysis_error_message(error: &crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError) -> String {
    match error {
        crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::ChapterEmpty => {
            "章节不存在或内容为空".to_string()
        }
        crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::ProjectMissing => {
            "Chapter or project was deleted before analysis".to_string()
        }
        crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::Internal(message) => {
            message.clone()
        }
    }
}

async fn fail_batch_generation_after_analysis(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_model: &chapter::Model,
    progress: &BatchGenerationStepProgress,
    analysis_error: String,
) -> BatchGenerationStepOutcome {
    let _ = upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        json!({
            "analysis_task_message": format!("第 {} 章分析失败，批量任务终止", chapter_model.chapter_number),
            "analysis_task_progress": 100,
            "analysis_last_error": analysis_error,
            "analysis_retry_count": 3,
            "analysis_max_retries": 3,
        }),
    )
    .await;

    persist_batch_generation_step_outcome(
        db,
        task_id,
        BatchGenerationRuntimePersistencePlan::failed(
            Some(&chapter_model.id),
            Some(chapter_model.chapter_number),
            Some(&chapter_model.title),
            progress.completed,
            progress.total_chapters,
            BatchGenerationFailureKind::GenerationError,
            3,
            format!("第{}章分析失败，已重试3次: {}", chapter_model.chapter_number, analysis_error),
        ),
        BatchGenerationStepOutcome::Stop,
    )
    .await
}

async fn maybe_stop_batch_generation_for_quality_gate_manual_review(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_model: &chapter::Model,
    progress: &BatchGenerationStepProgress,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<BatchGenerationStepOutcome> {
    let snapshot = load_batch_generation_snapshot(db, task_id)
        .await
        .ok()
        .flatten();
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.as_ref());
    let Some(manual_review_label) = resolve_batch_generation_quality_gate_manual_review_label(
        db,
        task_id,
        current_retry_count,
        max_retries,
    )
    .await
    else {
        return None;
    };

    let _ = upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_quality_gate_blocked_runtime_state_patch(
            workflow_runtime_state,
            chapter_model.chapter_number,
            &manual_review_label,
        ),
    )
    .await;

    let failure_message = format!(
        "第{}章触发质量门禁，需人工复核: {}",
        chapter_model.chapter_number, manual_review_label
    );

    Some(
        persist_batch_generation_step_outcome(
            db,
            task_id,
            BatchGenerationRuntimePersistencePlan::failed_quality_gate_blocked(
                Some(&chapter_model.id),
                Some(chapter_model.chapter_number),
                Some(&chapter_model.title),
                progress.completed,
                progress.total_chapters,
                current_retry_count,
                &manual_review_label,
                failure_message,
            ),
            BatchGenerationStepOutcome::Stop,
        )
        .await,
    )
}

async fn maybe_retry_batch_generation_for_quality_gate_repair(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_model: &chapter::Model,
    progress: &BatchGenerationStepProgress,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<BatchGenerationStepOutcome> {
    let snapshot = load_batch_generation_snapshot(db, task_id)
        .await
        .ok()
        .flatten();
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.as_ref());
    let quality_metrics_summary = snapshot
        .as_ref()
        .and_then(|item| item.quality_metrics_summary.as_ref());
    let latest_quality_metrics = snapshot
        .as_ref()
        .and_then(|item| item.latest_quality_metrics.as_ref());
    let active_story_repair_payload =
        active_story_repair_payload_from_runtime_state(workflow_runtime_state);
    let Some(retry_label) = resolve_failed_terminal_semantics_from_sources(
        Some(&json!([])),
        Some(&BatchGenerationQualityStatusContext {
            active_story_repair_payload: active_story_repair_payload,
            quality_metrics_summary: quality_metrics_summary.cloned(),
            latest_quality_metrics: latest_quality_metrics.cloned(),
        }),
        current_retry_count,
        max_retries,
    )
    .filter(|semantics| semantics.kind == BatchGenerationFailedTerminalKind::Retry)
    .map(|semantics| semantics.label)
    else {
        return None;
    };

    let next_retry_count = current_retry_count + 1;
    if !should_retry_batch_generation_attempt(next_retry_count, max_retries) {
        return None;
    }

    let retry_message = format!(
        "第{}章触发质量修复重试: {}",
        chapter_model.chapter_number, retry_label
    );
    persist_batch_generation_retry_attempt(
        db,
        task_id,
        chapter_model,
        progress,
        next_retry_count,
        max_retries,
        &retry_message,
    )
    .await;

    let _ = upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        Value::Object(build_retry_quality_runtime_patch_contract(
            chapter_model.chapter_number,
            &retry_label,
        )),
    )
    .await;

    Some(BatchGenerationStepOutcome::RetryCurrentChapter { next_retry_count })
}

async fn run_batch_generation_follow_up_analysis_with_failure_contract(
    db: &DatabaseConnection,
    task_id: &str,
    session: &BatchGenerationRuntimeSession,
    chapter_model: &chapter::Model,
    progress: &BatchGenerationStepProgress,
    generated: &GeneratedChapterResult,
) -> BatchGenerationStepOutcome {
    match run_batch_generation_follow_up_analysis(db, task_id, session, generated).await {
        Ok(()) => {
            let task_retry_context = batch_generation_task::Entity::find_by_id(task_id)
                .one(db)
                .await
                .ok()
                .flatten();
            let current_retry_count = task_retry_context
                .as_ref()
                .map(|task| task.current_retry_count.max(0))
                .unwrap_or(0);
            let max_retries = task_retry_context
                .as_ref()
                .map(|task| task.max_retries.max(0))
                .unwrap_or(0);
            if let Some(stop_outcome) = maybe_stop_batch_generation_for_quality_gate_manual_review(
                db,
                task_id,
                chapter_model,
                progress,
                current_retry_count,
                max_retries,
            )
            .await
            {
                return stop_outcome;
            }
            if let Some(retry_outcome) = maybe_retry_batch_generation_for_quality_gate_repair(
                db,
                task_id,
                chapter_model,
                progress,
                current_retry_count,
                max_retries,
            )
            .await
            {
                return retry_outcome;
            }

            BatchGenerationStepOutcome::Continue {
                progress: progress.advance(),
            }
        }
        Err(analysis_error) => {
            fail_batch_generation_after_analysis(db, task_id, chapter_model, progress, analysis_error).await
        }
    }
}

pub(crate) async fn execute_batch_generation_runtime(
    db: &DatabaseConnection,
    task_id: &str,
    execution_input: BatchGenerationExecutionInput,
) {
    let (session, chapter_ids) = BatchGenerationRuntimeSession::from_execution_input(execution_input);
    let _ = BatchGenerationRuntimePersistencePlan::preparing(session.total_chapters)
        .persist(db, task_id)
        .await;
    let mut progress = BatchGenerationStepProgress::new(0, session.total_chapters);

    for chapter_id in &chapter_ids {
        match execute_batch_generation_step(
            db,
            task_id,
            &session,
            chapter_id,
            progress.clone(),
        )
        .await
        {
            BatchGenerationStepOutcome::Continue { progress: next_progress } => {
                progress = next_progress;
            }
            BatchGenerationStepOutcome::RetryCurrentChapter { .. } => {
                continue;
            }
            BatchGenerationStepOutcome::Stop => {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        batch_generation_retry_backoff_seconds,
        build_batch_generation_retry_waiting_snapshot,
        build_batch_generation_resume_runtime_checkpoint,
        dispatch_batch_generation_runtime, BatchGenerationExecutionInput,
        restore_batch_generation_runtime_compat_options_from_runtime_state,
        BatchGenerationRuntimeSession, BatchGenerationStepOutcome,
        BatchGenerationStepProgress, BatchGenerationTaskStage, ModelFieldUpdate,
        TaskTimestampUpdate, run_batch_generation_follow_up_analysis,
        should_retry_batch_generation_attempt,
    };
    use crate::ai::AIConfig;
    use crate::models::{batch_generation_task, chapter};
    use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
    use crate::services::chapter_batch_generation_resume_semantics_service::{
        ResumeBatchGenerationCommandState, ResumeResetSemantics,
    };
    use crate::services::chapter_batch_generation_runtime_checkpoint_service::{
        build_batch_generation_runtime_checkpoint_for_stage, BatchGenerationSnapshotStage,
    };
    use crate::services::chapter_batch_generation_write_workflow_service::{
        build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload,
        BatchGenerationRequestRuntimeState,
    };
    use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions;
    use crate::services::chapter_single_generation_runtime_state_service::build_prompt_overrides_from_compat_options;
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::{json, Value};

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn build_resume_task() -> ResumeBatchGenerationCommandState {
        ResumeBatchGenerationCommandState {
            batch_id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            status: "failed".to_string(),
            chapter_count: 1,
            chapter_ids: json!(["chapter-2"]),
            target_word_count: 3000,
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_retry_count: 0,
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            max_retries: 3,
            created_at: None,
        }
    }

    #[test]
    fn should_build_batch_generation_runtime_checkpoint_for_stage() {
        let checkpoint = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::ChapterStarted,
            Some("chapter-3"),
            Some(3),
            2,
            5,
        );

        assert_eq!(checkpoint["phase"], "generating");
        assert_eq!(checkpoint["progress"], 55);
        assert_eq!(checkpoint["status"], "running");
        assert_eq!(checkpoint["last_event"], "chapter_start");
        assert_eq!(checkpoint["last_message"], "正在生成第 3 章...");
        assert_eq!(checkpoint["chapter_id"], "chapter-3");
        assert_eq!(checkpoint["current_chapter_id"], "chapter-3");
        assert_eq!(checkpoint["current_chapter_number"], 3);
        assert_eq!(checkpoint["completed"], 2);
        assert_eq!(checkpoint["total"], 5);
    }

    #[test]
    fn should_build_resume_runtime_checkpoint_with_seed_metadata() {
        let checkpoint = build_batch_generation_resume_runtime_checkpoint(
            &build_resume_task(),
            Some(json!({
                "resume_from_batch_id": "task-1",
                "current_retry_count": 0,
                "max_retries": 3,
                "active_story_repair_payload": {
                    "summary": "补强前章伏笔",
                    "source": "manual_request"
                }
            })),
        );

        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(checkpoint["progress"], 0);
        assert_eq!(checkpoint["status"], "pending");
        assert_eq!(checkpoint["last_event"], "resume");
        assert_eq!(checkpoint["current_chapter_id"], "chapter-2");
        assert_eq!(checkpoint["current_chapter_number"], 2);
        assert!(checkpoint.get("completed").is_none());
        assert!(checkpoint.get("total").is_none());
        assert_eq!(checkpoint["resume_from_batch_id"], "task-1");
        assert_eq!(checkpoint["current_retry_count"], 0);
        assert_eq!(checkpoint["max_retries"], 3);
        assert_eq!(
            checkpoint["active_story_repair_payload"]["summary"],
            "补强前章伏笔"
        );
    }

    #[test]
    fn should_resolve_batch_generation_task_stage_mutation_contracts() {
        let resume_reset = BatchGenerationTaskStage::ResumeReset;
        assert_eq!(resume_reset.status(0, 5), "pending");
        assert!(matches!(
            resume_reset.started_at_update(),
            TaskTimestampUpdate::Clear
        ));
        assert!(matches!(
            resume_reset.completed_at_update(0, 5),
            TaskTimestampUpdate::Clear
        ));
        assert!(matches!(
            resume_reset.error_message_update(None),
            ModelFieldUpdate::Set(None)
        ));
        assert!(matches!(
            resume_reset.current_retry_count_update(),
            ModelFieldUpdate::Set(0)
        ));
        assert!(matches!(
            resume_reset.current_chapter_id_update(Some("chapter-1")),
            ModelFieldUpdate::Set(Some(ref id)) if id == "chapter-1"
        ));

        let preparing = BatchGenerationTaskStage::Preparing;
        assert_eq!(preparing.status(0, 5), "running");
        assert!(matches!(preparing.started_at_update(), TaskTimestampUpdate::Now));
        assert!(matches!(
            preparing.completed_at_update(0, 5),
            TaskTimestampUpdate::Clear
        ));
        assert!(matches!(
            preparing.error_message_update(None),
            ModelFieldUpdate::Set(None)
        ));
        assert!(matches!(
            preparing.current_retry_count_update(),
            ModelFieldUpdate::Set(0)
        ));

        let started = BatchGenerationTaskStage::ChapterStarted;
        assert_eq!(started.status(1, 5), "running");
        assert!(matches!(
            started.current_chapter_id_update(Some("chapter-2")),
            ModelFieldUpdate::Set(Some(ref id)) if id == "chapter-2"
        ));
        assert!(matches!(
            started.current_chapter_number_update(Some(2)),
            ModelFieldUpdate::Set(Some(2))
        ));
        assert!(matches!(
            started.total_chapters_update(5),
            ModelFieldUpdate::Set(5)
        ));

        let completed = BatchGenerationTaskStage::ChapterSucceeded;
        assert_eq!(completed.status(5, 5), "completed");
        assert!(matches!(
            completed.completed_at_update(5, 5),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            completed.completed_chapters_update(5),
            ModelFieldUpdate::Set(5)
        ));

        let failed = BatchGenerationTaskStage::Failed;
        assert_eq!(failed.status(3, 5), "failed");
        assert!(matches!(
            failed.error_message_update(Some("boom".to_string())),
            ModelFieldUpdate::Set(Some(ref message)) if message == "boom"
        ));

        let cancelled = BatchGenerationTaskStage::Cancelled;
        assert_eq!(cancelled.status(2, 5), "cancelled");
        assert!(matches!(
            cancelled.completed_at_update(2, 5),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            cancelled.error_message_update(None),
            ModelFieldUpdate::Keep
        ));
    }

    #[test]
    fn should_apply_batch_generation_task_mutation_plan() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(0, 35, 0)
            .expect("valid time");
        let mut active: batch_generation_task::ActiveModel = build_task("pending").into();

        BatchGenerationTaskStage::ChapterStarted.apply_to_active_model(
            &mut active,
            Some("chapter-7"),
            Some(7),
            2,
            5,
            None,
            now,
        );

        assert_eq!(active.status, Set("running".to_string()));
        assert_eq!(active.error_message, Set(None));
        assert_eq!(active.completed_chapters, Set(2));
        assert_eq!(
            active.current_chapter_id,
            Set(Some("chapter-7".to_string()))
        );
        assert_eq!(active.current_chapter_number, Set(Some(7)));
        assert_eq!(active.total_chapters, Set(5));
    }

    #[test]
    fn should_resolve_batch_generation_retry_boundaries() {
        assert!(should_retry_batch_generation_attempt(1, 3));
        assert!(should_retry_batch_generation_attempt(3, 3));
        assert!(!should_retry_batch_generation_attempt(4, 3));
        assert!(!should_retry_batch_generation_attempt(-1, 3));
    }

    #[test]
    fn should_cap_batch_generation_retry_backoff_seconds() {
        assert_eq!(batch_generation_retry_backoff_seconds(0), 1);
        assert_eq!(batch_generation_retry_backoff_seconds(1), 2);
        assert_eq!(batch_generation_retry_backoff_seconds(2), 4);
        assert_eq!(batch_generation_retry_backoff_seconds(3), 8);
        assert_eq!(batch_generation_retry_backoff_seconds(4), 10);
        assert_eq!(batch_generation_retry_backoff_seconds(7), 10);
    }

    #[test]
    fn should_build_batch_generation_retry_waiting_snapshot() {
        let chapter_model = chapter::Model {
            id: "chapter-3".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 3,
            title: "夜航".to_string(),
            content: Some("content".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let progress = BatchGenerationStepProgress::new(1, 5);

        let snapshot = build_batch_generation_retry_waiting_snapshot(
            &chapter_model,
            &progress,
            2,
            3,
            4,
            "provider timeout",
        );

        assert_eq!(snapshot["phase"], "generating");
        assert_eq!(snapshot["status"], "running");
        assert_eq!(snapshot["last_event"], "chapter_retry");
        assert_eq!(snapshot["current_retry_count"], 2);
        assert_eq!(snapshot["max_retries"], 3);
        assert_eq!(snapshot["retry_backoff_seconds"], 4);
        assert_eq!(snapshot["last_error"], "provider timeout");
        assert_eq!(
            snapshot["last_message"],
            "第 3 章生成失败，4 秒后进行第 2 次重试"
        );
    }

    #[test]
    fn should_restore_batch_runtime_compat_options_from_snapshot_payload() {
        let base_compat_options = SingleChapterGenerationCompatOptions {
            style_id: None,
            enable_analysis: false,
            enable_mcp: true,
            web_research_enabled: false,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: Vec::new(),
            story_preserve_strengths: Vec::new(),
        };
        let runtime_state_payload = json!({
            "active_story_repair_payload": {
                "summary": "沿用批量修复摘要",
                "repair_targets": ["压缩说明段", "提前冲突触发"],
                "preserve_strengths": ["角色张力"],
                "source": "recent_history_summary",
                "scope": "batch"
            },
            "quality_metrics_summary": {
                "overall_score": 82
            }
        });
        let resolved = restore_batch_generation_runtime_compat_options_from_runtime_state(
            &base_compat_options,
            Some(&runtime_state_payload),
        );

        assert_eq!(resolved.story_repair_summary(), "沿用批量修复摘要");
        assert_eq!(
            resolved.story_repair_targets(),
            &["压缩说明段".to_string(), "提前冲突触发".to_string()]
        );
        assert_eq!(
            resolved.story_preserve_strengths(),
            &["角色张力".to_string()]
        );
    }

    #[test]
    fn should_fallback_to_base_compat_options_when_runtime_snapshot_missing() {
        let base_compat_options = SingleChapterGenerationCompatOptions {
            style_id: None,
            enable_analysis: false,
            enable_mcp: true,
            web_research_enabled: false,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: Some("来自初始请求".to_string()),
            story_repair_targets: vec!["初始目标".to_string()],
            story_preserve_strengths: vec!["初始优势".to_string()],
        };
        let resolved = restore_batch_generation_runtime_compat_options_from_runtime_state(
            &base_compat_options,
            None,
        );

        assert_eq!(resolved.story_repair_summary(), "来自初始请求");
        assert_eq!(resolved.story_repair_targets(), &["初始目标".to_string()]);
        assert_eq!(resolved.story_preserve_strengths(), &["初始优势".to_string()]);
    }

    #[test]
    fn should_build_refreshed_batch_runtime_state_with_existing_active_payload_and_recent_history() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                style_id: None,
                enable_analysis: false,
                enable_mcp: true,
                web_research_enabled: false,
                web_research_query: None,
                narrative_perspective: None,
                creative_mode: None,
                story_focus: None,
                plot_stage: None,
                story_creation_brief: None,
                quality_preset: None,
                quality_notes: None,
                story_repair_summary: Some("手工摘要".to_string()),
                story_repair_targets: vec!["手工目标".to_string(), "共同目标".to_string()],
                story_preserve_strengths: vec!["手工优势".to_string()],
            },
            Some("model-x".to_string()),
        );
        let existing_active_payload = json!({
            "summary": "运行态摘要",
            "repair_targets": ["共同目标", "运行态目标"],
            "preserve_strengths": ["运行态优势"],
            "source": "current_chapter_quality",
            "source_label": "Current chapter quality",
            "scope": "batch"
        });
        let recent_history_summary = json!({
            "overall_score": 84,
            "repair_guidance": {
                "summary": "历史摘要",
                "repair_targets": ["历史目标", "共同目标"],
                "preserve_strengths": ["历史优势"],
                "focus_areas": ["节奏", "冲突"]
            },
            "quality_gate": {
                "decision": "auto_repair",
                "label": "Quality gate",
                "summary": "近期质量波动"
            },
            "quality_runtime_context": {
                "scope": "batch",
                "recent_metrics": [{"score": 84}]
            }
        });

        let payload =
            build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
                &request_runtime_state,
                Some(&existing_active_payload),
                Some(&recent_history_summary),
            );

        assert_eq!(payload["active_story_repair_payload"]["summary"], "运行态摘要");
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["共同目标", "运行态目标", "历史目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["运行态优势", "历史优势"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "manual_plus_recent_history_summary"
        );
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 84.0);
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"][0]["score"],
            84
        );
    }

    #[test]
    fn should_build_batch_runtime_state_payload_with_fresh_latest_quality_metrics() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::default();
        let quality_summary = json!({
            "overall_score": 84,
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议继续修复"
            },
            "quality_runtime_context": {
                "scope": "batch",
                "recent_metrics": [{"score": 84}]
            }
        });
        let latest_quality_metrics = json!({
            "overall_score": 84,
            "pacing_score": 7.6,
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议继续修复"
            }
        });

        let payload = super::build_batch_generation_runtime_state_payload_from_current_quality(
            &request_runtime_state,
            None,
            None,
            None,
            &quality_summary,
            Some(&latest_quality_metrics),
        );

        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 84.0);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(payload["latest_quality_metrics"]["pacing_score"], 7.6);
        assert_eq!(
            payload["latest_quality_metrics"]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(
            payload["quality_metrics_history"],
            json!([{
                "overall_score": 84,
                "pacing_score": 7.6,
                "quality_gate": {
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            }])
        );
    }

    #[test]
    fn should_append_fresh_latest_quality_metrics_into_existing_history() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::default();
        let quality_summary = json!({
            "overall_score": 84,
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议继续修复"
            }
        });
        let existing_history = json!([
            {
                "overall_score": 81,
                "quality_gate": {
                    "decision": "passed",
                    "label": "通过"
                }
            }
        ]);
        let latest_quality_metrics = json!({
            "overall_score": 84,
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议继续修复"
            }
        });

        let payload = super::build_batch_generation_runtime_state_payload_from_current_quality(
            &request_runtime_state,
            None,
            None,
            Some(&existing_history),
            &quality_summary,
            Some(&latest_quality_metrics),
        );

        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 81);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 84.0);
        assert_eq!(payload["quality_metrics_summary"]["overall_score_delta"], 3.0);
        assert_eq!(payload["quality_metrics_summary"]["overall_score_trend"], "rising");
        assert_eq!(payload["quality_metrics_summary"]["quality_gate_counts"]["passed"], 1);
        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate_counts"]["auto_repair"],
            1
        );
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary_state"]["last_overall_score"], 84.0);
    }

    #[test]
    fn should_trim_quality_metrics_history_to_twenty_items() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::default();
        let quality_summary = json!({"overall_score": 90});
        let existing_history = Value::Array(
            (0..20)
                .map(|index| json!({"overall_score": index}))
                .collect(),
        );
        let latest_quality_metrics = json!({"overall_score": 20});

        let payload = super::build_batch_generation_runtime_state_payload_from_current_quality(
            &request_runtime_state,
            None,
            None,
            Some(&existing_history),
            &quality_summary,
            Some(&latest_quality_metrics),
        );

        let history = payload["quality_metrics_history"]
            .as_array()
            .expect("history should be an array");
        assert_eq!(history.len(), 20);
        assert_eq!(history.first().and_then(|item| item.get("overall_score")), Some(&json!(1)));
        assert_eq!(history.last().and_then(|item| item.get("overall_score")), Some(&json!(20)));
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 20);
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 20.0);
        assert_eq!(payload["quality_metrics_summary"]["overall_score_delta"], 19.0);
        assert_eq!(payload["quality_metrics_summary"]["overall_score_trend"], "rising");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 20);
    }

    #[test]
    fn should_build_batch_runtime_state_summary_from_rust_owned_history() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::default();
        let fallback_quality_summary = json!({
            "overall_score": 84,
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议继续修复"
            },
            "quality_runtime_context": {
                "scope": "batch",
                "recent_metrics": [{"score": 84}]
            }
        });
        let existing_history = json!([
            {
                "overall_score": 88,
                "repair_guidance": {
                    "summary": "保持优势",
                    "repair_targets": ["压缩铺垫"],
                    "preserve_strengths": ["尾章钩子"],
                    "focus_areas": ["pacing"]
                },
                "quality_gate": {
                    "decision": "passed",
                    "label": "当前章节通过"
                }
            }
        ]);
        let latest_quality_metrics = json!({
            "overall_score": 84,
            "repair_guidance": {
                "summary": "建议继续修复",
                "repair_targets": ["强化动机"],
                "preserve_strengths": ["人物口吻"],
                "focus_areas": ["character"]
            },
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议继续修复",
                "failed_metrics": [{"label": "Character"}]
            }
        });

        let payload = super::build_batch_generation_runtime_state_payload_from_current_quality(
            &request_runtime_state,
            None,
            None,
            Some(&existing_history),
            &fallback_quality_summary,
            Some(&latest_quality_metrics),
        );

        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 84.0);
        assert_eq!(payload["quality_metrics_summary"]["overall_score_delta"], -4.0);
        assert_eq!(payload["quality_metrics_summary"]["overall_score_trend"], "falling");
        assert_eq!(
            payload["quality_metrics_summary"]["recent_focus_areas"],
            json!(["character", "pacing"])
        );
        assert_eq!(payload["quality_metrics_summary"]["quality_gate"]["decision"], "auto_repair");
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary_state"]["first_overall_score"], 88.0);
        assert_eq!(payload["quality_metrics_summary_state"]["last_overall_score"], 84.0);
    }

    #[test]
    fn should_advance_batch_quality_summary_state_from_existing_runtime_state() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::default();
        let quality_summary = json!({"overall_score": 84});
        let existing_summary_state = json!({
            "scope": "batch",
            "chapter_count": 1,
            "first_overall_score": 88.0,
            "last_overall_score": 88.0,
            "recent_history": [{
                "overall_score": 88,
                "repair_guidance": {"focus_areas": ["pacing"]},
                "quality_gate": {"decision": "passed"}
            }],
            "overall_score_total": 88.0,
            "pacing_score_total": 8.2,
            "pacing_score_count": 1
        });
        let existing_history = json!([
            {
                "overall_score": 88,
                "pacing_score": 8.2,
                "repair_guidance": {"focus_areas": ["pacing"]},
                "quality_gate": {"decision": "passed"}
            }
        ]);
        let latest_quality_metrics = json!({
            "overall_score": 84,
            "pacing_score": 7.6,
            "repair_guidance": {"focus_areas": ["character"]},
            "quality_gate": {"decision": "auto_repair"}
        });

        let payload = super::build_batch_generation_runtime_state_payload_from_current_quality(
            &request_runtime_state,
            None,
            Some(&existing_summary_state),
            Some(&existing_history),
            &quality_summary,
            Some(&latest_quality_metrics),
        );

        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary_state"]["first_overall_score"], 88.0);
        assert_eq!(payload["quality_metrics_summary_state"]["last_overall_score"], 84.0);
        assert_eq!(payload["quality_metrics_summary_state"]["overall_score_total"], 172.0);
        assert_eq!(payload["quality_metrics_summary_state"]["pacing_score_total"], 15.8);
        assert_eq!(payload["quality_metrics_summary_state"]["pacing_score_count"], 2);
    }

    #[test]
    fn should_build_resume_runtime_checkpoint_for_single_generation_task() {
        let mut single_task = build_task("failed");
        single_task.chapter_count = 1;
        single_task.chapter_ids = json!(["chapter-1"]);
        single_task.current_chapter_id = Some("chapter-1".to_string());
        single_task.current_chapter_number = Some(3);

        let command_state = ResumeBatchGenerationCommandState::from_task(&single_task);
        let semantics = command_state.resolve_runtime_semantics();
        let with_chapter = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Resumed {
                include_progress_totals: semantics.include_progress_totals,
            },
            semantics.current_chapter_id.as_deref(),
            semantics.current_chapter_number,
            0,
            command_state.total_chapters,
        );
        assert_eq!(with_chapter["phase"], "pending");
        assert_eq!(with_chapter["progress"], 0);
        assert_eq!(with_chapter["status"], "pending");
        assert_eq!(with_chapter["last_event"], "resume");
        assert_eq!(
            with_chapter["last_message"],
            "批量生成任务已恢复，等待重新开始..."
        );
        assert_eq!(with_chapter["chapter_id"], "chapter-1");
        assert_eq!(with_chapter["current_chapter_id"], "chapter-1");
        assert_eq!(with_chapter["current_chapter_number"], 3);
        assert!(with_chapter.get("completed").is_none());
        assert!(with_chapter.get("total").is_none());
    }

    #[test]
    fn should_resolve_resume_reset_semantics_for_single_generation_task() {
        let mut single_task = build_task("failed");
        single_task.chapter_count = 1;
        single_task.current_chapter_id = Some("chapter-9".to_string());
        single_task.current_chapter_number = Some(9);
        single_task.completed_chapters = 1;
        single_task.failed_chapters = json!([{"chapter_id": "chapter-9"}]);
        single_task.current_retry_count = 2;

        let reset_plan =
            ResumeBatchGenerationCommandState::from_task(&single_task).resolve_reset_semantics();

        assert_eq!(reset_plan.current_chapter_id.as_deref(), Some("chapter-9"));
        assert_eq!(reset_plan.current_chapter_number, Some(9));
        assert_eq!(reset_plan.completed_chapters, 0);
        assert_eq!(reset_plan.failed_chapters, json!([]));
        assert_eq!(reset_plan.current_retry_count, 0);
    }

    #[test]
    fn should_resolve_resume_reset_semantics_for_batch_generation_task() {
        let mut batch_task = build_task("cancelled");
        batch_task.chapter_count = 3;
        batch_task.chapter_ids = json!(["chapter-1", "chapter-2", "chapter-3"]);
        batch_task.current_chapter_id = Some("chapter-2".to_string());
        batch_task.current_chapter_number = Some(2);
        batch_task.completed_chapters = 2;
        batch_task.failed_chapters = json!([{"chapter_id": "chapter-2"}]);
        batch_task.current_retry_count = 1;

        let reset_plan =
            ResumeBatchGenerationCommandState::from_task(&batch_task).resolve_reset_semantics();

        assert!(reset_plan.current_chapter_id.is_none());
        assert!(reset_plan.current_chapter_number.is_none());
        assert_eq!(reset_plan.completed_chapters, 0);
        assert_eq!(reset_plan.failed_chapters, json!([]));
        assert_eq!(reset_plan.current_retry_count, 0);
    }

    #[test]
    fn should_clear_batch_resume_runtime_position_and_progress() {
        let mut batch_task = build_task("cancelled");
        batch_task.chapter_count = 3;
        batch_task.chapter_ids = json!(["chapter-1", "chapter-2", "chapter-3"]);
        batch_task.total_chapters = 3;
        batch_task.completed_chapters = 2;
        batch_task.current_chapter_id = Some("chapter-2".to_string());
        batch_task.current_chapter_number = Some(2);

        let command_state = ResumeBatchGenerationCommandState::from_task(&batch_task);
        let reset_plan = command_state.resolve_reset_semantics();
        assert!(reset_plan.current_chapter_id.is_none());
        assert!(reset_plan.current_chapter_number.is_none());

        let checkpoint = reset_plan.build_resume_checkpoint(command_state.total_chapters);
        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(checkpoint["progress"], 0);
        assert_eq!(checkpoint["status"], "pending");
        assert_eq!(checkpoint["last_event"], "resume");
        assert_eq!(
            checkpoint["last_message"],
            "批量生成任务已恢复，等待重新开始..."
        );
        assert!(checkpoint["chapter_id"].is_null());
        assert!(checkpoint["current_chapter_id"].is_null());
        assert!(checkpoint["current_chapter_number"].is_null());
        assert_eq!(checkpoint["completed"], 0);
        assert_eq!(checkpoint["total"], 3);
    }

    #[test]
    fn should_keep_resume_reset_semantics_contract_through_runtime_state() {
        let semantics = ResumeResetSemantics {
            status: "pending",
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            include_progress_totals: false,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_retry_count: 0,
        };

        let checkpoint = semantics.build_resume_checkpoint(6);
        assert_eq!(checkpoint["status"], "pending");
        assert_eq!(checkpoint["last_event"], "resume");
        assert_eq!(checkpoint["chapter_id"], "chapter-2");
        assert_eq!(checkpoint["current_chapter_number"], 2);
        assert!(checkpoint.get("completed").is_none());
        assert!(checkpoint.get("total").is_none());
    }

    #[test]
    fn should_build_cancelled_runtime_checkpoint_with_terminal_progress() {
        let checkpoint = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Cancelled,
            None,
            None,
            2,
            5,
        );

        assert_eq!(checkpoint["phase"], "cancelled");
        assert_eq!(checkpoint["progress"], 100);
        assert_eq!(checkpoint["status"], "cancelled");
        assert_eq!(checkpoint["last_event"], "cancelled");
        assert_eq!(checkpoint["last_message"], "批量生成已取消");
        assert_eq!(checkpoint["completed"], 2);
        assert_eq!(checkpoint["total"], 5);
        assert!(checkpoint["chapter_id"].is_null());
        assert!(checkpoint["current_chapter_id"].is_null());
    }

    #[test]
    fn should_build_batch_generation_execution_input_from_runtime_owner() {
        let input = BatchGenerationExecutionInput {
            user_id: "user-10".to_string(),
            chapter_ids: vec!["chapter-3".to_string()],
            target_word_count: 2800,
            compat_options: SingleChapterGenerationCompatOptions::default(),
            ai_config: AIConfig::default(),
        };

        assert_eq!(input.user_id, "user-10");
        assert_eq!(input.chapter_ids, vec!["chapter-3".to_string()]);
        assert_eq!(input.target_word_count, 2800);
        assert_eq!(input.ai_config.provider, AIConfig::default().provider);
    }

    #[test]
    fn should_build_batch_generation_runtime_session_from_execution_owner() {
        let (session, chapter_ids) =
            BatchGenerationRuntimeSession::from_execution_input(BatchGenerationExecutionInput {
                user_id: "user-10".to_string(),
                chapter_ids: vec!["chapter-3".to_string(), "chapter-4".to_string()],
                target_word_count: 2800,
                compat_options: SingleChapterGenerationCompatOptions::default(),
                ai_config: AIConfig::default(),
            });

        assert_eq!(session.user_id, "user-10");
        assert_eq!(session.target_word_count, 2800);
        assert_eq!(session.total_chapters, 2);
        assert_eq!(session.compat_options, SingleChapterGenerationCompatOptions::default());
        assert_eq!(chapter_ids, vec!["chapter-3".to_string(), "chapter-4".to_string()]);
    }

    #[test]
    fn should_reuse_single_generation_prompt_overrides_for_batch_runtime() {
        let compat = SingleChapterGenerationCompatOptions {
            style_id: None,
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: true,
            web_research_query: Some("民国商会夜航协定".to_string()),
            narrative_perspective: Some("第一人称".to_string()),
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("climax".to_string()),
            story_creation_brief: Some("本章推进夜航谈判破局".to_string()),
            quality_preset: Some("immersive".to_string()),
            quality_notes: Some("压缩说明，强化动作反馈".to_string()),
            story_repair_summary: Some("中段节奏过慢".to_string()),
            story_repair_targets: vec!["提前冲突触发".to_string()],
            story_preserve_strengths: vec!["结尾钩子".to_string()],
        };

        let overrides = build_prompt_overrides_from_compat_options(&compat);

        assert_eq!(overrides.narrative_perspective.as_deref(), Some("第一人称"));
        assert_eq!(overrides.creative_mode.as_deref(), Some("hook"));
        assert_eq!(overrides.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(overrides.plot_stage.as_deref(), Some("climax"));
        assert_eq!(
            overrides.story_creation_brief.as_deref(),
            Some("本章推进夜航谈判破局")
        );
        assert_eq!(overrides.quality_preset.as_deref(), Some("immersive"));
        assert_eq!(
            overrides.quality_notes.as_deref(),
            Some("压缩说明，强化动作反馈")
        );
        assert!(overrides.web_research_enabled);
        assert_eq!(
            overrides.web_research_query.as_deref(),
            Some("民国商会夜航协定")
        );
        assert_eq!(
            overrides.story_repair_summary.as_deref(),
            Some("中段节奏过慢")
        );
        assert_eq!(overrides.story_repair_targets, vec!["提前冲突触发".to_string()]);
        assert_eq!(overrides.story_preserve_strengths, vec!["结尾钩子".to_string()]);
    }

    #[test]
    fn should_build_completed_batch_generation_analysis_snapshot() {
        let snapshot = super::build_batch_generation_analysis_completed_snapshot(
            &GeneratedChapterResult {
                chapter_id: "chapter-3".to_string(),
                chapter_number: 3,
                title: "第三章".to_string(),
                content: "正文".to_string(),
                word_count: 1234,
            },
            1,
        );

        assert_eq!(snapshot["analysis_task_message"], "第 3 章分析完成");
        assert_eq!(snapshot["analysis_task_progress"], 100);
        assert!(snapshot["analysis_last_error"].is_null());
        assert_eq!(snapshot["analysis_retry_count"], 1);
        assert_eq!(snapshot["analysis_max_retries"], 3);
        assert!(snapshot["quality_gate_decision"].is_null());
        assert!(snapshot["quality_gate_label"].is_null());
        assert!(snapshot["phase"].is_null());
    }

    #[test]
    fn should_build_quality_gate_blocked_runtime_state_patch_with_terminal_repair_payload() {
        let patch = super::build_quality_gate_blocked_runtime_state_patch(
            Some(&json!({
                "quality_metrics_summary": {
                    "overall_score": 7.2,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    }
                },
                "latest_quality_metrics": {
                    "overall_score": 7.1,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    }
                },
                "quality_metrics_history": [{
                    "overall_score": 7.1,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    }
                }],
                "quality_metrics_summary_state": {
                    "scope": "batch",
                    "chapter_count": 1,
                    "first_overall_score": 7.1,
                    "last_overall_score": 7.1,
                    "recent_history": [{
                        "overall_score": 7.1,
                        "quality_gate": {
                            "status": "warning",
                            "decision": "auto_repair",
                            "label": "建议继续修复"
                        }
                    }]
                },
                "quality_history_context": {
                    "scope": "batch",
                    "quality_gate_counts": {
                        "auto_repair": 1
                    },
                    "recent_manual_review_count": 0,
                    "recent_auto_repair_count": 1,
                    "recent_metrics": [{
                        "history_index": 0,
                        "overall_score": 7.2,
                        "quality_gate": {
                            "status": "warning",
                            "decision": "auto_repair",
                            "label": "建议继续修复"
                        }
                    }]
                },
                "active_story_repair_payload": {
                    "summary": "继续补强冲突",
                    "quality_gate_decision": "auto_repair",
                    "quality_gate_label": "建议继续修复",
                    "phase": "repair_pending"
                }
            })),
            7,
            "自动修复预算已耗尽",
        );

        assert_eq!(patch["quality_gate_decision"], "manual_review");
        assert_eq!(patch["quality_gate_label"], "自动修复预算已耗尽");
        assert_eq!(patch["phase"], "quality_blocked");
        assert_eq!(patch["quality_metrics_summary"]["quality_gate"]["status"], "failed");
        assert_eq!(
            patch["quality_metrics_summary"]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            patch["quality_metrics_summary"]["quality_gate"]["label"],
            "自动修复预算已耗尽"
        );
        assert_eq!(patch["latest_quality_metrics"]["quality_gate"]["status"], "failed");
        assert_eq!(
            patch["latest_quality_metrics"]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            patch["latest_quality_metrics"]["quality_gate"]["label"],
            "自动修复预算已耗尽"
        );
        assert_eq!(
            patch["quality_metrics_history"][0]["quality_gate"]["status"],
            "failed"
        );
        assert_eq!(
            patch["quality_metrics_history"][0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            patch["quality_metrics_history"][0]["quality_gate"]["label"],
            "自动修复预算已耗尽"
        );
        assert_eq!(
            patch["quality_metrics_summary_state"]["recent_history"][0]["quality_gate"]["status"],
            "failed"
        );
        assert_eq!(
            patch["quality_metrics_summary_state"]["recent_history"][0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            patch["quality_metrics_summary_state"]["recent_history"][0]["quality_gate"]["label"],
            "自动修复预算已耗尽"
        );
        assert_eq!(
            patch["active_story_repair_payload"]["quality_gate_decision"],
            "manual_review"
        );
        assert_eq!(
            patch["active_story_repair_payload"]["quality_gate_label"],
            "自动修复预算已耗尽"
        );
        assert_eq!(
            patch["active_story_repair_payload"]["phase"],
            "quality_blocked"
        );
        assert_eq!(
            patch["quality_history_context"]["recent_metrics"][0]["quality_gate"]["status"],
            "failed"
        );
        assert_eq!(
            patch["quality_history_context"]["recent_metrics"][0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            patch["quality_history_context"]["recent_metrics"][0]["quality_gate"]["label"],
            "自动修复预算已耗尽"
        );
        assert_eq!(
            patch["quality_history_context"]["quality_gate_counts"]["manual_review"],
            1
        );
        assert!(patch["quality_history_context"]["quality_gate_counts"]
            .get("auto_repair")
            .is_none());
        assert_eq!(patch["quality_history_context"]["recent_manual_review_count"], 1);
        assert_eq!(patch["quality_history_context"]["recent_auto_repair_count"], 0);
    }

    #[test]
    fn should_build_batch_generation_step_outcome_contract() {
        assert_eq!(
            BatchGenerationStepOutcome::Continue {
                progress: BatchGenerationStepProgress::new(2, 5),
            },
            BatchGenerationStepOutcome::Continue {
                progress: BatchGenerationStepProgress::new(2, 5),
            }
        );
        assert_eq!(
            BatchGenerationStepOutcome::RetryCurrentChapter { next_retry_count: 2 },
            BatchGenerationStepOutcome::RetryCurrentChapter { next_retry_count: 2 }
        );
        assert_eq!(BatchGenerationStepOutcome::Stop, BatchGenerationStepOutcome::Stop);
    }

    #[test]
    fn should_keep_batch_generation_step_call_contract_explicit() {
        let chapter_id = "chapter-3".to_string();
        let progress = BatchGenerationStepProgress::new(2, 5);

        assert_eq!(chapter_id, "chapter-3");
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total_chapters, 5);
    }

    #[test]
    fn should_build_batch_generation_step_progress_contract() {
        let progress = BatchGenerationStepProgress::new(2, 5);
        let next = progress.advance();

        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total_chapters, 5);
        assert_eq!(next.completed, 3);
        assert_eq!(next.total_chapters, 5);
    }

    #[test]
    fn should_build_batch_generation_step_result_contract() {
        let chapter_model = chapter::Model {
            id: "chapter-4".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 4,
            title: "第四章".to_string(),
            content: None,
            summary: None,
            word_count: 0,
            status: "pending".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let next_progress = BatchGenerationStepProgress::new(2, 5).advance();
        let persistence_plan = super::BatchGenerationRuntimePersistencePlan::chapter_succeeded(
            &chapter_model,
            next_progress.completed,
            next_progress.total_chapters,
        );

        assert_eq!(
            BatchGenerationStepOutcome::Continue {
                progress: next_progress,
            },
            BatchGenerationStepOutcome::Continue {
                progress: BatchGenerationStepProgress::new(3, 5),
            }
        );
        assert_eq!(persistence_plan.current_chapter_id.as_deref(), Some("chapter-4"));
        assert_eq!(persistence_plan.current_chapter_number, Some(4));
        assert_eq!(persistence_plan.completed_chapters, 3);
        assert_eq!(persistence_plan.total_chapters, 5);
        assert_eq!(persistence_plan.error_message, None);
        assert_eq!(persistence_plan.failed_chapter_entry, None);
    }

    #[test]
    fn should_keep_retry_input_owner_on_resolved_compat_options_contract() {
        let compat = SingleChapterGenerationCompatOptions {
            web_research_enabled: true,
            web_research_query: Some("晚清码头夜航税卡".to_string()),
            story_creation_brief: Some("重试时应沿用运行态修复输入".to_string()),
            quality_notes: Some("压缩说明".to_string()),
            story_repair_summary: Some("减少解释段".to_string()),
            story_repair_targets: vec!["提前冲突".to_string()],
            ..SingleChapterGenerationCompatOptions::default()
        };

        assert!(compat.web_research_enabled());
        assert_eq!(compat.web_research_query(), Some("晚清码头夜航税卡"));
        assert_eq!(compat.story_creation_brief(), "重试时应沿用运行态修复输入");
        assert_eq!(compat.quality_notes(), "压缩说明");
        assert_eq!(compat.story_repair_summary(), "减少解释段");
        assert_eq!(compat.story_repair_targets(), &["提前冲突".to_string()]);
    }

    #[test]
    fn should_build_retry_current_chapter_step_outcome_contract() {
        let outcome = BatchGenerationStepOutcome::RetryCurrentChapter {
            next_retry_count: 2,
        };

        assert_eq!(
            outcome,
            BatchGenerationStepOutcome::RetryCurrentChapter {
                next_retry_count: 2
            }
        );
    }

    #[test]
    fn should_build_failed_chapter_entry_with_runtime_owner_fields() {
        let entry = super::build_batch_generation_failed_chapter_entry(
            Some("chapter-7"),
            Some(7),
            Some("高潮前夜"),
            "生成失败：模型超时",
            2,
        );

        assert_eq!(entry["chapter_id"], "chapter-7");
        assert_eq!(entry["chapter_number"], 7);
        assert_eq!(entry["title"], "高潮前夜");
        assert_eq!(entry["error"], "生成失败：模型超时");
        assert_eq!(entry["retry_count"], 2);
    }

    #[test]
    fn should_build_quality_gate_blocked_failed_chapter_entry_with_terminal_semantics() {
        let entry = super::build_quality_gate_blocked_failed_chapter_entry(
            Some("chapter-7"),
            Some(7),
            Some("高潮前夜"),
            "第7章触发质量门禁，需人工复核: 自动修复预算已耗尽",
            3,
            "自动修复预算已耗尽",
        );

        assert_eq!(entry["chapter_id"], "chapter-7");
        assert_eq!(entry["retry_count"], 3);
        assert_eq!(entry["quality_gate_decision"], "manual_review");
        assert_eq!(entry["quality_gate_label"], "自动修复预算已耗尽");
        assert_eq!(entry["phase"], "quality_blocked");
    }

    #[test]
    fn should_apply_shared_manual_review_terminal_fields_and_quality_gate() {
        let mut payload = serde_json::Map::new();
        super::apply_manual_review_terminal_fields(&mut payload, "等待人工复核");
        assert_eq!(payload["quality_gate_decision"], "manual_review");
        assert_eq!(payload["quality_gate_label"], "等待人工复核");
        assert_eq!(payload["phase"], "quality_blocked");

        let mut gate = serde_json::Map::new();
        super::apply_manual_review_terminal_quality_gate(&mut gate, "等待人工复核");
        assert_eq!(gate["status"], "failed");
        assert_eq!(gate["decision"], "manual_review");
        assert_eq!(gate["label"], "等待人工复核");
    }

    #[test]
    fn should_apply_shared_terminal_normalization_to_history_and_context() {
        let mut history = json!([
            {
                "overall_score": 81,
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            }
        ]);
        super::normalize_terminal_quality_history(&mut history, "等待人工复核");
        assert_eq!(history[0]["quality_gate"]["status"], "failed");
        assert_eq!(history[0]["quality_gate"]["decision"], "manual_review");
        assert_eq!(history[0]["quality_gate"]["label"], "等待人工复核");

        let mut context = json!({
            "recent_metrics": [{
                "overall_score": 81,
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            }],
            "quality_gate_counts": {
                "auto_repair": 1
            },
            "recent_manual_review_count": 0,
            "recent_auto_repair_count": 1
        });
        super::normalize_terminal_quality_history_context(&mut context, "等待人工复核");
        assert_eq!(context["recent_metrics"][0]["quality_gate"]["status"], "failed");
        assert_eq!(context["recent_metrics"][0]["quality_gate"]["decision"], "manual_review");
        assert_eq!(context["recent_metrics"][0]["quality_gate"]["label"], "等待人工复核");
        assert_eq!(context["quality_gate_counts"]["manual_review"], 1);
        assert!(context["quality_gate_counts"].get("auto_repair").is_none());
        assert_eq!(context["recent_manual_review_count"], 1);
        assert_eq!(context["recent_auto_repair_count"], 0);
    }

    #[test]
    fn should_apply_shared_terminal_runtime_patch_sections() {
        let runtime_state = json!({
            "quality_metrics_summary": {
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            },
            "latest_quality_metrics": {
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            },
            "quality_metrics_history": [{
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            }],
            "quality_metrics_summary_state": {
                "recent_history": [{
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    }
                }]
            },
            "quality_history_context": {
                "recent_metrics": [{
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    }
                }],
                "quality_gate_counts": {"auto_repair": 1},
                "recent_manual_review_count": 0,
                "recent_auto_repair_count": 1
            }
        });

        let mut payload = serde_json::Map::new();
        super::apply_terminal_quality_runtime_patch_sections(
            &mut payload,
            Some(&runtime_state),
            "等待人工复核",
        );

        assert_eq!(payload["quality_metrics_summary"]["quality_gate"]["decision"], "manual_review");
        assert_eq!(payload["latest_quality_metrics"]["quality_gate"]["decision"], "manual_review");
        assert_eq!(payload["quality_metrics_history"][0]["quality_gate"]["decision"], "manual_review");
        assert_eq!(
            payload["quality_metrics_summary_state"]["recent_history"][0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"][0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(payload["quality_history_context"]["quality_gate_counts"]["manual_review"], 1);
    }

    #[test]
    fn should_build_and_apply_shared_manual_review_terminal_runtime_patch_contract() {
        let runtime_state = json!({
            "quality_metrics_summary": {
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            },
            "active_story_repair_payload": {
                "quality_gate_decision": "auto_repair",
                "quality_gate_label": "建议继续修复",
                "phase": "repair_pending"
            }
        });

        let mut payload =
            super::build_manual_review_terminal_runtime_patch_contract(7, "等待人工复核");
        super::apply_terminal_quality_runtime_patch_contract(
            &mut payload,
            Some(&runtime_state),
            "等待人工复核",
        );

        assert_eq!(payload["analysis_task_message"], "第 7 章触发质量门禁，需人工复核");
        assert_eq!(payload["analysis_task_progress"], 100);
        assert!(payload["analysis_last_error"].is_null());
        assert_eq!(payload["quality_gate_decision"], "manual_review");
        assert_eq!(payload["quality_gate_label"], "等待人工复核");
        assert_eq!(payload["phase"], "quality_blocked");
        assert_eq!(payload["quality_metrics_summary"]["quality_gate"]["decision"], "manual_review");
        assert_eq!(
            payload["active_story_repair_payload"]["quality_gate_decision"],
            "manual_review"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["quality_gate_label"],
            "等待人工复核"
        );
        assert_eq!(payload["active_story_repair_payload"]["phase"], "quality_blocked");
    }

    #[test]
    fn should_build_shared_retry_quality_runtime_patch_contract() {
        let mut payload =
            super::build_retry_quality_runtime_patch_contract(7, "自动修复后重试");
        payload.insert(
            "active_story_repair_payload".to_string(),
            json!({
                "summary": "继续补强冲突",
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核",
                "phase": "quality_blocked"
            }),
        );
        super::apply_retry_quality_runtime_patch_contract(&mut payload, "自动修复后重试");

        assert_eq!(payload["analysis_task_message"], "第 7 章触发质量修复，等待重试");
        assert_eq!(payload["analysis_task_progress"], 100);
        assert!(payload["analysis_last_error"].is_null());
        assert_eq!(payload["quality_gate_decision"], "auto_repair");
        assert_eq!(payload["quality_gate_label"], "自动修复后重试");
        assert_eq!(payload["phase"], "repair_pending");
        assert_eq!(
            payload["active_story_repair_payload"]["quality_gate_decision"],
            "auto_repair"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["quality_gate_label"],
            "自动修复后重试"
        );
        assert_eq!(payload["active_story_repair_payload"]["phase"], "repair_pending");
    }

    #[test]
    fn should_append_failed_chapter_entry_without_losing_existing_items() {
        let merged = super::append_failed_chapter_entry(
            &json!([{"chapter_id": "chapter-1"}]),
            Some(&json!({
                "chapter_id": "chapter-2",
                "error": "boom"
            })),
        );

        assert_eq!(merged, json!([
            {"chapter_id": "chapter-1"},
            {"chapter_id": "chapter-2", "error": "boom"}
        ]));
    }

    #[tokio::test]
    async fn should_skip_batch_generation_follow_up_analysis_when_disabled() {
        let (session, _) =
            BatchGenerationRuntimeSession::from_execution_input(BatchGenerationExecutionInput {
                user_id: "user-10".to_string(),
                chapter_ids: vec!["chapter-3".to_string()],
                target_word_count: 2800,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: false,
                    ..Default::default()
                },
                ai_config: AIConfig::default(),
            });

        let _ = run_batch_generation_follow_up_analysis(
            &sea_orm::DatabaseConnection::Disconnected,
            "task-1",
            &session,
            &GeneratedChapterResult {
                chapter_id: "chapter-3".to_string(),
                chapter_number: 3,
                title: "第三章".to_string(),
                content: "正文".to_string(),
                word_count: 2,
            },
        )
        .await;
    }

    #[tokio::test]
    async fn should_keep_batch_generation_runtime_dispatch_contract() {
        dispatch_batch_generation_runtime(
            sea_orm::DatabaseConnection::Disconnected,
            "task-5".to_string(),
            BatchGenerationExecutionInput {
                user_id: "user-5".to_string(),
                chapter_ids: vec!["chapter-5".to_string()],
                target_word_count: 2500,
                compat_options: SingleChapterGenerationCompatOptions::default(),
                ai_config: AIConfig::default(),
            },
        );
    }
}
