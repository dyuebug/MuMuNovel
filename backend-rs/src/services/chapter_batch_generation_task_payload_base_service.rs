use chrono::NaiveDateTime;
use serde_json::{json, Map, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_quality_runtime_context_service::{
    apply_batch_quality_runtime_context_to_payload, BatchGenerationQualityRuntimeContext,
};
use crate::services::chapter_batch_generation_quality_status_service::{
    insert_batch_generation_terminal_status_payload, BatchGenerationQualityStatusContext,
};
use crate::services::chapter_batch_generation_status_semantics_service::{
    batch_generation_stage_code, task_execution_mode, task_type,
};
use crate::services::chapter_generation_quality_runtime_context_service::{
    apply_generation_quality_runtime_context_to_payload, GenerationQualityRuntimeContext,
};

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
    const RAW_FIELDS: [&str; 10] = [
        "last_event",
        "last_message",
        "candidate_index",
        "candidate_count",
        "word_count",
        "generation_path",
        "attempt_kind",
        "winner_candidate_index",
        "pre_compaction_total_length",
        "context_budget_limit",
    ];
    const BOOL_FIELDS: [&str; 3] = [
        "rerank_used",
        "word_budget_repair_used",
        "compaction_applied",
    ];

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
        apply_batch_generation_loading_stage_fields,
        build_batch_generation_command_summary_payload,
        build_batch_generation_task_response_payload_from_runtime_parts,
        build_batch_generation_task_runtime_payload,
        build_batch_generation_task_runtime_payload_from_runtime_parts,
        build_batch_generation_task_runtime_payload_from_task_state,
        build_batch_generation_task_view_payload_from_task_state,
        build_batch_generation_task_view_payload_with_quality_context,
        checkpoint_with_runtime_metadata, BatchGenerationCommandProgressSummary,
        BatchGenerationTaskResponsePayloadOptions, BatchGenerationTaskResponseQualityPayload,
        BatchGenerationTaskViewPayloadVariant,
    };
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_quality_runtime_context_service::BatchGenerationQualityRuntimeContext;
    use crate::services::chapter_batch_generation_quality_status_service::BatchGenerationQualityStatusContext;
    use crate::services::chapter_batch_generation_status_semantics_service::task_type;

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
            total_chapters: 2,
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
}
