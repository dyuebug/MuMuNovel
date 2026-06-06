use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

use crate::ai::{service::AIService, AIConfig};
use crate::models::{batch_generation_snapshot, batch_generation_task, chapter, plot_analysis};
use crate::services::chapter_analysis_runtime_service::{
    analyze_generated_chapter_follow_up, prepare_chapter_analysis_execution,
};
use crate::services::chapter_batch_generation_quality_runtime_context_service::{
    apply_batch_quality_runtime_context_to_payload,
    resolve_batch_quality_runtime_context_from_current_quality,
    resolve_batch_quality_runtime_context_from_persisted_sources,
    resolve_batch_quality_runtime_context_preserving_existing_quality_state,
    BatchGenerationQualityRuntimeContext,
};
use crate::services::chapter_batch_generation_quality_status_service::{
    resolve_failed_terminal_semantics_from_sources, BatchGenerationFailedTerminalKind,
    BatchGenerationFailedTerminalSemantics, BatchGenerationQualityStatusContext,
};
use crate::services::chapter_batch_generation_read_context_service::build_batch_generation_status_task_payload_with_quality_context;
use crate::services::chapter_batch_generation_runtime_checkpoint_service::{
    build_batch_generation_runtime_checkpoint_for_stage, BatchGenerationFailureKind,
    BatchGenerationSnapshotStage,
};
use crate::services::chapter_batch_generation_snapshot_service::{
    project_merged_batch_generation_runtime_state, upsert_batch_generation_runtime_snapshot,
    BatchGenerationQueuedSnapshotPlan, BatchGenerationResumeSnapshotPlan,
};
use crate::services::chapter_batch_generation_task_payload_base_service::{
    build_batch_generation_command_summary_payload,
    build_batch_generation_task_response_payload_from_runtime_parts,
    BatchGenerationCommandProgressSummary, BatchGenerationTaskResponsePayloadOptions,
    BatchGenerationTaskResponseQualityPayload,
};
use crate::services::chapter_batch_generation_write_workflow_service::load_recent_batch_story_repair_quality_summary;
use crate::services::chapter_generation_execution_config_service::prepare_generation_execution_config;
use crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig;
use crate::services::chapter_generation_prerequisite_service::check_chapter_generation_prerequisites;
use crate::services::chapter_generation_quality_runtime_context_service::{
    apply_generation_quality_runtime_context_to_payload,
    resolve_generation_quality_runtime_context_from_persisted_sources,
};
use crate::services::chapter_generation_request_runtime_state_service::{
    active_story_repair_payload_from_runtime_state, parse_batch_generation_request_runtime_state,
    BatchGenerationRequestRuntimeState,
};
use crate::services::chapter_generation_research_payload_service::build_single_chapter_research_provider_payload;
use crate::services::chapter_generation_runtime_service::{
    generate_and_persist_chapter_content_with_provider_payload, GeneratedChapterResult,
};
use crate::services::chapter_generation_snapshot_query_service::load_chapter_generation_snapshot;
use crate::services::chapter_generation_terminal_runtime_patch_service::{
    apply_manual_review_terminal_fields as shared_apply_manual_review_terminal_fields,
    build_quality_gate_blocked_runtime_state_patch_from_workflow_state as shared_build_quality_gate_blocked_runtime_state_patch_from_workflow_state,
    build_retry_quality_runtime_patch_contract_from_workflow_state as shared_build_retry_quality_runtime_patch_contract_from_workflow_state,
};
use crate::services::chapter_single_generation_prepare_service::{
    prepare_single_chapter_runtime_launch_input_from_request_runtime_state,
    SingleChapterGenerationCompatOptions, SingleChapterGenerationTarget,
};
use crate::services::chapter_single_generation_runtime_state_service::{
    build_prompt_overrides_from_compat_options, SingleGenerationRuntimeLaunchInput,
};
use crate::services::chapter_story_repair_quality_context_service::{
    resolve_active_story_repair_payload_with_quality_fallback,
    resolve_resumed_active_story_repair_payload,
    restore_story_repair_compat_options_from_active_snapshot,
};

use super::chapter_batch_generation_resume_semantics_service::{
    ResumeBatchGenerationCommandState, ResumeResetSemantics,
};

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationExecutionInput {
    pub(crate) user_id: String,
    pub(crate) chapter_ids: Vec<String>,
    pub(crate) target_word_count: i32,
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) ai_config: AIConfig,
}

pub(crate) fn build_batch_generation_execution_input(
    user_id: String,
    chapter_ids: Vec<String>,
    target_word_count: i32,
    compat_options: SingleChapterGenerationCompatOptions,
    execution_config: PreparedGenerationExecutionConfig,
) -> BatchGenerationExecutionInput {
    BatchGenerationExecutionInput {
        user_id,
        chapter_ids,
        target_word_count,
        compat_options,
        ai_config: execution_config.ai_config,
    }
}

pub(crate) fn restore_batch_generation_runtime_compat_options_from_runtime_state_seed(
    base_compat_options: &SingleChapterGenerationCompatOptions,
    runtime_state_seed: Option<&Value>,
) -> SingleChapterGenerationCompatOptions {
    match runtime_state_seed {
        Some(runtime_state_seed) => {
            let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::from_sources(
                Some(runtime_state_seed.clone()),
                None,
                None,
                None,
            );
            restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
                base_compat_options,
                &persisted_runtime_context,
            )
        }
        None => base_compat_options.clone(),
    }
}

pub(crate) fn build_batch_generation_runtime_launch_input_from_runtime_state_seed(
    user_id: String,
    chapter_ids: Vec<String>,
    target_word_count: i32,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_state_seed: Option<&Value>,
    execution_config: PreparedGenerationExecutionConfig,
) -> BatchGenerationExecutionInput {
    let resolved_compat_options =
        restore_batch_generation_runtime_compat_options_from_runtime_state_seed(
            &request_runtime_state.compat_options,
            runtime_state_seed,
        );

    build_batch_generation_execution_input(
        user_id,
        chapter_ids,
        target_word_count,
        resolved_compat_options,
        execution_config,
    )
}

pub(crate) fn build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed(
    user_id: String,
    chapter_ids: Vec<String>,
    total_chapters: i32,
    target_word_count: i32,
    runtime_state_seed: Value,
    execution_config: PreparedGenerationExecutionConfig,
) -> (
    BatchGenerationQueuedSnapshotPlan,
    BatchGenerationExecutionInput,
) {
    let request_runtime_state =
        parse_batch_generation_request_runtime_state(Some(&runtime_state_seed));
    let runtime_input = build_batch_generation_runtime_launch_input_from_runtime_state_seed(
        user_id,
        chapter_ids,
        target_word_count,
        &request_runtime_state,
        Some(&runtime_state_seed),
        execution_config,
    );
    let startup_snapshot_plan = BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
        total_chapters,
        Some(runtime_state_seed),
    );

    (startup_snapshot_plan, runtime_input)
}

pub(crate) async fn prepare_batch_generation_runtime_launch_input_from_request_runtime_state(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_ids: Vec<String>,
    target_word_count: i32,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_state_seed: Option<&Value>,
) -> Result<BatchGenerationExecutionInput, String> {
    let model_override = request_runtime_state.model_override.clone();
    let execution_config =
        prepare_generation_execution_config(db, user_id, model_override.as_deref()).await?;

    Ok(
        build_batch_generation_runtime_launch_input_from_runtime_state_seed(
            user_id.to_string(),
            chapter_ids,
            target_word_count,
            request_runtime_state,
            runtime_state_seed,
            execution_config,
        ),
    )
}

pub(crate) fn dispatch_batch_generation_runtime(
    db: DatabaseConnection,
    task_id: String,
    execution_input: BatchGenerationExecutionInput,
) {
    tokio::spawn(async move {
        BatchGenerationRuntimeLifecyclePlan::start(&db, &task_id, execution_input).await;
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
    plan: BatchGenerationResumeResetPersistencePlan,
) -> Result<(), String> {
    plan.persist(db).await
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationCancelledPersistencePlan {
    batch_id: String,
    merged_runtime_state: Value,
    quality_status_context: BatchGenerationQualityStatusContext,
}

impl BatchGenerationCancelledPersistencePlan {
    pub(crate) fn from_sources(
        task: &batch_generation_task::Model,
        snapshot: Option<&batch_generation_snapshot::Model>,
    ) -> Self {
        let checkpoint = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Cancelled,
            None,
            None,
            task.completed_chapters,
            task.total_chapters,
        );
        let merged_runtime_state = project_merged_batch_generation_runtime_state(
            snapshot.and_then(|item| item.workflow_runtime_state.as_ref()),
            &checkpoint,
        );
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                snapshot,
                Some(&merged_runtime_state),
            );

        Self {
            batch_id: task.id.clone(),
            merged_runtime_state,
            quality_status_context,
        }
    }

    fn build_status_payload_for_task(&self, task: &batch_generation_task::Model) -> Value {
        build_batch_generation_status_task_payload_with_quality_context(
            &task,
            Some(&self.merged_runtime_state),
            &self.quality_status_context,
        )
    }

    fn build_response_payload_for_task(&self, task: batch_generation_task::Model) -> Value {
        let mut payload = match self.build_status_payload_for_task(&task) {
            Value::Object(payload) => payload,
            _ => serde_json::Map::new(),
        };
        let summary_payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: task.id.clone(),
                total_chapters: task.total_chapters,
                completed_chapters: task.completed_chapters,
            },
            "Batch generation cancelled",
        );
        if let Value::Object(summary_fields) = summary_payload {
            payload.extend(summary_fields);
        }

        Value::Object(payload)
    }

    #[cfg(test)]
    pub(crate) fn response_payload_for_test(&self, task: batch_generation_task::Model) -> Value {
        self.build_response_payload_for_task(task)
    }

    pub(crate) async fn persist(self, db: &DatabaseConnection) -> Result<Value, String> {
        let BatchGenerationCancelledPersistencePlan {
            batch_id,
            merged_runtime_state,
            quality_status_context,
        } = self;

        let response_task = if let Some(task_model) =
            batch_generation_task::Entity::find_by_id(&batch_id)
                .one(db)
                .await
                .map_err(|error| error.to_string())?
        {
            let completed_chapters = task_model.completed_chapters;
            let total_chapters = task_model.total_chapters;
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            BatchGenerationTaskStage::Cancelled.apply_to_active_model(
                &mut active,
                None,
                None,
                completed_chapters,
                total_chapters,
                None,
                Utc::now().naive_utc(),
            );
            active.update(db).await.map_err(|error| error.to_string())?
        } else {
            return Err("Batch generation task not found during cancel persistence".to_string());
        };
        let response_owner = BatchGenerationCancelledPersistencePlan {
            batch_id: batch_id.clone(),
            merged_runtime_state: merged_runtime_state.clone(),
            quality_status_context,
        };
        upsert_batch_generation_runtime_snapshot(db, &batch_id, merged_runtime_state).await?;

        Ok(response_owner.build_response_payload_for_task(response_task))
    }
}

