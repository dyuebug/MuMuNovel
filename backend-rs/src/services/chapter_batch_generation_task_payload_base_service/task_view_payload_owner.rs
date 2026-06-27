use chrono::NaiveDateTime;
use serde_json::{json, Map, Value};

use super::{
    batch_generation_stage_code, checkpoint_with_runtime_metadata,
    insert_batch_generation_terminal_status_payload, task_execution_mode, task_type, to_iso,
    BatchGenerationQualityStatusContext, BatchGenerationTaskResponsePayloadOptions,
    BatchGenerationTaskResponseQualityPayload, BatchGenerationTaskViewPayloadVariant,
};
use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    apply_batch_quality_runtime_context_to_payload,
    apply_generation_quality_runtime_context_to_payload,
};

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

fn runtime_candidate_gateway_metadata(workflow_runtime_state: Option<&Value>) -> Option<Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("candidate_gateway"))
        .filter(|metadata| metadata.is_object())
        .cloned()
}

fn insert_runtime_candidate_gateway_projection(
    payload: &mut Map<String, Value>,
    workflow_runtime_state: Option<&Value>,
) {
    if let Some(candidate_gateway) = runtime_candidate_gateway_metadata(workflow_runtime_state) {
        payload.insert("candidate_gateway".to_string(), candidate_gateway);
    }
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

    crate::services::chapter_candidate_runtime_state_service::insert_python_query_snapshot_candidate_runtime_fields(checkpoint);

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

    insert_runtime_candidate_gateway_projection(&mut payload, workflow_runtime_state);

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

    let mut payload = build_batch_generation_task_runtime_payload(
        &task.id,
        task_type(task),
        &task.project_id,
        &task.status,
        task.current_chapter_id.as_deref(),
        task.created_at,
        checkpoint,
        stage_code,
        execution_mode,
    );
    insert_runtime_candidate_gateway_projection(&mut payload, workflow_runtime_state);
    payload
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

pub(crate) fn build_batch_generation_status_task_payload_with_quality_context(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
    quality_status_context: &BatchGenerationQualityStatusContext,
) -> Value {
    Value::Object(
        build_batch_generation_task_view_payload_with_quality_context(
            task,
            workflow_runtime_state,
            Some(quality_status_context),
            BatchGenerationTaskViewPayloadVariant::StatusTask,
        ),
    )
}

pub(crate) fn build_batch_generation_status_task_payload_from_task_and_snapshot_projection(
    task: &batch_generation_task::Model,
    snapshot: Option<&batch_generation_snapshot::Model>,
    workflow_runtime_state: Option<&Value>,
) -> Value {
    let quality_status_context =
        BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            snapshot,
            workflow_runtime_state,
        );

    build_batch_generation_status_task_payload_with_quality_context(
        task,
        workflow_runtime_state,
        &quality_status_context,
    )
}

pub(crate) fn build_batch_generation_task_view_payload_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_task_payload_base_service::task_view_payload_owner",
        "scope": "batch_generation_runtime_checkpoint_projection_task_view_status_payload_and_loading_stage_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service/task_view_payload_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "checkpoint_with_runtime_metadata",
                "build_batch_generation_task_runtime_payload",
                "build_batch_generation_task_response_payload_from_runtime_parts",
                "build_batch_generation_task_runtime_payload_from_task_state",
                "build_batch_generation_task_view_payload_from_task_state",
                "build_batch_generation_task_view_payload_with_quality_context",
                "build_batch_generation_status_task_payload_with_quality_context",
                "build_batch_generation_status_task_payload_from_task_and_snapshot_projection"
            ],
            "runtime_checkpoint_projection": [
                "resolve_batch_progress_phase",
                "compose_python_query_snapshot_stage_code",
                "resolve_batch_checkpoint_progress",
                "insert_python_query_snapshot_runtime_fields"
            ],
            "payload_fields": [
                "checkpoint",
                "candidate_gateway",
                "total",
                "completed",
                "current_chapter_number",
                "started_at",
                "completed_at",
                "error_message",
                "failed_chapters",
                "terminal_reason",
                "terminal_label",
                "review_required",
                "can_resume"
            ],
            "loading_stage_projection": [
                "apply_batch_generation_loading_stage_fields"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_read_context_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_batch_generation_write_workflow_service"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_task_payload_base_service",
            "cargo test api::health",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "checkpoint_projection_owner": "chapter_batch_generation_task_payload_base_service::task_view_payload_owner",
            "task_runtime_payload_owner": "build_batch_generation_task_runtime_payload_from_task_state",
            "status_payload_projection_owner": "build_batch_generation_status_task_payload_from_task_and_snapshot_projection",
            "loading_stage_projection_owner": "apply_batch_generation_loading_stage_fields",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation task-view payload direct source-map deleted; surviving Python closeout work for this owner is now limited to shared batch-generation-snapshot schema/runtime/database/API-test hold and shared batch-generation-task schema/runtime/API/test-support packages",
            "status": "rust_batch_generation_task_view_payload_owner_direct_source_map_deleted"
        },
        "shared_schema_hold_status": {
            "batch_generation_task_model": "shared_python_runtime_api_and_test_support_reference",
            "default_python_module_consumers": [
                "backend/tests/test_support/database_test_support.py",
                "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
            ],
            "dedicated_python_regression_surfaces": [
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_batch_status_resume.py"
            ],
            "test_support_consumers": [
                "backend/tests/test_support/batch_generation_status_read_owner_test_adapter.py",
                "backend/tests/test_support/batch_generation_orchestration_test_adapter.py",
                "backend/tests/test_support/batch_generation_route_test_adapter.py"
            ],
            "physical_closeout_ready": false
        },
        "rollback_boundary": {
            "source_map_policy": "batch_generation_task_view_payload_owner_is_rust_only_and_surviving_python_schema_runtime_surfaces_are_tracked_by_shared_task_contracts",
            "projected_fields": [
                "checkpoint",
                "candidate_gateway",
                "terminal_reason",
                "terminal_label",
                "review_required",
                "can_resume"
            ]
        }
    })
}
