use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::batch_generation_snapshot;
use crate::services::chapter_batch_generation_read_context_service::{
    load_owned_batch_generation_task_sources, LoadOwnedBatchGenerationTaskSourcesError,
};
#[cfg(test)]
use crate::services::chapter_batch_generation_runtime_state_service::BatchGenerationPersistedRuntimeContext;
use crate::services::chapter_batch_generation_runtime_state_service::ResumeBatchGenerationCommandState;
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
#[cfg(test)]
use crate::services::chapter_generation_execution_contract_service::{
    BatchGenerationRequestRuntimeState, SingleChapterGenerationCompatOptions,
};
#[cfg(test)]
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::BatchGenerationQualityRuntimeContext;
#[cfg(test)]
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::{
    resolve_resumed_active_story_repair_payload,
    restore_story_repair_compat_options_from_active_snapshot,
};

use super::{
    build_batch_generation_resume_launch_owner_contract,
    BatchGenerationResumeLaunchPersistencePlan, PrepareOwnedBatchGenerationResumeError,
    ResumeBatchGenerationDomainError, ResumeBatchGenerationTaskCommandError,
};

pub(crate) fn build_batch_generation_resume_task_command_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_resume_task_command_service::resume_task_command_owner",
        "scope": "batch_generation_resume_route_facing_command_and_owned_source_loading",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service/resume_task_command_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs"
        ],
        "behavior_contract": {
            "command_entrypoints": [
                "prepare_owned_batch_generation_resume",
                "resume_owned_batch_generation_task_command",
                "prepare_batch_generation_resume",
                "BatchGenerationResumeLaunchPersistencePlan::persist_and_dispatch"
            ],
            "owned_source_loading": [
                "load_owned_batch_generation_task_sources",
                "map_prepare_owned_batch_generation_resume_sources_error",
                "ResumeBatchGenerationCommandState::from_task"
            ],
            "runtime_restore": [
                "prepare_batch_generation_resume_restored_runtime_state",
                "prepare_single_chapter_runtime_launch",
                "prepare_batch_runtime_launch"
            ],
            "story_repair_quality_restore": [
                "restore_resume_compat_options_from_runtime_context",
                "resolve_resume_active_story_repair_payload_from_runtime_context",
                "restored_resume_quality_runtime_context_from_persisted_context"
            ],
            "gateway_config": [
                "ChapterCandidateRouteGatewayConfig",
                "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config",
                "BatchGenerationExecutionInput.candidate_gateway_config"
            ],
            "domain_errors": [
                "InvalidStatus",
                "NoResumableChaptersFound",
                "NoChaptersLeftToResume",
                "SingleChapterUnavailable",
                "ChaptersUnavailable",
                "PrerequisitesBlocked",
                "Internal"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation::resume_batch_generation",
            "chapter_batch_generation_resume_task_command_service::resume_owned_batch_generation_task_command",
            "chapter-batch-generation-active-gateway-smoke-rust",
            "chapter_batch_generation_runtime_state_service",
            "chapter_single_generation_runtime_state_service"
        ],
        "resume_launch_owner_contract": build_batch_generation_resume_launch_owner_contract(),
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "route_owner": "chapter_batch_generation::resume_batch_generation",
            "task_command_owner": "resume_owned_batch_generation_task_command",
            "command_owner": "prepare_owned_batch_generation_resume",
            "execution_selection_owner": "ResumeBatchGenerationCommandState::resolve_execution_selection",
            "validated_plan_owner": "ValidatedResumeExecutionPlan::from_command_state",
            "dispatch_plan_owner": "ResumeExecutionDispatchPlan::from_validated_execution",
            "reset_persistence_owner": "BatchGenerationResumeResetPersistencePlan::from_resume_task_with_existing_runtime_state",
            "persist_and_dispatch_owner": "BatchGenerationResumeLaunchPersistencePlan::persist_and_dispatch",
            "runtime_restore_owner": "prepare_batch_generation_resume_restored_runtime_state",
            "single_generation_gateway_owner": "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config",
            "batch_runtime_gateway_owner": "BatchGenerationExecutionInput.candidate_gateway_config",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation resume command source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages",
            "status": "rust_batch_generation_resume_task_command_owner_source_map_deleted"
        },
        "validation_boundary": [
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "runtime_knob": "python_candidate_executor_fallback",
            "source_map_policy": "batch_generation_resume_task_command_owner_is_rust_only_and_surviving_resume_route_service_surfaces_are_tracked_by_external_command_contracts",
            "python_fallback_removal_ready": true,
            "rollback_files": []
        }
    })
}

