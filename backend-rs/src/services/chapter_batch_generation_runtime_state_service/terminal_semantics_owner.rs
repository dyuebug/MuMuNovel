use serde_json::{json, Value};

use crate::models::batch_generation_snapshot;
use crate::services::chapter_batch_generation_task_payload_base_service::{
    resolve_failed_terminal_semantics_from_sources, BatchGenerationFailedTerminalSemantics,
    BatchGenerationQualityStatusContext,
};

pub(crate) fn build_batch_generation_terminal_semantics_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::quality_gate_terminal_semantics_projection",
        "scope": "quality_runtime_state_snapshot_retry_budget_and_failed_terminal_semantics_resolution",
        "python_source_map": [
            "backend/app/services/batch_generation_quality_status_service.py",
            "backend/app/services/batch_generation_retry_service.py",
            "backend/app/services/task_workflow_runtime_service.py",
            "backend/app/api/chapter_batch_generation_routes.py",
            "backend/app/api/chapters.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/terminal_semantics_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/retry_routing_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs"
        ],
        "behavior_contract": {
            "terminal_resolution_entrypoints": [
                "resolve_batch_generation_quality_gate_terminal_semantics",
                "BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state",
                "resolve_failed_terminal_semantics_from_sources"
            ],
            "state_contract": {
                "quality_status_owner": "snapshot and current workflow runtime-state are normalized into one batch quality status context before terminal resolution",
                "retry_budget_owner": "current_retry_count and max_retries still decide retry vs manual-review terminal semantics through the shared failed-terminal resolver",
                "terminal_resolution_owner": "runtime driver continues to consume one optional terminal semantics projection without changing post-analysis routing behavior"
            }
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service::runtime_driver_owner",
            "chapter_batch_generation_runtime_state_service::retry_routing_owner",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "task_payload_owner_contract": {
            "owner": "chapter_batch_generation_task_payload_base_service",
            "quality_terminal_status_owner": "resolve_failed_terminal_semantics_from_sources"
        },
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_batch_terminal_semantics_shells_as_source_map_until_explicit_freeze_delete_round",
            "runtime_state_keys": [
                "quality_metrics_summary",
                "latest_quality_metrics",
                "active_story_repair_payload",
                "quality_history_context"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_quality_gate_smoke"
        }
    })
}

pub(crate) fn resolve_batch_generation_quality_gate_terminal_semantics(
    snapshot: Option<&batch_generation_snapshot::Model>,
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
