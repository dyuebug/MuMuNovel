use chrono::NaiveDateTime;
use serde_json::{json, Map, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_candidate_runtime_state_service::build_chapter_candidate_runtime_state_owner_contract;
use crate::services::chapter_generation_execution_contract_service::build_batch_request_runtime_state_owner_contract;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;

const SINGLE_CHAPTER_GENERATION_TASK_TYPE: &str = "chapter_single_generate";
const BATCH_CHAPTER_GENERATION_TASK_TYPE: &str = "chapters_batch_generate";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationTaskKind {
    SingleChapter,
    Batch,
}

pub(crate) fn batch_generation_task_kind(
    chapter_count: i32,
    chapter_ids: &Value,
) -> BatchGenerationTaskKind {
    if chapter_count == 1 && chapter_ids.as_array().is_some_and(|items| items.len() == 1) {
        BatchGenerationTaskKind::SingleChapter
    } else {
        BatchGenerationTaskKind::Batch
    }
}

pub(crate) fn task_kind(task: &batch_generation_task::Model) -> BatchGenerationTaskKind {
    batch_generation_task_kind(task.chapter_count, &task.chapter_ids)
}

pub(crate) fn batch_generation_task_type(kind: BatchGenerationTaskKind) -> &'static str {
    match kind {
        BatchGenerationTaskKind::SingleChapter => SINGLE_CHAPTER_GENERATION_TASK_TYPE,
        BatchGenerationTaskKind::Batch => BATCH_CHAPTER_GENERATION_TASK_TYPE,
    }
}

pub(crate) fn task_type(task: &batch_generation_task::Model) -> &'static str {
    batch_generation_task_type(task_kind(task))
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationCommandProgressSummary {
    pub(crate) batch_id: String,
    pub(crate) total_chapters: i32,
    pub(crate) completed_chapters: i32,
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

pub(crate) fn build_batch_generation_payload_metadata_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_task_payload_base_service::metadata_owner",
        "scope": "batch_generation_task_kind_type_stage_execution_checkpoint_summary_and_eta_owner",
        "python_source_map": [
            "backend/app/models/batch_generation_task.py",
            "backend/app/services/batch_generation/status_response_builder.py",
            "backend/app/services/batch_generation/create_service.py",
            "backend/app/services/batch_generation/resume_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service/metadata_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service/task_view_payload_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/startup_and_command_projection_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "batch_generation_task_kind",
                "task_kind",
                "batch_generation_task_type",
                "task_type",
                "batch_generation_stage_code",
                "task_execution_mode",
                "to_iso",
                "checkpoint_with_runtime_metadata",
                "build_batch_generation_command_summary_payload",
                "estimated_task_minutes"
            ],
            "task_type_policy": "single chapter tasks require chapter_count = 1 and a one-item chapter_ids array; malformed python-compatible single shapes fall back to batch semantics",
            "runtime_checkpoint_policy": "runtime metadata keeps python-compatible checkpoint fields while overriding stage_code and execution_mode with the active rust owner values",
            "summary_payload_policy": "cancel, create, and resume command summaries share one batch_id/completed/total/message envelope",
            "eta_policy": "estimated minutes keep the existing batch create heuristic based on target word count and optional analysis overhead"
        },
        "active_consumers": [
            "chapter_batch_generation_task_payload_base_service::task_view_payload_owner",
            "chapter_batch_generation_runtime_state_service::startup_and_command_projection_owner",
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation_resume_task_command_service"
        ],
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "request_runtime_state_owner_contract": build_batch_request_runtime_state_owner_contract(),
        "candidate_runtime_state_owner_contract": build_chapter_candidate_runtime_state_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_task_payload_base_service",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_batch_payload_metadata_shells_as_source_map_until_explicit_freeze_delete_round",
            "cutover_gate": "same_round_focused_rust_validation_and_following_parent_aggregator_reexport"
        }
    })
}
