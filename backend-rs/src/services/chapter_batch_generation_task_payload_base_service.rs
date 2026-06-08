use chrono::NaiveDateTime;
use serde_json::{json, Map, Value};

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_candidate_runtime_state_service::insert_python_query_snapshot_candidate_runtime_fields;
use crate::services::chapter_generation_quality_gate_semantics_service::{
    manual_review_label, manual_review_label_from_quality_context_with_retry_budget,
    retryable_repair_label, retryable_repair_label_from_quality_context_with_retry_budget,
};
use crate::services::chapter_generation_quality_runtime_context_service::{
    apply_batch_quality_runtime_context_to_payload,
    apply_generation_quality_runtime_context_to_payload,
    resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state,
    BatchGenerationQualityRuntimeContext, GenerationQualityRuntimeContext,
};
use crate::services::chapter_generation_task_semantics_service::task_type;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BatchGenerationQualityStatusContext {
    pub latest_quality_metrics: Option<Value>,
    pub quality_metrics_history: Option<Value>,
    pub quality_metrics_summary_state: Option<Value>,
    pub quality_metrics_summary: Option<Value>,
    pub quality_history_context: Option<Value>,
    pub active_story_repair_payload: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationFailedTerminalKind {
    ManualReview,
    Retry,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationFailedTerminalSemantics {
    pub(crate) kind: BatchGenerationFailedTerminalKind,
    pub(crate) reason: &'static str,
    pub(crate) label: String,
    pub(crate) review_required: bool,
    pub(crate) can_resume: bool,
}

impl BatchGenerationQualityStatusContext {
    pub fn from_snapshot_and_runtime_state(
        snapshot: Option<&batch_generation_snapshot::Model>,
        workflow_runtime_state: Option<&Value>,
    ) -> Self {
        let active_story_repair_payload =
            Self::active_story_repair_payload_from_runtime_state(workflow_runtime_state);
        let quality_runtime_context =
            resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state(
                snapshot,
                workflow_runtime_state,
            );

        Self::from_runtime_quality_context_and_active_payload(
            &quality_runtime_context,
            active_story_repair_payload.as_ref(),
        )
    }

    pub fn insert_into_payload(&self, payload: &mut Map<String, Value>) {
        payload.insert(
            "latest_quality_metrics".to_string(),
            serde_json::json!(self.latest_quality_metrics),
        );
        payload.insert(
            "quality_metrics_history".to_string(),
            serde_json::json!(self.quality_metrics_history),
        );
        payload.insert(
            "quality_metrics_summary_state".to_string(),
            serde_json::json!(self.quality_metrics_summary_state),
        );
        payload.insert(
            "quality_metrics_summary".to_string(),
            serde_json::json!(self.quality_metrics_summary),
        );
        payload.insert(
            "quality_history_context".to_string(),
            serde_json::json!(self.quality_history_context),
        );
        payload.insert(
            "active_story_repair_payload".to_string(),
            serde_json::json!(self.active_story_repair_payload),
        );
    }

    pub fn from_runtime_quality_context_and_active_payload(
        quality_runtime_context: &BatchGenerationQualityRuntimeContext,
        active_story_repair_payload: Option<&Value>,
    ) -> Self {
        Self {
            latest_quality_metrics: quality_runtime_context.latest_quality_metrics.clone(),
            quality_metrics_history: quality_runtime_context.quality_metrics_history.clone(),
            quality_metrics_summary_state: quality_runtime_context
                .quality_metrics_summary_state
                .clone(),
            quality_metrics_summary: quality_runtime_context.quality_metrics_summary.clone(),
            quality_history_context: quality_runtime_context.quality_history_context.clone(),
            active_story_repair_payload: active_story_repair_payload.cloned(),
        }
    }

    fn active_story_repair_payload_from_runtime_state(
        workflow_runtime_state: Option<&Value>,
    ) -> Option<Value> {
        workflow_runtime_state
            .and_then(Value::as_object)
            .and_then(|state| state.get("active_story_repair_payload"))
            .filter(|payload| payload.is_object())
            .cloned()
    }
}

pub(crate) fn insert_batch_generation_terminal_status_payload(
    payload: &mut Map<String, Value>,
    task: &batch_generation_task::Model,
    failed_chapters: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
) {
    let (terminal_reason, terminal_label, review_required, can_resume) = if task.status == "failed"
    {
        resolve_failed_terminal_semantics(task, failed_chapters, quality_status_context)
            .map(|semantics| match semantics.kind {
                BatchGenerationFailedTerminalKind::ManualReview => (
                    Some(semantics.reason),
                    Some(semantics.label),
                    semantics.review_required,
                    semantics.can_resume,
                ),
                BatchGenerationFailedTerminalKind::Retry
                | BatchGenerationFailedTerminalKind::Error => {
                    (Some("error"), Some("执行失败".to_string()), false, true)
                }
            })
            .unwrap_or((Some("error"), Some("执行失败".to_string()), false, true))
    } else {
        match task.status.as_str() {
            "completed" => (Some("completed"), Some("已完成".to_string()), false, false),
            "cancelled" => (Some("cancelled"), Some("已取消".to_string()), false, true),
            _ => (None, None, false, false),
        }
    };

    payload.insert(
        "terminal_reason".to_string(),
        serde_json::json!(terminal_reason),
    );
    payload.insert(
        "terminal_label".to_string(),
        serde_json::json!(terminal_label),
    );
    payload.insert(
        "review_required".to_string(),
        serde_json::json!(review_required),
    );
    payload.insert("can_resume".to_string(), serde_json::json!(can_resume));
}

pub(crate) fn resolve_failed_terminal_semantics(
    task: &batch_generation_task::Model,
    failed_chapters: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
) -> Option<BatchGenerationFailedTerminalSemantics> {
    resolve_failed_terminal_semantics_from_sources(
        failed_chapters,
        quality_status_context,
        task.current_retry_count,
        task.max_retries,
    )
}

pub(crate) fn resolve_failed_terminal_semantics_from_sources(
    failed_chapters: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<BatchGenerationFailedTerminalSemantics> {
    if let Some(label) = manual_review_label(failed_chapters).or_else(|| {
        quality_status_context.and_then(|context| {
            manual_review_label_from_quality_context_with_retry_budget(
                context.active_story_repair_payload.as_ref(),
                context.quality_metrics_summary.as_ref(),
                context.latest_quality_metrics.as_ref(),
                current_retry_count,
                max_retries,
            )
        })
    }) {
        return Some(BatchGenerationFailedTerminalSemantics {
            kind: BatchGenerationFailedTerminalKind::ManualReview,
            reason: "manual_review",
            label,
            review_required: true,
            can_resume: false,
        });
    }

    if let Some(label) = retryable_repair_label(failed_chapters, current_retry_count, max_retries)
        .or_else(|| {
            quality_status_context.and_then(|context| {
                retryable_repair_label_from_quality_context_with_retry_budget(
                    context.active_story_repair_payload.as_ref(),
                    context.quality_metrics_summary.as_ref(),
                    context.latest_quality_metrics.as_ref(),
                    current_retry_count,
                    max_retries,
                )
            })
        })
    {
        return Some(BatchGenerationFailedTerminalSemantics {
            kind: BatchGenerationFailedTerminalKind::Retry,
            reason: "retry",
            label,
            review_required: false,
            can_resume: true,
        });
    }

    Some(BatchGenerationFailedTerminalSemantics {
        kind: BatchGenerationFailedTerminalKind::Error,
        reason: "error",
        label: "执行失败".to_string(),
        review_required: false,
        can_resume: true,
    })
}

pub(crate) fn batch_generation_stage_code(status: &str) -> &'static str {
    match status {
        "completed" => "6.writing.completed",
        "failed" => "6.writing.failed",
        "cancelled" => "6.writing.cancelled",
        "running" => "6.writing.generating",
        _ => "6.writing.pending",
    }
}

pub(crate) fn task_execution_mode() -> &'static str {
    "interactive"
}

pub(crate) fn to_iso(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.and_utc().to_rfc3339())
}