pub(crate) async fn prepare_batch_generation_resume(
    db: &DatabaseConnection,
    command_state: ResumeBatchGenerationCommandState,
    user_id: &str,
    snapshot: Option<&batch_generation_snapshot::Model>,
    single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<BatchGenerationResumeLaunchPersistencePlan, ResumeBatchGenerationDomainError> {
    BatchGenerationResumeLaunchPersistencePlan::prepare(
        db,
        command_state,
        user_id,
        snapshot,
        single_generation_gateway_config,
    )
    .await
}

pub(crate) async fn prepare_owned_batch_generation_resume(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
    single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<BatchGenerationResumeLaunchPersistencePlan, PrepareOwnedBatchGenerationResumeError> {
    let (task, snapshot) = load_owned_batch_generation_task_sources(db, batch_id, user_id)
        .await
        .map_err(map_prepare_owned_batch_generation_resume_sources_error)?
        .into_parts();
    let command_state = ResumeBatchGenerationCommandState::from_task(&task);

    prepare_batch_generation_resume(
        db,
        command_state,
        user_id,
        snapshot.as_ref(),
        single_generation_gateway_config,
    )
    .await
    .map_err(PrepareOwnedBatchGenerationResumeError::Domain)
}

pub(crate) async fn resume_owned_batch_generation_task_command(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
    single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<Value, ResumeBatchGenerationTaskCommandError> {
    prepare_owned_batch_generation_resume(db, batch_id, user_id, single_generation_gateway_config)
        .await
        .map_err(ResumeBatchGenerationTaskCommandError::from)?
        .persist_and_dispatch(db)
        .await
        .map_err(ResumeBatchGenerationTaskCommandError::Config)
}

fn map_prepare_owned_batch_generation_resume_sources_error(
    error: LoadOwnedBatchGenerationTaskSourcesError,
) -> PrepareOwnedBatchGenerationResumeError {
    match error {
        LoadOwnedBatchGenerationTaskSourcesError::Task(error) => {
            PrepareOwnedBatchGenerationResumeError::Task(error)
        }
        LoadOwnedBatchGenerationTaskSourcesError::Snapshot(error) => {
            PrepareOwnedBatchGenerationResumeError::Config(error)
        }
    }
}

#[cfg(test)]
pub(super) fn restored_resume_quality_runtime_context_from_persisted_context(
    task_kind: crate::services::chapter_batch_generation_task_payload_base_service::BatchGenerationTaskKind,
    persisted_runtime_context: &BatchGenerationPersistedRuntimeContext,
) -> BatchGenerationQualityRuntimeContext {
    persisted_runtime_context.restored_quality_runtime_context(task_kind)
}

#[cfg(test)]
pub(super) fn restored_resume_quality_runtime_context(
    task_kind: crate::services::chapter_batch_generation_task_payload_base_service::BatchGenerationTaskKind,
    snapshot: Option<&batch_generation_snapshot::Model>,
    workflow_runtime_state: Option<&Value>,
) -> BatchGenerationQualityRuntimeContext {
    let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::from_sources(
        workflow_runtime_state.cloned(),
        snapshot.and_then(|item| item.quality_metrics_history.clone()),
        snapshot.and_then(|item| item.quality_metrics_summary.clone()),
        snapshot.and_then(|item| item.latest_quality_metrics.clone()),
    );

    restored_resume_quality_runtime_context_from_persisted_context(
        task_kind,
        &persisted_runtime_context,
    )
}

#[cfg(test)]
pub(super) fn restore_resume_compat_options_from_runtime_context(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_active_story_repair_payload: Option<&Value>,
    restored_quality_context: &BatchGenerationQualityRuntimeContext,
) -> SingleChapterGenerationCompatOptions {
    restore_story_repair_compat_options_from_active_snapshot(
        &request_runtime_state.compat_options,
        runtime_active_story_repair_payload,
        restored_quality_context.quality_metrics_summary.as_ref(),
        restored_quality_context.latest_quality_metrics.as_ref(),
    )
}

#[cfg(test)]
pub(super) fn resolve_resume_active_story_repair_payload_from_runtime_context(
    runtime_active_story_repair_payload: Option<&Value>,
    request_active_story_repair_payload: Option<&Value>,
    restored_quality_context: &BatchGenerationQualityRuntimeContext,
    scope: &str,
) -> Option<Value> {
    resolve_resumed_active_story_repair_payload(
        runtime_active_story_repair_payload,
        restored_quality_context.quality_metrics_summary.as_ref(),
        restored_quality_context.latest_quality_metrics.as_ref(),
        request_active_story_repair_payload,
        scope,
        "recent_history_summary",
        "Recent history summary",
    )
}
