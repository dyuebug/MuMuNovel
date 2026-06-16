use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::chapter_batch_generation_runtime_state_service::build_batch_generation_runtime_state_owner_contract;
use crate::services::chapter_batch_generation_task_payload_base_service::build_chapter_batch_generation_task_payload_base_owner_contract;
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_execution_contract_service::build_generation_execution_config_owner_contract;
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract;
use crate::services::project_service::{ProjectAccessQueryError, ProjectService};

use super::{
    build_batch_generation_create_launch_owner_contract,
    build_batch_generation_request_prepare_owner_contract,
    start_owned_batch_generation_create_launch_from_route_payload,
    BatchGenerationCreateRouteRequest, PrepareBatchGenerationCreateRequestError,
};

pub(crate) fn build_batch_generation_write_workflow_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_write_workflow_service",
        "scope": "batch_generation_create_write_workflow_persist_dispatch_and_response_payload",
        "python_source_map": [
            "backend/app/api/chapter_batch_generation_routes.py",
            "backend/app/api/chapters.py",
            "backend/app/services/batch_generation/create_service.py",
            "backend/app/services/batch_generation/status_response_builder.py",
            "backend/app/services/batch_generation_candidate_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/api/chapter_batch_generation.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/write_workflow_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "create_entrypoints": [
                "build_batch_generation_create_workflow_request_from_route_payload",
                "prepare_owned_batch_generation_create_workflow",
                "start_owned_batch_generation_write_workflow"
            ],
            "persistence_contract": [
                "BatchGenerationTaskPersistenceSeed::into_active_model",
                "BatchGenerationCreateLaunchPersistencePlan::persist_and_dispatch",
                "BatchGenerationQueuedSnapshotPlan::persist"
            ],
            "response_payload_entrypoints": [
                "BatchGenerationQueuedSnapshotPlan::into_create_response_payload"
            ],
            "runtime_dispatch": [
                "build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed",
                "dispatch_batch_generation_runtime",
                "ChapterCandidateRouteGatewayConfig"
            ],
            "runtime_state_seed_entrypoints": [
                "BatchGenerationCreateStartupRuntimeState::prepare",
                "BatchGenerationCreateStartupRuntimeState::from_recent_history_summary",
                "build_batch_generation_runtime_state_payload_from_parts",
                "build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload"
            ],
            "response_payload_fields": [
                "batch_id",
                "project_id",
                "status",
                "message",
                "total_chapters",
                "completed_chapters",
                "failed_chapters",
                "current_chapter_id",
                "current_chapter_number",
                "estimated_time_minutes",
                "checkpoint",
                "candidate_gateway",
                "active_story_repair_payload",
                "quality_metrics_summary"
            ],
            "gateway_config": [
                "route/AppConfig supplied ChapterCandidateRouteGatewayConfig",
                "create workflow persists candidate_gateway into startup runtime state"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation::create_batch_generation",
            "chapter_batch_generation_active_gateway_smoke_service",
            "chapter_batch_generation_runtime_state_service"
        ],
        "create_launch_owner_contract": build_batch_generation_create_launch_owner_contract(),
        "request_prepare_owner_contract": build_batch_generation_request_prepare_owner_contract(),
        "runtime_state_owner_contract": build_batch_generation_runtime_state_owner_contract(),
        "task_payload_owner_contract": build_chapter_batch_generation_task_payload_base_owner_contract(),
        "story_repair_quality_context_owner_contract": build_story_repair_quality_context_owner_contract(),
        "generation_execution_config_owner_contract": build_generation_execution_config_owner_contract(),
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "create_route_owner": "chapter_batch_generation::create_batch_generation",
            "create_workflow_owner": "start_owned_batch_generation_write_workflow",
            "task_persistence_owner": "BatchGenerationTaskPersistenceSeed::into_active_model",
            "create_persistence_owner": "BatchGenerationCreateLaunchPersistencePlan::persist_and_dispatch",
            "runtime_dispatch_owner": "dispatch_batch_generation_runtime",
            "gateway_config_owner": "ChapterCandidateRouteGatewayConfig",
            "response_payload_owner": "BatchGenerationQueuedSnapshotPlan::into_create_response_payload",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_batch_generation_write_workflow_owner_ready_for_source_map_closeout_review"
        },
        "validation_boundary": [
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "runtime_knob": "python_candidate_executor_fallback",
            "source_map_policy": "keep_python_batch_generation_route_and_service_shells_as_source_map_until_explicit_freeze_delete_round",
            "python_bootstrap_status": "lazy_imported_and_registered_for_explicit_gateway_rollback_only",
            "python_fallback_removal_ready": true,
            "rollback_files": [
                "backend/app/api/chapter_batch_generation_routes.py",
                "backend/app/services/batch_generation/create_service.py",
                "backend/app/services/batch_generation/status_response_builder.py",
            ]
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CreateBatchGenerationWriteWorkflowError {
    ProjectAccess(ProjectAccessQueryError),
    Prepare(PrepareBatchGenerationCreateRequestError),
    Config(String),
    Internal(String),
}

pub(crate) async fn start_owned_batch_generation_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    route_request: BatchGenerationCreateRouteRequest,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
    ProjectService::ensure_owned_access(db, project_id, user_id)
        .await
        .map_err(CreateBatchGenerationWriteWorkflowError::ProjectAccess)?;
    start_owned_batch_generation_create_launch_from_route_payload(
        db,
        project_id,
        user_id,
        route_request,
        candidate_gateway_config,
        Utc::now().naive_utc(),
    )
    .await
}