pub(crate) fn checkpoint_with_runtime_metadata(
    workflow_runtime_state: Option<&Value>,
    stage_code: &str,
    execution_mode: &str,
) -> Map<String, Value> {
    let mut checkpoint = workflow_runtime_state
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    checkpoint.insert("stage_code".to_string(), json!(stage_code));
    checkpoint.insert("execution_mode".to_string(), json!(execution_mode));
    checkpoint
}

fn checkpoint_with_task_metadata(
    workflow_runtime_state: Option<&Value>,
    task: &batch_generation_task::Model,
    stage_code: &str,
    execution_mode: &str,
    progress_phase: &str,
) -> Map<String, Value> {
    let mut checkpoint =
        checkpoint_with_runtime_metadata(workflow_runtime_state, stage_code, execution_mode);
    let progress = resolve_batch_checkpoint_progress(&checkpoint, task);

    checkpoint.insert(
        "current_chapter_id".to_string(),
        json!(task.current_chapter_id.clone()),
    );
    checkpoint.insert(
        "current_chapter_number".to_string(),
        json!(task.current_chapter_number),
    );
    checkpoint.insert(
        "current_retry_count".to_string(),
        json!(task.current_retry_count),
    );
    checkpoint.insert("max_retries".to_string(), json!(task.max_retries));
    checkpoint.insert("progress_phase".to_string(), json!(progress_phase));
    checkpoint.insert("progress".to_string(), json!(progress));
    insert_python_query_snapshot_runtime_fields(&mut checkpoint);
    checkpoint
}

fn resolve_batch_progress_phase(
    workflow_runtime_state: Option<&Value>,
    task: &batch_generation_task::Model,
) -> String {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("phase"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase)
        .unwrap_or_else(|| default_batch_progress_phase(task).to_string())
}

fn compose_python_query_snapshot_stage_code(progress_phase: &str) -> String {
    if progress_phase.is_empty() || progress_phase == "init" {
        "6.writing".to_string()
    } else {
        format!("6.writing.{progress_phase}")
    }
}

fn default_batch_progress_phase(task: &batch_generation_task::Model) -> &'static str {
    match task.status.as_str() {
        "pending" => "init",
        "completed" => "complete",
        "failed" => "failed",
        "cancelled" => "cancelled",
        _ if task.current_retry_count > 0 => "generating",
        _ if task.current_chapter_number.is_some() => "generating",
        _ => "loading",
    }
}

fn resolve_batch_checkpoint_progress(
    checkpoint: &Map<String, Value>,
    task: &batch_generation_task::Model,
) -> i32 {
    let progress = checkpoint
        .get("progress")
        .and_then(Value::as_i64)
        .map(|value| value as i32)
        .unwrap_or_else(|| fallback_batch_checkpoint_progress(task));

    progress.clamp(0, 100)
}

fn fallback_batch_checkpoint_progress(task: &batch_generation_task::Model) -> i32 {
    if task.status == "completed" {
        return 100;
    }

    let completed = task.completed_chapters.max(0);
    let total = task.total_chapters.max(1);
    ((completed as f64 / total as f64) * 100.0) as i32
}

