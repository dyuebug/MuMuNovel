use chrono::NaiveDateTime;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

pub(crate) mod persistence_dispatch_owner;
pub(crate) mod startup_seed_owner;

pub(crate) use self::persistence_dispatch_owner::{
    build_batch_generation_create_persistence_owner_contract,
    start_owned_batch_generation_create_launch,
};
#[cfg(test)]
pub(crate) use self::persistence_dispatch_owner::{
    build_batch_generation_task_active_model, BatchGenerationCreateLaunchPersistencePlan,
    BatchGenerationTaskPersistenceSeed, PreparedBatchGenerationCreateWorkflowLaunch,
};
pub(crate) use self::startup_seed_owner::build_batch_generation_create_startup_seed_owner_contract;
#[cfg(test)]
pub(crate) use self::startup_seed_owner::{
    build_batch_generation_runtime_state_payload_from_parts,
    build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload,
    select_batch_generation_create_effective_style_id, BatchGenerationCreateRuntimeSeed,
    BatchGenerationCreateStartupRuntimeState, BatchGenerationCreateStartupSeedSource,
};
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;

use super::{
    build_batch_generation_create_workflow_request_from_route_payload,
    BatchGenerationCreateRouteRequest, BatchGenerationCreateWorkflowRequest,
    CreateBatchGenerationWriteWorkflowError,
};

pub(crate) fn build_batch_generation_create_launch_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_write_workflow_service::create_launch_startup_seed_and_persistence",
        "scope": "create_runtime_seed_workflow_launch_persistence_dispatch_and_response_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/create_launch_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/api/chapter_batch_generation.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "startup_seed_entrypoints": [
                "BatchGenerationCreateStartupRuntimeState::prepare",
                "BatchGenerationCreateStartupRuntimeState::from_recent_history_summary",
                "BatchGenerationCreateStartupRuntimeState::into_runtime_seed",
                "BatchGenerationCreateRuntimeSeed::prepare",
                "BatchGenerationCreateRuntimeSeed::from_runtime_state_payload",
                "BatchGenerationCreateRuntimeSeed::into_workflow_launch_parts"
            ],
            "launch_projection_entrypoints": [
                "prepare_owned_batch_generation_create_workflow",
                "PreparedBatchGenerationCreateWorkflowLaunch::prepare",
                "PreparedBatchGenerationCreateWorkflowLaunch::from_runtime_seed",
                "BatchGenerationCreateLaunchPersistencePlan::prepare",
                "BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch"
            ],
            "persistence_and_dispatch_entrypoints": [
                "BatchGenerationTaskPersistenceSeed::into_active_model",
                "BatchGenerationCreateLaunchPersistencePlan::persist_and_dispatch",
                "BatchGenerationQueuedSnapshotPlan::persist",
                "dispatch_batch_generation_runtime"
            ],
            "response_projection_entrypoints": [
                "BatchGenerationQueuedSnapshotPlan::into_create_response_payload"
            ],
            "response_projection_fields": [
                "batch_id",
                "project_id",
                "message",
                "chapters_to_generate",
                "estimated_time_minutes",
                "checkpoint",
                "candidate_gateway",
                "active_story_repair_payload",
                "quality_metrics_summary",
                "quality_history_context"
            ],
            "runtime_seed_dependencies": [
                "build_batch_generation_runtime_state_payload_from_parts",
                "build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload",
                "resolve_batch_generation_create_effective_style_id",
                "prepare_generation_execution_config"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation::create_batch_generation",
            "chapter_batch_generation_runtime_state_service",
            "chapter-batch-generation-active-gateway-smoke-rust"
        ],
        "startup_seed_owner_contract": build_batch_generation_create_startup_seed_owner_contract(),
        "persistence_owner_contract": build_batch_generation_create_persistence_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "runtime_knob": "python_candidate_executor_fallback",
            "source_map_policy": "batch_generation_create_launch_owner_is_rust_only_and_surviving_create_route_service_surfaces_are_tracked_by_external_launch_contracts",
            "runtime_state_keys": [
                "batch_request_runtime_state",
                "quality_metrics_summary",
                "quality_metrics_summary_state",
                "quality_metrics_history",
                "latest_quality_metrics",
                "quality_history_context",
                "active_story_repair_payload",
                "candidate_gateway"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_batch_create_route_smoke"
        }
    })
}

pub(crate) async fn start_owned_batch_generation_create_launch_from_route_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    route_request: BatchGenerationCreateRouteRequest,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    now: NaiveDateTime,
) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
    start_owned_batch_generation_create_launch(
        db,
        project_id,
        user_id,
        build_batch_generation_create_workflow_request_from_route_payload(route_request),
        candidate_gateway_config,
        now,
    )
    .await
}
