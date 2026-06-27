use chrono::NaiveDateTime;
use serde_json::{json, Map, Value};

use crate::models::batch_generation_task;

const SINGLE_GENERATION_TASK_TYPE: &str = "chapter_single_generate";
const SINGLE_GENERATION_EXECUTION_MODE: &str = "interactive";
const SINGLE_GENERATION_ACTIVE_TASK_STATUSES: [&str; 2] = ["pending", "running"];

pub(crate) fn build_single_generation_task_view_payload_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_prepare_service::task_view_payload_owner",
        "scope": "single_generation_runtime_payload_base_task_view_projection_and_candidate_gateway_metadata",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_single_generation_runtime_payload_base",
                "build_single_generation_task_view_payload_from_task_state"
            ],
            "payload_fields": [
                "batch_id",
                "task_type",
                "project_id",
                "status",
                "stage_code",
                "execution_mode",
                "current_chapter_id",
                "checkpoint",
                "created_at",
                "candidate_gateway",
                "total",
                "completed",
                "current_chapter_number",
                "started_at",
                "completed_at",
                "error_message"
            ],
            "stage_contract": [
                "single_generation_pending_stage_code",
                "single_generation_active_task_statuses",
                "status_to_stage_code preserves completed failed cancelled running pending semantics"
            ],
            "candidate_gateway_contract": [
                "single_generation_runtime_candidate_gateway_metadata",
                "candidate_gateway metadata is forwarded only when runtime state carries an object payload"
            ],
            "background_existing_task_contract": [
                "load_existing_single_chapter_background_task_payload",
                "estimated_time_minutes",
                "active_story_repair_payload snapshot is forwarded from workflow runtime state"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_prepare_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_single_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_single_generation_prepare_service",
            "cargo test api::health",
            "cargo check"
        ]
    })
}

pub(crate) fn estimated_single_generation_task_minutes(
    target_word_count: i32,
    enable_analysis: bool,
) -> i32 {
    let generation_time = (target_word_count as f64 / 3000.0) * 2.0;
    let analysis_time = if enable_analysis { 1.0 } else { 0.0 };
    ((generation_time + analysis_time) as i32).max(1)
}

pub(crate) fn single_generation_pending_stage_code() -> &'static str {
    "6.writing.pending"
}

pub(crate) fn single_generation_active_task_statuses() -> [&'static str; 2] {
    SINGLE_GENERATION_ACTIVE_TASK_STATUSES
}

fn single_generation_runtime_candidate_gateway_metadata(
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("candidate_gateway"))
        .filter(|metadata| metadata.is_object())
        .cloned()
}

pub(crate) fn build_single_generation_runtime_payload_base(
    task_id: &str,
    project_id: &str,
    chapter_id: Option<&str>,
    status: &str,
    workflow_runtime_state: Option<&Value>,
    created_at: Option<NaiveDateTime>,
) -> Map<String, Value> {
    let stage_code = match status {
        "completed" => "6.writing.completed",
        "failed" => "6.writing.failed",
        "cancelled" => "6.writing.cancelled",
        "running" => "6.writing.generating",
        _ => single_generation_pending_stage_code(),
    };
    let mut checkpoint = workflow_runtime_state
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    checkpoint.insert("stage_code".to_string(), json!(stage_code));
    checkpoint.insert(
        "execution_mode".to_string(),
        json!(SINGLE_GENERATION_EXECUTION_MODE),
    );

    let mut payload = Map::new();
    payload.insert("batch_id".to_string(), json!(task_id));
    payload.insert("task_type".to_string(), json!(SINGLE_GENERATION_TASK_TYPE));
    payload.insert("project_id".to_string(), json!(project_id));
    payload.insert("status".to_string(), json!(status));
    payload.insert("stage_code".to_string(), json!(stage_code));
    payload.insert(
        "execution_mode".to_string(),
        json!(SINGLE_GENERATION_EXECUTION_MODE),
    );
    payload.insert(
        "current_chapter_id".to_string(),
        json!(chapter_id.map(str::to_string)),
    );
    payload.insert("checkpoint".to_string(), Value::Object(checkpoint));
    payload.insert(
        "created_at".to_string(),
        json!(created_at.map(|datetime| datetime.and_utc().to_rfc3339())),
    );
    if let Some(candidate_gateway) =
        single_generation_runtime_candidate_gateway_metadata(workflow_runtime_state)
    {
        payload.insert("candidate_gateway".to_string(), candidate_gateway);
    }

    payload
}

fn single_generation_task_chapter_id(task: &batch_generation_task::Model) -> Option<&str> {
    task.current_chapter_id.as_deref().or_else(|| {
        task.chapter_ids
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| {
                item.as_str()
                    .or_else(|| item.get("id").and_then(Value::as_str))
            })
    })
}

fn single_generation_to_iso(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.and_utc().to_rfc3339())
}

pub(crate) fn build_single_generation_task_view_payload_from_task_state(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
) -> Map<String, Value> {
    let mut payload = build_single_generation_runtime_payload_base(
        &task.id,
        &task.project_id,
        single_generation_task_chapter_id(task),
        &task.status,
        workflow_runtime_state,
        task.created_at,
    );
    payload.insert("total".to_string(), json!(task.total_chapters));
    payload.insert("completed".to_string(), json!(task.completed_chapters));
    payload.insert(
        "current_chapter_number".to_string(),
        json!(task.current_chapter_number),
    );
    payload.insert(
        "started_at".to_string(),
        json!(single_generation_to_iso(task.started_at)),
    );
    payload.insert(
        "completed_at".to_string(),
        json!(single_generation_to_iso(task.completed_at)),
    );
    payload.insert("error_message".to_string(), json!(task.error_message));

    payload
}