fn insert_python_query_snapshot_runtime_fields(checkpoint: &mut Map<String, Value>) {
    const RAW_FIELDS: [&str; 4] = [
        "last_event",
        "last_message",
        "pre_compaction_total_length",
        "context_budget_limit",
    ];
    const BOOL_FIELDS: [&str; 1] = ["compaction_applied"];

    insert_python_query_snapshot_candidate_runtime_fields(checkpoint);

    for key in RAW_FIELDS {
        checkpoint
            .entry(key.to_string())
            .or_insert_with(|| Value::Null);
    }
    for key in BOOL_FIELDS {
        let value = checkpoint
            .get(key)
            .and_then(Value::as_bool)
            .map(Value::Bool)
            .unwrap_or(Value::Null);
        checkpoint.insert(key.to_string(), value);
    }

    let compaction_details = checkpoint
        .get("compaction_details")
        .and_then(Value::as_object)
        .cloned()
        .map(Value::Object)
        .unwrap_or(Value::Null);
    checkpoint.insert("compaction_details".to_string(), compaction_details);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationCommandProgressSummary {
    pub(crate) batch_id: String,
    pub(crate) total_chapters: i32,
    pub(crate) completed_chapters: i32,
}

impl BatchGenerationCommandProgressSummary {
    pub(crate) fn batch_id(&self) -> &str {
        &self.batch_id
    }
}

pub(crate) fn build_batch_generation_command_summary_payload(
    progress: BatchGenerationCommandProgressSummary,
    message: impl Into<String>,
) -> Value {
    let mut payload = Map::new();
    payload.insert("total_chapters".to_string(), json!(progress.total_chapters));
    payload.insert(
        "completed_chapters".to_string(),
        json!(progress.completed_chapters),
    );
    payload.insert("batch_id".to_string(), json!(progress.batch_id));
    payload.insert("message".to_string(), json!(message.into()));
    Value::Object(payload)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BatchGenerationTaskResponseQualityPayload {
    Batch {
        quality_runtime_context: BatchGenerationQualityRuntimeContext,
        quality_metrics_summary: Option<Value>,
    },
    Single {
        quality_runtime_context: GenerationQualityRuntimeContext,
        latest_quality_metrics: Option<Value>,
        quality_metrics_summary: Option<Value>,
        quality_metrics_history: Option<Value>,
    },
}

#[derive(Debug, Default)]
pub(crate) struct BatchGenerationTaskResponsePayloadOptions {
    pub(crate) checkpoint_override: Option<(String, Value)>,
    pub(crate) summary_payload: Option<Value>,
    pub(crate) quality_payload: Option<BatchGenerationTaskResponseQualityPayload>,
    pub(crate) active_story_repair_payload: Option<Value>,
    pub(crate) quality_history_context: Option<Value>,
    pub(crate) extra_fields: Vec<(String, Value)>,
    pub(crate) apply_loading_stage_fields: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationTaskViewPayloadVariant {
    ActiveTaskListItem,
    ActiveProjectTask,
    StatusTask,
}

pub(crate) fn estimated_task_minutes(
    total_chapters: usize,
    target_word_count: i32,
    enable_analysis: bool,
) -> i32 {
    let generation_time_per_chapter = (target_word_count as f64 / 3000.0) * 2.0;
    let analysis_time_per_chapter = if enable_analysis { 1.0 } else { 0.0 };
    let total_time =
        total_chapters as f64 * (generation_time_per_chapter + analysis_time_per_chapter);

    (total_time as i32).max(1)
}

pub(crate) fn build_batch_generation_task_runtime_payload(
    batch_id: impl Into<String>,
    task_type: impl Into<String>,
    project_id: impl Into<String>,
    status: impl Into<String>,
    current_chapter_id: Option<&str>,
    created_at: Option<NaiveDateTime>,
    checkpoint: Map<String, Value>,
    stage_code: impl Into<String>,
    execution_mode: impl Into<String>,
) -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert("batch_id".to_string(), json!(batch_id.into()));
    payload.insert("task_type".to_string(), json!(task_type.into()));
    payload.insert("project_id".to_string(), json!(project_id.into()));
    let status = status.into();
    payload.insert("status".to_string(), json!(status));
    payload.insert("stage_code".to_string(), json!(stage_code.into()));
    payload.insert("execution_mode".to_string(), json!(execution_mode.into()));
    payload.insert(
        "current_chapter_id".to_string(),
        json!(current_chapter_id.map(str::to_string)),
    );
    payload.insert("checkpoint".to_string(), Value::Object(checkpoint));
    payload.insert("created_at".to_string(), json!(to_iso(created_at)));
    payload
}

pub(crate) fn apply_batch_generation_loading_stage_fields(payload: &mut Map<String, Value>) {
    payload.insert("stage_code".to_string(), json!("6.writing.loading"));
    if let Some(checkpoint) = payload.get_mut("checkpoint").and_then(Value::as_object_mut) {
        checkpoint.insert("stage_code".to_string(), json!("6.writing.loading"));
        checkpoint.insert("progress_phase".to_string(), json!("loading"));
    }
}

pub(crate) fn build_batch_generation_task_response_payload_from_runtime_parts(
    batch_id: impl Into<String>,
    task_type: impl Into<String>,
    project_id: impl Into<String>,
    status: impl Into<String>,
    current_chapter_id: Option<&str>,
    created_at: Option<NaiveDateTime>,
    workflow_runtime_state: Option<&Value>,
    options: BatchGenerationTaskResponsePayloadOptions,
) -> Map<String, Value> {
    let batch_id = batch_id.into();
    let task_type = task_type.into();
    let project_id = project_id.into();
    let status = status.into();
    let stage_code = batch_generation_stage_code(&status);
    let execution_mode = task_execution_mode();
    let mut checkpoint =
        checkpoint_with_runtime_metadata(workflow_runtime_state, stage_code, execution_mode);

    if let Some((key, value)) = options.checkpoint_override {
        checkpoint.insert(key, value);
    }

    let mut payload = build_batch_generation_task_runtime_payload(
        batch_id,
        task_type,
        project_id,
        status,
        current_chapter_id,
        created_at,
        checkpoint,
        stage_code,
        execution_mode,
    );

    if let Some(summary_payload) = options.summary_payload {
        if let Value::Object(summary_fields) = summary_payload {
            payload.extend(summary_fields);
        }
    }

    if let Some(quality_payload) = options.quality_payload {
        match quality_payload {
            BatchGenerationTaskResponseQualityPayload::Batch {
                quality_runtime_context,
                quality_metrics_summary,
            } => apply_batch_quality_runtime_context_to_payload(
                &mut payload,
                quality_runtime_context,
                quality_metrics_summary,
            ),
            BatchGenerationTaskResponseQualityPayload::Single {
                quality_runtime_context,
                latest_quality_metrics,
                quality_metrics_summary,
                quality_metrics_history,
            } => apply_generation_quality_runtime_context_to_payload(
                &mut payload,
                quality_runtime_context,
                latest_quality_metrics,
                quality_metrics_summary,
                quality_metrics_history,
            ),
        }
    }

    if let Some(active_story_repair_payload) = options.active_story_repair_payload {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }
    if let Some(quality_history_context) = options.quality_history_context {
        payload.insert(
            "quality_history_context".to_string(),
            quality_history_context,
        );
    }
    for (key, value) in options.extra_fields {
        payload.insert(key, value);
    }
    if options.apply_loading_stage_fields {
        apply_batch_generation_loading_stage_fields(&mut payload);
    }

    payload
}

pub(crate) fn build_batch_generation_task_runtime_payload_from_runtime_parts(
    batch_id: impl Into<String>,
    task_type: impl Into<String>,
    project_id: impl Into<String>,
    status: impl Into<String>,
    current_chapter_id: Option<&str>,
    created_at: Option<NaiveDateTime>,
    workflow_runtime_state: Option<&Value>,
    checkpoint_override: Option<(&str, Value)>,
) -> Map<String, Value> {
    build_batch_generation_task_response_payload_from_runtime_parts(
        batch_id,
        task_type,
        project_id,
        status,
        current_chapter_id,
        created_at,
        workflow_runtime_state,
        BatchGenerationTaskResponsePayloadOptions {
            checkpoint_override: checkpoint_override.map(|(key, value)| (key.to_string(), value)),
            ..Default::default()
        },
    )
}

pub(crate) fn build_batch_generation_task_runtime_payload_from_task_state(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
) -> Map<String, Value> {
    let progress_phase = resolve_batch_progress_phase(workflow_runtime_state, task);
    let stage_code = compose_python_query_snapshot_stage_code(&progress_phase);
    let execution_mode = task_execution_mode();
    let checkpoint = checkpoint_with_task_metadata(
        workflow_runtime_state,
        task,
        &stage_code,
        execution_mode,
        &progress_phase,
    );

    build_batch_generation_task_runtime_payload(
        &task.id,
        task_type(task),
        &task.project_id,
        &task.status,
        task.current_chapter_id.as_deref(),
        task.created_at,
        checkpoint,
        stage_code,
        execution_mode,
    )
}

pub(crate) fn build_batch_generation_task_view_payload_from_task_state(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
) -> Map<String, Value> {
    let mut payload =
        build_batch_generation_task_runtime_payload_from_task_state(task, workflow_runtime_state);

    payload.insert("total".to_string(), json!(task.total_chapters));
    payload.insert("completed".to_string(), json!(task.completed_chapters));
    payload.insert(
        "current_chapter_number".to_string(),
        json!(task.current_chapter_number),
    );
    payload.insert("started_at".to_string(), json!(to_iso(task.started_at)));
    payload.insert("completed_at".to_string(), json!(to_iso(task.completed_at)));
    payload.insert("error_message".to_string(), json!(task.error_message));

    payload
}

pub(crate) fn build_batch_generation_task_view_payload_with_quality_context(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
    variant: BatchGenerationTaskViewPayloadVariant,
) -> Map<String, Value> {
    let mut payload =
        build_batch_generation_task_view_payload_from_task_state(task, workflow_runtime_state);

    if let Some(quality_status_context) = quality_status_context {
        quality_status_context.insert_into_payload(&mut payload);
    }

    match variant {
        BatchGenerationTaskViewPayloadVariant::ActiveTaskListItem => {}
        BatchGenerationTaskViewPayloadVariant::ActiveProjectTask => {
            payload.remove("task_type");
            payload.remove("project_id");
            payload.remove("completed_at");
            payload.remove("error_message");
        }
        BatchGenerationTaskViewPayloadVariant::StatusTask => {
            payload.insert(
                "current_retry_count".to_string(),
                json!(task.current_retry_count),
            );
            payload.insert("max_retries".to_string(), json!(task.max_retries));
            payload.insert("failed_chapters".to_string(), task.failed_chapters.clone());
            insert_batch_generation_terminal_status_payload(
                &mut payload,
                task,
                Some(&task.failed_chapters),
                quality_status_context,
            );
        }
    }

    payload
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        apply_batch_generation_loading_stage_fields, batch_generation_stage_code,
        build_batch_generation_command_summary_payload,
        build_batch_generation_task_response_payload_from_runtime_parts,
        build_batch_generation_task_runtime_payload,
        build_batch_generation_task_runtime_payload_from_runtime_parts,
        build_batch_generation_task_runtime_payload_from_task_state,
        build_batch_generation_task_view_payload_from_task_state,
        build_batch_generation_task_view_payload_with_quality_context,
        checkpoint_with_runtime_metadata, insert_batch_generation_terminal_status_payload,
        resolve_failed_terminal_semantics, resolve_failed_terminal_semantics_from_sources,
        task_execution_mode, BatchGenerationCommandProgressSummary,
        BatchGenerationFailedTerminalKind, BatchGenerationQualityStatusContext,
        BatchGenerationTaskResponsePayloadOptions, BatchGenerationTaskResponseQualityPayload,
        BatchGenerationTaskViewPayloadVariant,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_generation_quality_runtime_context_service::BatchGenerationQualityRuntimeContext;
    use crate::services::chapter_generation_task_semantics_service::task_type;

    fn build_task_shape(
        status: &str,
        chapter_count: i32,
        chapter_ids: Value,
        total_chapters: i32,
    ) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count,
            chapter_ids,
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 2,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn build_task(status: &str) -> batch_generation_task::Model {
        build_task_shape(status, 1, json!(["chapter-1"]), 2)
    }

    fn snapshot_with_quality_fields() -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            workflow_runtime_state: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_resolve_batch_generation_stage_code() {
        let cases = [
            ("completed", "6.writing.completed"),
            ("failed", "6.writing.failed"),
            ("cancelled", "6.writing.cancelled"),
            ("running", "6.writing.generating"),
            ("pending", "6.writing.pending"),
            ("unknown", "6.writing.pending"),
        ];

        for (status, expected) in cases {
            assert_eq!(batch_generation_stage_code(status), expected);
        }
    }

    #[test]
    fn should_keep_batch_generation_execution_mode_interactive() {
        let single = build_task_shape("running", 1, json!(["chapter-1"]), 1);
        let batch = build_task_shape("running", 2, json!(["chapter-1", "chapter-2"]), 2);
        let malformed_single =
            build_task_shape("running", 1, json!({"chapter_id": "chapter-1"}), 1);

        assert_eq!(task_execution_mode(), "interactive");
        assert_eq!(task_execution_mode(), "interactive");
        assert_eq!(task_execution_mode(), "interactive");

        assert_eq!(single.chapter_count, 1);
        assert_eq!(batch.chapter_count, 2);
        assert_eq!(malformed_single.chapter_count, 1);
    }

    #[test]
    fn should_build_checkpoint_with_runtime_metadata_without_runtime_state() {
        let checkpoint = checkpoint_with_runtime_metadata(None, "6.writing.pending", "batch");

        assert_eq!(checkpoint["stage_code"], "6.writing.pending");
        assert_eq!(checkpoint["execution_mode"], "batch");
    }

    #[test]
    fn should_preserve_checkpoint_fields_and_override_runtime_metadata() {
        let runtime_state = json!({
            "progress": 42,
            "stage_code": "stale-stage",
            "execution_mode": "stale-mode"
        });

        let checkpoint =
            checkpoint_with_runtime_metadata(Some(&runtime_state), "6.writing.completed", "single");

        assert_eq!(checkpoint["progress"], 42);
        assert_eq!(checkpoint["stage_code"], "6.writing.completed");
        assert_eq!(checkpoint["execution_mode"], "single");
    }

    #[test]
    fn should_build_task_checkpoint_with_python_compatible_runtime_fields() {
        let task = build_task("running");
        let payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&json!({
                "phase": "generating",
                "progress": 42,
                "last_event": "progress",
            })),
        );

        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["progress_phase"], "generating");
        assert_eq!(payload["checkpoint"]["current_chapter_id"], "chapter-1");
        assert_eq!(payload["checkpoint"]["current_chapter_number"], 1);
        assert_eq!(payload["checkpoint"]["current_retry_count"], 2);
        assert_eq!(payload["checkpoint"]["max_retries"], 3);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.generating");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
    }

    #[test]
    fn should_build_stage_code_from_runtime_phase_like_python_query_snapshot() {
        let task = build_task("running");
        let payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&json!({
                "phase": "  PARSING  ",
                "progress": 10
            })),
        );

        assert_eq!(payload["stage_code"], "6.writing.parsing");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.parsing");
        assert_eq!(payload["checkpoint"]["progress_phase"], "parsing");
    }

    #[test]
    fn should_use_python_base_stage_code_for_init_progress_phase() {
        let mut task = build_task("pending");
        task.current_retry_count = 0;
        task.current_chapter_number = None;
        let payload = build_batch_generation_task_runtime_payload_from_task_state(&task, None);

        assert_eq!(payload["stage_code"], "6.writing");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing");
        assert_eq!(payload["checkpoint"]["progress_phase"], "init");
    }

    #[test]
    fn should_fallback_checkpoint_progress_phase_from_task_status() {
        let task = build_task("cancelled");
        let payload = build_batch_generation_task_runtime_payload_from_task_state(&task, None);

        assert_eq!(payload["checkpoint"]["progress_phase"], "cancelled");
        assert_eq!(payload["checkpoint"]["current_chapter_id"], "chapter-1");
        assert_eq!(payload["checkpoint"]["current_chapter_number"], 1);
        assert_eq!(payload["checkpoint"]["current_retry_count"], 2);
        assert_eq!(payload["checkpoint"]["max_retries"], 3);
    }

    #[test]
    fn should_fallback_checkpoint_progress_like_python_query_snapshot() {
        let mut running = build_task("running");
        running.completed_chapters = 1;
        running.total_chapters = 4;
        let running_payload =
            build_batch_generation_task_runtime_payload_from_task_state(&running, None);

        assert_eq!(running_payload["checkpoint"]["progress"], 25);
        assert_eq!(
            running_payload["checkpoint"]["progress_phase"],
            "generating"
        );

        let mut completed = build_task("completed");
        completed.completed_chapters = 1;
        completed.total_chapters = 4;
        let completed_payload =
            build_batch_generation_task_runtime_payload_from_task_state(&completed, None);

        assert_eq!(completed_payload["checkpoint"]["progress"], 100);
        assert_eq!(
            completed_payload["checkpoint"]["progress_phase"],
            "complete"
        );
    }

    #[test]
    fn should_clamp_checkpoint_progress_from_runtime_state() {
        let task = build_task("running");
        let high_payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&json!({"progress": 120})),
        );
        let low_payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&json!({"progress": -5})),
        );

        assert_eq!(high_payload["checkpoint"]["progress"], 100);
        assert_eq!(low_payload["checkpoint"]["progress"], 0);
    }

    #[test]
    fn should_fallback_checkpoint_progress_phase_like_python_query_snapshot() {
        let mut pending = build_task("pending");
        pending.current_retry_count = 0;
        pending.current_chapter_number = None;
        let pending_payload =
            build_batch_generation_task_runtime_payload_from_task_state(&pending, None);

        assert_eq!(pending_payload["checkpoint"]["progress_phase"], "init");

        let mut loading = build_task("running");
        loading.current_retry_count = 0;
        loading.current_chapter_number = None;
        let loading_payload =
            build_batch_generation_task_runtime_payload_from_task_state(&loading, None);

        assert_eq!(loading_payload["checkpoint"]["progress_phase"], "loading");
    }

    #[test]
    fn should_insert_python_query_snapshot_runtime_diagnostic_fields() {
        let task = build_task("running");
        let payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&json!({
                "phase": "generating",
                "progress": 42,
                "last_event": "progress",
                "candidate_index": 1,
                "rerank_used": true,
                "word_budget_repair_used": "not-a-bool",
                "compaction_applied": false,
                "compaction_details": {"method": "summary"}
            })),
        );
        let checkpoint = &payload["checkpoint"];

        assert_eq!(checkpoint["last_event"], "progress");
        assert_eq!(checkpoint["last_message"], Value::Null);
        assert_eq!(checkpoint["candidate_index"], 1);
        assert_eq!(checkpoint["candidate_count"], Value::Null);
        assert_eq!(checkpoint["word_count"], Value::Null);
        assert_eq!(checkpoint["generation_path"], Value::Null);
        assert_eq!(checkpoint["attempt_kind"], Value::Null);
        assert_eq!(checkpoint["rerank_used"], true);
        assert_eq!(checkpoint["word_budget_repair_used"], Value::Null);
        assert_eq!(checkpoint["winner_candidate_index"], Value::Null);
        assert_eq!(checkpoint["pre_compaction_total_length"], Value::Null);
        assert_eq!(checkpoint["context_budget_limit"], Value::Null);
        assert_eq!(checkpoint["compaction_applied"], false);
        assert_eq!(checkpoint["compaction_details"]["method"], "summary");
    }

    #[test]
    fn should_null_non_object_compaction_details_like_python_query_snapshot() {
        let task = build_task("running");
        let payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&json!({
                "compaction_details": "not-an-object",
                "rerank_used": "not-a-bool",
                "word_budget_repair_used": 1,
                "compaction_applied": {}
            })),
        );
        let checkpoint = &payload["checkpoint"];

        assert_eq!(checkpoint["compaction_details"], Value::Null);
        assert_eq!(checkpoint["rerank_used"], Value::Null);
        assert_eq!(checkpoint["word_budget_repair_used"], Value::Null);
        assert_eq!(checkpoint["compaction_applied"], Value::Null);
    }

    #[test]
    fn should_build_batch_generation_command_summary_payload() {
        let payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: "task-3".to_string(),
                total_chapters: 5,
                completed_chapters: 2,
            },
            "Batch generation cancelled",
        );

        assert_eq!(payload["batch_id"], "task-3");
        assert_eq!(payload["total_chapters"], 5);
        assert_eq!(payload["completed_chapters"], 2);
        assert_eq!(payload["message"], "Batch generation cancelled");
    }

    #[test]
    fn should_build_batch_generation_command_progress_summary() {
        let payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: "task-7".to_string(),
                total_chapters: 6,
                completed_chapters: 4,
            },
            "Batch generation completed",
        );

        assert_eq!(payload["batch_id"], "task-7");
        assert_eq!(payload["total_chapters"], 6);
        assert_eq!(payload["completed_chapters"], 4);
        assert_eq!(payload["message"], "Batch generation completed");
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload() {
        let task = build_task("running");
        let checkpoint = checkpoint_with_runtime_metadata(
            Some(&json!({"progress": 42})),
            "6.writing.generating",
            "interactive",
        );

        let payload = build_batch_generation_task_runtime_payload(
            &task.id,
            task_type(&task),
            &task.project_id,
            &task.status,
            task.current_chapter_id.as_deref(),
            task.created_at,
            checkpoint,
            "6.writing.generating",
            "interactive",
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["stage_code"], "6.writing.generating");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["current_chapter_id"], "chapter-1");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.generating");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload_from_parts() {
        let checkpoint = checkpoint_with_runtime_metadata(
            Some(&json!({"progress": 42})),
            "6.writing.pending",
            "interactive",
        );

        let payload = build_batch_generation_task_runtime_payload(
            "task-9",
            "chapters_batch_generate",
            "project-9",
            "pending",
            Some("chapter-7"),
            None,
            checkpoint,
            "6.writing.pending",
            "interactive",
        );

        assert_eq!(payload["batch_id"], "task-9");
        assert_eq!(payload["task_type"], "chapters_batch_generate");
        assert_eq!(payload["project_id"], "project-9");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["current_chapter_id"], "chapter-7");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload_from_runtime_parts() {
        let payload = build_batch_generation_task_runtime_payload_from_runtime_parts(
            "task-9",
            "chapters_batch_generate",
            "project-9",
            "pending",
            Some("chapter-7"),
            None,
            Some(&json!({"progress": 42})),
            Some(("chapter_id", json!("chapter-8"))),
        );

        assert_eq!(payload["batch_id"], "task-9");
        assert_eq!(payload["task_type"], "chapters_batch_generate");
        assert_eq!(payload["project_id"], "project-9");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["current_chapter_id"], "chapter-7");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-8");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload_from_task_state() {
        let task = build_task("running");

        let payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&json!({"progress": 42})),
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["stage_code"], "6.writing.generating");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.generating");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
    }

    #[test]
    fn should_build_batch_generation_task_view_payload_from_task_state() {
        let mut task = build_task("running");
        task.started_at = Some(
            chrono::NaiveDate::from_ymd_opt(2026, 5, 21)
                .expect("valid date")
                .and_hms_opt(9, 30, 0)
                .expect("valid time"),
        );
        task.completed_at = Some(
            chrono::NaiveDate::from_ymd_opt(2026, 5, 21)
                .expect("valid date")
                .and_hms_opt(10, 30, 0)
                .expect("valid time"),
        );
        task.error_message = Some("boom".to_string());

        let payload = build_batch_generation_task_view_payload_from_task_state(
            &task,
            Some(&json!({"progress": 42})),
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["total"], 2);
        assert_eq!(payload["completed"], 1);
        assert_eq!(payload["current_chapter_number"], 1);
        assert_eq!(payload["started_at"], "2026-05-21T09:30:00+00:00");
        assert_eq!(payload["completed_at"], "2026-05-21T10:30:00+00:00");
        assert_eq!(payload["error_message"], "boom");
        assert_eq!(payload["checkpoint"]["progress"], 42);
    }

    #[test]
    fn should_build_status_task_view_payload_with_shared_owner_variant() {
        let task = build_task("completed");
        let runtime_state = json!({
            "progress": 60,
            "active_story_repair_payload": {
                "mode": "repair"
            }
        });
        let quality_status_context = BatchGenerationQualityStatusContext {
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary_state: Some(json!({"scope": "batch"})),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            quality_history_context: None,
            active_story_repair_payload: Some(json!({"mode": "repair"})),
        };

        let payload = build_batch_generation_task_view_payload_with_quality_context(
            &task,
            Some(&runtime_state),
            Some(&quality_status_context),
            BatchGenerationTaskViewPayloadVariant::StatusTask,
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["current_retry_count"], 2);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(payload["failed_chapters"], json!([]));
        assert_eq!(payload["terminal_reason"], "completed");
        assert_eq!(payload["review_required"], false);
        assert_eq!(payload["can_resume"], false);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload_from_runtime_parts_with_checkpoint_override(
    ) {
        let task = build_task("running");

        let payload = build_batch_generation_task_runtime_payload_from_runtime_parts(
            &task.id,
            task_type(&task),
            &task.project_id,
            &task.status,
            task.current_chapter_id.as_deref(),
            task.created_at,
            Some(&json!({"progress": 42})),
            Some(("chapter_id", json!("chapter-9"))),
        );

        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-9");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.generating");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
    }

    #[test]
    fn should_build_batch_generation_task_response_payload_with_shared_owner_fields() {
        let payload = build_batch_generation_task_response_payload_from_runtime_parts(
            "task-1",
            "chapters_batch_generate",
            "project-1",
            "pending",
            Some("chapter-1"),
            None,
            Some(&json!({
                "phase": "pending",
                "progress": 0
            })),
            BatchGenerationTaskResponsePayloadOptions {
                checkpoint_override: Some(("chapter_id".to_string(), json!("chapter-1"))),
                summary_payload: Some(build_batch_generation_command_summary_payload(
                    BatchGenerationCommandProgressSummary {
                        batch_id: "task-1".to_string(),
                        total_chapters: 2,
                        completed_chapters: 0,
                    },
                    "Task resumed and queued",
                )),
                quality_payload: Some(BatchGenerationTaskResponseQualityPayload::Batch {
                    quality_runtime_context: BatchGenerationQualityRuntimeContext {
                        quality_history_context: Some(json!({"scope": "batch"})),
                        ..Default::default()
                    },
                    quality_metrics_summary: Some(json!({"overall_score": 91})),
                }),
                active_story_repair_payload: Some(json!({"summary": "shared"})),
                quality_history_context: Some(json!({"scope": "batch"})),
                extra_fields: vec![("resumed_from_batch_id".to_string(), json!("task-1"))],
                ..Default::default()
            },
        );

        assert_eq!(payload["message"], "Task resumed and queued");
        assert_eq!(payload["completed_chapters"], 0);
        assert_eq!(payload["total_chapters"], 2);
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 91);
        assert_eq!(payload["active_story_repair_payload"]["summary"], "shared");
        assert_eq!(payload["quality_history_context"]["scope"], "batch");
        assert_eq!(payload["resumed_from_batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-1");
    }

    #[test]
    fn should_apply_shared_loading_stage_fields_for_response_payload() {
        let mut payload = build_batch_generation_task_runtime_payload_from_runtime_parts(
            "task-9",
            "chapters_batch_generate",
            "project-9",
            "pending",
            Some("chapter-7"),
            None,
            Some(&json!({"progress": 42})),
            Some(("chapter_id", json!("chapter-8"))),
        );

        apply_batch_generation_loading_stage_fields(&mut payload);

        assert_eq!(payload["stage_code"], "6.writing.loading");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.loading");
        assert_eq!(payload["checkpoint"]["progress_phase"], "loading");
    }

    #[test]
    fn should_extract_active_story_repair_payload_from_runtime_state() {
        let runtime_state = json!({
            "active_story_repair_payload": {
                "mode": "repair",
                "attempt": 2
            }
        });

        let payload =
            BatchGenerationQualityStatusContext::active_story_repair_payload_from_runtime_state(
                Some(&runtime_state),
            );

        assert_eq!(payload, Some(json!({"mode": "repair", "attempt": 2})));
    }

    #[test]
    fn should_ignore_non_object_active_story_repair_payload() {
        let runtime_state = json!({
            "active_story_repair_payload": "not-an-object"
        });

        assert_eq!(
            BatchGenerationQualityStatusContext::active_story_repair_payload_from_runtime_state(
                Some(&runtime_state),
            ),
            None
        );
        assert_eq!(
            BatchGenerationQualityStatusContext::active_story_repair_payload_from_runtime_state(
                None
            ),
            None
        );
    }

    #[test]
    fn should_build_quality_status_context_from_snapshot_and_runtime_state() {
        let snapshot = snapshot_with_quality_fields();
        let runtime_state = json!({
            "active_story_repair_payload": {
                "mode": "repair"
            }
        });

        let context = BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            Some(&snapshot),
            Some(&runtime_state),
        );

        assert_eq!(context.latest_quality_metrics, Some(json!({"score": 91})));
        assert_eq!(
            context.quality_metrics_history,
            Some(json!([{"score": 90}]))
        );
        assert_eq!(
            context.quality_metrics_summary,
            Some(json!({"summary": "ok"}))
        );
        assert_eq!(
            context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("scope")),
            Some(&json!("batch"))
        );
        assert_eq!(
            context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(1))
        );
        assert_eq!(context.quality_history_context, None);
        assert_eq!(
            context.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_terminal_status_payload_for_completed_cancelled_and_default_tasks() {
        let mut completed = batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: "completed".to_string(),
            total_chapters: 2,
            completed_chapters: 2,
            failed_chapters: json!([]),
            current_chapter_id: None,
            current_chapter_number: None,
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let cancelled = batch_generation_task::Model {
            status: "cancelled".to_string(),
            ..completed.clone()
        };
        let pending = batch_generation_task::Model {
            status: "pending".to_string(),
            ..completed.clone()
        };

        let mut completed_payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut completed_payload,
            &completed,
            None,
            None,
        );
        assert_eq!(completed_payload["terminal_reason"], "completed");
        assert_eq!(completed_payload["terminal_label"], "已完成");
        assert_eq!(completed_payload["review_required"], false);
        assert_eq!(completed_payload["can_resume"], false);

        let mut cancelled_payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut cancelled_payload,
            &cancelled,
            None,
            None,
        );
        assert_eq!(cancelled_payload["terminal_reason"], "cancelled");
        assert_eq!(cancelled_payload["terminal_label"], "已取消");
        assert_eq!(cancelled_payload["review_required"], false);
        assert_eq!(cancelled_payload["can_resume"], true);

        let mut pending_payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(&mut pending_payload, &pending, None, None);
        assert_eq!(pending_payload["terminal_reason"], Value::Null);
        assert_eq!(pending_payload["terminal_label"], Value::Null);
        assert_eq!(pending_payload["review_required"], false);
        assert_eq!(pending_payload["can_resume"], false);

        completed.status = "failed".to_string();
        completed.failed_chapters = json!([{
            "quality_gate_decision": "manual_review",
            "quality_gate_label": "待补充"
        }]);
        let mut manual_review_payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut manual_review_payload,
            &completed,
            Some(&completed.failed_chapters),
            None,
        );
        assert_eq!(manual_review_payload["terminal_reason"], "manual_review");
        assert_eq!(manual_review_payload["terminal_label"], "待补充");
        assert_eq!(manual_review_payload["review_required"], true);
        assert_eq!(manual_review_payload["can_resume"], false);
    }

    #[test]
    fn should_resolve_terminal_semantics_for_manual_review_failed_task() {
        let task = batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: "failed".to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([{
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "待补充"
            }]),
            current_chapter_id: None,
            current_chapter_number: None,
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };

        let semantics = resolve_failed_terminal_semantics(&task, Some(&task.failed_chapters), None)
            .expect("failed terminal semantics");

        assert_eq!(
            semantics.kind,
            BatchGenerationFailedTerminalKind::ManualReview
        );
        assert_eq!(semantics.reason, "manual_review");
        assert_eq!(semantics.label, "待补充");
        assert!(semantics.review_required);
        assert!(!semantics.can_resume);
    }

    #[test]
    fn should_resolve_terminal_semantics_from_quality_context_and_retry_budget() {
        let manual_review_context = BatchGenerationQualityStatusContext {
            latest_quality_metrics: Some(json!({
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "等待人工复核"
                }
            })),
            quality_metrics_history: None,
            quality_metrics_summary_state: None,
            quality_metrics_summary: None,
            quality_history_context: None,
            active_story_repair_payload: None,
        };
        let retry_context = BatchGenerationQualityStatusContext {
            latest_quality_metrics: Some(json!({
                "quality_gate": {
                    "decision": "auto_repair",
                    "label": "自动修复后重试"
                }
            })),
            quality_metrics_history: None,
            quality_metrics_summary_state: None,
            quality_metrics_summary: None,
            quality_history_context: None,
            active_story_repair_payload: None,
        };
        let exhausted_context = BatchGenerationQualityStatusContext {
            latest_quality_metrics: Some(json!({
                "quality_gate": {
                    "decision": "auto_repair",
                    "label": "自动修复预算已耗尽"
                }
            })),
            quality_metrics_history: None,
            quality_metrics_summary_state: None,
            quality_metrics_summary: None,
            quality_history_context: None,
            active_story_repair_payload: None,
        };

        let manual_review_semantics = resolve_failed_terminal_semantics_from_sources(
            Some(&json!([])),
            Some(&manual_review_context),
            0,
            3,
        )
        .expect("manual review semantics");
        assert_eq!(
            manual_review_semantics.kind,
            BatchGenerationFailedTerminalKind::ManualReview
        );
        assert_eq!(manual_review_semantics.reason, "manual_review");
        assert_eq!(manual_review_semantics.label, "等待人工复核");
        assert!(manual_review_semantics.review_required);
        assert!(!manual_review_semantics.can_resume);

        let retry_semantics = resolve_failed_terminal_semantics_from_sources(
            Some(&json!([])),
            Some(&retry_context),
            1,
            3,
        )
        .expect("retry semantics");
        assert_eq!(
            retry_semantics.kind,
            BatchGenerationFailedTerminalKind::Retry
        );
        assert_eq!(retry_semantics.reason, "retry");
        assert_eq!(retry_semantics.label, "自动修复后重试");
        assert!(!retry_semantics.review_required);
        assert!(retry_semantics.can_resume);

        let exhausted_semantics = resolve_failed_terminal_semantics_from_sources(
            Some(&json!([])),
            Some(&exhausted_context),
            3,
            3,
        )
        .expect("exhausted semantics");
        assert_eq!(
            exhausted_semantics.kind,
            BatchGenerationFailedTerminalKind::ManualReview
        );
        assert_eq!(exhausted_semantics.reason, "manual_review");
        assert_eq!(exhausted_semantics.label, "自动修复预算已耗尽");
        assert!(exhausted_semantics.review_required);
        assert!(!exhausted_semantics.can_resume);
    }
}