#[cfg(test)]
pub(crate) fn build_batch_generation_resume_runtime_checkpoint(
    task: &ResumeBatchGenerationCommandState,
    runtime_state_seed: Option<Value>,
) -> Value {
    task.resolve_reset_semantics()
        .build_resume_checkpoint_with_seed(task.total_chapters, runtime_state_seed)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationResumeResetPersistencePlan {
    batch_id: String,
    total_chapters: i32,
    reset_semantics: ResumeResetSemantics,
    resume_checkpoint: Value,
    task_reset_plan: BatchGenerationResumeTaskResetMutationPlan,
    resume_snapshot_plan: BatchGenerationResumeSnapshotPlan,
}

impl BatchGenerationResumeResetPersistencePlan {
    #[cfg(test)]
    pub(crate) fn from_resume_task(
        task: &ResumeBatchGenerationCommandState,
        runtime_state_seed: Option<Value>,
    ) -> Self {
        Self::from_resume_task_with_existing_runtime_state(task, runtime_state_seed, None)
    }

    pub(crate) fn from_resume_task_with_existing_runtime_state(
        task: &ResumeBatchGenerationCommandState,
        runtime_state_seed: Option<Value>,
        existing_workflow_runtime_state: Option<Value>,
    ) -> Self {
        let reset_semantics = task.resolve_reset_semantics();
        let resume_checkpoint = reset_semantics
            .build_resume_checkpoint_with_seed(task.total_chapters, runtime_state_seed);
        Self {
            batch_id: task.batch_id.clone(),
            total_chapters: task.total_chapters,
            task_reset_plan: BatchGenerationResumeTaskResetMutationPlan::from_reset_semantics(
                task.total_chapters,
                &reset_semantics,
            ),
            resume_snapshot_plan: BatchGenerationResumeSnapshotPlan::from_resume_checkpoint(
                existing_workflow_runtime_state,
                resume_checkpoint.clone(),
            ),
            resume_checkpoint,
            reset_semantics,
        }
    }

    pub(crate) fn total_chapters(&self) -> i32 {
        self.total_chapters
    }

    pub(crate) fn completed_chapters(&self) -> i32 {
        self.reset_semantics.completed_chapters
    }

    pub(crate) fn status(&self) -> &'static str {
        self.reset_semantics.status
    }

    pub(crate) fn current_chapter_id(&self) -> Option<&str> {
        self.reset_semantics.current_chapter_id.as_deref()
    }

    pub(crate) fn checkpoint(&self) -> &Value {
        &self.resume_checkpoint
    }

    #[cfg(test)]
    pub(crate) fn resume_snapshot_plan(&self) -> &BatchGenerationResumeSnapshotPlan {
        &self.resume_snapshot_plan
    }

    pub(crate) fn single_quality_runtime_context(
        &self,
    ) -> crate::services::chapter_generation_quality_runtime_context_service::GenerationQualityRuntimeContext
    {
        resolve_generation_quality_runtime_context_from_persisted_sources(
            "chapter",
            self.latest_quality_metrics(),
            self.quality_metrics_history(),
            self.quality_metrics_summary_state(),
            self.quality_metrics_summary(),
        )
    }

    pub(crate) fn batch_quality_runtime_context(&self) -> BatchGenerationQualityRuntimeContext {
        resolve_batch_quality_runtime_context_from_persisted_sources(
            self.latest_quality_metrics(),
            self.quality_metrics_history(),
            self.quality_metrics_summary_state(),
            self.quality_metrics_summary(),
        )
    }

    pub(crate) fn latest_quality_metrics(&self) -> Option<&Value> {
        self.resume_checkpoint.get("latest_quality_metrics")
    }

    pub(crate) fn quality_metrics_history(&self) -> Option<&Value> {
        self.resume_checkpoint.get("quality_metrics_history")
    }

    pub(crate) fn quality_metrics_summary_state(&self) -> Option<&Value> {
        self.resume_checkpoint.get("quality_metrics_summary_state")
    }

    pub(crate) fn quality_metrics_summary(&self) -> Option<&Value> {
        self.resume_checkpoint.get("quality_metrics_summary")
    }

    pub(crate) fn active_story_repair_payload(&self) -> Option<Value> {
        active_story_repair_payload_from_runtime_state(Some(&self.resume_checkpoint))
    }

    pub(crate) fn quality_history_context_for_task_kind(
        &self,
        task_kind: crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind,
    ) -> Option<Value> {
        self.resume_checkpoint
            .get("quality_history_context")
            .cloned()
            .or_else(|| match task_kind {
                crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::SingleChapter => {
                    self.single_quality_runtime_context().quality_history_context
                }
                crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::Batch => {
                    self.batch_quality_runtime_context().quality_history_context
                }
            })
    }

    pub(crate) fn into_resume_response_payload(
        self,
        command_state: &ResumeBatchGenerationCommandState,
    ) -> Value {
        let task_kind = command_state.task_kind();
        let summary = BatchGenerationCommandProgressSummary {
            batch_id: command_state.batch_id.clone(),
            total_chapters: self.total_chapters(),
            completed_chapters: self.completed_chapters(),
        };
        let quality_payload = match task_kind {
            crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::SingleChapter => {
                Some(BatchGenerationTaskResponseQualityPayload::Single {
                    quality_runtime_context: self.single_quality_runtime_context(),
                    latest_quality_metrics: self.latest_quality_metrics().cloned(),
                    quality_metrics_summary: self.quality_metrics_summary().cloned(),
                    quality_metrics_history: self.quality_metrics_history().cloned(),
                })
            }
            crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::Batch => {
                Some(BatchGenerationTaskResponseQualityPayload::Batch {
                    quality_runtime_context: self.batch_quality_runtime_context(),
                    quality_metrics_summary: self.quality_metrics_summary().cloned(),
                })
            }
        };
        let payload = build_batch_generation_task_response_payload_from_runtime_parts(
            command_state.batch_id.as_str(),
            crate::services::chapter_batch_generation_status_semantics_service::batch_generation_task_type(
                task_kind,
            ),
            &command_state.project_id,
            self.status(),
            self.current_chapter_id(),
            command_state.created_at,
            Some(self.checkpoint()),
            BatchGenerationTaskResponsePayloadOptions {
                checkpoint_override: Some((
                    "chapter_id".to_string(),
                    json!(self.current_chapter_id()),
                )),
                summary_payload: Some(build_batch_generation_command_summary_payload(
                    summary,
                    "Task resumed and queued",
                )),
                quality_payload,
                active_story_repair_payload: self.active_story_repair_payload(),
                quality_history_context: self.quality_history_context_for_task_kind(task_kind),
                extra_fields: vec![(
                    "resumed_from_batch_id".to_string(),
                    json!(command_state.batch_id.clone()),
                )],
                apply_loading_stage_fields: true,
            },
        );

        Value::Object(payload)
    }

    #[cfg(test)]
    pub(crate) fn from_contract_for_test(
        batch_id: String,
        total_chapters: i32,
        reset_semantics: ResumeResetSemantics,
        resume_checkpoint: Value,
    ) -> Self {
        let task_reset_plan = BatchGenerationResumeTaskResetMutationPlan::from_reset_semantics(
            total_chapters,
            &reset_semantics,
        );
        let resume_snapshot_plan = BatchGenerationResumeSnapshotPlan::from_resume_checkpoint(
            None,
            resume_checkpoint.clone(),
        );
        Self {
            batch_id,
            total_chapters,
            reset_semantics,
            resume_checkpoint,
            task_reset_plan,
            resume_snapshot_plan,
        }
    }

    pub(crate) async fn persist(self, db: &DatabaseConnection) -> Result<(), String> {
        let BatchGenerationResumeResetPersistencePlan {
            batch_id,
            task_reset_plan,
            resume_snapshot_plan,
            ..
        } = self;
        let mut active = batch_generation_task::ActiveModel {
            id: Set(batch_id.clone()),
            ..Default::default()
        };
        task_reset_plan.apply_to_active_model(&mut active, Utc::now().naive_utc());

        active.update(db).await.map_err(|error| error.to_string())?;
        resume_snapshot_plan.persist_replace(db, &batch_id).await
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BatchGenerationResumeTaskResetMutationPlan {
    failed_chapters: Value,
    current_chapter_id: Option<String>,
    current_chapter_number: Option<i32>,
    completed_chapters: i32,
    total_chapters: i32,
}

impl BatchGenerationResumeTaskResetMutationPlan {
    fn from_reset_semantics(total_chapters: i32, reset_semantics: &ResumeResetSemantics) -> Self {
        Self {
            failed_chapters: reset_semantics.failed_chapters.clone(),
            current_chapter_id: reset_semantics.current_chapter_id.clone(),
            current_chapter_number: reset_semantics.current_chapter_number,
            completed_chapters: reset_semantics.completed_chapters,
            total_chapters,
        }
    }

    fn apply_to_active_model(
        self,
        active: &mut batch_generation_task::ActiveModel,
        now: chrono::NaiveDateTime,
    ) {
        active.failed_chapters = Set(self.failed_chapters);
        BatchGenerationTaskStage::ResumeReset.apply_to_active_model(
            active,
            self.current_chapter_id.as_deref(),
            self.current_chapter_number,
            self.completed_chapters,
            self.total_chapters,
            None,
            now,
        );
    }
}

fn append_failed_chapter_entry(failed_chapters: &Value, failed_entry: Option<&Value>) -> Value {
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
    terminal_semantics: &BatchGenerationFailedTerminalSemantics,
    workflow_runtime_state: Option<&Value>,
) -> Value {
    let mut entry = build_batch_generation_failed_chapter_entry(
        chapter_id,
        chapter_number,
        chapter_title,
        task_error_message,
        retry_count,
    );
    if let Some(object) = entry.as_object_mut() {
        if terminal_semantics.kind == BatchGenerationFailedTerminalKind::ManualReview {
            apply_manual_review_terminal_fields(object, &terminal_semantics.label);
        }
        object.insert("quality_gate_status".to_string(), json!("failed"));
        object.insert(
            "quality_gate_failed_metrics".to_string(),
            json!(extract_quality_gate_failed_metrics_from_runtime_state(
                workflow_runtime_state
            )),
        );
    }
    entry
}

fn extract_quality_gate_failed_metrics_from_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> Vec<String> {
    let mut collected = Vec::new();

    for candidate in [
        workflow_runtime_state.and_then(|state| state.get("active_story_repair_payload")),
        workflow_runtime_state.and_then(|state| state.get("quality_metrics_summary")),
        workflow_runtime_state.and_then(|state| state.get("latest_quality_metrics")),
    ] {
        collected.extend(extract_quality_gate_failed_metrics_from_payload(candidate));
        if !collected.is_empty() {
            break;
        }
    }

    let mut seen = std::collections::HashSet::new();
    collected
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

fn extract_quality_gate_failed_metrics_from_payload(value: Option<&Value>) -> Vec<String> {
    let Some(payload) = value.and_then(Value::as_object) else {
        return Vec::new();
    };

    let direct_items = payload
        .get("quality_gate_failed_metrics")
        .and_then(Value::as_array);
    let nested_items = payload
        .get("quality_gate")
        .and_then(Value::as_object)
        .and_then(|gate| gate.get("failed_metrics"))
        .and_then(Value::as_array);

    direct_items
        .or(nested_items)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(str::to_string).or_else(|| {
                        item.as_object()
                            .and_then(|entry| entry.get("label"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                })
                .collect()
        })
        .unwrap_or_default()
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

    fn completed_at_update(
        self,
        completed_chapters: i32,
        total_chapters: i32,
    ) -> TaskTimestampUpdate {
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

    fn error_message_update(
        self,
        error_message: Option<String>,
    ) -> ModelFieldUpdate<Option<String>> {
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

    fn current_chapter_id_update(
        self,
        current_chapter_id: Option<&str>,
    ) -> ModelFieldUpdate<Option<String>> {
        match self {
            Self::ResumeReset | Self::ChapterStarted | Self::ChapterSucceeded | Self::Failed => {
                ModelFieldUpdate::Set(current_chapter_id.map(str::to_string))
            }
            Self::Preparing | Self::Cancelled => ModelFieldUpdate::Keep,
        }
    }

    fn current_chapter_number_update(
        self,
        current_chapter_number: Option<i32>,
    ) -> ModelFieldUpdate<Option<i32>> {
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
        failed_retry_count: i32,
        failed_entry_error: String,
        task_error_message: String,
    ) -> Self {
        let failed_chapter_entry = Some(build_batch_generation_failed_chapter_entry(
            chapter_id,
            chapter_number,
            chapter_title,
            &failed_entry_error,
            failed_retry_count,
        ));
        Self {
            task_stage: BatchGenerationTaskStage::Failed,
            checkpoint_stage: BatchGenerationSnapshotStage::Failed(failure_kind),
            current_chapter_id: chapter_id.map(str::to_string),
            current_chapter_number: chapter_number,
            completed_chapters,
            total_chapters,
            current_retry_count: Some(failed_retry_count.max(0)),
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
        terminal_semantics: &BatchGenerationFailedTerminalSemantics,
        workflow_runtime_state: Option<&Value>,
        task_error_message: String,
    ) -> Self {
        let failed_chapter_entry = Some(build_quality_gate_blocked_failed_chapter_entry(
            chapter_id,
            chapter_number,
            chapter_title,
            &task_error_message,
            retry_count,
            terminal_semantics,
            workflow_runtime_state,
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

    pub(crate) async fn persist(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> Result<(), String> {
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

fn should_retry_batch_generation_attempt(next_retry_count: i32, max_retries: i32) -> bool {
    next_retry_count >= 0 && next_retry_count <= max_retries.max(0)
}

fn batch_generation_retry_backoff_seconds(next_retry_count: i32) -> u64 {
    let exponent = next_retry_count.clamp(0, 4) as u32;
    2_u64.pow(exponent).min(10)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchGenerationRetryPersistenceContract {
    Generic,
    QualityGate {
        terminal_semantics: BatchGenerationFailedTerminalSemantics,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationRetryPersistencePlan {
    current_chapter_id: String,
    current_chapter_number: Option<i32>,
    completed_chapters: i32,
    total_chapters: i32,
    next_retry_count: i32,
    max_retries: i32,
    wait_seconds: u64,
    error_message: String,
    retry_contract: BatchGenerationRetryPersistenceContract,
}

impl BatchGenerationRetryPersistencePlan {
    fn new(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        next_retry_count: i32,
        max_retries: i32,
        error_message: &str,
        retry_contract: BatchGenerationRetryPersistenceContract,
    ) -> Self {
        Self::from_step_context(
            &chapter_model.id,
            Some(chapter_model.chapter_number),
            progress,
            next_retry_count,
            max_retries,
            error_message,
            retry_contract,
        )
    }

    fn from_step_context(
        chapter_id: &str,
        chapter_number: Option<i32>,
        progress: &BatchGenerationStepProgress,
        next_retry_count: i32,
        max_retries: i32,
        error_message: &str,
        retry_contract: BatchGenerationRetryPersistenceContract,
    ) -> Self {
        Self {
            current_chapter_id: chapter_id.to_string(),
            current_chapter_number: chapter_number,
            completed_chapters: progress.completed,
            total_chapters: progress.total_chapters,
            next_retry_count: next_retry_count.max(0),
            max_retries,
            wait_seconds: batch_generation_retry_backoff_seconds(next_retry_count),
            error_message: error_message.to_string(),
            retry_contract,
        }
    }

    fn build_waiting_snapshot(&self) -> Value {
        let mut checkpoint = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::ChapterStarted,
            Some(&self.current_chapter_id),
            self.current_chapter_number,
            self.completed_chapters,
            self.total_chapters,
        );
        if let Some(checkpoint_object) = checkpoint.as_object_mut() {
            checkpoint_object.insert(
                "last_event".to_string(),
                Value::String("chapter_retry".to_string()),
            );
            checkpoint_object.insert(
                "last_message".to_string(),
                Value::String(match self.current_chapter_number {
                    Some(chapter_number) => format!(
                        "第 {} 章生成失败，{} 秒后进行第 {} 次重试",
                        chapter_number, self.wait_seconds, self.next_retry_count
                    ),
                    None => format!(
                        "章节生成失败，{} 秒后进行第 {} 次重试",
                        self.wait_seconds, self.next_retry_count
                    ),
                }),
            );
            checkpoint_object.insert(
                "current_retry_count".to_string(),
                Value::Number(self.next_retry_count.into()),
            );
            checkpoint_object.insert(
                "max_retries".to_string(),
                Value::Number(self.max_retries.into()),
            );
            checkpoint_object.insert(
                "retry_backoff_seconds".to_string(),
                Value::Number((self.wait_seconds as i64).into()),
            );
            checkpoint_object.insert(
                "last_error".to_string(),
                Value::String(self.error_message.clone()),
            );
            if let BatchGenerationRetryPersistenceContract::QualityGate { terminal_semantics } =
                &self.retry_contract
            {
                checkpoint_object.insert(
                    "terminal_reason".to_string(),
                    json!(terminal_semantics.reason),
                );
                checkpoint_object.insert(
                    "terminal_label".to_string(),
                    json!(terminal_semantics.label.clone()),
                );
                checkpoint_object.insert(
                    "review_required".to_string(),
                    json!(terminal_semantics.review_required),
                );
                checkpoint_object.insert(
                    "can_resume".to_string(),
                    json!(terminal_semantics.can_resume),
                );
                if terminal_semantics.kind == BatchGenerationFailedTerminalKind::Retry {
                    checkpoint_object
                        .insert("quality_gate_decision".to_string(), json!("auto_repair"));
                    checkpoint_object.insert(
                        "quality_gate_label".to_string(),
                        json!(terminal_semantics.label.clone()),
                    );
                    checkpoint_object.insert("phase".to_string(), json!("repair_pending"));
                }
            }
        }
        checkpoint
    }

    fn apply_to_active_model(&self, active: &mut batch_generation_task::ActiveModel) {
        active.status = Set("running".to_string());
        active.error_message = Set(None);
        active.current_chapter_id = Set(Some(self.current_chapter_id.clone()));
        active.current_chapter_number = Set(self.current_chapter_number);
        active.current_retry_count = Set(self.next_retry_count);
    }

    async fn persist(self, db: &DatabaseConnection, task_id: &str) {
        if let Ok(Some(task_model)) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
        {
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            self.apply_to_active_model(&mut active);
            let _ = active.update(db).await;
        }

        let _ =
            upsert_batch_generation_runtime_snapshot(db, task_id, self.build_waiting_snapshot())
                .await;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchGenerationGenericFailureRoutingPlan {
    Retry(BatchGenerationRetryPersistencePlan),
    Stop(BatchGenerationRuntimePersistencePlan),
}

impl BatchGenerationGenericFailureRoutingPlan {
    fn from_step_error(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        current_retry_count: i32,
        max_retries: i32,
        failure_kind: BatchGenerationFailureKind,
        error_message: &str,
    ) -> Self {
        Self::from_step_context(
            &chapter_model.id,
            Some(chapter_model.chapter_number),
            Some(&chapter_model.title),
            progress,
            current_retry_count,
            max_retries,
            failure_kind,
            error_message,
        )
    }

    fn from_step_context(
        chapter_id: &str,
        chapter_number: Option<i32>,
        chapter_title: Option<&str>,
        progress: &BatchGenerationStepProgress,
        current_retry_count: i32,
        max_retries: i32,
        failure_kind: BatchGenerationFailureKind,
        error_message: &str,
    ) -> Self {
        let next_retry_count = current_retry_count + 1;
        if should_retry_batch_generation_attempt(next_retry_count, max_retries) {
            return Self::Retry(BatchGenerationRetryPersistencePlan::from_step_context(
                chapter_id,
                chapter_number,
                progress,
                next_retry_count,
                max_retries,
                error_message,
                BatchGenerationRetryPersistenceContract::Generic,
            ));
        }

        Self::Stop(BatchGenerationRuntimePersistencePlan::failed(
            Some(chapter_id),
            chapter_number,
            chapter_title,
            progress.completed,
            progress.total_chapters,
            failure_kind,
            next_retry_count - 1,
            error_message.to_string(),
            build_batch_generation_failed_task_error_message(
                chapter_number,
                next_retry_count - 1,
                error_message,
            ),
        ))
    }

    async fn persist_and_resolve(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> BatchGenerationAttemptProgression {
        match self {
            BatchGenerationGenericFailureRoutingPlan::Retry(plan) => {
                let next_retry_count = plan.next_retry_count;
                plan.persist(db, task_id).await;
                BatchGenerationRetryProgressionPlan::new(next_retry_count)
                    .execute()
                    .await
            }
            BatchGenerationGenericFailureRoutingPlan::Stop(plan) => {
                persist_batch_generation_runtime_plan(db, task_id, plan).await;
                BatchGenerationAttemptProgression::Driver(
                    BatchGenerationRuntimeDriverProgression::Stop,
                )
            }
        }
    }
}

fn build_batch_generation_failed_task_error_message(
    chapter_number: Option<i32>,
    retry_count: i32,
    error_message: &str,
) -> String {
    match chapter_number {
        Some(chapter_number) => format!(
            "第{}章生成失败(重试{}次): {}",
            chapter_number,
            retry_count.max(0),
            error_message
        ),
        None => format!(
            "章节生成失败(重试{}次): {}",
            retry_count.max(0),
            error_message
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchGenerationQualityGateRoutingPlan {
    Retry {
        runtime_state_patch: Value,
        persistence_plan: BatchGenerationRetryPersistencePlan,
        next_retry_count: i32,
    },
    Stop {
        runtime_state_patch: Value,
        persistence_plan: BatchGenerationRuntimePersistencePlan,
    },
}

impl BatchGenerationQualityGateRoutingPlan {
    fn from_terminal_semantics(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        workflow_runtime_state: Option<&Value>,
        current_retry_count: i32,
        max_retries: i32,
        terminal_semantics: BatchGenerationFailedTerminalSemantics,
    ) -> Option<Self> {
        match terminal_semantics.kind {
            BatchGenerationFailedTerminalKind::ManualReview => {
                let manual_review_label = terminal_semantics.label.clone();
                let failure_message = format!(
                    "第{}章触发质量门禁，需人工复核: {}",
                    chapter_model.chapter_number, manual_review_label
                );
                Some(Self::Stop {
                    runtime_state_patch:
                        shared_build_quality_gate_blocked_runtime_state_patch_from_workflow_state(
                            workflow_runtime_state,
                            chapter_model.chapter_number,
                            &manual_review_label,
                        ),
                    persistence_plan:
                        BatchGenerationRuntimePersistencePlan::failed_quality_gate_blocked(
                            Some(&chapter_model.id),
                            Some(chapter_model.chapter_number),
                            Some(&chapter_model.title),
                            progress.completed,
                            progress.total_chapters,
                            current_retry_count,
                            &terminal_semantics,
                            workflow_runtime_state,
                            failure_message,
                        ),
                })
            }
            BatchGenerationFailedTerminalKind::Retry => {
                let next_retry_count = current_retry_count + 1;
                if !should_retry_batch_generation_attempt(next_retry_count, max_retries) {
                    return None;
                }

                let retry_label = terminal_semantics.label.clone();
                let retry_message = format!(
                    "第{}章触发质量修复重试: {}",
                    chapter_model.chapter_number, retry_label
                );
                Some(Self::Retry {
                    runtime_state_patch: Value::Object(
                        shared_build_retry_quality_runtime_patch_contract_from_workflow_state(
                            workflow_runtime_state,
                            chapter_model.chapter_number,
                            &retry_label,
                        ),
                    ),
                    persistence_plan: BatchGenerationRetryPersistencePlan::new(
                        chapter_model,
                        progress,
                        next_retry_count,
                        max_retries,
                        &retry_message,
                        BatchGenerationRetryPersistenceContract::QualityGate { terminal_semantics },
                    ),
                    next_retry_count,
                })
            }
            BatchGenerationFailedTerminalKind::Error => None,
        }
    }

    async fn persist_and_resolve(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> BatchGenerationAttemptProgression {
        match self {
            BatchGenerationQualityGateRoutingPlan::Retry {
                runtime_state_patch,
                persistence_plan,
                next_retry_count,
            } => {
                persistence_plan.persist(db, task_id).await;
                let _ = upsert_batch_generation_runtime_snapshot(db, task_id, runtime_state_patch)
                    .await;
                BatchGenerationRetryProgressionPlan::new(next_retry_count)
                    .execute()
                    .await
            }
            BatchGenerationQualityGateRoutingPlan::Stop {
                runtime_state_patch,
                persistence_plan,
            } => {
                let _ = upsert_batch_generation_runtime_snapshot(db, task_id, runtime_state_patch)
                    .await;
                persist_batch_generation_runtime_plan(db, task_id, persistence_plan).await;
                BatchGenerationAttemptProgression::Driver(
                    BatchGenerationRuntimeDriverProgression::Stop,
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationAnalysisCompletionPersistencePlan {
    current_quality_snapshot: Option<Value>,
    completed_snapshot: Value,
}

impl BatchGenerationAnalysisCompletionPersistencePlan {
    async fn build_current_quality_snapshot(
        db: &DatabaseConnection,
        batch_task_id: &str,
        chapter_id: &str,
    ) -> Option<Value> {
        let latest_analysis = plot_analysis::Entity::find()
            .filter(plot_analysis::Column::ChapterId.eq(chapter_id))
            .order_by_desc(plot_analysis::Column::CreatedAt)
            .one(db)
            .await
            .ok()
            .flatten()?;
        let quality_summary =
            build_current_chapter_quality_summary_from_plot_analysis(&latest_analysis)?;
        let latest_quality_metrics =
            build_current_chapter_latest_quality_metrics_from_plot_analysis(&latest_analysis);
        let persisted_runtime_context = load_chapter_generation_snapshot(db, batch_task_id)
            .await
            .ok()
            .map(BatchGenerationPersistedRuntimeContext::from_snapshot)
            .unwrap_or_default();

        Some(
            persisted_runtime_context.build_current_chapter_quality_runtime_snapshot(
                &quality_summary,
                latest_quality_metrics.as_ref(),
            ),
        )
    }

    async fn from_generated_result(
        db: &DatabaseConnection,
        batch_task_id: &str,
        generated: &GeneratedChapterResult,
        analysis_retry_count: i32,
    ) -> Self {
        Self {
            current_quality_snapshot: Self::build_current_quality_snapshot(
                db,
                batch_task_id,
                &generated.chapter_id,
            )
            .await,
            completed_snapshot: build_batch_generation_analysis_completed_snapshot(
                generated,
                analysis_retry_count,
            ),
        }
    }

    async fn persist(self, db: &DatabaseConnection, batch_task_id: &str) -> Option<Value> {
        if let Some(current_quality_snapshot) = self.current_quality_snapshot.as_ref() {
            let _ = upsert_batch_generation_runtime_snapshot(
                db,
                batch_task_id,
                current_quality_snapshot.clone(),
            )
            .await;
        }
        let _ =
            upsert_batch_generation_runtime_snapshot(db, batch_task_id, self.completed_snapshot)
                .await;

        self.current_quality_snapshot
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationAnalysisStartedPersistencePlan {
    started_snapshot: Value,
}

impl BatchGenerationAnalysisStartedPersistencePlan {
    fn from_generated_result(
        analysis_task_id: Option<&str>,
        generated: &GeneratedChapterResult,
        analysis_retry_count: i32,
    ) -> Self {
        Self {
            started_snapshot: build_batch_generation_analysis_started_snapshot(
                analysis_task_id,
                generated,
                analysis_retry_count,
            ),
        }
    }

    async fn persist(self, db: &DatabaseConnection, batch_task_id: &str) {
        let _ = upsert_batch_generation_runtime_snapshot(db, batch_task_id, self.started_snapshot)
            .await;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchGenerationAnalysisRoutingPlan {
    Retry {
        retry_snapshot: Value,
        next_retry_count: i32,
        wait_seconds: u64,
    },
    Stop {
        error_message: String,
    },
}

impl BatchGenerationAnalysisRoutingPlan {
    fn from_analysis_error_message(
        chapter_number: i32,
        error_message: String,
        analysis_retry_count: i32,
    ) -> Self {
        if should_stop_batch_generation_analysis_without_retry(&error_message) {
            return Self::Stop { error_message };
        }

        if analysis_retry_count < 2 {
            let next_retry_count = analysis_retry_count + 1;
            let wait_seconds = 2_i32.pow(next_retry_count as u32).min(10) as u64;
            return Self::Retry {
                retry_snapshot: json!({
                    "last_event": "analysis_retry",
                    "last_message": format!("第 {} 章分析失败，准备重试", chapter_number),
                    "progress": 85,
                    "phase": "parsing",
                    "analysis_task_message": format!("第 {} 章分析失败，准备重试", chapter_number),
                    "analysis_task_progress": 85,
                    "analysis_last_error": error_message,
                    "analysis_retry_count": next_retry_count,
                    "analysis_max_retries": 3,
                }),
                next_retry_count,
                wait_seconds,
            };
        }

        Self::Stop { error_message }
    }

    async fn persist_and_resolve(
        self,
        db: &DatabaseConnection,
        batch_task_id: &str,
    ) -> Result<(), String> {
        match self {
            BatchGenerationAnalysisRoutingPlan::Retry {
                retry_snapshot,
                wait_seconds,
                ..
            } => {
                let _ = upsert_batch_generation_runtime_snapshot(db, batch_task_id, retry_snapshot)
                    .await;
                sleep(Duration::from_secs(wait_seconds)).await;
                Ok(())
            }
            BatchGenerationAnalysisRoutingPlan::Stop { error_message } => Err(error_message),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchGenerationAnalysisAttemptResolution {
    Completed(Option<Value>),
    Retry,
}

fn should_stop_batch_generation_analysis_without_retry(error_message: &str) -> bool {
    matches!(
        error_message,
        "章节不存在或内容为空"
            | "章节或项目已删除，无法继续分析"
            | "Chapter or project was deleted before analysis"
    )
}

#[derive(Debug, Clone, PartialEq)]
struct BatchGenerationAnalysisAttemptPlan {
    generated_result: GeneratedChapterResult,
    analysis_retry_count: i32,
}

impl BatchGenerationAnalysisAttemptPlan {
    fn from_generated_result(
        generated_result: &GeneratedChapterResult,
        analysis_retry_count: i32,
    ) -> Self {
        Self {
            generated_result: generated_result.clone(),
            analysis_retry_count,
        }
    }

    async fn persist_started(
        &self,
        db: &DatabaseConnection,
        batch_task_id: &str,
        analysis_task_id: Option<&str>,
    ) {
        BatchGenerationAnalysisStartedPersistencePlan::from_generated_result(
            analysis_task_id,
            &self.generated_result,
            self.analysis_retry_count,
        )
        .persist(db, batch_task_id)
        .await;
    }

    async fn execute(
        self,
        db: &DatabaseConnection,
        batch_task_id: &str,
        session: &BatchGenerationRuntimeSession,
    ) -> Result<BatchGenerationAnalysisAttemptResolution, String> {
        let prepared_analysis = prepare_chapter_analysis_execution(
            db,
            &self.generated_result.chapter_id,
            &session.user_id,
        )
        .await
        .ok();
        let analysis_task_id = prepared_analysis.as_ref().map(|item| item.task_id());
        self.persist_started(db, batch_task_id, analysis_task_id)
            .await;

        let generated_result = self.generated_result;
        let analysis_retry_count = self.analysis_retry_count;

        let result = if let Some(prepared_analysis) = prepared_analysis {
            prepared_analysis.execute(db, &session.user_id).await
        } else {
            analyze_generated_chapter_follow_up(db, &session.user_id, &generated_result)
                .await
                .map_err(|error| format_analysis_error_message(&error))
        };

        Self::resolve_result(
            db,
            batch_task_id,
            &generated_result,
            analysis_retry_count,
            result,
        )
        .await
    }

    async fn resolve_result(
        db: &DatabaseConnection,
        batch_task_id: &str,
        generated_result: &GeneratedChapterResult,
        analysis_retry_count: i32,
        result: Result<Value, String>,
    ) -> Result<BatchGenerationAnalysisAttemptResolution, String> {
        match result {
            Ok(_) => {
                let completion_plan =
                    BatchGenerationAnalysisCompletionPersistencePlan::from_generated_result(
                        db,
                        batch_task_id,
                        generated_result,
                        analysis_retry_count,
                    )
                    .await;
                Ok(BatchGenerationAnalysisAttemptResolution::Completed(
                    completion_plan.persist(db, batch_task_id).await,
                ))
            }
            Err(error_message) => {
                match BatchGenerationAnalysisRoutingPlan::from_analysis_error_message(
                    generated_result.chapter_number,
                    error_message,
                    analysis_retry_count,
                )
                .persist_and_resolve(db, batch_task_id)
                .await
                {
                    Ok(()) => Ok(BatchGenerationAnalysisAttemptResolution::Retry),
                    Err(error_message) => Err(error_message),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct BatchGenerationPersistedRuntimeContext {
    workflow_runtime_state: Option<Value>,
    request_runtime_state: BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<Value>,
    quality_metrics_history: Option<Value>,
    quality_metrics_summary_state: Option<Value>,
    quality_metrics_summary: Option<Value>,
    latest_quality_metrics: Option<Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct RestoredResumeRuntimeStateProjection {
    pub(crate) quality_status_context: BatchGenerationQualityStatusContext,
    pub(crate) request_runtime_state: BatchGenerationRequestRuntimeState,
    pub(crate) runtime_state_seed: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RestoredResumeRuntimeLaunchParts {
    pub(crate) request_runtime_state: BatchGenerationRequestRuntimeState,
    pub(crate) runtime_state_seed: Option<Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedBatchGenerationResumeRuntimeLaunch {
    pub(crate) runtime_input: BatchGenerationExecutionInput,
    pub(crate) runtime_state_seed: Option<Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedSingleChapterResumeRuntimeLaunch {
    pub(crate) runtime_input: SingleGenerationRuntimeLaunchInput,
    pub(crate) runtime_state_seed: Option<Value>,
}

impl RestoredResumeRuntimeStateProjection {
    pub(crate) fn from_persisted_runtime_context(
        task_kind: crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind,
        batch_id: &str,
        max_retries: i32,
        persisted_runtime_context: &BatchGenerationPersistedRuntimeContext,
    ) -> Self {
        persisted_runtime_context.build_restored_resume_runtime_state(
            task_kind,
            batch_id,
            max_retries,
        )
    }

    #[cfg(test)]
    pub(crate) fn from_sources(
        task_kind: crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind,
        batch_id: &str,
        max_retries: i32,
        workflow_runtime_state: Option<&Value>,
        snapshot: Option<&batch_generation_snapshot::Model>,
        request_runtime_state: &BatchGenerationRequestRuntimeState,
    ) -> Self {
        let workflow_runtime_state = match workflow_runtime_state.cloned() {
            Some(Value::Object(mut state)) => {
                state
                    .entry("batch_request_runtime_state".to_string())
                    .or_insert_with(|| json!(request_runtime_state));
                Some(Value::Object(state))
            }
            _ => {
                let state = serde_json::Map::from_iter([(
                    "batch_request_runtime_state".to_string(),
                    json!(request_runtime_state),
                )]);
                Some(Value::Object(state))
            }
        };
        let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::from_sources(
            workflow_runtime_state,
            snapshot.and_then(|item| item.quality_metrics_history.clone()),
            snapshot.and_then(|item| item.quality_metrics_summary.clone()),
            snapshot.and_then(|item| item.latest_quality_metrics.clone()),
        );

        Self::from_persisted_runtime_context(
            task_kind,
            batch_id,
            max_retries,
            &persisted_runtime_context,
        )
    }

    pub(crate) fn is_manual_review_blocked(
        &self,
        command_state: &ResumeBatchGenerationCommandState,
    ) -> bool {
        resolve_failed_terminal_semantics_from_sources(
            Some(&command_state.failed_chapters),
            Some(&self.quality_status_context),
            command_state.current_retry_count,
            command_state.max_retries,
        )
        .as_ref()
        .is_some_and(|semantics| semantics.kind == BatchGenerationFailedTerminalKind::ManualReview)
    }

    pub(crate) fn into_launch_parts(self) -> RestoredResumeRuntimeLaunchParts {
        RestoredResumeRuntimeLaunchParts {
            request_runtime_state: self.request_runtime_state,
            runtime_state_seed: self.runtime_state_seed,
        }
    }

    pub(crate) async fn prepare_batch_runtime_launch(
        self,
        db: &DatabaseConnection,
        user_id: &str,
        chapter_ids: Vec<String>,
        target_word_count: i32,
    ) -> Result<PreparedBatchGenerationResumeRuntimeLaunch, String> {
        let RestoredResumeRuntimeLaunchParts {
            request_runtime_state,
            runtime_state_seed,
        } = self.into_launch_parts();
        let runtime_input =
            prepare_batch_generation_runtime_launch_input_from_request_runtime_state(
                db,
                user_id,
                chapter_ids,
                target_word_count,
                &request_runtime_state,
                runtime_state_seed.as_ref(),
            )
            .await?;

        Ok(PreparedBatchGenerationResumeRuntimeLaunch {
            runtime_input,
            runtime_state_seed,
        })
    }

    pub(crate) async fn prepare_single_chapter_runtime_launch(
        self,
        db: &DatabaseConnection,
        user_id: &str,
        chapter_target: &SingleChapterGenerationTarget,
        target_word_count: i32,
    ) -> Result<PreparedSingleChapterResumeRuntimeLaunch, String> {
        let RestoredResumeRuntimeLaunchParts {
            request_runtime_state,
            runtime_state_seed,
        } = self.into_launch_parts();
        let runtime_input = prepare_single_chapter_runtime_launch_input_from_request_runtime_state(
            db,
            user_id,
            chapter_target,
            &request_runtime_state,
            target_word_count,
        )
        .await
        .map_err(|error| error.detail_message())?;

        Ok(PreparedSingleChapterResumeRuntimeLaunch {
            runtime_input,
            runtime_state_seed,
        })
    }
}

impl BatchGenerationPersistedRuntimeContext {
    pub(crate) fn from_snapshot(snapshot: Option<batch_generation_snapshot::Model>) -> Self {
        let workflow_runtime_state = snapshot
            .as_ref()
            .and_then(|item| item.workflow_runtime_state.clone());
        let snapshot_quality_metrics_history = snapshot
            .as_ref()
            .and_then(|item| item.quality_metrics_history.clone());
        let snapshot_quality_metrics_summary = snapshot
            .as_ref()
            .and_then(|item| item.quality_metrics_summary.clone());
        let snapshot_latest_quality_metrics = snapshot
            .as_ref()
            .and_then(|item| item.latest_quality_metrics.clone());

        Self::from_sources(
            workflow_runtime_state,
            snapshot_quality_metrics_history,
            snapshot_quality_metrics_summary,
            snapshot_latest_quality_metrics,
        )
    }

    pub(crate) fn from_sources(
        workflow_runtime_state: Option<Value>,
        snapshot_quality_metrics_history: Option<Value>,
        snapshot_quality_metrics_summary: Option<Value>,
        snapshot_latest_quality_metrics: Option<Value>,
    ) -> Self {
        let request_runtime_state =
            parse_batch_generation_request_runtime_state(workflow_runtime_state.as_ref());
        let explicit_story_repair_payload =
            active_story_repair_payload_from_runtime_state(workflow_runtime_state.as_ref());
        let latest_quality_metrics = snapshot_latest_quality_metrics.or_else(|| {
            workflow_runtime_state
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|state| state.get("latest_quality_metrics").cloned())
        });
        let quality_metrics_history = snapshot_quality_metrics_history.or_else(|| {
            workflow_runtime_state
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|state| state.get("quality_metrics_history").cloned())
        });
        let quality_metrics_summary_state = workflow_runtime_state
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|state| state.get("quality_metrics_summary_state").cloned());
        let quality_metrics_summary = snapshot_quality_metrics_summary.or_else(|| {
            workflow_runtime_state
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|state| state.get("quality_metrics_summary").cloned())
        });

        Self {
            workflow_runtime_state,
            request_runtime_state,
            explicit_story_repair_payload,
            quality_metrics_history,
            quality_metrics_summary_state,
            quality_metrics_summary,
            latest_quality_metrics,
        }
    }

    pub(crate) fn has_workflow_runtime_state(&self) -> bool {
        self.workflow_runtime_state.is_some()
    }

    pub(crate) fn request_runtime_state(&self) -> &BatchGenerationRequestRuntimeState {
        &self.request_runtime_state
    }

    pub(crate) fn explicit_story_repair_payload(&self) -> Option<&Value> {
        self.explicit_story_repair_payload.as_ref()
    }

    pub(crate) fn quality_metrics_history(&self) -> Option<&Value> {
        self.quality_metrics_history.as_ref()
    }

    pub(crate) fn quality_metrics_summary_state(&self) -> Option<&Value> {
        self.quality_metrics_summary_state.as_ref()
    }

    pub(crate) fn quality_metrics_summary(&self) -> Option<&Value> {
        self.quality_metrics_summary.as_ref()
    }

    pub(crate) fn latest_quality_metrics(&self) -> Option<&Value> {
        self.latest_quality_metrics.as_ref()
    }

    pub(crate) fn restored_quality_runtime_context(
        &self,
        task_kind: crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind,
    ) -> BatchGenerationQualityRuntimeContext {
        match task_kind {
            crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::SingleChapter => {
                let resolved = resolve_generation_quality_runtime_context_from_persisted_sources(
                    "chapter",
                    self.latest_quality_metrics(),
                    self.quality_metrics_history(),
                    self.quality_metrics_summary_state(),
                    self.quality_metrics_summary(),
                );

                BatchGenerationQualityRuntimeContext {
                    latest_quality_metrics: resolved.latest_quality_metrics,
                    quality_metrics_history: resolved.quality_metrics_history,
                    quality_metrics_summary_state: resolved.quality_metrics_summary_state,
                    quality_metrics_summary: resolved.quality_metrics_summary,
                    quality_history_context: resolved.quality_history_context,
                }
            }
            crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::Batch => {
                resolve_batch_quality_runtime_context_from_persisted_sources(
                    self.latest_quality_metrics(),
                    self.quality_metrics_history(),
                    self.quality_metrics_summary_state(),
                    self.quality_metrics_summary(),
                )
            }
        }
    }

    pub(crate) fn restored_resume_compat_options(
        &self,
        restored_quality_context: &BatchGenerationQualityRuntimeContext,
    ) -> SingleChapterGenerationCompatOptions {
        restore_story_repair_compat_options_from_active_snapshot(
            &self.request_runtime_state.compat_options,
            self.explicit_story_repair_payload(),
            restored_quality_context.quality_metrics_summary.as_ref(),
            restored_quality_context.latest_quality_metrics.as_ref(),
        )
    }

    pub(crate) fn resolved_resume_active_story_repair_payload(
        &self,
        request_active_story_repair_payload: Option<&Value>,
        restored_quality_context: &BatchGenerationQualityRuntimeContext,
        scope: &str,
    ) -> Option<Value> {
        resolve_resumed_active_story_repair_payload(
            self.explicit_story_repair_payload(),
            restored_quality_context.quality_metrics_summary.as_ref(),
            restored_quality_context.latest_quality_metrics.as_ref(),
            request_active_story_repair_payload,
            scope,
            "recent_history_summary",
            "Recent history summary",
        )
    }

    pub(crate) fn resume_quality_status_context(
        &self,
        restored_quality_context: &BatchGenerationQualityRuntimeContext,
    ) -> BatchGenerationQualityStatusContext {
        BatchGenerationQualityStatusContext::from_runtime_quality_context_and_active_payload(
            restored_quality_context,
            self.explicit_story_repair_payload(),
        )
    }

    pub(crate) fn build_restored_resume_runtime_state(
        &self,
        task_kind: crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind,
        batch_id: &str,
        max_retries: i32,
    ) -> RestoredResumeRuntimeStateProjection {
        let restored_quality_context = self.restored_quality_runtime_context(task_kind);
        let restored_compat_options =
            self.restored_resume_compat_options(&restored_quality_context);
        let runtime_scope = match task_kind {
            crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::SingleChapter => "chapter",
            crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::Batch => "batch",
        };
        let restored_request_runtime_state = BatchGenerationRequestRuntimeState::new(
            restored_compat_options,
            self.request_runtime_state.model_override.clone(),
        );
        let request_active_story_repair_payload =
            restored_request_runtime_state.active_story_repair_payload_with_scope(runtime_scope);
        let active_story_repair_payload = self.resolved_resume_active_story_repair_payload(
            request_active_story_repair_payload.as_ref(),
            &restored_quality_context,
            runtime_scope,
        );
        let quality_status_context = self.resume_quality_status_context(&restored_quality_context);
        let runtime_state_seed = build_resume_runtime_state_seed(
            task_kind,
            batch_id,
            max_retries,
            active_story_repair_payload,
            restored_quality_context,
        );

        RestoredResumeRuntimeStateProjection {
            quality_status_context,
            request_runtime_state: restored_request_runtime_state,
            runtime_state_seed,
        }
    }

    pub(crate) fn restored_batch_runtime_compat_options(
        &self,
        base_compat_options: &SingleChapterGenerationCompatOptions,
    ) -> SingleChapterGenerationCompatOptions {
        if !self.has_workflow_runtime_state() {
            return base_compat_options.clone();
        }

        let restored_quality_context = self.restored_quality_runtime_context(
            crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::Batch,
        );

        restore_story_repair_compat_options_from_active_snapshot(
            base_compat_options,
            self.explicit_story_repair_payload(),
            restored_quality_context.quality_metrics_summary.as_ref(),
            restored_quality_context.latest_quality_metrics.as_ref(),
        )
    }

    pub(crate) fn build_refreshed_runtime_state_preserving_quality(
        &self,
        refreshed_quality_summary: Option<&Value>,
    ) -> Option<Value> {
        self.has_workflow_runtime_state().then(|| {
            build_batch_generation_runtime_state_payload_preserving_quality_state(
                self.request_runtime_state(),
                self.explicit_story_repair_payload(),
                self.quality_metrics_summary_state(),
                self.quality_metrics_history(),
                self.quality_metrics_summary(),
                refreshed_quality_summary,
                self.latest_quality_metrics(),
            )
        })
    }

    pub(crate) fn build_current_chapter_quality_runtime_snapshot(
        &self,
        quality_summary: &Value,
        latest_quality_metrics: Option<&Value>,
    ) -> Value {
        build_batch_generation_runtime_state_payload_from_current_quality(
            self.request_runtime_state(),
            self.explicit_story_repair_payload(),
            self.quality_metrics_summary_state(),
            self.quality_metrics_history(),
            quality_summary,
            latest_quality_metrics,
        )
    }
}

fn restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
    base_compat_options: &SingleChapterGenerationCompatOptions,
    persisted_runtime_context: &BatchGenerationPersistedRuntimeContext,
) -> SingleChapterGenerationCompatOptions {
    persisted_runtime_context.restored_batch_runtime_compat_options(base_compat_options)
}

fn build_resume_runtime_state_seed(
    task_kind: crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind,
    batch_id: &str,
    max_retries: i32,
    active_story_repair_payload: Option<Value>,
    restored_quality_context: BatchGenerationQualityRuntimeContext,
) -> Option<Value> {
    let mut runtime_state = serde_json::Map::from_iter([
        (
            "resume_from_batch_id".to_string(),
            json!(batch_id.to_string()),
        ),
        ("current_retry_count".to_string(), json!(0)),
        ("max_retries".to_string(), json!(max_retries)),
    ]);

    if let Some(payload) = active_story_repair_payload {
        runtime_state.insert("active_story_repair_payload".to_string(), payload);
    }
    match task_kind {
        crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::SingleChapter => {
            let quality_runtime_context =
                crate::services::chapter_generation_quality_runtime_context_service::GenerationQualityRuntimeContext {
                    latest_quality_metrics: restored_quality_context.latest_quality_metrics.clone(),
                    quality_metrics_history: restored_quality_context.quality_metrics_history.clone(),
                    quality_metrics_summary_state: restored_quality_context
                        .quality_metrics_summary_state
                        .clone(),
                    quality_metrics_summary: restored_quality_context
                        .quality_metrics_summary
                        .clone(),
                    quality_history_context: restored_quality_context
                        .quality_history_context
                        .clone(),
                };
            apply_generation_quality_runtime_context_to_payload(
                &mut runtime_state,
                quality_runtime_context,
                None,
                None,
                None,
            );
        }
        crate::services::chapter_batch_generation_status_semantics_service::BatchGenerationTaskKind::Batch => {
            apply_batch_quality_runtime_context_to_payload(
                &mut runtime_state,
                restored_quality_context,
                None,
            );
        }
    }

    Some(Value::Object(runtime_state))
}

#[derive(Debug, Clone, PartialEq)]
struct BatchGenerationAttemptInputPlan {
    provider_payload:
        crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload,
    prompt_overrides:
        crate::services::chapter_generation_prompt_service::ChapterGenerationPromptOverrides,
}

impl BatchGenerationAttemptInputPlan {
    async fn resolve_compat_options(
        db: &DatabaseConnection,
        task_id: &str,
        base_compat_options: &SingleChapterGenerationCompatOptions,
    ) -> SingleChapterGenerationCompatOptions {
        let persisted_runtime_context = load_chapter_generation_snapshot(db, task_id)
            .await
            .ok()
            .map(BatchGenerationPersistedRuntimeContext::from_snapshot)
            .unwrap_or_default();

        restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
            base_compat_options,
            &persisted_runtime_context,
        )
    }

    async fn prepare(
        db: &DatabaseConnection,
        task_id: &str,
        session: &BatchGenerationRuntimeSession,
        chapter_model: &chapter::Model,
    ) -> Result<Self, String> {
        let resolved_compat_options =
            Self::resolve_compat_options(db, task_id, &session.compat_options).await;
        let prompt_overrides = build_prompt_overrides_from_compat_options(&resolved_compat_options);
        let provider_payload = build_single_chapter_research_provider_payload(
            db,
            &session.user_id,
            &SingleChapterGenerationTarget {
                project_id: chapter_model.project_id.clone(),
                chapter_id: chapter_model.id.clone(),
                chapter_number: chapter_model.chapter_number,
                title: chapter_model.title.clone(),
            },
            &resolved_compat_options,
        )
        .await?;

        Ok(Self {
            provider_payload,
            prompt_overrides,
        })
    }

    async fn execute(
        db: &DatabaseConnection,
        task_id: &str,
        session: &BatchGenerationRuntimeSession,
        chapter_model: &chapter::Model,
    ) -> Result<GeneratedChapterResult, String> {
        let attempt_input = Self::prepare(db, task_id, session, chapter_model).await?;
        let Self {
            provider_payload,
            prompt_overrides,
        } = attempt_input;

        generate_and_persist_chapter_content_with_provider_payload(
            db,
            &session.ai_service,
            &session.user_id,
            &chapter_model.id,
            session.target_word_count,
            provider_payload,
            &prompt_overrides,
        )
        .await
    }

    #[cfg(test)]
    fn from_sources(
        provider_payload: crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload,
        prompt_overrides: crate::services::chapter_generation_prompt_service::ChapterGenerationPromptOverrides,
    ) -> Self {
        Self {
            provider_payload,
            prompt_overrides,
        }
    }
}

fn normalized_quality_guidance_items(value: Option<&Value>, limit: usize) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
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

fn build_batch_generation_runtime_state_payload_preserving_quality_state(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<&Value>,
    existing_quality_metrics_summary_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    existing_quality_summary: Option<&Value>,
    refreshed_quality_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Value {
    let resolved_quality_context =
        resolve_batch_quality_runtime_context_preserving_existing_quality_state(
            existing_quality_metrics_summary_state,
            existing_quality_metrics_history,
            existing_quality_summary,
            refreshed_quality_summary,
            latest_quality_metrics,
        );
    let resolved_quality_summary = resolved_quality_context
        .quality_metrics_summary
        .clone()
        .unwrap_or(Value::Null);
    let mut payload = serde_json::Map::from_iter([(
        "batch_request_runtime_state".to_string(),
        json!(request_runtime_state),
    )]);

    let active_story_repair_payload = resolve_active_story_repair_payload_with_quality_fallback(
        explicit_story_repair_payload,
        Some(&resolved_quality_summary),
        latest_quality_metrics,
        "batch",
        "current_quality_state_refresh",
        "Current quality state refresh",
    );

    if let Some(active_story_repair_payload) = active_story_repair_payload {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }
    apply_batch_quality_runtime_context_to_payload(
        &mut payload,
        resolved_quality_context,
        Some(resolved_quality_summary.clone()),
    );

    Value::Object(payload)
}

fn build_batch_generation_runtime_state_payload_from_current_quality(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<&Value>,
    existing_quality_metrics_summary_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    quality_summary: &Value,
    latest_quality_metrics: Option<&Value>,
) -> Value {
    let resolved_quality_context = resolve_batch_quality_runtime_context_from_current_quality(
        existing_quality_metrics_summary_state,
        existing_quality_metrics_history,
        quality_summary,
        latest_quality_metrics,
    );
    let resolved_quality_summary = resolved_quality_context
        .quality_metrics_summary
        .clone()
        .unwrap_or_else(|| quality_summary.clone());
    let mut payload = serde_json::Map::from_iter([(
        "batch_request_runtime_state".to_string(),
        json!(request_runtime_state),
    )]);

    let active_story_repair_payload = resolve_active_story_repair_payload_with_quality_fallback(
        explicit_story_repair_payload,
        Some(&resolved_quality_summary),
        latest_quality_metrics,
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
    apply_batch_quality_runtime_context_to_payload(
        &mut payload,
        resolved_quality_context,
        Some(resolved_quality_summary.clone()),
    );

    Value::Object(payload)
}

async fn persist_batch_generation_runtime_plan(
    db: &DatabaseConnection,
    task_id: &str,
    persistence_plan: BatchGenerationRuntimePersistencePlan,
) {
    let _ = persistence_plan.persist(db, task_id).await;
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedBatchGenerationStepExecution {
    chapter_model: chapter::Model,
    retry_count: i32,
    max_retries: i32,
}

impl PreparedBatchGenerationStepExecution {
    async fn start(
        db: &DatabaseConnection,
        task_id: &str,
        session: &BatchGenerationRuntimeSession,
        chapter_id: &str,
        progress: &BatchGenerationStepProgress,
    ) -> BatchGenerationRuntimeDriverProgression {
        let mut preparation_retry_count = None;

        loop {
            let prepared_step = match Self::prepare(db, task_id, chapter_id, progress).await {
                Ok(prepared_step) => prepared_step,
                Err(BatchGenerationAttemptProgression::Retry(next_retry_count)) => {
                    preparation_retry_count = Some(next_retry_count);
                    continue;
                }
                Err(BatchGenerationAttemptProgression::Driver(driver_progression)) => {
                    return driver_progression;
                }
            };

            let prepared_step = if let Some(retry_count) = preparation_retry_count.take() {
                Self {
                    retry_count,
                    ..prepared_step
                }
            } else {
                prepared_step
            };

            return prepared_step.execute(db, task_id, session, progress).await;
        }
    }

    async fn prepare(
        db: &DatabaseConnection,
        task_id: &str,
        chapter_id: &str,
        progress: &BatchGenerationStepProgress,
    ) -> Result<Self, BatchGenerationAttemptProgression> {
        let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .ok()
            .flatten()
        else {
            return Err(BatchGenerationAttemptProgression::Driver(
                BatchGenerationRuntimeDriverProgression::Stop,
            ));
        };
        if task_model.status == "cancelled" {
            persist_batch_generation_runtime_plan(
                db,
                task_id,
                BatchGenerationRuntimePersistencePlan::cancelled(
                    progress.completed,
                    progress.total_chapters,
                ),
            )
            .await;
            return Err(BatchGenerationAttemptProgression::Driver(
                BatchGenerationRuntimeDriverProgression::Stop,
            ));
        }

        let chapter_model = match chapter::Entity::find_by_id(chapter_id).one(db).await {
            Ok(Some(chapter_model)) => chapter_model,
            Ok(None) => {
                return Err(BatchGenerationGenericFailureRoutingPlan::from_step_context(
                    chapter_id,
                    None,
                    None,
                    progress,
                    task_model.current_retry_count,
                    task_model.max_retries.max(0),
                    BatchGenerationFailureKind::MissingChapter,
                    &format!("章节 {} 不存在", chapter_id),
                )
                .persist_and_resolve(db, task_id)
                .await);
            }
            Err(error) => {
                return Err(BatchGenerationGenericFailureRoutingPlan::from_step_context(
                    chapter_id,
                    None,
                    None,
                    progress,
                    task_model.current_retry_count,
                    task_model.max_retries.max(0),
                    BatchGenerationFailureKind::LoadChapterError,
                    &format!("加载章节失败: {}", error),
                )
                .persist_and_resolve(db, task_id)
                .await);
            }
        };
        if chapter_model.project_id != task_model.project_id {
            return Err(BatchGenerationGenericFailureRoutingPlan::from_step_context(
                chapter_id,
                None,
                None,
                progress,
                task_model.current_retry_count,
                task_model.max_retries.max(0),
                BatchGenerationFailureKind::GenerationError,
                &format!("章节 {} 项目不匹配", chapter_id),
            )
            .persist_and_resolve(db, task_id)
            .await);
        }

        Ok(Self {
            chapter_model,
            retry_count: task_model.current_retry_count.max(0),
            max_retries: task_model.max_retries.max(0),
        })
    }

    async fn execute(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        session: &BatchGenerationRuntimeSession,
        progress: &BatchGenerationStepProgress,
    ) -> BatchGenerationRuntimeDriverProgression {
        let Self {
            chapter_model,
            retry_count: initial_retry_count,
            max_retries,
        } = self;
        let mut retry_count = initial_retry_count;

        loop {
            let _ = BatchGenerationRuntimePersistencePlan::chapter_started(
                &chapter_model,
                progress.completed,
                progress.total_chapters,
                retry_count,
            )
            .persist(db, task_id)
            .await;

            let prerequisite =
                match check_chapter_generation_prerequisites(db, &chapter_model).await {
                    Ok(prerequisite) => prerequisite,
                    Err(error) => {
                        match BatchGenerationGenericFailureRoutingPlan::from_step_error(
                            &chapter_model,
                            progress,
                            retry_count,
                            max_retries,
                            BatchGenerationFailureKind::GenerationError,
                            &error,
                        )
                        .persist_and_resolve(db, task_id)
                        .await
                        {
                            BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                                retry_count = next_retry_count;
                                continue;
                            }
                            BatchGenerationAttemptProgression::Driver(driver_progression) => {
                                return driver_progression;
                            }
                        }
                    }
                };
            if !prerequisite.can_generate {
                match BatchGenerationGenericFailureRoutingPlan::from_step_error(
                    &chapter_model,
                    progress,
                    retry_count,
                    max_retries,
                    BatchGenerationFailureKind::GenerationError,
                    &format!("章节生成失败: {}", prerequisite.error_message),
                )
                .persist_and_resolve(db, task_id)
                .await
                {
                    BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                        retry_count = next_retry_count;
                        continue;
                    }
                    BatchGenerationAttemptProgression::Driver(driver_progression) => {
                        return driver_progression;
                    }
                }
            }

            match BatchGenerationAttemptInputPlan::execute(db, task_id, session, &chapter_model)
                .await
            {
                Ok(generated_result) => {
                    match BatchGenerationPostWriteGuardPlan::for_chapter(&chapter_model.id)
                        .execute(db, task_id)
                        .await
                    {
                        Ok(BatchGenerationPostWriteGuardOutcome::Continue) => {}
                        Ok(BatchGenerationPostWriteGuardOutcome::Stop) => {
                            return BatchGenerationRuntimeDriverProgression::Stop;
                        }
                        Err(error) => {
                            match BatchGenerationGenericFailureRoutingPlan::from_step_error(
                                &chapter_model,
                                progress,
                                retry_count,
                                max_retries,
                                BatchGenerationFailureKind::GenerationError,
                                &error,
                            )
                            .persist_and_resolve(db, task_id)
                            .await
                            {
                                BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                                    retry_count = next_retry_count;
                                    continue;
                                }
                                BatchGenerationAttemptProgression::Driver(driver_progression) => {
                                    return driver_progression;
                                }
                            }
                        }
                    }

                    match BatchGenerationFollowUpAnalysisPlan::from_generated_result(
                        &generated_result,
                    )
                    .execute(db, task_id, session)
                    .await
                    {
                        Ok(current_quality_runtime_state) => {
                            match BatchGenerationPostAnalysisTerminalPlan::on_success(
                                &chapter_model,
                                progress,
                                current_quality_runtime_state,
                            )
                            .execute(db, task_id)
                            .await
                            {
                                BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                                    retry_count = next_retry_count;
                                    continue;
                                }
                                BatchGenerationAttemptProgression::Driver(driver_progression) => {
                                    return driver_progression;
                                }
                            }
                        }
                        Err(analysis_error) => {
                            match BatchGenerationPostAnalysisTerminalPlan::on_failure(
                                &chapter_model,
                                progress,
                                analysis_error,
                            )
                            .execute(db, task_id)
                            .await
                            {
                                BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                                    retry_count = next_retry_count;
                                    continue;
                                }
                                BatchGenerationAttemptProgression::Driver(driver_progression) => {
                                    return driver_progression;
                                }
                            }
                        }
                    }
                }
                Err(task_error_message) => {
                    match BatchGenerationGenericFailureRoutingPlan::from_step_error(
                        &chapter_model,
                        progress,
                        retry_count,
                        max_retries,
                        BatchGenerationFailureKind::GenerationError,
                        &task_error_message,
                    )
                    .persist_and_resolve(db, task_id)
                    .await
                    {
                        BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                            retry_count = next_retry_count;
                        }
                        BatchGenerationAttemptProgression::Driver(driver_progression) => {
                            return driver_progression;
                        }
                    }
                }
            }
        }
    }

    #[cfg(test)]
    fn from_task_and_chapter(
        task_model: &batch_generation_task::Model,
        chapter_model: &chapter::Model,
    ) -> Self {
        Self {
            chapter_model: chapter_model.clone(),
            retry_count: task_model.current_retry_count.max(0),
            max_retries: task_model.max_retries.max(0),
        }
    }
}

struct BatchGenerationRuntimeLifecyclePlan {
    session: BatchGenerationRuntimeSession,
    chapter_ids: Vec<String>,
}

impl BatchGenerationRuntimeLifecyclePlan {
    async fn start(
        db: &DatabaseConnection,
        task_id: &str,
        execution_input: BatchGenerationExecutionInput,
    ) {
        Self::from_execution_input(execution_input)
            .execute(db, task_id)
            .await;
    }

    fn from_execution_input(execution_input: BatchGenerationExecutionInput) -> Self {
        let (session, chapter_ids) =
            BatchGenerationRuntimeSession::from_execution_input(execution_input);

        Self {
            session,
            chapter_ids,
        }
    }

    async fn execute(self, db: &DatabaseConnection, task_id: &str) {
        let _ = BatchGenerationRuntimePersistencePlan::preparing(self.session.total_chapters)
            .persist(db, task_id)
            .await;
        let mut progress = BatchGenerationStepProgress::new(0, self.session.total_chapters);

        for chapter_id in &self.chapter_ids {
            match PreparedBatchGenerationStepExecution::start(
                db,
                task_id,
                &self.session,
                chapter_id,
                &progress,
            )
            .await
            {
                BatchGenerationRuntimeDriverProgression::Continue(next_progress) => {
                    progress = next_progress;
                }
                BatchGenerationRuntimeDriverProgression::Stop => {
                    return;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BatchGenerationRetryProgressionPlan {
    next_retry_count: i32,
}

impl BatchGenerationRetryProgressionPlan {
    fn new(next_retry_count: i32) -> Self {
        Self { next_retry_count }
    }

    async fn execute(self) -> BatchGenerationAttemptProgression {
        sleep(Duration::from_secs(batch_generation_retry_backoff_seconds(
            self.next_retry_count,
        )))
        .await;
        BatchGenerationAttemptProgression::Retry(self.next_retry_count)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationPostWriteGuardPlan {
    chapter_id: String,
}

impl BatchGenerationPostWriteGuardPlan {
    fn for_chapter(chapter_id: &str) -> Self {
        Self {
            chapter_id: chapter_id.to_string(),
        }
    }

    async fn execute(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> Result<BatchGenerationPostWriteGuardOutcome, String> {
        let task_exists = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
            .is_some();
        if !task_exists {
            return Ok(Self::resolve(false, true));
        }

        let chapter_exists = chapter::Entity::find_by_id(&self.chapter_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
            .is_some();
        Ok(Self::resolve(true, chapter_exists))
    }

    fn resolve(task_exists: bool, chapter_exists: bool) -> BatchGenerationPostWriteGuardOutcome {
        if task_exists && chapter_exists {
            BatchGenerationPostWriteGuardOutcome::Continue
        } else {
            BatchGenerationPostWriteGuardOutcome::Stop
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum BatchGenerationPostAnalysisTerminalOutcome {
    Success {
        current_quality_runtime_state: Option<Value>,
    },
    Failure {
        analysis_error: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct BatchGenerationPostAnalysisTerminalPlan {
    chapter_model: chapter::Model,
    progress: BatchGenerationStepProgress,
    outcome: BatchGenerationPostAnalysisTerminalOutcome,
}

impl BatchGenerationPostAnalysisTerminalPlan {
    fn on_success(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        current_quality_runtime_state: Option<Value>,
    ) -> Self {
        Self {
            chapter_model: chapter_model.clone(),
            progress: progress.clone(),
            outcome: BatchGenerationPostAnalysisTerminalOutcome::Success {
                current_quality_runtime_state,
            },
        }
    }

    fn on_failure(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        analysis_error: String,
    ) -> Self {
        Self {
            chapter_model: chapter_model.clone(),
            progress: progress.clone(),
            outcome: BatchGenerationPostAnalysisTerminalOutcome::Failure { analysis_error },
        }
    }

    async fn execute(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> BatchGenerationAttemptProgression {
        let Self {
            chapter_model,
            progress,
            outcome,
        } = self;

        match outcome {
            BatchGenerationPostAnalysisTerminalOutcome::Success {
                current_quality_runtime_state,
            } => {
                Self {
                    chapter_model,
                    progress,
                    outcome: BatchGenerationPostAnalysisTerminalOutcome::Success {
                        current_quality_runtime_state: None,
                    },
                }
                .resolve_analysis_success_outcome(db, task_id, current_quality_runtime_state)
                .await
            }
            BatchGenerationPostAnalysisTerminalOutcome::Failure { analysis_error } => {
                Self {
                    chapter_model,
                    progress,
                    outcome: BatchGenerationPostAnalysisTerminalOutcome::Failure {
                        analysis_error: String::new(),
                    },
                }
                .fail_after_analysis(db, task_id, analysis_error)
                .await
            }
        }
    }

    async fn resolve_analysis_success_outcome(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        current_quality_runtime_state: Option<Value>,
    ) -> BatchGenerationAttemptProgression {
        if let Some(quality_gate_outcome) = self
            .resolve_quality_gate_outcome(db, task_id, current_quality_runtime_state.as_ref())
            .await
        {
            return quality_gate_outcome;
        }

        let next_progress = self.progress.advance();
        BatchGenerationAttemptProgression::Driver(
            self.persist_post_generation_success(db, task_id, next_progress)
                .await,
        )
    }

    async fn resolve_quality_gate_outcome(
        &self,
        db: &DatabaseConnection,
        task_id: &str,
        current_quality_runtime_state: Option<&Value>,
    ) -> Option<BatchGenerationAttemptProgression> {
        let (snapshot, current_retry_count, max_retries) =
            Self::load_quality_gate_retry_budget_context(db, task_id).await;
        let persisted_workflow_runtime_state = snapshot
            .as_ref()
            .and_then(|item| item.workflow_runtime_state.as_ref());
        let workflow_runtime_state =
            current_quality_runtime_state.or(persisted_workflow_runtime_state);
        let Some(terminal_semantics) = resolve_batch_generation_quality_gate_terminal_semantics(
            snapshot.as_ref(),
            workflow_runtime_state,
            current_retry_count,
            max_retries,
        ) else {
            return None;
        };

        let routing_plan = BatchGenerationQualityGateRoutingPlan::from_terminal_semantics(
            &self.chapter_model,
            &self.progress,
            workflow_runtime_state,
            current_retry_count,
            max_retries,
            terminal_semantics,
        )?;

        Some(routing_plan.persist_and_resolve(db, task_id).await)
    }

    async fn load_quality_gate_retry_budget_context(
        db: &DatabaseConnection,
        task_id: &str,
    ) -> (Option<batch_generation_snapshot::Model>, i32, i32) {
        let snapshot = load_chapter_generation_snapshot(db, task_id)
            .await
            .ok()
            .flatten();
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

        (snapshot, current_retry_count, max_retries)
    }

    async fn fail_after_analysis(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        analysis_error: String,
    ) -> BatchGenerationAttemptProgression {
        let _ = upsert_batch_generation_runtime_snapshot(
            db,
            task_id,
            json!({
                "last_event": "analysis_failed",
                "last_message": format!("第 {} 章分析失败，批量任务终止", self.chapter_model.chapter_number),
                "progress": 100,
                "phase": "failed",
                "analysis_task_message": format!("第 {} 章分析失败，批量任务终止", self.chapter_model.chapter_number),
                "analysis_task_progress": 100,
                "analysis_last_error": analysis_error,
                "analysis_retry_count": 3,
                "analysis_max_retries": 3,
            }),
        )
        .await;

        persist_batch_generation_runtime_plan(
            db,
            task_id,
            BatchGenerationRuntimePersistencePlan::failed(
                Some(&self.chapter_model.id),
                Some(self.chapter_model.chapter_number),
                Some(&self.chapter_model.title),
                self.progress.completed,
                self.progress.total_chapters,
                BatchGenerationFailureKind::GenerationError,
                3,
                format!("章节分析失败，已重试3次: {}", analysis_error),
                format!(
                    "第{}章分析失败，已重试3次: {}",
                    self.chapter_model.chapter_number, analysis_error
                ),
            ),
        )
        .await;

        BatchGenerationAttemptProgression::Driver(BatchGenerationRuntimeDriverProgression::Stop)
    }

    async fn persist_post_generation_success(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        next_progress: BatchGenerationStepProgress,
    ) -> BatchGenerationRuntimeDriverProgression {
        self.refresh_runtime_story_repair_state(db, task_id).await;
        persist_batch_generation_runtime_plan(
            db,
            task_id,
            BatchGenerationRuntimePersistencePlan::chapter_succeeded(
                &self.chapter_model,
                next_progress.completed,
                next_progress.total_chapters,
            ),
        )
        .await;

        BatchGenerationRuntimeDriverProgression::Continue(next_progress)
    }

    async fn refresh_runtime_story_repair_state(&self, db: &DatabaseConnection, task_id: &str) {
        let persisted_runtime_context = load_chapter_generation_snapshot(db, task_id)
            .await
            .ok()
            .map(BatchGenerationPersistedRuntimeContext::from_snapshot)
            .unwrap_or_default();
        if !persisted_runtime_context.has_workflow_runtime_state() {
            return;
        }

        let quality_summary = load_recent_batch_story_repair_quality_summary(
            db,
            &self.chapter_model.project_id,
            self.chapter_model.chapter_number + 1,
        )
        .await
        .ok()
        .flatten();
        let Some(refreshed_runtime_state) = persisted_runtime_context
            .build_refreshed_runtime_state_preserving_quality(quality_summary.as_ref())
        else {
            return;
        };

        let _ =
            upsert_batch_generation_runtime_snapshot(db, task_id, refreshed_runtime_state).await;
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BatchGenerationFollowUpAnalysisPlan {
    generated_result: GeneratedChapterResult,
}

impl BatchGenerationFollowUpAnalysisPlan {
    fn from_generated_result(generated_result: &GeneratedChapterResult) -> Self {
        Self {
            generated_result: generated_result.clone(),
        }
    }

    async fn execute(
        self,
        db: &DatabaseConnection,
        batch_task_id: &str,
        session: &BatchGenerationRuntimeSession,
    ) -> Result<Option<Value>, String> {
        if !session.compat_options.enable_analysis() {
            return Ok(None);
        }

        for analysis_retry_count in 0..3 {
            match BatchGenerationAnalysisAttemptPlan::from_generated_result(
                &self.generated_result,
                analysis_retry_count,
            )
            .execute(db, batch_task_id, session)
            .await?
            {
                BatchGenerationAnalysisAttemptResolution::Completed(current_quality_snapshot) => {
                    return Ok(current_quality_snapshot);
                }
                BatchGenerationAnalysisAttemptResolution::Retry => {
                    continue;
                }
            }
        }

        Err("章节分析失败".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchGenerationRuntimeDriverProgression {
    Continue(BatchGenerationStepProgress),
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchGenerationAttemptProgression {
    Retry(i32),
    Driver(BatchGenerationRuntimeDriverProgression),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchGenerationPostWriteGuardOutcome {
    Continue,
    Stop,
}

fn build_batch_generation_analysis_completed_snapshot(
    generated: &GeneratedChapterResult,
    analysis_retry_count: i32,
) -> serde_json::Value {
    json!({
        "last_event": "analysis_completed",
        "last_message": format!("第 {} 章分析完成", generated.chapter_number),
        "progress": 100,
        "analysis_task_message": format!("第 {} 章分析完成", generated.chapter_number),
        "analysis_task_progress": 100,
        "analysis_last_error": Value::Null,
        "analysis_retry_count": analysis_retry_count,
        "analysis_max_retries": 3,
    })
}

fn build_batch_generation_analysis_started_snapshot(
    analysis_task_id: Option<&str>,
    generated: &GeneratedChapterResult,
    analysis_retry_count: i32,
) -> serde_json::Value {
    json!({
        "last_event": "analysis_started",
        "last_message": "正在分析章节",
        "progress": 85,
        "phase": "parsing",
        "analysis_task_id": analysis_task_id,
        "analysis_task_message": format!("第 {} 章分析任务已启动", generated.chapter_number),
        "analysis_task_progress": 85,
        "analysis_started_chapter_id": generated.chapter_id,
        "analysis_started_chapter_number": generated.chapter_number,
        "analysis_started_at": chrono::Utc::now().to_rfc3339(),
        "analysis_retry_count": analysis_retry_count,
        "analysis_max_retries": 3,
    })
}

fn apply_manual_review_terminal_fields(
    object: &mut serde_json::Map<String, Value>,
    manual_review_label: &str,
) {
    shared_apply_manual_review_terminal_fields(object, manual_review_label);
}

fn resolve_batch_generation_quality_gate_terminal_semantics(
    snapshot: Option<&crate::models::batch_generation_snapshot::Model>,
    workflow_runtime_state: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<BatchGenerationFailedTerminalSemantics> {
    let quality_status_context =
        BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            snapshot,
            workflow_runtime_state,
        );
    resolve_failed_terminal_semantics_from_sources(
        Some(&json!([])),
        Some(&quality_status_context),
        current_retry_count,
        max_retries,
    )
}

fn format_analysis_error_message(
    error: &crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError,
) -> String {
    match error {
        crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::ChapterEmpty => {
            "章节不存在或内容为空".to_string()
        }
        crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::ProjectMissing => {
            "章节或项目已删除，无法继续分析".to_string()
        }
        crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::Internal(message) => {
            message.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        batch_generation_retry_backoff_seconds, build_batch_generation_execution_input,
        build_batch_generation_resume_runtime_checkpoint,
        build_batch_generation_runtime_launch_input_from_runtime_state_seed,
        dispatch_batch_generation_runtime,
        restore_batch_generation_runtime_compat_options_from_persisted_runtime_context,
        should_retry_batch_generation_attempt, BatchGenerationAnalysisAttemptPlan,
        BatchGenerationAttemptInputPlan, BatchGenerationAttemptProgression,
        BatchGenerationExecutionInput, BatchGenerationFollowUpAnalysisPlan,
        BatchGenerationPersistedRuntimeContext, BatchGenerationPostAnalysisTerminalOutcome,
        BatchGenerationPostAnalysisTerminalPlan, BatchGenerationPostWriteGuardOutcome,
        BatchGenerationPostWriteGuardPlan, BatchGenerationRetryProgressionPlan,
        BatchGenerationRuntimeDriverProgression, BatchGenerationRuntimeLifecyclePlan,
        BatchGenerationRuntimeSession, BatchGenerationStepProgress, BatchGenerationTaskStage,
        ModelFieldUpdate, PreparedBatchGenerationStepExecution, TaskTimestampUpdate,
    };
    use crate::ai::AIConfig;
    use crate::models::{batch_generation_snapshot, batch_generation_task, chapter};
    use crate::services::chapter_batch_generation_quality_status_service::{
        BatchGenerationFailedTerminalKind, BatchGenerationFailedTerminalSemantics,
    };
    use crate::services::chapter_batch_generation_resume_semantics_service::{
        ResumeBatchGenerationCommandState, ResumeResetSemantics,
    };
    use crate::services::chapter_batch_generation_runtime_checkpoint_service::{
        build_batch_generation_runtime_checkpoint_for_stage, BatchGenerationFailureKind,
        BatchGenerationSnapshotStage,
    };
    use crate::services::chapter_batch_generation_write_workflow_service::build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload;
    use crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_quality_runtime_context_service::{
        normalize_terminal_quality_history as shared_normalize_terminal_quality_history,
        normalize_terminal_quality_history_context as shared_normalize_terminal_quality_history_context,
    };
    use crate::services::chapter_generation_request_runtime_state_service::BatchGenerationRequestRuntimeState;
    use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
    use crate::services::chapter_generation_terminal_runtime_patch_service::{
        apply_terminal_quality_runtime_patch_contract as shared_apply_terminal_quality_runtime_patch_contract,
        build_manual_review_terminal_runtime_patch_contract as shared_build_manual_review_terminal_runtime_patch_contract,
        build_quality_gate_blocked_runtime_state_patch_from_workflow_state as shared_build_quality_gate_blocked_runtime_state_patch_from_workflow_state,
        build_retry_quality_runtime_patch_contract_from_workflow_state as shared_build_retry_quality_runtime_patch_contract_from_workflow_state,
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

    fn build_snapshot(
        latest_quality_metrics: Option<Value>,
        quality_metrics_summary: Option<Value>,
        workflow_runtime_state: Option<Value>,
    ) -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics,
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary,
            workflow_runtime_state,
            created_at: None,
            updated_at: None,
        }
    }

    fn build_snapshot_with_runtime_state(
        workflow_runtime_state: Value,
        quality_metrics_history: Option<Value>,
    ) -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: None,
            quality_metrics_history,
            quality_metrics_summary: None,
            workflow_runtime_state: Some(workflow_runtime_state),
            created_at: None,
            updated_at: None,
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
        assert!(matches!(
            preparing.started_at_update(),
            TaskTimestampUpdate::Now
        ));
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

        let terminal_semantics = BatchGenerationFailedTerminalSemantics {
            kind: BatchGenerationFailedTerminalKind::Retry,
            reason: "retry",
            label: "自动修复后重试".to_string(),
            review_required: false,
            can_resume: true,
        };
        let plan = super::BatchGenerationRetryPersistencePlan::new(
            &chapter_model,
            &progress,
            2,
            3,
            "provider timeout",
            super::BatchGenerationRetryPersistenceContract::QualityGate { terminal_semantics },
        );
        let snapshot = plan.build_waiting_snapshot();

        assert_eq!(snapshot["phase"], "repair_pending");
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
        assert_eq!(snapshot["terminal_reason"], "retry");
        assert_eq!(snapshot["terminal_label"], "自动修复后重试");
        assert_eq!(snapshot["review_required"], false);
        assert_eq!(snapshot["can_resume"], true);
        assert_eq!(snapshot["quality_gate_decision"], "auto_repair");
        assert_eq!(snapshot["quality_gate_label"], "自动修复后重试");
        assert_eq!(snapshot["phase"], "repair_pending");
    }

    #[test]
    fn should_build_generic_batch_generation_retry_waiting_snapshot_without_quality_terminal_fields(
    ) {
        let chapter_model = chapter::Model {
            id: "chapter-8".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 8,
            title: "风雨桥".to_string(),
            content: Some("content".to_string()),
            summary: None,
            word_count: 1400,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let progress = BatchGenerationStepProgress::new(3, 9);

        let plan = super::BatchGenerationRetryPersistencePlan::new(
            &chapter_model,
            &progress,
            1,
            3,
            "provider timeout",
            super::BatchGenerationRetryPersistenceContract::Generic,
        );
        let snapshot = plan.build_waiting_snapshot();

        assert_eq!(snapshot["status"], "running");
        assert_eq!(snapshot["last_event"], "chapter_retry");
        assert_eq!(snapshot["current_retry_count"], 1);
        assert_eq!(snapshot["max_retries"], 3);
        assert_eq!(snapshot["retry_backoff_seconds"], 2);
        assert_eq!(snapshot["last_error"], "provider timeout");
        assert_eq!(
            snapshot["last_message"],
            "第 8 章生成失败，2 秒后进行第 1 次重试"
        );
        assert!(snapshot.get("terminal_reason").is_none());
        assert!(snapshot.get("terminal_label").is_none());
        assert!(snapshot.get("review_required").is_none());
        assert!(snapshot.get("can_resume").is_none());
        assert!(snapshot.get("quality_gate_decision").is_none());
        assert!(snapshot.get("quality_gate_label").is_none());
        assert_eq!(snapshot["phase"], "generating");
    }

    #[test]
    fn should_build_generic_retry_waiting_snapshot_without_chapter_number() {
        let plan = super::BatchGenerationRetryPersistencePlan::from_step_context(
            "chapter-missing",
            None,
            &BatchGenerationStepProgress::new(1, 5),
            2,
            3,
            "章节 chapter-missing 不存在",
            super::BatchGenerationRetryPersistenceContract::Generic,
        );
        let snapshot = plan.build_waiting_snapshot();

        assert_eq!(snapshot["chapter_id"], "chapter-missing");
        assert!(snapshot["current_chapter_number"].is_null());
        assert_eq!(
            snapshot["last_message"],
            "章节生成失败，4 秒后进行第 2 次重试"
        );
        assert_eq!(snapshot["last_error"], "章节 chapter-missing 不存在");
    }

    #[test]
    fn should_apply_batch_generation_retry_persistence_plan_to_active_model() {
        let chapter_model = chapter::Model {
            id: "chapter-9".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 9,
            title: "雾中灯".to_string(),
            content: Some("content".to_string()),
            summary: None,
            word_count: 1600,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let progress = BatchGenerationStepProgress::new(4, 10);
        let plan = super::BatchGenerationRetryPersistencePlan::new(
            &chapter_model,
            &progress,
            2,
            5,
            "generation timeout",
            super::BatchGenerationRetryPersistenceContract::Generic,
        );
        let mut active: batch_generation_task::ActiveModel = build_task("failed").into();

        plan.apply_to_active_model(&mut active);

        assert_eq!(active.status, Set("running".to_string()));
        assert_eq!(active.error_message, Set(None));
        assert_eq!(
            active.current_chapter_id,
            Set(Some("chapter-9".to_string()))
        );
        assert_eq!(active.current_chapter_number, Set(Some(9)));
        assert_eq!(active.current_retry_count, Set(2));
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
        let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::from_sources(
            Some(runtime_state_payload),
            None,
            None,
            None,
        );
        let resolved =
            restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
                &base_compat_options,
                &persisted_runtime_context,
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
        let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::default();
        let resolved =
            restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
                &base_compat_options,
                &persisted_runtime_context,
            );

        assert_eq!(resolved.story_repair_summary(), "来自初始请求");
        assert_eq!(resolved.story_repair_targets(), &["初始目标".to_string()]);
        assert_eq!(
            resolved.story_preserve_strengths(),
            &["初始优势".to_string()]
        );
    }

    #[test]
    fn should_restore_batch_runtime_compat_options_from_history_only_quality_runtime_context() {
        let base_compat_options = SingleChapterGenerationCompatOptions::default();
        let runtime_state_payload = json!({
            "quality_metrics_history": [
                {
                    "overall_score": 82,
                    "repair_guidance": {
                        "summary": "沿用批量历史修复建议",
                        "repair_targets": ["压缩说明段", "提前冲突触发"],
                        "preserve_strengths": ["角色张力"]
                    }
                }
            ]
        });
        let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::from_sources(
            Some(runtime_state_payload),
            None,
            None,
            None,
        );
        let resolved =
            restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
                &base_compat_options,
                &persisted_runtime_context,
            );

        assert_eq!(resolved.story_repair_summary(), "沿用批量历史修复建议");
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
    fn should_restore_batch_runtime_compat_options_from_latest_quality_metrics_when_summary_missing(
    ) {
        let base_compat_options = SingleChapterGenerationCompatOptions::default();
        let runtime_state_payload = json!({
            "latest_quality_metrics": {
                "overall_score": 82,
                "repair_guidance": {
                    "summary": "沿用批量最新修复建议",
                    "repair_targets": ["补强节奏", "提前冲突触发"],
                    "preserve_strengths": ["角色张力"]
                }
            }
        });
        let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::from_sources(
            Some(runtime_state_payload),
            None,
            None,
            None,
        );
        let resolved =
            restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
                &base_compat_options,
                &persisted_runtime_context,
            );

        assert_eq!(resolved.story_repair_summary(), "沿用批量最新修复建议");
        assert_eq!(
            resolved.story_repair_targets(),
            &["补强节奏".to_string(), "提前冲突触发".to_string()]
        );
        assert_eq!(
            resolved.story_preserve_strengths(),
            &["角色张力".to_string()]
        );
    }

    #[test]
    fn should_build_refreshed_batch_runtime_state_with_existing_active_payload_and_recent_history()
    {
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

        let payload = build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
            &request_runtime_state,
            Some(&existing_active_payload),
            Some(&recent_history_summary),
            None,
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "运行态摘要"
        );
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
    fn should_prefer_snapshot_quality_fields_when_restoring_batch_runtime_compat_options_from_persisted_owner(
    ) {
        let base_compat_options = SingleChapterGenerationCompatOptions::default();
        let runtime_state_payload = json!({
            "quality_metrics_summary": {
                "overall_score": 71,
                "repair_guidance": {
                    "summary": "运行态摘要不应优先",
                    "repair_targets": ["运行态目标"],
                    "preserve_strengths": ["运行态优势"]
                }
            }
        });
        let snapshot_quality_summary = json!({
            "overall_score": 88,
            "repair_guidance": {
                "summary": "应优先使用快照摘要",
                "repair_targets": ["快照目标"],
                "preserve_strengths": ["快照优势"]
            }
        });

        let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::from_sources(
            Some(runtime_state_payload),
            None,
            Some(snapshot_quality_summary),
            None,
        );
        let resolved =
            restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
                &base_compat_options,
                &persisted_runtime_context,
            );

        assert_eq!(resolved.story_repair_summary(), "应优先使用快照摘要");
        assert_eq!(resolved.story_repair_targets(), &["快照目标".to_string()]);
        assert_eq!(
            resolved.story_preserve_strengths(),
            &["快照优势".to_string()]
        );
    }

    #[test]
    fn should_preserve_rust_owned_batch_quality_state_when_refreshing_story_repair_payload() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::default();
        let existing_active_payload = json!({
            "summary": "运行态摘要",
            "repair_targets": ["运行态目标"],
            "preserve_strengths": ["运行态优势"],
            "source": "current_chapter_quality",
            "source_label": "Current chapter quality",
            "scope": "batch"
        });
        let existing_history = json!([
            {
                "overall_score": 88,
                "quality_gate": {
                    "decision": "passed",
                    "label": "通过"
                }
            },
            {
                "overall_score": 84,
                "repair_guidance": {
                    "summary": "最新建议优先处理节奏",
                    "repair_targets": ["最新目标", "历史目标"],
                    "preserve_strengths": ["最新优势"],
                    "focus_areas": ["节奏", "信息密度"]
                },
                "quality_gate": {
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            }
        ]);
        let existing_summary_state = json!({
            "scope": "batch",
            "chapter_count": 2,
            "first_overall_score": 88.0,
            "last_overall_score": 84.0,
            "overall_score_total": 172.0,
            "recent_history": [
                {
                    "overall_score": 88,
                    "quality_gate": {"decision": "passed", "label": "通过"}
                },
                {
                    "overall_score": 84,
                    "repair_guidance": {"focus_areas": ["节奏", "信息密度"]},
                    "quality_gate": {"decision": "auto_repair", "label": "建议继续修复"}
                }
            ]
        });
        let existing_quality_summary = json!({
            "overall_score": 84,
            "chapter_count": 2,
            "overall_score_delta": -4.0,
            "overall_score_trend": "falling",
            "recent_focus_areas": ["节奏", "信息密度"],
            "repair_guidance": {
                "summary": "最新建议优先处理节奏",
                "repair_targets": ["最新目标", "历史目标"],
                "preserve_strengths": ["最新优势"]
            },
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议继续修复"
            },
            "quality_runtime_context": {
                "scope": "batch",
                "recent_metrics": [
                    {"history_index": 0, "overall_score": 88},
                    {"history_index": 1, "overall_score": 84}
                ]
            }
        });
        let refreshed_recent_summary = json!({
            "overall_score": 79,
            "repair_guidance": {
                "summary": "旧聚合摘要",
                "repair_targets": ["旧目标"]
            },
            "quality_gate": {
                "decision": "manual_review",
                "label": "旧聚合标签"
            },
            "quality_runtime_context": {
                "scope": "batch",
                "recent_metrics": [{"score": 79}]
            }
        });
        let latest_quality_metrics = json!({
            "overall_score": 84,
            "repair_guidance": {
                "summary": "最新建议优先处理节奏",
                "repair_targets": ["最新目标", "历史目标"],
                "preserve_strengths": ["最新优势"],
                "focus_areas": ["节奏", "信息密度"]
            },
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议继续修复"
            }
        });

        let payload = super::build_batch_generation_runtime_state_payload_preserving_quality_state(
            &request_runtime_state,
            Some(&existing_active_payload),
            Some(&existing_summary_state),
            Some(&existing_history),
            Some(&existing_quality_summary),
            Some(&refreshed_recent_summary),
            Some(&latest_quality_metrics),
        );

        assert_eq!(
            payload["quality_metrics_history"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 88);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 84.0);
        assert_eq!(
            payload["quality_metrics_summary"]["overall_score_trend"],
            "falling"
        );
        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary_state"]["last_overall_score"],
            84.0
        );
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "运行态摘要"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["运行态目标", "最新目标", "历史目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["运行态优势", "最新优势"])
        );
    }

    #[test]
    fn should_keep_persisted_batch_runtime_context_owner_contract() {
        let context = BatchGenerationPersistedRuntimeContext::from_snapshot(Some(
            build_snapshot_with_runtime_state(
                json!({
                    "batch_request_runtime_state": BatchGenerationRequestRuntimeState::new(
                        SingleChapterGenerationCompatOptions {
                            story_repair_summary: Some("来自运行时请求".to_string()),
                            story_repair_targets: vec!["补强节奏".to_string()],
                            ..SingleChapterGenerationCompatOptions::default()
                        },
                        Some("gpt-4.1".to_string())
                    ),
                    "active_story_repair_payload": {
                        "summary": "来自运行时活动修复",
                        "repair_targets": ["补强节奏"],
                        "scope": "batch"
                    },
                    "quality_metrics_history": [
                        {"overall_score": 81}
                    ],
                    "quality_metrics_summary_state": {
                        "chapter_count": 2
                    },
                    "quality_metrics_summary": {
                        "overall_score": 84
                    },
                    "latest_quality_metrics": {
                        "overall_score": 85
                    }
                }),
                Some(json!([
                    {"overall_score": 91}
                ])),
            ),
        ));

        assert!(context.has_workflow_runtime_state());
        assert_eq!(
            context.request_runtime_state().model_override.as_deref(),
            Some("gpt-4.1")
        );
        assert_eq!(
            context
                .explicit_story_repair_payload()
                .and_then(|payload| payload.get("summary")),
            Some(&json!("来自运行时活动修复"))
        );
        assert_eq!(
            context.quality_metrics_history(),
            Some(&json!([
                {"overall_score": 91}
            ]))
        );
        assert_eq!(
            context.quality_metrics_summary_state(),
            Some(&json!({
                "chapter_count": 2
            }))
        );
        assert_eq!(
            context.quality_metrics_summary(),
            Some(&json!({
                "overall_score": 84
            }))
        );
        assert_eq!(
            context.latest_quality_metrics(),
            Some(&json!({
                "overall_score": 85
            }))
        );
    }

    #[test]
    fn should_prefer_snapshot_quality_fields_in_persisted_batch_runtime_context_owner() {
        let context = BatchGenerationPersistedRuntimeContext::from_snapshot(Some(
            batch_generation_snapshot::Model {
                id: "snapshot-1".to_string(),
                batch_task_id: "task-1".to_string(),
                latest_quality_metrics: Some(json!({
                    "overall_score": 91
                })),
                quality_metrics_history: Some(json!([
                    {"overall_score": 88},
                    {"overall_score": 91}
                ])),
                quality_metrics_summary: Some(json!({
                    "overall_score": 91,
                    "scope": "batch"
                })),
                workflow_runtime_state: Some(json!({
                    "batch_request_runtime_state": BatchGenerationRequestRuntimeState::new(
                        SingleChapterGenerationCompatOptions::default(),
                        Some("gpt-4.1".to_string())
                    ),
                    "latest_quality_metrics": {
                        "overall_score": 77
                    },
                    "quality_metrics_history": [
                        {"overall_score": 70}
                    ],
                    "quality_metrics_summary_state": {
                        "chapter_count": 2
                    },
                    "quality_metrics_summary": {
                        "overall_score": 77
                    }
                })),
                created_at: None,
                updated_at: None,
            },
        ));

        assert_eq!(
            context.latest_quality_metrics(),
            Some(&json!({
                "overall_score": 91
            }))
        );
        assert_eq!(
            context.quality_metrics_history(),
            Some(&json!([
                {"overall_score": 88},
                {"overall_score": 91}
            ]))
        );
        assert_eq!(
            context.quality_metrics_summary(),
            Some(&json!({
                "overall_score": 91,
                "scope": "batch"
            }))
        );
        assert_eq!(
            context.quality_metrics_summary_state(),
            Some(&json!({
                "chapter_count": 2
            }))
        );
    }

    #[test]
    fn should_build_batch_runtime_state_payload_with_fresh_latest_quality_metrics() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::default();
        let quality_summary = json!({
            "overall_score": 84,
            "repair_guidance": {
                "summary": "聚合摘要",
                "repair_targets": ["历史目标"],
                "preserve_strengths": ["历史优势"]
            },
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
            "repair_guidance": {
                "summary": "最新建议优先处理节奏",
                "repair_targets": ["最新目标", "历史目标"],
                "preserve_strengths": ["最新优势"],
                "focus_areas": ["节奏", "信息密度"]
            },
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
            payload["active_story_repair_payload"]["summary"],
            "最新建议优先处理节奏"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["最新目标", "历史目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["最新优势"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["focus_areas"],
            json!(["节奏", "信息密度"])
        );
        assert_eq!(
            payload["quality_metrics_history"],
            json!([{
                "overall_score": 84,
                "pacing_score": 7.6,
                "repair_guidance": {
                    "summary": "最新建议优先处理节奏",
                    "repair_targets": ["最新目标", "历史目标"],
                    "preserve_strengths": ["最新优势"],
                    "focus_areas": ["节奏", "信息密度"]
                },
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
        assert_eq!(
            payload["quality_metrics_summary"]["overall_score_delta"],
            3.0
        );
        assert_eq!(
            payload["quality_metrics_summary"]["overall_score_trend"],
            "rising"
        );
        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate_counts"]["passed"],
            1
        );
        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate_counts"]["auto_repair"],
            1
        );
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary_state"]["last_overall_score"],
            84.0
        );
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
        assert_eq!(
            history.first().and_then(|item| item.get("overall_score")),
            Some(&json!(1))
        );
        assert_eq!(
            history.last().and_then(|item| item.get("overall_score")),
            Some(&json!(20))
        );
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 20);
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 20.0);
        assert_eq!(
            payload["quality_metrics_summary"]["overall_score_delta"],
            19.0
        );
        assert_eq!(
            payload["quality_metrics_summary"]["overall_score_trend"],
            "rising"
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["chapter_count"],
            20
        );
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
        assert_eq!(
            payload["quality_metrics_summary"]["overall_score_delta"],
            -4.0
        );
        assert_eq!(
            payload["quality_metrics_summary"]["overall_score_trend"],
            "falling"
        );
        assert_eq!(
            payload["quality_metrics_summary"]["recent_focus_areas"],
            json!(["character", "pacing"])
        );
        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary_state"]["first_overall_score"],
            88.0
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["last_overall_score"],
            84.0
        );
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
        assert_eq!(
            payload["quality_metrics_summary_state"]["first_overall_score"],
            88.0
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["last_overall_score"],
            84.0
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["overall_score_total"],
            172.0
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["pacing_score_total"],
            15.8
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["pacing_score_count"],
            2
        );
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
    fn should_build_cancelled_persistence_plan_with_merged_status_payload_owner() {
        let mut task = build_task("running");
        task.id = "task-22".to_string();
        task.project_id = "project-7".to_string();
        task.total_chapters = 5;
        task.completed_chapters = 2;
        task.current_chapter_id = Some("chapter-3".to_string());
        task.current_chapter_number = Some(3);

        let snapshot = build_snapshot(
            Some(json!({"score": 91})),
            Some(json!({"summary": "ok"})),
            Some(json!({
                "progress": 55,
                "phase": "generating",
                "status": "running",
                "active_story_repair_payload": {
                    "mode": "repair"
                },
                "quality_metrics_history": [{"score": 90}]
            })),
        );

        let payload =
            super::BatchGenerationCancelledPersistencePlan::from_sources(&task, Some(&snapshot))
                .build_response_payload_for_task(batch_generation_task::Model {
                    status: "cancelled".to_string(),
                    ..task.clone()
                });

        assert_eq!(payload["batch_id"], "task-22");
        assert_eq!(payload["project_id"], "project-7");
        assert_eq!(payload["message"], "Batch generation cancelled");
        assert_eq!(payload["completed_chapters"], 2);
        assert_eq!(payload["total_chapters"], 5);
        assert_eq!(payload["status"], "cancelled");
        assert_eq!(payload["stage_code"], "6.writing.cancelled");
        assert_eq!(payload["checkpoint"]["status"], "cancelled");
        assert_eq!(payload["checkpoint"]["phase"], "cancelled");
        assert_eq!(payload["checkpoint"]["last_event"], "cancelled");
        assert_eq!(payload["checkpoint"]["last_message"], "批量生成已取消");
        assert_eq!(payload["checkpoint"]["completed"], 2);
        assert_eq!(payload["checkpoint"]["total"], 5);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_history"][0]["score"], 90);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
        assert_eq!(payload["terminal_reason"], "cancelled");
        assert_eq!(payload["terminal_label"], "已取消");
        assert_eq!(payload["review_required"], false);
        assert_eq!(payload["can_resume"], true);
    }

    #[test]
    fn should_build_batch_generation_execution_input_from_runtime_owner() {
        let input = build_batch_generation_execution_input(
            "user-10".to_string(),
            vec!["chapter-3".to_string()],
            2800,
            SingleChapterGenerationCompatOptions::default(),
            PreparedGenerationExecutionConfig {
                ai_config: AIConfig::default(),
                provider_payload: crate::services::chapter_generation_prompt_context_provider_service::build_placeholder_prompt_context_provider_payload(),
            },
        );

        assert_eq!(input.user_id, "user-10");
        assert_eq!(input.chapter_ids, vec!["chapter-3".to_string()]);
        assert_eq!(input.target_word_count, 2800);
        assert_eq!(input.ai_config.provider, AIConfig::default().provider);
    }

    #[test]
    fn should_build_batch_generation_launch_input_from_runtime_state_seed_owner() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                enable_analysis: true,
                enable_mcp: true,
                web_research_enabled: true,
                web_research_query: Some("江南夜航税卡".to_string()),
                quality_notes: Some("保留动作反馈".to_string()),
                ..SingleChapterGenerationCompatOptions::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let quality_summary = json!({
            "overall_score": 82,
            "repair_guidance": {
                "summary": "历史摘要",
                "repair_targets": ["历史目标"],
                "preserve_strengths": ["历史优势"],
                "focus_areas": ["节奏"]
            }
        });
        let latest_quality_metrics = json!({
            "overall_score": 79,
            "repair_guidance": {
                "summary": "最新建议优先压缩说明",
                "repair_targets": ["最新目标", "历史目标"],
                "preserve_strengths": ["最新优势"],
                "focus_areas": ["节奏", "信息密度"]
            },
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议继续修复"
            }
        });
        let runtime_state_seed =
            build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
                &request_runtime_state,
                None,
                Some(&quality_summary),
                Some(&latest_quality_metrics),
            );

        let input = build_batch_generation_runtime_launch_input_from_runtime_state_seed(
            "user-10".to_string(),
            vec!["chapter-3".to_string(), "chapter-4".to_string()],
            3200,
            &request_runtime_state,
            Some(&runtime_state_seed),
            PreparedGenerationExecutionConfig {
                ai_config: AIConfig::default(),
                provider_payload: crate::services::chapter_generation_prompt_context_provider_service::build_placeholder_prompt_context_provider_payload(),
            },
        );

        assert_eq!(input.user_id, "user-10");
        assert_eq!(
            input.chapter_ids,
            vec!["chapter-3".to_string(), "chapter-4".to_string()]
        );
        assert_eq!(input.target_word_count, 3200);
        assert_eq!(
            input.compat_options.story_repair_summary(),
            "最新建议优先压缩说明"
        );
        assert_eq!(
            input.compat_options.story_repair_targets(),
            &["最新目标".to_string(), "历史目标".to_string()]
        );
        assert_eq!(
            input.compat_options.story_preserve_strengths(),
            &["最新优势".to_string(), "历史优势".to_string()]
        );
        assert_eq!(input.compat_options.quality_notes(), "保留动作反馈");
        assert_eq!(
            input.compat_options.web_research_query(),
            Some("江南夜航税卡")
        );
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
        assert_eq!(
            session.compat_options,
            SingleChapterGenerationCompatOptions::default()
        );
        assert_eq!(
            chapter_ids,
            vec!["chapter-3".to_string(), "chapter-4".to_string()]
        );
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
        assert_eq!(
            overrides.story_repair_targets,
            vec!["提前冲突触发".to_string()]
        );
        assert_eq!(
            overrides.story_preserve_strengths,
            vec!["结尾钩子".to_string()]
        );
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
        assert_eq!(snapshot["last_event"], "analysis_completed");
        assert_eq!(snapshot["last_message"], "第 3 章分析完成");
        assert_eq!(snapshot["progress"], 100);
        assert!(snapshot.get("quality_gate_decision").is_none());
        assert!(snapshot.get("quality_gate_label").is_none());
        assert!(snapshot.get("phase").is_none());
    }

    #[tokio::test]
    async fn should_build_batch_generation_analysis_started_persistence_plan() {
        let plan = super::BatchGenerationAnalysisStartedPersistencePlan::from_generated_result(
            Some("analysis-task-3"),
            &GeneratedChapterResult {
                chapter_id: "chapter-3".to_string(),
                chapter_number: 3,
                title: "第三章".to_string(),
                content: "正文".to_string(),
                word_count: 1234,
            },
            1,
        );

        assert_eq!(plan.started_snapshot["analysis_task_id"], "analysis-task-3");
        assert_eq!(plan.started_snapshot["last_event"], "analysis_started");
        assert_eq!(plan.started_snapshot["last_message"], "正在分析章节");
        assert_eq!(plan.started_snapshot["progress"], 85);
        assert_eq!(plan.started_snapshot["phase"], "parsing");
        assert_eq!(
            plan.started_snapshot["analysis_task_message"],
            "第 3 章分析任务已启动"
        );
        assert_eq!(plan.started_snapshot["analysis_task_progress"], 85);
        assert_eq!(
            plan.started_snapshot["analysis_started_chapter_id"],
            "chapter-3"
        );
        assert_eq!(plan.started_snapshot["analysis_started_chapter_number"], 3);
        assert_eq!(plan.started_snapshot["analysis_retry_count"], 1);
        assert_eq!(plan.started_snapshot["analysis_max_retries"], 3);
        assert!(plan.started_snapshot["analysis_started_at"]
            .as_str()
            .is_some_and(|started_at| !started_at.is_empty()));
    }

    #[tokio::test]
    async fn should_build_batch_generation_analysis_started_persistence_plan_without_task_id() {
        let plan = super::BatchGenerationAnalysisStartedPersistencePlan::from_generated_result(
            None,
            &GeneratedChapterResult {
                chapter_id: "chapter-5".to_string(),
                chapter_number: 5,
                title: "第五章".to_string(),
                content: "正文".to_string(),
                word_count: 1536,
            },
            0,
        );

        assert!(plan.started_snapshot["analysis_task_id"].is_null());
        assert_eq!(plan.started_snapshot["last_event"], "analysis_started");
        assert_eq!(plan.started_snapshot["last_message"], "正在分析章节");
        assert_eq!(plan.started_snapshot["progress"], 85);
        assert_eq!(plan.started_snapshot["phase"], "parsing");
        assert_eq!(
            plan.started_snapshot["analysis_task_message"],
            "第 5 章分析任务已启动"
        );
        assert_eq!(plan.started_snapshot["analysis_task_progress"], 85);
        assert_eq!(
            plan.started_snapshot["analysis_started_chapter_id"],
            "chapter-5"
        );
        assert_eq!(plan.started_snapshot["analysis_started_chapter_number"], 5);
        assert_eq!(plan.started_snapshot["analysis_retry_count"], 0);
        assert_eq!(plan.started_snapshot["analysis_max_retries"], 3);
        assert!(plan.started_snapshot["analysis_started_at"]
            .as_str()
            .is_some_and(|started_at| !started_at.is_empty()));
    }

    #[tokio::test]
    async fn should_build_batch_generation_analysis_completion_persistence_plan() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let plan = super::BatchGenerationAnalysisCompletionPersistencePlan::from_generated_result(
            &db,
            "task-3",
            &GeneratedChapterResult {
                chapter_id: "chapter-3".to_string(),
                chapter_number: 3,
                title: "第三章".to_string(),
                content: "正文".to_string(),
                word_count: 1234,
            },
            1,
        )
        .await;

        assert_eq!(
            plan.completed_snapshot["analysis_task_message"],
            "第 3 章分析完成"
        );
        assert_eq!(plan.completed_snapshot["analysis_task_progress"], 100);
        assert_eq!(plan.completed_snapshot["analysis_retry_count"], 1);
        assert_eq!(plan.completed_snapshot["analysis_max_retries"], 3);
        assert!(plan.current_quality_snapshot.is_none());
    }

    #[tokio::test]
    async fn should_resolve_batch_generation_analysis_attempt_success_to_completed_owner() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let generated_result = GeneratedChapterResult {
            chapter_id: "chapter-3".to_string(),
            chapter_number: 3,
            title: "第三章".to_string(),
            content: "正文".to_string(),
            word_count: 1234,
        };

        let resolution = super::BatchGenerationAnalysisAttemptPlan::resolve_result(
            &db,
            "task-3",
            &generated_result,
            1,
            Ok(json!({"status": "completed"})),
        )
        .await
        .expect("resolve analysis attempt");

        assert!(matches!(
            resolution,
            super::BatchGenerationAnalysisAttemptResolution::Completed(None)
        ));
    }

    #[tokio::test]
    async fn should_resolve_batch_generation_analysis_attempt_error_to_retry_owner() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let generated_result = GeneratedChapterResult {
            chapter_id: "chapter-3".to_string(),
            chapter_number: 3,
            title: "第三章".to_string(),
            content: "正文".to_string(),
            word_count: 1234,
        };

        let resolution = super::BatchGenerationAnalysisAttemptPlan::resolve_result(
            &db,
            "task-3",
            &generated_result,
            1,
            Err("analysis timeout".to_string()),
        )
        .await;

        assert!(matches!(
            resolution,
            Ok(super::BatchGenerationAnalysisAttemptResolution::Retry)
        ));
    }

    #[test]
    fn should_keep_batch_generation_analysis_attempt_plan_owner_contract() {
        let generated_result = GeneratedChapterResult {
            chapter_id: "chapter-18-a".to_string(),
            chapter_number: 18,
            title: "第十八章".to_string(),
            content: "正文".to_string(),
            word_count: 2300,
        };

        let owner = BatchGenerationAnalysisAttemptPlan::from_generated_result(&generated_result, 2);

        assert_eq!(owner.generated_result, generated_result);
        assert_eq!(owner.analysis_retry_count, 2);
    }

    #[test]
    fn should_route_batch_generation_analysis_error_to_retry_owner() {
        let plan = super::BatchGenerationAnalysisRoutingPlan::from_analysis_error_message(
            6,
            "analysis timeout".to_string(),
            1,
        );

        assert!(matches!(
            plan,
            super::BatchGenerationAnalysisRoutingPlan::Retry {
                retry_snapshot,
                next_retry_count,
                wait_seconds,
            } if retry_snapshot["analysis_task_message"] == "第 6 章分析失败，准备重试"
                && retry_snapshot["last_event"] == "analysis_retry"
                && retry_snapshot["last_message"] == "第 6 章分析失败，准备重试"
                && retry_snapshot["progress"] == 85
                && retry_snapshot["phase"] == "parsing"
                && retry_snapshot["analysis_last_error"] == "analysis timeout"
                && next_retry_count == 2
                && wait_seconds == 4
        ));
    }

    #[test]
    fn should_route_batch_generation_analysis_error_to_stop_owner_after_budget_exhausted() {
        let plan = super::BatchGenerationAnalysisRoutingPlan::from_analysis_error_message(
            6,
            "analysis timeout".to_string(),
            2,
        );

        assert!(matches!(
            plan,
            super::BatchGenerationAnalysisRoutingPlan::Stop { error_message }
                if error_message == "analysis timeout"
        ));
    }

    #[test]
    fn should_stop_batch_generation_analysis_project_missing_without_retry() {
        let plan = super::BatchGenerationAnalysisRoutingPlan::from_analysis_error_message(
            6,
            "章节或项目已删除，无法继续分析".to_string(),
            0,
        );

        assert!(matches!(
            plan,
            super::BatchGenerationAnalysisRoutingPlan::Stop { error_message }
                if error_message == "章节或项目已删除，无法继续分析"
        ));
    }

    #[test]
    fn should_stop_batch_generation_analysis_empty_chapter_without_retry() {
        let plan = super::BatchGenerationAnalysisRoutingPlan::from_analysis_error_message(
            6,
            "章节不存在或内容为空".to_string(),
            0,
        );

        assert!(matches!(
            plan,
            super::BatchGenerationAnalysisRoutingPlan::Stop { error_message }
                if error_message == "章节不存在或内容为空"
        ));
    }

    #[test]
    fn should_format_project_missing_analysis_error_with_chinese_contract() {
        let message = super::format_analysis_error_message(
            &crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::ProjectMissing,
        );

        assert_eq!(message, "章节或项目已删除，无法继续分析");
    }

    #[test]
    fn should_build_quality_gate_blocked_runtime_state_patch_with_terminal_repair_payload() {
        let patch = shared_build_quality_gate_blocked_runtime_state_patch_from_workflow_state(
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
        assert_eq!(
            patch["quality_metrics_summary"]["quality_gate"]["status"],
            "failed"
        );
        assert_eq!(
            patch["quality_metrics_summary"]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            patch["quality_metrics_summary"]["quality_gate"]["label"],
            "自动修复预算已耗尽"
        );
        assert_eq!(
            patch["latest_quality_metrics"]["quality_gate"]["status"],
            "failed"
        );
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
        assert_eq!(
            patch["quality_history_context"]["recent_manual_review_count"],
            1
        );
        assert_eq!(
            patch["quality_history_context"]["recent_auto_repair_count"],
            0
        );
    }

    #[test]
    fn should_build_quality_gate_blocked_runtime_state_patch_from_summary_only_quality_context() {
        let patch = shared_build_quality_gate_blocked_runtime_state_patch_from_workflow_state(
            Some(&json!({
                "batch_request_runtime_state": {
                    "compat_options": {}
                },
                "quality_metrics_summary": {
                    "overall_score": 84,
                    "repair_guidance": {
                        "summary": "当前章节需要压缩说明"
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    },
                    "quality_runtime_context": {
                        "scope": "batch",
                        "recent_metrics": [
                            {
                                "overall_score": 84,
                                "quality_gate": {
                                    "status": "warning",
                                    "decision": "auto_repair",
                                    "label": "建议继续修复"
                                }
                            },
                            {
                                "overall_score": 88,
                                "repair_guidance": {
                                    "summary": "上一章总体稳定"
                                },
                                "quality_gate": {
                                    "status": "passed",
                                    "decision": "continue",
                                    "label": "通过"
                                }
                            }
                        ]
                    }
                },
                "active_story_repair_payload": {
                    "summary": "继续补强冲突",
                    "scope": "batch",
                    "quality_gate_decision": "auto_repair",
                    "quality_gate_label": "建议继续修复",
                    "phase": "repair_pending"
                }
            })),
            11,
            "自动修复预算已耗尽",
        );

        assert_eq!(patch["quality_gate_decision"], "manual_review");
        assert_eq!(
            patch["quality_metrics_summary"]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(patch["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(patch["quality_metrics_history"][0]["overall_score"], 88);
        assert_eq!(patch["quality_metrics_history"][1]["overall_score"], 84);
        assert_eq!(
            patch["quality_metrics_history"][1]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(patch["quality_metrics_summary_state"]["scope"], "batch");
        assert_eq!(patch["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(patch["quality_history_context"]["scope"], "batch");
        assert_eq!(
            patch["quality_history_context"]["quality_gate_counts"]["manual_review"],
            2
        );
        assert!(patch["quality_history_context"]["quality_gate_counts"]
            .get("auto_repair")
            .is_none());
        assert_eq!(
            patch["active_story_repair_payload"]["quality_gate_decision"],
            "manual_review"
        );
    }

    #[test]
    fn should_build_batch_generation_runtime_driver_progression_contract() {
        assert_eq!(
            BatchGenerationRuntimeDriverProgression::Continue(BatchGenerationStepProgress::new(
                2, 5
            ),),
            BatchGenerationRuntimeDriverProgression::Continue(BatchGenerationStepProgress::new(
                2, 5
            ),)
        );
        assert_eq!(
            BatchGenerationRuntimeDriverProgression::Stop,
            BatchGenerationRuntimeDriverProgression::Stop
        );
        assert_eq!(
            BatchGenerationAttemptProgression::Retry(2),
            BatchGenerationAttemptProgression::Retry(2)
        );
        assert_eq!(
            BatchGenerationAttemptProgression::Driver(
                BatchGenerationRuntimeDriverProgression::Stop,
            ),
            BatchGenerationAttemptProgression::Driver(
                BatchGenerationRuntimeDriverProgression::Stop,
            )
        );
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
            BatchGenerationRuntimeDriverProgression::Continue(next_progress),
            BatchGenerationRuntimeDriverProgression::Continue(BatchGenerationStepProgress::new(
                3, 5
            ),)
        );
        assert_eq!(
            persistence_plan.current_chapter_id.as_deref(),
            Some("chapter-4")
        );
        assert_eq!(persistence_plan.current_chapter_number, Some(4));
        assert_eq!(persistence_plan.completed_chapters, 3);
        assert_eq!(persistence_plan.total_chapters, 5);
        assert_eq!(persistence_plan.error_message, None);
        assert_eq!(persistence_plan.failed_chapter_entry, None);
    }

    #[test]
    fn should_keep_batch_generation_runtime_public_start_owner_contract() {
        let runtime_input = BatchGenerationExecutionInput {
            user_id: "user-8".to_string(),
            chapter_ids: vec!["chapter-8".to_string(), "chapter-9".to_string()],
            target_word_count: 3600,
            compat_options: SingleChapterGenerationCompatOptions {
                enable_analysis: true,
                ..Default::default()
            },
            ai_config: AIConfig::default(),
        };

        let lifecycle =
            BatchGenerationRuntimeLifecyclePlan::from_execution_input(runtime_input.clone());
        let public_start = BatchGenerationRuntimeLifecyclePlan::from_execution_input(runtime_input);

        assert_eq!(lifecycle.session.user_id, "user-8");
        assert_eq!(lifecycle.session.target_word_count, 3600);
        assert!(lifecycle.session.compat_options.enable_analysis());
        assert_eq!(lifecycle.session.total_chapters, 2);
        assert_eq!(
            lifecycle.chapter_ids,
            vec!["chapter-8".to_string(), "chapter-9".to_string()]
        );
        assert_eq!(public_start.chapter_ids.len(), 2);
        assert_eq!(public_start.session.total_chapters, 2);
    }

    #[test]
    fn should_keep_batch_generation_attempt_input_plan_owner_contract() {
        let provider_payload =
            crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload {
                characters_info: "[]".to_string(),
                chapter_careers: "[]".to_string(),
                recent_chapters_context: "前情".to_string(),
                previous_chapter_summary: "上章摘要".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: "[]".to_string(),
                research_query: "夜航税卡".to_string(),
                research_assets: "[]".to_string(),
                external_assets: "[]".to_string(),
                reference_assets: "[]".to_string(),
                mcp_references: String::new(),
            };
        let prompt_overrides =
            crate::services::chapter_generation_prompt_service::ChapterGenerationPromptOverrides {
                creative_mode: Some("hook".to_string()),
                quality_notes: Some("压缩说明".to_string()),
                ..crate::services::chapter_generation_prompt_service::ChapterGenerationPromptOverrides::default()
            };

        let owner = BatchGenerationAttemptInputPlan::from_sources(
            provider_payload.clone(),
            prompt_overrides.clone(),
        );

        assert_eq!(owner.provider_payload, provider_payload);
        assert_eq!(owner.prompt_overrides, prompt_overrides);
    }

    #[test]
    fn should_keep_prepared_batch_generation_step_execution_owner_contract() {
        let task_model = batch_generation_task::Model {
            id: "task-16".to_string(),
            project_id: "project-16".to_string(),
            user_id: "user-16".to_string(),
            start_chapter_number: 1,
            chapter_count: 3,
            chapter_ids: serde_json::json!(["chapter-16", "chapter-17", "chapter-18"]),
            style_id: None,
            target_word_count: 2200,
            enable_analysis: true,
            status: "running".to_string(),
            total_chapters: 3,
            completed_chapters: 1,
            failed_chapters: serde_json::json!([]),
            current_chapter_id: Some("chapter-16".to_string()),
            current_chapter_number: Some(16),
            current_retry_count: 2,
            max_retries: 6,
            created_at: Some(chrono::Utc::now().naive_utc()),
            started_at: Some(chrono::Utc::now().naive_utc()),
            completed_at: None,
            error_message: None,
        };
        let chapter_model = chapter::Model {
            id: "chapter-16".to_string(),
            project_id: "project-16".to_string(),
            chapter_number: 16,
            title: "第十六章".to_string(),
            content: Some("正文".to_string()),
            summary: None,
            word_count: 1900,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };

        let owner = PreparedBatchGenerationStepExecution::from_task_and_chapter(
            &task_model,
            &chapter_model,
        );

        assert_eq!(owner.chapter_model.id, "chapter-16");
        assert_eq!(owner.chapter_model.project_id, "project-16");
        assert_eq!(owner.retry_count, 2);
        assert_eq!(owner.max_retries, 6);
    }

    #[test]
    fn should_keep_batch_generation_follow_up_analysis_plan_owner_contract() {
        let generated_result = GeneratedChapterResult {
            chapter_id: "chapter-19".to_string(),
            chapter_number: 19,
            title: "第十九章".to_string(),
            content: "正文".to_string(),
            word_count: 2600,
        };

        let owner = BatchGenerationFollowUpAnalysisPlan::from_generated_result(&generated_result);

        assert_eq!(owner.generated_result, generated_result);
    }

    #[test]
    fn should_keep_batch_generation_post_analysis_terminal_plan_owner_contract() {
        let chapter_model = chapter::Model {
            id: "chapter-20-t".to_string(),
            project_id: "project-20".to_string(),
            chapter_number: 20,
            title: "第二十章".to_string(),
            content: Some("正文".to_string()),
            summary: None,
            word_count: 2800,
            status: "completed".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let progress = BatchGenerationStepProgress::new(6, 12);
        let success_state = Some(json!({
            "quality_metrics_summary": {
                "overall_score": 8.8
            }
        }));

        let success_owner = BatchGenerationPostAnalysisTerminalPlan::on_success(
            &chapter_model,
            &progress,
            success_state.clone(),
        );
        assert_eq!(success_owner.chapter_model.id, "chapter-20-t");
        assert_eq!(success_owner.progress, progress);
        assert_eq!(
            success_owner.outcome,
            BatchGenerationPostAnalysisTerminalOutcome::Success {
                current_quality_runtime_state: success_state
            }
        );

        let failure_owner = BatchGenerationPostAnalysisTerminalPlan::on_failure(
            &chapter_model,
            &progress,
            "章节分析失败".to_string(),
        );
        assert_eq!(failure_owner.chapter_model.id, "chapter-20-t");
        assert_eq!(failure_owner.progress, progress);
        assert_eq!(
            failure_owner.outcome,
            BatchGenerationPostAnalysisTerminalOutcome::Failure {
                analysis_error: "章节分析失败".to_string()
            }
        );
    }

    #[tokio::test]
    async fn should_build_batch_generation_retry_progression_plan_owner_contract() {
        let outcome = BatchGenerationRetryProgressionPlan::new(2).execute().await;

        assert_eq!(outcome, BatchGenerationAttemptProgression::Retry(2));
    }

    #[test]
    fn should_build_batch_generation_post_write_guard_plan_owner_contract() {
        let owner = BatchGenerationPostWriteGuardPlan::for_chapter("chapter-22");

        assert_eq!(owner.chapter_id, "chapter-22");
        assert_eq!(
            BatchGenerationPostWriteGuardPlan::resolve(true, true),
            BatchGenerationPostWriteGuardOutcome::Continue
        );
        assert_eq!(
            BatchGenerationPostWriteGuardPlan::resolve(false, true),
            BatchGenerationPostWriteGuardOutcome::Stop
        );
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
    fn should_build_retry_current_chapter_attempt_progression_contract() {
        let outcome = BatchGenerationAttemptProgression::Retry(2);

        assert_eq!(outcome, BatchGenerationAttemptProgression::Retry(2));
    }

    #[test]
    fn should_continue_post_write_guard_only_when_task_and_chapter_still_exist() {
        assert_eq!(
            super::BatchGenerationPostWriteGuardPlan::resolve(true, true),
            super::BatchGenerationPostWriteGuardOutcome::Continue
        );
        assert_eq!(
            super::BatchGenerationPostWriteGuardPlan::resolve(false, true),
            super::BatchGenerationPostWriteGuardOutcome::Stop
        );
        assert_eq!(
            super::BatchGenerationPostWriteGuardPlan::resolve(true, false),
            super::BatchGenerationPostWriteGuardOutcome::Stop
        );
        assert_eq!(
            super::BatchGenerationPostWriteGuardPlan::resolve(false, false),
            super::BatchGenerationPostWriteGuardOutcome::Stop
        );
    }

    #[test]
    fn should_route_generic_step_error_to_retry_owner_when_budget_allows() {
        let chapter_model = chapter::Model {
            id: "chapter-10".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 10,
            title: "长夜".to_string(),
            content: Some("content".to_string()),
            summary: None,
            word_count: 1800,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let plan = super::BatchGenerationGenericFailureRoutingPlan::from_step_error(
            &chapter_model,
            &BatchGenerationStepProgress::new(2, 6),
            1,
            3,
            BatchGenerationFailureKind::GenerationError,
            "provider timeout",
        );

        assert!(matches!(
            plan,
            super::BatchGenerationGenericFailureRoutingPlan::Retry(
                super::BatchGenerationRetryPersistencePlan {
                    next_retry_count,
                    max_retries,
                    error_message,
                    ..
                }
            ) if next_retry_count == 2 && max_retries == 3 && error_message == "provider timeout"
        ));
    }

    #[test]
    fn should_route_generic_step_error_to_failed_owner_when_retry_budget_exhausted() {
        let chapter_model = chapter::Model {
            id: "chapter-11".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 11,
            title: "回潮".to_string(),
            content: Some("content".to_string()),
            summary: None,
            word_count: 1900,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let plan = super::BatchGenerationGenericFailureRoutingPlan::from_step_error(
            &chapter_model,
            &BatchGenerationStepProgress::new(4, 6),
            3,
            3,
            BatchGenerationFailureKind::GenerationError,
            "provider timeout",
        );

        assert!(matches!(
            plan,
            super::BatchGenerationGenericFailureRoutingPlan::Stop(
                super::BatchGenerationRuntimePersistencePlan {
                    completed_chapters,
                    total_chapters,
                    current_retry_count,
                    error_message,
                    failed_chapter_entry,
                    ..
                }
            ) if completed_chapters == 4
                && total_chapters == 6
                && current_retry_count == Some(3)
                && error_message.as_deref() == Some("第11章生成失败(重试3次): provider timeout")
                && failed_chapter_entry
                    .as_ref()
                    .and_then(|entry| entry.get("retry_count"))
                    == Some(&json!(3))
                && failed_chapter_entry
                    .as_ref()
                    .and_then(|entry| entry.get("error"))
                    == Some(&json!("provider timeout"))
        ));
    }

    #[test]
    fn should_route_project_mismatch_step_error_to_failed_owner_without_chapter_number() {
        let plan = super::BatchGenerationGenericFailureRoutingPlan::from_step_context(
            "chapter-12",
            None,
            None,
            &BatchGenerationStepProgress::new(1, 6),
            3,
            3,
            BatchGenerationFailureKind::GenerationError,
            "章节 chapter-12 项目不匹配",
        );

        assert!(matches!(
            plan,
            super::BatchGenerationGenericFailureRoutingPlan::Stop(
                super::BatchGenerationRuntimePersistencePlan {
                    completed_chapters,
                    total_chapters,
                    current_retry_count,
                    error_message,
                    failed_chapter_entry,
                    ..
                }
            ) if completed_chapters == 1
                && total_chapters == 6
                && current_retry_count == Some(3)
                && error_message.as_deref()
                    == Some("章节生成失败(重试3次): 章节 chapter-12 项目不匹配")
                && failed_chapter_entry
                    .as_ref()
                    .and_then(|entry| entry.get("error"))
                    == Some(&json!("章节 chapter-12 项目不匹配"))
                && failed_chapter_entry
                    .as_ref()
                    .and_then(|entry| entry.get("retry_count"))
                    == Some(&json!(3))
        ));
    }

    #[test]
    fn should_route_prerequisite_block_message_to_failed_owner() {
        let chapter_model = chapter::Model {
            id: "chapter-13".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 13,
            title: "断桥".to_string(),
            content: Some("content".to_string()),
            summary: None,
            word_count: 2000,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let plan = super::BatchGenerationGenericFailureRoutingPlan::from_step_error(
            &chapter_model,
            &BatchGenerationStepProgress::new(2, 6),
            3,
            3,
            BatchGenerationFailureKind::GenerationError,
            "章节生成失败: 前置章节尚未完成: 2, 3 章",
        );

        assert!(matches!(
            plan,
            super::BatchGenerationGenericFailureRoutingPlan::Stop(
                super::BatchGenerationRuntimePersistencePlan {
                    error_message,
                    failed_chapter_entry,
                    ..
                }
            ) if error_message.as_deref()
                == Some("第13章生成失败(重试3次): 章节生成失败: 前置章节尚未完成: 2, 3 章")
                && failed_chapter_entry
                    .as_ref()
                    .and_then(|entry| entry.get("error"))
                    == Some(&json!("章节生成失败: 前置章节尚未完成: 2, 3 章"))
        ));
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
    fn should_build_failed_persistence_plan_with_split_entry_and_task_errors() {
        let plan = super::BatchGenerationRuntimePersistencePlan::failed(
            Some("chapter-7"),
            Some(7),
            Some("高潮前夜"),
            2,
            5,
            crate::services::chapter_batch_generation_runtime_checkpoint_service::BatchGenerationFailureKind::GenerationError,
            3,
            "provider timeout".to_string(),
            "第7章生成失败(重试3次): provider timeout".to_string(),
        );

        assert_eq!(plan.current_retry_count, Some(3));
        assert_eq!(
            plan.error_message.as_deref(),
            Some("第7章生成失败(重试3次): provider timeout")
        );
        assert_eq!(
            plan.failed_chapter_entry
                .as_ref()
                .and_then(|entry| entry.get("error")),
            Some(&json!("provider timeout"))
        );
        assert_eq!(
            plan.failed_chapter_entry
                .as_ref()
                .and_then(|entry| entry.get("retry_count")),
            Some(&json!(3))
        );
    }

    #[test]
    fn should_build_generic_failed_task_error_message_without_chapter_number() {
        assert_eq!(
            super::build_batch_generation_failed_task_error_message(
                None,
                3,
                "章节 chapter-missing 不存在"
            ),
            "章节生成失败(重试3次): 章节 chapter-missing 不存在"
        );
    }

    #[test]
    fn should_build_quality_gate_blocked_failed_chapter_entry_with_terminal_semantics() {
        let entry = super::build_quality_gate_blocked_failed_chapter_entry(
            Some("chapter-7"),
            Some(7),
            Some("高潮前夜"),
            "第7章触发质量门禁，需人工复核: 自动修复预算已耗尽",
            3,
            &BatchGenerationFailedTerminalSemantics {
                kind: BatchGenerationFailedTerminalKind::ManualReview,
                reason: "manual_review",
                label: "自动修复预算已耗尽".to_string(),
                review_required: true,
                can_resume: false,
            },
            Some(&json!({
                "active_story_repair_payload": {
                    "quality_gate_failed_metrics": ["节奏", "信息密度"]
                }
            })),
        );

        assert_eq!(entry["chapter_id"], "chapter-7");
        assert_eq!(entry["retry_count"], 3);
        assert_eq!(entry["quality_gate_decision"], "manual_review");
        assert_eq!(entry["quality_gate_label"], "自动修复预算已耗尽");
        assert_eq!(entry["quality_gate_status"], "failed");
        assert_eq!(
            entry["quality_gate_failed_metrics"],
            json!(["节奏", "信息密度"])
        );
        assert_eq!(entry["phase"], "quality_blocked");
        assert!(entry.get("terminal_reason").is_none());
        assert!(entry.get("terminal_label").is_none());
        assert!(entry.get("review_required").is_none());
        assert!(entry.get("can_resume").is_none());
    }

    #[test]
    fn should_build_quality_gate_blocked_persistence_plan_from_terminal_semantics_owner() {
        let persistence_plan =
            super::BatchGenerationRuntimePersistencePlan::failed_quality_gate_blocked(
                Some("chapter-7"),
                Some(7),
                Some("高潮前夜"),
                2,
                5,
                3,
                &BatchGenerationFailedTerminalSemantics {
                    kind: BatchGenerationFailedTerminalKind::ManualReview,
                    reason: "manual_review",
                    label: "自动修复预算已耗尽".to_string(),
                    review_required: true,
                    can_resume: false,
                },
                Some(&json!({
                    "quality_metrics_summary": {
                        "quality_gate": {
                            "failed_metrics": [{"label": "节奏"}]
                        }
                    }
                })),
                "第7章触发质量门禁，需人工复核: 自动修复预算已耗尽".to_string(),
            );

        assert_eq!(
            persistence_plan.current_chapter_id.as_deref(),
            Some("chapter-7")
        );
        assert_eq!(persistence_plan.current_chapter_number, Some(7));
        assert_eq!(persistence_plan.completed_chapters, 2);
        assert_eq!(persistence_plan.total_chapters, 5);
        assert_eq!(
            persistence_plan.error_message.as_deref(),
            Some("第7章触发质量门禁，需人工复核: 自动修复预算已耗尽")
        );
        assert_eq!(persistence_plan.current_retry_count, Some(3));
        assert_eq!(
            persistence_plan
                .failed_chapter_entry
                .as_ref()
                .and_then(|entry| entry.get("quality_gate_label")),
            Some(&json!("自动修复预算已耗尽"))
        );
        assert_eq!(
            persistence_plan
                .failed_chapter_entry
                .as_ref()
                .and_then(|entry| entry.get("quality_gate_status")),
            Some(&json!("failed"))
        );
        assert_eq!(
            persistence_plan
                .failed_chapter_entry
                .as_ref()
                .and_then(|entry| entry.get("quality_gate_failed_metrics")),
            Some(&json!(["节奏"]))
        );
        assert!(persistence_plan
            .failed_chapter_entry
            .as_ref()
            .and_then(|entry| entry.get("terminal_reason"))
            .is_none());
    }

    #[test]
    fn should_extract_quality_gate_failed_metrics_from_runtime_state_sources() {
        let from_active_payload =
            super::extract_quality_gate_failed_metrics_from_runtime_state(Some(&json!({
                "active_story_repair_payload": {
                    "quality_gate_failed_metrics": ["节奏", "信息密度", "节奏"]
                }
            })));
        assert_eq!(from_active_payload, vec!["节奏", "信息密度"]);

        let from_quality_gate =
            super::extract_quality_gate_failed_metrics_from_runtime_state(Some(&json!({
                "quality_metrics_summary": {
                    "quality_gate": {
                        "failed_metrics": [{"label": "人物"}, {"label": "节奏"}]
                    }
                }
            })));
        assert_eq!(from_quality_gate, vec!["人物", "节奏"]);
    }

    #[test]
    fn should_apply_shared_manual_review_terminal_fields_and_quality_gate() {
        let mut payload = serde_json::Map::new();
        super::apply_manual_review_terminal_fields(&mut payload, "等待人工复核");
        assert_eq!(payload["quality_gate_decision"], "manual_review");
        assert_eq!(payload["quality_gate_label"], "等待人工复核");
        assert_eq!(payload["phase"], "quality_blocked");

        let mut gate = json!({});
        crate::services::chapter_generation_quality_runtime_context_service::normalize_terminal_quality_gate_payload(
            &mut gate,
            "等待人工复核",
        );
        assert_eq!(gate["quality_gate"]["status"], "failed");
        assert_eq!(gate["quality_gate"]["decision"], "manual_review");
        assert_eq!(gate["quality_gate"]["label"], "等待人工复核");
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
        shared_normalize_terminal_quality_history(&mut history, "等待人工复核");
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
        shared_normalize_terminal_quality_history_context(&mut context, "等待人工复核");
        assert_eq!(
            context["recent_metrics"][0]["quality_gate"]["status"],
            "failed"
        );
        assert_eq!(
            context["recent_metrics"][0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            context["recent_metrics"][0]["quality_gate"]["label"],
            "等待人工复核"
        );
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
        shared_apply_terminal_quality_runtime_patch_contract(
            &mut payload,
            Some(&runtime_state),
            runtime_state.get("active_story_repair_payload"),
            "等待人工复核",
        );

        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            payload["latest_quality_metrics"]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            payload["quality_metrics_history"][0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["recent_history"][0]["quality_gate"]
                ["decision"],
            "manual_review"
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"][0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            payload["quality_history_context"]["quality_gate_counts"]["manual_review"],
            1
        );
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
            shared_build_manual_review_terminal_runtime_patch_contract(7, "等待人工复核");
        shared_apply_terminal_quality_runtime_patch_contract(
            &mut payload,
            Some(&runtime_state),
            runtime_state.get("active_story_repair_payload"),
            "等待人工复核",
        );

        assert_eq!(
            payload["analysis_task_message"],
            "第 7 章触发质量门禁，需人工复核"
        );
        assert_eq!(payload["analysis_task_progress"], 100);
        assert!(payload["analysis_last_error"].is_null());
        assert_eq!(payload["quality_gate_decision"], "manual_review");
        assert_eq!(payload["quality_gate_label"], "等待人工复核");
        assert_eq!(payload["phase"], "quality_blocked");
        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["quality_gate_decision"],
            "manual_review"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["quality_gate_label"],
            "等待人工复核"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["phase"],
            "quality_blocked"
        );
    }

    #[test]
    fn should_build_shared_retry_quality_runtime_patch_contract() {
        let runtime_state = json!({
            "active_story_repair_payload": {
                "summary": "继续补强冲突",
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核",
                "phase": "quality_blocked"
            }
        });
        let payload = shared_build_retry_quality_runtime_patch_contract_from_workflow_state(
            Some(&runtime_state),
            7,
            "自动修复后重试",
        );

        assert_eq!(
            payload["analysis_task_message"],
            "第 7 章触发质量修复，等待重试"
        );
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
        assert_eq!(
            payload["active_story_repair_payload"]["phase"],
            "repair_pending"
        );
    }

    #[test]
    fn should_build_shared_retry_quality_runtime_patch_from_summary_only_quality_context() {
        let runtime_state = json!({
            "batch_request_runtime_state": {
                "compat_options": {}
            },
            "quality_metrics_summary": {
                "overall_score": 84,
                "repair_guidance": {
                    "summary": "当前章节需要压缩说明"
                },
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                },
                "quality_runtime_context": {
                    "scope": "batch",
                    "recent_metrics": [
                        {
                            "overall_score": 84,
                            "quality_gate": {
                                "status": "warning",
                                "decision": "auto_repair",
                                "label": "建议继续修复"
                            }
                        },
                        {
                            "overall_score": 88,
                            "repair_guidance": {
                                "summary": "上一章总体稳定"
                            },
                            "quality_gate": {
                                "status": "passed",
                                "decision": "continue",
                                "label": "通过"
                            }
                        }
                    ]
                }
            },
            "active_story_repair_payload": {
                "summary": "继续补强冲突",
                "scope": "batch",
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核",
                "phase": "quality_blocked"
            }
        });

        let payload = shared_build_retry_quality_runtime_patch_contract_from_workflow_state(
            Some(&runtime_state),
            7,
            "自动修复后重试",
        );

        assert_eq!(payload["quality_gate_decision"], "auto_repair");
        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 88);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 84);
        assert_eq!(
            payload["quality_metrics_history"][1]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "batch");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(payload["quality_history_context"]["scope"], "batch");
        assert_eq!(
            payload["quality_history_context"]["quality_gate_counts"]["auto_repair"],
            2
        );
        assert!(payload["quality_history_context"]["quality_gate_counts"]
            .get("manual_review")
            .is_none());
        assert_eq!(
            payload["active_story_repair_payload"]["quality_gate_decision"],
            "auto_repair"
        );
    }

    #[test]
    fn should_resolve_manual_review_terminal_semantics_from_current_quality_runtime_state() {
        let current_quality_runtime_state = json!({
            "quality_metrics_summary": {
                "quality_gate": {
                    "status": "failed",
                    "decision": "manual_review",
                    "label": "需要人工复核"
                }
            },
            "latest_quality_metrics": {
                "quality_gate": {
                    "status": "failed",
                    "decision": "manual_review",
                    "label": "需要人工复核"
                }
            },
            "active_story_repair_payload": {
                "summary": "继续补强冲突",
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "需要人工复核",
                "phase": "quality_blocked"
            }
        });

        let semantics = super::resolve_batch_generation_quality_gate_terminal_semantics(
            None,
            Some(&current_quality_runtime_state),
            3,
            3,
        )
        .expect("manual review terminal semantics");

        assert_eq!(
            semantics.kind,
            BatchGenerationFailedTerminalKind::ManualReview
        );
        assert_eq!(semantics.reason, "manual_review");
        assert_eq!(semantics.label, "需要人工复核");
        assert!(semantics.review_required);
        assert!(!semantics.can_resume);
    }

    #[test]
    fn should_resolve_retry_terminal_semantics_from_current_quality_runtime_state() {
        let current_quality_runtime_state = json!({
            "quality_metrics_summary": {
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "自动修复后重试"
                }
            },
            "latest_quality_metrics": {
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "自动修复后重试"
                }
            },
            "active_story_repair_payload": {
                "summary": "继续补强冲突",
                "quality_gate_decision": "auto_repair",
                "quality_gate_label": "自动修复后重试",
                "phase": "repair_pending"
            }
        });

        let semantics = super::resolve_batch_generation_quality_gate_terminal_semantics(
            None,
            Some(&current_quality_runtime_state),
            0,
            3,
        )
        .expect("retry terminal semantics");

        assert_eq!(semantics.kind, BatchGenerationFailedTerminalKind::Retry);
        assert_eq!(semantics.reason, "retry");
        assert_eq!(semantics.label, "自动修复后重试");
        assert!(!semantics.review_required);
        assert!(semantics.can_resume);
    }

    #[test]
    fn should_route_quality_gate_manual_review_to_stop_owner() {
        let chapter_model = chapter::Model {
            id: "chapter-12".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "潮声".to_string(),
            content: Some("content".to_string()),
            summary: None,
            word_count: 2100,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let plan = super::BatchGenerationQualityGateRoutingPlan::from_terminal_semantics(
            &chapter_model,
            &BatchGenerationStepProgress::new(3, 7),
            Some(&json!({
                "active_story_repair_payload": {
                    "quality_gate_decision": "manual_review",
                    "quality_gate_label": "需要人工复核",
                    "phase": "quality_blocked"
                }
            })),
            3,
            3,
            BatchGenerationFailedTerminalSemantics {
                kind: BatchGenerationFailedTerminalKind::ManualReview,
                reason: "manual_review",
                label: "需要人工复核".to_string(),
                review_required: true,
                can_resume: false,
            },
        )
        .expect("manual review routing plan");

        assert!(matches!(
            plan,
            super::BatchGenerationQualityGateRoutingPlan::Stop {
                runtime_state_patch,
                persistence_plan:
                    super::BatchGenerationRuntimePersistencePlan {
                        current_retry_count,
                        error_message,
                        ..
                    },
            } if runtime_state_patch["quality_gate_decision"] == "manual_review"
                && current_retry_count == Some(3)
                && error_message.as_deref() == Some("第12章触发质量门禁，需人工复核: 需要人工复核")
        ));
    }

    #[test]
    fn should_route_quality_gate_auto_repair_to_retry_owner() {
        let chapter_model = chapter::Model {
            id: "chapter-13".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 13,
            title: "回港".to_string(),
            content: Some("content".to_string()),
            summary: None,
            word_count: 2200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };
        let plan = super::BatchGenerationQualityGateRoutingPlan::from_terminal_semantics(
            &chapter_model,
            &BatchGenerationStepProgress::new(1, 7),
            Some(&json!({
                "active_story_repair_payload": {
                    "quality_gate_decision": "auto_repair",
                    "quality_gate_label": "自动修复后重试",
                    "phase": "repair_pending"
                }
            })),
            1,
            3,
            BatchGenerationFailedTerminalSemantics {
                kind: BatchGenerationFailedTerminalKind::Retry,
                reason: "retry",
                label: "自动修复后重试".to_string(),
                review_required: false,
                can_resume: true,
            },
        )
        .expect("retry routing plan");

        assert!(matches!(
            plan,
            super::BatchGenerationQualityGateRoutingPlan::Retry {
                runtime_state_patch,
                next_retry_count,
                persistence_plan:
                    super::BatchGenerationRetryPersistencePlan {
                        error_message,
                        ..
                    },
            } if runtime_state_patch["quality_gate_decision"] == "auto_repair"
                && next_retry_count == 2
                && error_message == "第13章触发质量修复重试: 自动修复后重试"
        ));
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

        assert_eq!(
            merged,
            json!([
                {"chapter_id": "chapter-1"},
                {"chapter_id": "chapter-2", "error": "boom"}
            ])
        );
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

        let _ =
            BatchGenerationFollowUpAnalysisPlan::from_generated_result(&GeneratedChapterResult {
                chapter_id: "chapter-3".to_string(),
                chapter_number: 3,
                title: "第三章".to_string(),
                content: "正文".to_string(),
                word_count: 2,
            })
            .execute(
                &sea_orm::DatabaseConnection::Disconnected,
                "task-1",
                &session,
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
