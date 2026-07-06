use serde_json::{json, Value};
pub(crate) mod attempt_input_owner;
pub(crate) mod follow_up_analysis_owner;
pub(crate) mod quality_payload_owner;
pub(crate) mod resume_restore_owner;
pub(crate) mod resume_semantics_owner;
pub(crate) mod retry_routing_owner;
pub(crate) mod runtime_checkpoint_owner;
pub(crate) mod runtime_driver_owner;
pub(crate) mod runtime_launch_owner;
pub(crate) mod runtime_persistence_owner;
pub(crate) mod runtime_snapshot_owner;
pub(crate) mod selected_candidate_event_owner;
pub(crate) mod startup_and_command_projection_owner;
pub(crate) mod terminal_runtime_patch_owner;
pub(crate) mod terminal_semantics_owner;

pub(crate) use self::attempt_input_owner::{
    build_batch_generation_attempt_input_owner_contract, BatchGenerationAttemptInputPlan,
};
#[cfg(test)]
pub(crate) use self::follow_up_analysis_owner::{
    build_batch_generation_analysis_completed_snapshot, format_analysis_error_message,
    BatchGenerationAnalysisAttemptPlan, BatchGenerationAnalysisAttemptResolution,
    BatchGenerationAnalysisCompletionPersistencePlan, BatchGenerationAnalysisRoutingPlan,
    BatchGenerationAnalysisStartedPersistencePlan,
};
pub(crate) use self::follow_up_analysis_owner::{
    build_batch_generation_follow_up_analysis_owner_contract, BatchGenerationFollowUpAnalysisPlan,
};
pub(crate) use self::quality_payload_owner::{
    build_batch_generation_quality_payload_owner_contract,
    build_batch_generation_runtime_state_payload_from_current_quality,
    build_batch_generation_runtime_state_payload_preserving_quality_state,
    build_current_chapter_latest_quality_metrics_from_plot_analysis,
    build_current_chapter_quality_summary_from_plot_analysis,
};
#[cfg(test)]
pub(crate) use self::resume_restore_owner::build_batch_generation_resume_runtime_checkpoint;
pub(crate) use self::resume_restore_owner::restore_batch_generation_runtime_compat_options_from_persisted_runtime_context;
pub(crate) use self::resume_restore_owner::{
    build_batch_generation_resume_restore_owner_contract,
    prepare_batch_generation_resume_restored_runtime_state, BatchGenerationPersistedRuntimeContext,
    PrepareBatchGenerationResumeRuntimeStateError, PreparedBatchGenerationResumeRuntimeLaunch,
    PreparedSingleChapterResumeRuntimeLaunch, RestoredResumeRuntimeStateProjection,
};
pub(crate) use self::resume_semantics_owner::{
    build_batch_generation_resume_semantics_owner_contract, ResolveResumeExecutionSelectionError,
    ResumeBatchGenerationCommandState, ResumeExecutionSelection, ResumeResetSemantics,
};
#[cfg(test)]
pub(crate) use self::retry_routing_owner::{
    batch_generation_retry_backoff_seconds, build_batch_generation_failed_task_error_message,
    should_retry_batch_generation_attempt, BatchGenerationRetryPersistenceContract,
    BatchGenerationRetryPersistencePlan, BatchGenerationRetryProgressionPlan,
};
pub(crate) use self::retry_routing_owner::{
    build_batch_generation_retry_routing_owner_contract, BatchGenerationGenericFailureRoutingPlan,
    BatchGenerationQualityGateRoutingPlan,
};
pub(crate) use self::runtime_checkpoint_owner::{
    build_batch_generation_runtime_checkpoint_for_stage,
    build_batch_generation_runtime_checkpoint_owner_contract, BatchGenerationFailureKind,
    BatchGenerationSnapshotStage,
};
#[cfg(test)]
pub(crate) use self::runtime_checkpoint_owner::{
    build_pending_batch_generation_runtime_checkpoint,
    checkpoint_message_for_batch_generation_failure, compute_batch_running_progress,
};
pub(crate) use self::runtime_driver_owner::{
    build_batch_generation_runtime_driver_owner_contract, BatchGenerationAttemptProgression,
    BatchGenerationRuntimeDriverProgression, BatchGenerationRuntimeLifecyclePlan,
    BatchGenerationStepProgress,
};
#[cfg(test)]
pub(crate) use self::runtime_driver_owner::{
    build_non_applied_generated_result_quality_runtime_state,
    BatchGenerationPostAnalysisTerminalOutcome, BatchGenerationPostAnalysisTerminalPlan,
    BatchGenerationPostWriteGuardOutcome, BatchGenerationPostWriteGuardPlan,
    PreparedBatchGenerationStepExecution,
};
pub(crate) use self::runtime_launch_owner::{
    build_batch_generation_execution_input,
    build_batch_generation_runtime_launch_input_from_runtime_state_seed,
    build_batch_generation_runtime_launch_owner_contract, dispatch_batch_generation_runtime,
    prepare_batch_generation_runtime_launch_input_from_request_runtime_state,
    BatchGenerationExecutionInput, BatchGenerationRuntimeSession,
};
#[cfg(test)]
pub(crate) use self::runtime_persistence_owner::{
    append_failed_chapter_entry, build_batch_generation_failed_chapter_entry,
    build_quality_gate_blocked_failed_chapter_entry,
    extract_quality_gate_failed_metrics_from_runtime_state, ModelFieldUpdate, TaskTimestampUpdate,
};
pub(crate) use self::runtime_persistence_owner::{
    build_batch_generation_runtime_persistence_owner_contract,
    BatchGenerationResumeTaskResetMutationPlan, BatchGenerationRuntimePersistencePlan,
    BatchGenerationTaskStage,
};
pub(crate) use self::runtime_snapshot_owner::{
    build_batch_generation_runtime_snapshot_owner_contract, merge_batch_generation_runtime_state,
    persist_batch_generation_runtime_plan, persist_batch_generation_runtime_snapshot_replace,
    project_merged_batch_generation_runtime_state, upsert_batch_generation_runtime_snapshot,
};
pub(crate) use self::selected_candidate_event_owner::{
    build_batch_generation_selected_candidate_event_owner_contract,
    build_batch_generation_selected_candidate_event_snapshot,
};
#[cfg(test)]
pub(crate) use self::startup_and_command_projection_owner::PrepareBatchGenerationCancelPersistenceError;
pub(crate) use self::startup_and_command_projection_owner::{
    build_batch_generation_startup_and_command_projection_owner_contract,
    build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed,
    cancel_owned_batch_generation_runtime_command, reset_batch_generation_task_for_resume,
    BatchGenerationQueuedCreateResponseChapter, BatchGenerationQueuedSnapshotPlan,
    BatchGenerationResumeResetPersistencePlan, CancelBatchGenerationTaskCommandError,
};
#[cfg(test)]
pub(crate) use self::startup_and_command_projection_owner::{
    prepare_batch_generation_cancel_persistence_plan, BatchGenerationCancelledPersistencePlan,
    BatchGenerationResumeSnapshotPlan,
};
#[cfg(test)]
pub(crate) use self::terminal_runtime_patch_owner::build_quality_gate_blocked_runtime_state_patch_from_workflow_state;
pub(crate) use self::terminal_runtime_patch_owner::{
    apply_manual_review_terminal_fields, build_generation_terminal_runtime_patch_owner_contract,
    build_retry_quality_runtime_patch_contract_from_workflow_state,
};
pub(crate) use self::terminal_semantics_owner::{
    build_batch_generation_terminal_semantics_owner_contract,
    resolve_batch_generation_quality_gate_terminal_semantics,
};

use crate::services::chapter_batch_generation_task_payload_base_service::{
    estimated_task_minutes, BatchGenerationQualityStatusContext,
};
use crate::services::chapter_generation_execution_contract_service::{
    build_batch_request_runtime_state_owner_contract, PreparedGenerationExecutionConfig,
};
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::{
    build_chapter_generation_snapshot_owner_contract, load_chapter_generation_snapshot,
};
use crate::services::chapter_single_generation_runtime_seed_service::prepare_single_chapter_runtime_launch_input_from_request_runtime_state;
pub(crate) fn build_batch_generation_runtime_state_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service",
        "scope": "batch_generation_runtime_lifecycle_execution_input_checkpoint_candidate_events_retry_terminal_and_follow_up_analysis",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "runtime_entrypoints": [
                "build_batch_generation_execution_input",
                "BatchGenerationRuntimeLifecyclePlan::from_execution_input",
                "BatchGenerationRuntimeLifecyclePlan::start",
                "PreparedBatchGenerationStepExecution::start"
            ],
            "response_payload_entrypoints": [
                "CancelledBatchGenerationStatusProjection::build_status_payload_for_task",
                "build_batch_generation_status_task_payload_with_quality_context",
                "BatchGenerationQueuedSnapshotPlan::into_create_response_payload",
                "BatchGenerationResumeResetPersistencePlan::into_resume_response_payload",
                "BatchGenerationCancelledPersistencePlan::build_response_payload_for_task"
            ],
            "cancel_prepare_entrypoints": [
                "prepare_batch_generation_cancel_persistence_plan",
                "BatchGenerationCancelledPersistencePlan::from_sources"
            ],
            "cancel_command_entrypoints": [
                "cancel_owned_batch_generation_runtime_command",
                "BatchGenerationCancelledPersistencePlan::persist"
            ],
            "resume_restore_entrypoints": [
                "prepare_batch_generation_resume_restored_runtime_state",
                "BatchGenerationPersistedRuntimeContext::from_snapshot",
                "RestoredResumeRuntimeStateProjection::from_persisted_runtime_context"
            ],
            "candidate_gateway_entrypoints": [
                "generate_and_persist_chapter_content_with_candidate_route_gateway",
                "build_batch_generation_selected_candidate_event_snapshot",
                "build_batch_generation_selected_candidate_event_batch"
            ],
            "checkpoint_entrypoints": [
                "build_pending_batch_generation_runtime_checkpoint",
                "BatchGenerationSnapshotStage::build_checkpoint",
                "BatchGenerationRuntimePersistencePlan::persist"
            ],
            "retry_and_terminal_entrypoints": [
                "BatchGenerationRetryPersistencePlan",
                "BatchGenerationRetryProgressionPlan::execute",
                "BatchGenerationQualityGateRoutingPlan",
                "BatchGenerationPostAnalysisTerminalPlan",
                "resolve_batch_generation_quality_gate_terminal_semantics",
                "resolve_failed_terminal_semantics_from_sources"
            ],
            "follow_up_analysis_entrypoints": [
                "BatchGenerationFollowUpAnalysisPlan::from_generated_result",
                "prepare_chapter_analysis_execution",
                "analyze_generated_chapter_follow_up"
            ],
            "runtime_state_keys": [
                "progress",
                "checkpoint",
                "candidate_gateway",
                "selected_candidate_events",
                "quality_metrics_summary_state",
                "active_story_repair_payload"
            ],
            "gateway_config": [
                "ChapterCandidateRouteGatewayConfig",
                "batch create/resume routes pass route/AppConfig supplied gateway config into runtime launch"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_batch_generation_read_context_service",
            "chapter-batch-generation-active-gateway-smoke-rust",
            "chapter_batch_generation"
        ],
        "selected_candidate_event_owner_contract": build_batch_generation_selected_candidate_event_owner_contract(),
        "terminal_runtime_patch_owner_contract": build_generation_terminal_runtime_patch_owner_contract(),
        "resume_restore_owner_contract": build_batch_generation_resume_restore_owner_contract(),
        "follow_up_analysis_owner_contract": build_batch_generation_follow_up_analysis_owner_contract(),
        "retry_routing_owner_contract": build_batch_generation_retry_routing_owner_contract(),
        "startup_and_command_projection_owner_contract": build_batch_generation_startup_and_command_projection_owner_contract(),
        "runtime_driver_owner_contract": build_batch_generation_runtime_driver_owner_contract(),
        "attempt_input_owner_contract": build_batch_generation_attempt_input_owner_contract(),
        "runtime_persistence_owner_contract": build_batch_generation_runtime_persistence_owner_contract(),
        "quality_payload_owner_contract": build_batch_generation_quality_payload_owner_contract(),
        "runtime_checkpoint_owner_contract": build_batch_generation_runtime_checkpoint_owner_contract(),
        "resume_semantics_owner_contract": build_batch_generation_resume_semantics_owner_contract(),
        "runtime_launch_owner_contract": build_batch_generation_runtime_launch_owner_contract(),
        "runtime_snapshot_owner_contract": build_batch_generation_runtime_snapshot_owner_contract(),
        "terminal_semantics_owner_contract": build_batch_generation_terminal_semantics_owner_contract(),
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "runtime_lifecycle_owner": "BatchGenerationRuntimeLifecyclePlan::start",
            "step_execution_owner": "PreparedBatchGenerationStepExecution::start",
            "candidate_gateway_owner": "generate_and_persist_chapter_content_with_candidate_route_gateway",
            "selected_candidate_event_owner": "build_batch_generation_selected_candidate_event_batch",
            "checkpoint_owner": "BatchGenerationRuntimePersistencePlan::persist",
            "retry_progression_owner": "BatchGenerationRetryProgressionPlan::execute",
            "terminal_runtime_owner": "BatchGenerationPostAnalysisTerminalPlan",
            "follow_up_analysis_owner": "BatchGenerationFollowUpAnalysisPlan::from_generated_result",
            "create_response_payload_owner": "BatchGenerationQueuedSnapshotPlan::into_create_response_payload",
            "resume_response_payload_owner": "BatchGenerationResumeResetPersistencePlan::into_resume_response_payload",
            "cancel_response_payload_owner": "BatchGenerationCancelledPersistencePlan::build_response_payload_for_task",
            "cancel_prepare_owner": "prepare_batch_generation_cancel_persistence_plan",
            "cancel_task_command_owner": "cancel_owned_batch_generation_runtime_command",
            "resume_restore_owner": "prepare_batch_generation_resume_restored_runtime_state",
            "gateway_config_owner": "ChapterCandidateRouteGatewayConfig",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages",
            "status": "rust_batch_generation_runtime_state_owner_with_deleted_route_package_source_map"
        },
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "runtime_knob": "python_candidate_executor_fallback",
            "source_map_policy": "batch_generation_route_package_source_map_deleted_surviving_python_closeout_moves_to_read_context_and_shared_projection_packages",
            "python_fallback_removal_ready": true,
            "rollback_files": []
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        batch_generation_retry_backoff_seconds, build_batch_generation_attempt_input_owner_contract,
        build_batch_generation_execution_input, build_batch_generation_resume_runtime_checkpoint,
        build_batch_generation_runtime_checkpoint_for_stage,
        build_batch_generation_runtime_launch_input_from_runtime_state_seed,
        build_batch_generation_quality_payload_owner_contract,
        build_batch_generation_runtime_persistence_owner_contract,
        build_batch_generation_selected_candidate_event_owner_contract,
        build_batch_generation_runtime_state_owner_contract,
        build_pending_batch_generation_runtime_checkpoint,
        prepare_batch_generation_cancel_persistence_plan,
        prepare_batch_generation_resume_restored_runtime_state,
        build_quality_gate_blocked_runtime_state_patch_from_workflow_state,
        build_retry_quality_runtime_patch_contract_from_workflow_state,
        checkpoint_message_for_batch_generation_failure, compute_batch_running_progress,
        dispatch_batch_generation_runtime, merge_batch_generation_runtime_state,
        restore_batch_generation_runtime_compat_options_from_persisted_runtime_context,
        should_retry_batch_generation_attempt, BatchGenerationAnalysisAttemptPlan,
        BatchGenerationAttemptInputPlan, BatchGenerationAttemptProgression,
        BatchGenerationExecutionInput, BatchGenerationFailureKind,
        BatchGenerationFollowUpAnalysisPlan, BatchGenerationPersistedRuntimeContext,
        BatchGenerationPostAnalysisTerminalOutcome, BatchGenerationPostAnalysisTerminalPlan,
        BatchGenerationPostWriteGuardOutcome, BatchGenerationPostWriteGuardPlan,
        PrepareBatchGenerationCancelPersistenceError,
        PrepareBatchGenerationResumeRuntimeStateError,
        BatchGenerationQueuedSnapshotPlan, BatchGenerationResumeSnapshotPlan,
        BatchGenerationRetryProgressionPlan, BatchGenerationRuntimeDriverProgression,
        BatchGenerationRuntimeLifecyclePlan, BatchGenerationRuntimeSession,
        BatchGenerationSnapshotStage, BatchGenerationStepProgress, BatchGenerationTaskStage,
        ModelFieldUpdate, PreparedBatchGenerationStepExecution, ResumeBatchGenerationCommandState,
        ResumeResetSemantics, TaskTimestampUpdate,
    };
    use crate::ai::AIConfig;
    use crate::models::{batch_generation_snapshot, batch_generation_task, chapter};
    use crate::services::chapter_batch_generation_task_payload_base_service::{
        BatchGenerationFailedTerminalKind, BatchGenerationFailedTerminalSemantics,
    };
    use crate::services::chapter_batch_generation_write_workflow_service::build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload;
    use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
    use crate::services::chapter_generation_execution_contract_service::BatchGenerationRequestRuntimeState;
    use crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_execution_contract_service::{
        build_prompt_overrides_from_compat_options, SingleChapterGenerationCompatOptions,
    };
    use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
        normalize_terminal_quality_history as shared_normalize_terminal_quality_history,
        normalize_terminal_quality_history_context as shared_normalize_terminal_quality_history_context,
    };
    use crate::services::chapter_batch_generation_runtime_state_service::terminal_runtime_patch_owner::{
        apply_terminal_quality_runtime_patch_contract,
        build_manual_review_terminal_runtime_patch_contract,
    };
    use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
    use chrono::NaiveDate;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
        QueryFilter, Schema, Set,
    };
    use serde_json::{json, Value};

    #[test]
    fn should_publish_batch_generation_runtime_state_owner_contract() {
        let contract = build_batch_generation_runtime_state_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service"
        );
        assert!(contract["python_source_map"]
            .as_array()
            .expect("python source map")
            .is_empty());
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs"
        );
        assert_eq!(
            contract["rust_owner_map"][1],
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_entrypoints"][0],
            "build_batch_generation_execution_input"
        );
        assert_eq!(
            contract["behavior_contract"]["candidate_gateway_entrypoints"][1],
            "build_batch_generation_selected_candidate_event_snapshot"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_entrypoints"][0],
            "CancelledBatchGenerationStatusProjection::build_status_payload_for_task"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_entrypoints"][1],
            "build_batch_generation_status_task_payload_with_quality_context"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_entrypoints"][2],
            "BatchGenerationQueuedSnapshotPlan::into_create_response_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_entrypoints"][3],
            "BatchGenerationResumeResetPersistencePlan::into_resume_response_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["cancel_prepare_entrypoints"][0],
            "prepare_batch_generation_cancel_persistence_plan"
        );
        assert_eq!(
            contract["behavior_contract"]["cancel_command_entrypoints"][0],
            "cancel_owned_batch_generation_runtime_command"
        );
        assert_eq!(
            contract["behavior_contract"]["resume_restore_entrypoints"][0],
            "prepare_batch_generation_resume_restored_runtime_state"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["create_response_payload_owner"],
            "BatchGenerationQueuedSnapshotPlan::into_create_response_payload"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["resume_response_payload_owner"],
            "BatchGenerationResumeResetPersistencePlan::into_resume_response_payload"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["cancel_prepare_owner"],
            "prepare_batch_generation_cancel_persistence_plan"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["cancel_task_command_owner"],
            "cancel_owned_batch_generation_runtime_command"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["resume_restore_owner"],
            "prepare_batch_generation_resume_restored_runtime_state"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["cancel_response_payload_owner"],
            "BatchGenerationCancelledPersistencePlan::build_response_payload_for_task"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            "chapter_generation_runtime_service::snapshot_persistence_owner"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["behavior_contract"]["write_functions"]
                [0],
            "persist_chapter_generation_runtime_snapshot"
        );
        assert_eq!(
            contract["behavior_contract"]["retry_and_terminal_entrypoints"][3],
            "BatchGenerationPostAnalysisTerminalPlan"
        );
        assert_eq!(
            contract["behavior_contract"]["retry_and_terminal_entrypoints"][4],
            "resolve_batch_generation_quality_gate_terminal_semantics"
        );
        assert_eq!(
            contract["behavior_contract"]["retry_and_terminal_entrypoints"][5],
            "resolve_failed_terminal_semantics_from_sources"
        );
        assert_eq!(
            contract["behavior_contract"]["follow_up_analysis_entrypoints"][2],
            "analyze_generated_chapter_follow_up"
        );
        assert_eq!(
            contract["active_consumers"][3],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
        assert_eq!(contract["active_consumers"][4], "chapter_batch_generation");
        assert_eq!(
            contract["selected_candidate_event_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::selected_candidate_event_owner"
        );
        assert_eq!(
            contract["terminal_runtime_patch_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::terminal_runtime_patch"
        );
        assert_eq!(
            contract["runtime_persistence_owner_contract"]["python_source_map"]
                .as_array()
                .expect("runtime persistence python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["resume_restore_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::resume_restore_runtime_projection"
        );
        assert_eq!(
            contract["follow_up_analysis_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::follow_up_analysis_runtime_projection"
        );
        assert_eq!(
            contract["retry_routing_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::retry_failure_quality_gate_routing"
        );
        assert_eq!(
            contract["retry_routing_owner_contract"]["python_source_map"]
                .as_array()
                .expect("retry routing python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["startup_and_command_projection_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::startup_cancel_resume_task_payload_projection"
        );
        assert_eq!(
            contract["startup_and_command_projection_owner_contract"]["python_source_map"]
                .as_array()
                .expect("startup and command python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["runtime_driver_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::runtime_driver_execution_chain"
        );
        assert_eq!(
            contract["attempt_input_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::attempt_input_prompt_provider_gateway_execution"
        );
        assert_eq!(
            contract["runtime_persistence_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::runtime_persistence_task_mutation_projection"
        );
        assert_eq!(
            contract["runtime_launch_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::runtime_launch_session_dispatch"
        );
        assert_eq!(
            contract["runtime_snapshot_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::runtime_snapshot_merge_write_projection"
        );
        assert_eq!(
            contract["resume_semantics_owner_contract"]["python_source_map"]
                .as_array()
                .expect("resume semantics python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["resume_semantics_owner_contract"]["rollback_boundary"]["source_map_policy"],
            "batch_generation_resume_semantics_owner_is_rust_only_and_surviving_resume_route_surfaces_are_tracked_by_external_command_contracts"
        );
        assert_eq!(
            contract["runtime_launch_owner_contract"]["rollback_boundary"]["source_map_policy"],
            "batch_generation_runtime_launch_owner_is_rust_only_and_surviving_launch_dispatch_surfaces_are_tracked_by_external_runtime_contracts"
        );
        assert_eq!(
            contract["runtime_checkpoint_owner_contract"]["rollback_boundary"]["source_map_policy"],
            "batch_generation_runtime_checkpoint_owner_is_rust_only_and_surviving_checkpoint_projection_surfaces_are_tracked_by_external_runtime_contracts"
        );
        assert_eq!(
            contract["terminal_semantics_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::quality_gate_terminal_semantics_projection"
        );
        assert_eq!(
            contract["quality_payload_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::quality_payload_current_quality_projection"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["runtime_lifecycle_owner"],
            "BatchGenerationRuntimeLifecyclePlan::start"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["candidate_gateway_owner"],
            "generate_and_persist_chapter_content_with_candidate_route_gateway"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["selected_candidate_event_owner"],
            "build_batch_generation_selected_candidate_event_batch"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_route_package_source_map_deleted_surviving_python_closeout_moves_to_read_context_and_shared_projection_packages"
        );
        assert!(contract["rollback_boundary"]["rollback_files"]
            .as_array()
            .expect("rollback files")
            .is_empty());
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_batch_generation_runtime_state_owner_with_deleted_route_package_source_map"
        );
    }

    #[test]
    fn should_publish_selected_candidate_event_owner_contract() {
        let contract = build_batch_generation_selected_candidate_event_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::selected_candidate_event_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "build_batch_generation_selected_candidate_event_snapshot"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][1],
            "build_batch_generation_selected_candidate_event_batch"
        );
        assert_eq!(
            contract["behavior_contract"]["projection_helpers"][0],
            "snapshot_chapter_candidate_event_view"
        );
        assert_eq!(
            contract["behavior_contract"]["projection_helpers"][3],
            "quality_gate_plan_allows_selected_candidate_chunks"
        );
        assert!(contract
            .as_object()
            .and_then(|item| item.get("terminal_runtime_patch_owner_contract"))
            .is_none());
        assert_eq!(
            contract["active_consumers"][1],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_batch_generation_selected_candidate_event_owner_source_map_deleted"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_selected_candidate_event_owner_is_rust_only_and_surviving_python_runtime_surfaces_are_tracked_by_external_runtime_state_contracts"
        );
    }

    #[test]
    fn should_publish_terminal_runtime_patch_owner_contract() {
        let contract = super::build_generation_terminal_runtime_patch_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::terminal_runtime_patch"
        );
        assert_eq!(
            contract["behavior_contract"]["patch_entrypoints"][0],
            "build_quality_gate_blocked_runtime_state_patch"
        );
        assert_eq!(
            contract["behavior_contract"]["patch_entrypoints"][3],
            "build_retry_quality_runtime_patch_contract_from_workflow_state"
        );
        assert_eq!(
            contract["behavior_contract"]["patch_helpers"][0],
            "infer_quality_scope_from_workflow_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["patch_helpers"][5],
            "insert_retry_active_story_repair_payload"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            "chapter_generation_runtime_service::quality_runtime_context_owner"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["behavior_contract"]["quality_fields"],
            json!([
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_metrics_summary",
                "quality_history_context"
            ])
        );
        assert_eq!(
            contract["request_runtime_state_owner_contract"]["owner"],
            "chapter_generation_execution_contract_service::request_runtime_state"
        );
        assert_eq!(
            contract["request_runtime_state_owner_contract"]["behavior_contract"]["entrypoints"][3],
            "parse_batch_generation_request_runtime_state"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("terminal runtime patch python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["active_consumers"][2],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_terminal_runtime_patch_owner_is_rust_only_and_surviving_story_repair_task_runtime_surfaces_are_tracked_by_external_runtime_contracts"
        );
    }

    #[test]
    fn should_publish_resume_restore_owner_contract() {
        let contract = super::build_batch_generation_resume_restore_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::resume_restore_runtime_projection"
        );
        assert_eq!(
            contract["behavior_contract"]["restore_entrypoints"][0],
            "prepare_batch_generation_resume_restored_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["restore_entrypoints"][3],
            "BatchGenerationPersistedRuntimeContext::build_restored_resume_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["compat_and_quality_restore_helpers"][0],
            "BatchGenerationPersistedRuntimeContext::restored_resume_compat_options"
        );
        assert_eq!(
            contract["behavior_contract"]["compat_and_quality_restore_helpers"][3],
            "BatchGenerationPersistedRuntimeContext::restored_quality_runtime_context"
        );
        assert_eq!(
            contract["behavior_contract"]["launch_projection_entrypoints"][2],
            "RestoredResumeRuntimeStateProjection::into_launch_parts"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            "chapter_generation_runtime_service::quality_runtime_context_owner"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["rust_owner_map"][2],
            "resolve_generation_quality_runtime_context_from_persisted_sources"
        );
        assert_eq!(
            contract["request_runtime_state_owner_contract"]["owner"],
            "chapter_generation_execution_contract_service::request_runtime_state"
        );
        assert_eq!(
            contract["request_runtime_state_owner_contract"]["behavior_contract"]["entrypoints"][3],
            "parse_batch_generation_request_runtime_state"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["owner"],
            "chapter_generation_runtime_service::story_repair_quality_context_owner"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["behavior_contract"]
                ["resume_precedence"][0],
            "runtime_active_story_repair_payload"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["behavior_contract"]
                ["active_payload_fields"][0],
            "summary"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("resume restore python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_batch_generation_resume_task_command_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_resume_restore_owner_is_rust_only_and_surviving_story_repair_runtime_surfaces_are_tracked_by_external_runtime_contracts"
        );
    }

    #[test]
    fn should_publish_follow_up_analysis_owner_contract() {
        let contract = super::build_batch_generation_follow_up_analysis_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::follow_up_analysis_runtime_projection"
        );
        assert_eq!(
            contract["behavior_contract"]["attempt_entrypoints"][0],
            "BatchGenerationFollowUpAnalysisPlan::from_generated_result"
        );
        assert_eq!(
            contract["behavior_contract"]["attempt_entrypoints"][4],
            "BatchGenerationAnalysisAttemptPlan::resolve_result"
        );
        assert_eq!(
            contract["behavior_contract"]["persistence_entrypoints"][3],
            "BatchGenerationAnalysisCompletionPersistencePlan::persist"
        );
        assert_eq!(
            contract["behavior_contract"]["routing_entrypoints"][2],
            "should_stop_batch_generation_analysis_without_retry"
        );
        assert_eq!(
            contract["behavior_contract"]["analysis_gateways"][1],
            "analyze_generated_chapter_follow_up"
        );
        assert_eq!(
            contract["analysis_runtime_owner_contract"]["owner"],
            "chapter_analysis_runtime_service"
        );
        assert_eq!(
            contract["analysis_runtime_owner_contract"]["behavior_contract"]
                ["runtime_trigger_owner"],
            "trigger_runtime_owner::execute_prepared_chapter_analysis_trigger"
        );
        assert_eq!(
            contract["analysis_runtime_owner_contract"]["behavior_contract"]["query_owner_module"],
            "chapter_analysis_runtime_service::query_owner"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            "chapter_generation_runtime_service::snapshot_persistence_owner"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["behavior_contract"]["read_functions"]
                [0],
            "load_chapter_generation_snapshot"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            "chapter_generation_runtime_service::quality_runtime_context_owner"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["rust_owner_map"][2],
            "resolve_generation_quality_runtime_context_from_persisted_sources"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_batch_generation_runtime_state_service"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("follow up analysis python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_follow_up_analysis_owner_is_rust_only_and_surviving_analysis_runtime_surfaces_are_tracked_by_external_analysis_contracts"
        );
    }

    #[test]
    fn should_publish_retry_routing_owner_contract() {
        let contract = super::build_batch_generation_retry_routing_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::retry_failure_quality_gate_routing"
        );
        assert_eq!(
            contract["behavior_contract"]["retry_persistence_entrypoints"][0],
            "BatchGenerationRetryPersistencePlan::new"
        );
        assert_eq!(
            contract["behavior_contract"]["retry_persistence_entrypoints"][3],
            "BatchGenerationRetryPersistencePlan::persist"
        );
        assert_eq!(
            contract["behavior_contract"]["routing_entrypoints"][0],
            "BatchGenerationGenericFailureRoutingPlan::from_step_error"
        );
        assert_eq!(
            contract["behavior_contract"]["routing_entrypoints"][4],
            "BatchGenerationQualityGateRoutingPlan::persist_and_resolve"
        );
        assert_eq!(
            contract["behavior_contract"]["progression_entrypoints"][1],
            "BatchGenerationRetryProgressionPlan::execute"
        );
        assert_eq!(
            contract["behavior_contract"]["retry_policy_helpers"][2],
            "build_batch_generation_failed_task_error_message"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_state_patch_dependencies"][1],
            "build_retry_quality_runtime_patch_contract_from_workflow_state"
        );
        assert_eq!(
            contract["terminal_runtime_patch_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::terminal_runtime_patch"
        );
        assert_eq!(
            contract["terminal_runtime_patch_owner_contract"]["behavior_contract"]
                ["patch_entrypoints"][0],
            "build_quality_gate_blocked_runtime_state_patch"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            "chapter_generation_runtime_service::snapshot_persistence_owner"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["behavior_contract"]["write_functions"]
                [1],
            "upsert_chapter_generation_runtime_snapshot"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter_batch_generation_active_gateway_smoke_service"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("retry routing python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_retry_routing_owner_is_rust_only_and_surviving_retry_quality_gate_surfaces_are_tracked_by_external_runtime_contracts"
        );
    }

    #[test]
    fn should_publish_startup_and_command_projection_owner_contract() {
        let contract =
            super::build_batch_generation_startup_and_command_projection_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::startup_cancel_resume_task_payload_projection"
        );
        assert_eq!(
            contract["behavior_contract"]["startup_snapshot_entrypoints"][1],
            "BatchGenerationQueuedSnapshotPlan::into_create_response_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["cancel_command_entrypoints"][3],
            "cancel_owned_batch_generation_runtime_command"
        );
        assert_eq!(
            contract["behavior_contract"]["resume_reset_entrypoints"][1],
            "BatchGenerationResumeResetPersistencePlan::into_resume_response_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["response_projection_fields"][0],
            "checkpoint"
        );
        assert_eq!(
            contract["behavior_contract"]["response_projection_fields"][11],
            "can_resume"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_state_seed_dependencies"][0],
            "runtime_state_with_candidate_gateway_metadata"
        );
        assert_eq!(contract["active_consumers"][3], "chapter_batch_generation");
        assert_eq!(
            contract["task_payload_owner_contract"]["owner"],
            "chapter_batch_generation_task_payload_base_service"
        );
        assert_eq!(
            contract["task_payload_owner_contract"]["quality_terminal_status_owner_contract"]
                ["owner"],
            "chapter_batch_generation_task_payload_base_service::quality_terminal_status_owner"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            "chapter_generation_runtime_service::quality_runtime_context_owner"
        );
        assert_eq!(
            contract["request_runtime_state_owner_contract"]["owner"],
            "chapter_generation_execution_contract_service::request_runtime_state"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["owner"],
            "chapter_generation_runtime_service::story_repair_quality_context_owner"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_state_keys"][7],
            "resumed_from_batch_id"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("startup command python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_startup_command_projection_owner_is_rust_only_and_surviving_startup_cancel_resume_surfaces_are_tracked_by_external_route_contracts"
        );
    }

    #[test]
    fn should_publish_runtime_driver_owner_contract() {
        let contract = super::build_batch_generation_runtime_driver_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::runtime_driver_execution_chain"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_lifecycle_entrypoints"][0],
            "BatchGenerationRuntimeLifecyclePlan::start"
        );
        assert_eq!(
            contract["behavior_contract"]["step_execution_entrypoints"][2],
            "PreparedBatchGenerationStepExecution::execute"
        );
        assert_eq!(
            contract["behavior_contract"]["post_write_and_terminal_entrypoints"][4],
            "BatchGenerationPostAnalysisTerminalPlan::execute"
        );
        assert_eq!(
            contract["behavior_contract"]["follow_up_analysis_entrypoints"][1],
            "BatchGenerationFollowUpAnalysisPlan::execute"
        );
        assert_eq!(
            contract["behavior_contract"]["driver_outcome_contract"]["retry_owner"],
            "BatchGenerationAttemptProgression::Retry"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_dependencies"][3],
            "load_recent_batch_story_repair_quality_summary"
        );
        assert_eq!(
            contract["selected_candidate_event_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::selected_candidate_event_owner"
        );
        assert_eq!(
            contract["follow_up_analysis_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::follow_up_analysis_runtime_projection"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["owner"],
            "chapter_generation_runtime_service::story_repair_quality_context_owner"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            "chapter_generation_runtime_service::snapshot_persistence_owner"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["behavior_contract"]["write_functions"]
                [1],
            "upsert_chapter_generation_runtime_snapshot"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter_batch_generation_active_gateway_smoke_service"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("runtime driver python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_runtime_driver_owner_is_rust_only_and_surviving_driver_orchestration_surfaces_are_tracked_by_external_runtime_contracts"
        );
    }

    #[test]
    fn should_publish_runtime_persistence_owner_contract() {
        let contract = build_batch_generation_runtime_persistence_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::runtime_persistence_task_mutation_projection"
        );
        assert_eq!(
            contract["behavior_contract"]["task_mutation_entrypoints"][0],
            "BatchGenerationResumeTaskResetMutationPlan::from_reset_semantics"
        );
        assert_eq!(
            contract["behavior_contract"]["task_stage_helpers"][2],
            "BatchGenerationTaskStage::completed_at_update"
        );
        assert_eq!(
            contract["behavior_contract"]["failed_entry_entrypoints"][2],
            "build_quality_gate_blocked_failed_chapter_entry"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_checkpoint_entrypoints"][6],
            "BatchGenerationRuntimePersistencePlan::persist"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            "chapter_generation_runtime_service::snapshot_persistence_owner"
        );
        assert_eq!(
            contract["terminal_runtime_patch_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service::terminal_runtime_patch"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["active_consumers"][2],
            "chapter_batch_generation_runtime_state_service::retry_routing_owner"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_runtime_persistence_owner_is_rust_only_and_surviving_task_mutation_failed_entry_surfaces_are_tracked_by_external_persistence_contracts"
        );
    }

    #[test]
    fn should_publish_attempt_input_owner_contract() {
        let contract = build_batch_generation_attempt_input_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::attempt_input_prompt_provider_gateway_execution"
        );
        assert_eq!(
            contract["behavior_contract"]["compat_restore_entrypoints"][0],
            "BatchGenerationAttemptInputPlan::resolve_compat_options"
        );
        assert_eq!(
            contract["behavior_contract"]["prepare_entrypoints"][2],
            "build_single_chapter_research_provider_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["execute_entrypoints"][1],
            "generate_and_persist_chapter_content_with_candidate_route_gateway"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            "chapter_generation_runtime_service::snapshot_persistence_owner"
        );
        assert_eq!(
            contract["request_runtime_state_owner_contract"]["owner"],
            "chapter_generation_execution_contract_service::request_runtime_state"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter_batch_generation_runtime_state_service::runtime_driver_owner"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("attempt input python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_attempt_input_owner_is_rust_only_and_surviving_prompt_provider_gateway_surfaces_are_tracked_by_external_runtime_contracts"
        );
    }

    #[test]
    fn should_publish_quality_payload_owner_contract() {
        let contract = build_batch_generation_quality_payload_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::quality_payload_current_quality_projection"
        );
        assert_eq!(
            contract["behavior_contract"]["plot_analysis_projection_entrypoints"][1],
            "build_current_chapter_quality_summary_from_plot_analysis"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_payload_entrypoints"][1],
            "build_batch_generation_runtime_state_payload_from_current_quality"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            "chapter_generation_runtime_service::quality_runtime_context_owner"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["owner"],
            "chapter_generation_runtime_service::story_repair_quality_context_owner"
        );
        assert_eq!(
            contract["request_runtime_state_owner_contract"]["owner"],
            "chapter_generation_execution_contract_service::request_runtime_state"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter_batch_generation_runtime_state_service::follow_up_analysis_owner"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("quality payload python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_quality_payload_owner_is_rust_only_and_surviving_quality_runtime_surfaces_are_tracked_by_external_runtime_contracts"
        );
    }

    #[test]
    fn should_publish_terminal_semantics_owner_contract() {
        let contract = super::build_batch_generation_terminal_semantics_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::quality_gate_terminal_semantics_projection"
        );
        assert_eq!(
            contract["behavior_contract"]["terminal_resolution_entrypoints"][0],
            "resolve_batch_generation_quality_gate_terminal_semantics"
        );
        assert_eq!(
            contract["task_payload_owner_contract"]["owner"],
            "chapter_batch_generation_task_payload_base_service"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("terminal semantics python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["active_consumers"][2],
            "chapter_batch_generation_active_gateway_smoke_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_terminal_semantics_owner_is_rust_only_and_surviving_quality_terminal_surfaces_are_tracked_by_external_task_contracts"
        );
    }

    #[test]
    fn should_publish_runtime_launch_owner_contract() {
        let contract = super::build_batch_generation_runtime_launch_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::runtime_launch_session_dispatch"
        );
        assert_eq!(
            contract["behavior_contract"]["execution_input_entrypoints"][0],
            "build_batch_generation_execution_input"
        );
        assert_eq!(
            contract["behavior_contract"]["dispatch_entrypoints"][1],
            "BatchGenerationRuntimeLifecyclePlan::start"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("runtime launch python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["active_consumers"][4],
            "chapter_batch_generation_write_workflow_service::create_launch_owner"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_runtime_launch_owner_is_rust_only_and_surviving_launch_dispatch_surfaces_are_tracked_by_external_runtime_contracts"
        );
    }

    #[test]
    fn should_publish_runtime_snapshot_owner_contract() {
        let contract = super::build_batch_generation_runtime_snapshot_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_runtime_state_service::runtime_snapshot_merge_write_projection"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_state_entrypoints"][0],
            "merge_batch_generation_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["snapshot_write_entrypoints"][1],
            "persist_batch_generation_runtime_plan"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("runtime snapshot python source map"),
            &Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["source_map_closeout_status"]
                ["compat_shell_status"],
            "physically_deleted"
        );
        assert_eq!(
            contract["active_consumers"][3],
            "chapter_batch_generation_runtime_state_service::follow_up_analysis_owner"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_runtime_snapshot_owner_is_rust_only_and_surviving_snapshot_runtime_surfaces_are_tracked_by_external_persistence_contracts"
        );
    }

    fn test_candidate_gateway_config() -> ChapterCandidateRouteGatewayConfig {
        ChapterCandidateRouteGatewayConfig {
            rust_executor_enabled: true,
            fallback_on_rust_error: false,
            disabled_reason: Some("test batch candidate gateway".to_string()),
            rollback_boundary: "test_batch_candidate_gateway".to_string(),
        }
    }

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
        assert!(checkpoint["analysis_task_id"].is_null());
        assert!(checkpoint["analysis_started_chapter_id"].is_null());
    }

    #[test]
    fn should_compute_batch_running_progress_with_floor_and_clamp() {
        assert_eq!(compute_batch_running_progress(0, 0), 15);
        assert_eq!(compute_batch_running_progress(2, 5), 55);
        assert_eq!(compute_batch_running_progress(5, 5), 95);
        assert_eq!(compute_batch_running_progress(7, 5), 95);
    }

    #[test]
    fn should_build_pending_batch_generation_runtime_checkpoint_for_queue_and_resume() {
        let queued = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Queued,
            None,
            None,
            0,
            4,
        );
        assert_eq!(queued["phase"], "pending");
        assert_eq!(queued["progress"], 0);
        assert_eq!(queued["status"], "pending");
        assert_eq!(queued["last_event"], "queued");
        assert_eq!(queued["last_message"], "批量生成任务已创建，等待开始...");
        assert_eq!(queued["completed"], 0);
        assert_eq!(queued["total"], 4);
        assert!(queued["analysis_task_id"].is_null());

        let resumed = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Resumed {
                include_progress_totals: false,
            },
            Some("chapter-3"),
            Some(3),
            0,
            5,
        );
        assert_eq!(resumed["phase"], "pending");
        assert_eq!(resumed["status"], "pending");
        assert_eq!(resumed["last_event"], "resume");
        assert_eq!(
            resumed["last_message"],
            "批量生成任务已恢复，等待重新开始..."
        );
        assert!(resumed["analysis_task_id"].is_null());
        assert!(resumed.get("completed").is_none());
        assert!(resumed.get("total").is_none());
    }

    #[test]
    fn should_build_pending_batch_generation_runtime_checkpoint_with_progress_totals() {
        let checkpoint = build_pending_batch_generation_runtime_checkpoint(
            "queued",
            "批量生成任务已创建，等待开始...",
            None,
            None,
            Some((0, 4)),
        );

        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(checkpoint["progress"], 0);
        assert_eq!(checkpoint["status"], "pending");
        assert_eq!(checkpoint["last_event"], "queued");
        assert_eq!(checkpoint["completed"], 0);
        assert_eq!(checkpoint["total"], 4);
        assert!(checkpoint.get("analysis_task_id").is_none());
        assert!(checkpoint["chapter_id"].is_null());
    }

    #[test]
    fn should_build_cancelled_and_failed_runtime_checkpoints() {
        let cancelled = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Cancelled,
            None,
            None,
            2,
            5,
        );
        assert_eq!(cancelled["phase"], "cancelled");
        assert_eq!(cancelled["progress"], 100);
        assert_eq!(cancelled["status"], "cancelled");
        assert_eq!(cancelled["last_event"], "cancelled");
        assert_eq!(cancelled["last_message"], "批量生成已取消");
        assert!(cancelled["analysis_task_id"].is_null());

        let failed = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Failed(BatchGenerationFailureKind::LoadChapterError),
            Some("chapter-2"),
            Some(2),
            1,
            5,
        );
        assert_eq!(failed["phase"], "failed");
        assert_eq!(failed["progress"], 100);
        assert_eq!(failed["status"], "failed");
        assert_eq!(failed["last_event"], "error");
        assert_eq!(failed["last_message"], "批量生成失败：加载章节异常");
        assert!(failed["analysis_task_id"].is_null());
    }

    #[test]
    fn should_resolve_checkpoint_message_for_batch_failure_kind() {
        assert_eq!(
            checkpoint_message_for_batch_generation_failure(
                BatchGenerationFailureKind::MissingChapter
            ),
            "批量生成失败：章节不存在"
        );
        assert_eq!(
            checkpoint_message_for_batch_generation_failure(
                BatchGenerationFailureKind::LoadChapterError
            ),
            "批量生成失败：加载章节异常"
        );
        assert_eq!(
            checkpoint_message_for_batch_generation_failure(
                BatchGenerationFailureKind::GenerationError
            ),
            "批量生成失败"
        );
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
    fn should_prepare_resume_restored_runtime_state_owner_contract() {
        let mut task = build_task("failed");
        task.id = "task-resume-owner-1".to_string();
        task.current_retry_count = 2;
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let snapshot = build_snapshot_with_runtime_state(
            json!({
                "phase": "failed",
                "last_event": "error",
                "active_story_repair_payload": {
                    "summary": "继续沿用"
                }
            }),
            Some(json!([{"overall_score": 88}])),
        );

        let (restored_runtime_state, existing_workflow_runtime_state) =
            prepare_batch_generation_resume_restored_runtime_state(&command_state, Some(&snapshot))
                .expect("resume launch restored state");

        assert_eq!(
            existing_workflow_runtime_state.expect("workflow runtime state")["phase"],
            "failed"
        );
        let runtime_state_seed = restored_runtime_state
            .runtime_state_seed
            .as_ref()
            .expect("runtime state seed");
        assert_eq!(runtime_state_seed["current_retry_count"], 0);
        assert_eq!(runtime_state_seed["max_retries"], 3);
        assert_eq!(
            runtime_state_seed["resume_from_batch_id"],
            "task-resume-owner-1"
        );
        assert_eq!(
            runtime_state_seed["active_story_repair_payload"]["summary"],
            "继续沿用"
        );
        assert_eq!(
            runtime_state_seed["quality_metrics_history"][0]["overall_score"],
            88
        );
    }

    #[test]
    fn should_reject_resume_restore_when_status_is_not_resumable() {
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("running"));

        let result = prepare_batch_generation_resume_restored_runtime_state(&command_state, None);

        assert!(matches!(
            result,
            Err(PrepareBatchGenerationResumeRuntimeStateError::InvalidStatus)
        ));
    }

    #[test]
    fn should_allow_resume_restore_when_manual_review_is_telemetry_only() {
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = build_snapshot_with_runtime_state(
            json!({
                "active_story_repair_payload": {
                    "summary": "需要人工处理",
                    "quality_gate_decision": "manual_review",
                    "quality_gate_label": "建议继续修复"
                }
            }),
            None,
        );

        let result =
            prepare_batch_generation_resume_restored_runtime_state(&command_state, Some(&snapshot));

        assert!(result.is_ok());
    }

    #[test]
    fn should_build_new_batch_generation_task_runtime_snapshot_for_queue() {
        let snapshot = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Queued,
            None,
            None,
            0,
            4,
        );

        assert_eq!(snapshot["phase"], "pending");
        assert_eq!(snapshot["progress"], 0);
        assert_eq!(snapshot["status"], "pending");
        assert_eq!(snapshot["last_event"], "queued");
        assert_eq!(snapshot["last_message"], "批量生成任务已创建，等待开始...");
        assert_eq!(snapshot["completed"], 0);
        assert_eq!(snapshot["total"], 4);
    }

    #[test]
    fn should_build_batch_generation_queued_snapshot_plan_from_runtime_seed() {
        let plan = BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
            4,
            Some(json!({
                "quality_metrics_summary": {"chapter_count": 2},
                "active_story_repair_payload": {"summary": "沿用修复建议"}
            })),
        );

        assert_eq!(plan.runtime_state()["phase"], "pending");
        assert_eq!(plan.runtime_state()["last_event"], "queued");
        assert_eq!(plan.runtime_state()["total"], 4);
        assert_eq!(
            plan.runtime_state()["quality_metrics_summary"]["chapter_count"],
            2
        );
        assert_eq!(
            plan.runtime_state()["active_story_repair_payload"]["summary"],
            "沿用修复建议"
        );
    }

    #[test]
    fn should_expose_response_ready_quality_contract_from_batch_generation_queued_snapshot_plan() {
        let plan = BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
            2,
            Some(json!({
                "quality_metrics_summary": {
                    "chapter_count": 2,
                    "overall_score": 86.0,
                    "quality_runtime_context": {
                        "recent_metrics": [
                            {"overall_score": 86}
                        ],
                        "history_scope": "batch"
                    }
                },
                "quality_metrics_summary_state": {
                    "scope": "batch",
                    "chapter_count": 2,
                    "first_overall_score": 82.0,
                    "last_overall_score": 86.0
                },
                "quality_metrics_history": [
                    {"overall_score": 82},
                    {"overall_score": 86}
                ],
                "latest_quality_metrics": {
                    "overall_score": 86,
                    "quality_gate": {
                        "decision": "repair"
                    }
                },
                "quality_history_context": {
                    "scope": "batch",
                    "source": "queued_snapshot_test"
                },
                "active_story_repair_payload": {
                    "summary": "沿用批量修复建议",
                    "repair_targets": ["压缩说明"],
                    "source": "recent_history_summary",
                    "scope": "batch"
                }
            })),
        );

        let quality_runtime_context = plan.quality_runtime_context();

        assert_eq!(
            quality_runtime_context
                .quality_metrics_summary
                .as_ref()
                .and_then(|summary| summary.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            quality_runtime_context
                .latest_quality_metrics
                .as_ref()
                .and_then(|metrics| metrics.get("overall_score")),
            Some(&json!(86))
        );
        assert_eq!(
            plan.quality_history_context()
                .as_ref()
                .and_then(|context| context.get("source")),
            Some(&json!("queued_snapshot_test"))
        );
        assert_eq!(
            plan.active_story_repair_payload()
                .as_ref()
                .and_then(|payload| payload.get("summary")),
            Some(&json!("沿用批量修复建议"))
        );
    }

    #[test]
    fn should_build_batch_generation_resume_snapshot_plan_from_existing_runtime_state() {
        let plan = BatchGenerationResumeSnapshotPlan::from_resume_checkpoint(
            Some(json!({
                "phase": "failed",
                "last_event": "error",
                "quality_metrics_history": [{"overall_score": 79}]
            })),
            json!({
                "phase": "pending",
                "last_event": "resume",
                "current_chapter_id": "chapter-2"
            }),
        );

        assert_eq!(plan.runtime_state()["phase"], "pending");
        assert_eq!(plan.runtime_state()["last_event"], "resume");
        assert_eq!(plan.runtime_state()["current_chapter_id"], "chapter-2");
        assert_eq!(
            plan.runtime_state()["quality_metrics_history"][0]["overall_score"],
            79
        );
    }

    #[test]
    fn should_merge_object_runtime_state_updates_into_existing_snapshot_state() {
        let merged = merge_batch_generation_runtime_state(
            Some(json!({
                "phase": "generating",
                "progress": 45,
                "checkpoint": {"completed": 1, "total": 3}
            })),
            json!({
                "progress": 60,
                "last_event": "progress"
            }),
        );

        assert_eq!(merged["phase"], "generating");
        assert_eq!(merged["progress"], 60);
        assert_eq!(merged["checkpoint"]["completed"], 1);
        assert_eq!(merged["checkpoint"]["total"], 3);
        assert_eq!(merged["last_event"], "progress");
    }

    #[test]
    fn should_replace_runtime_state_when_existing_snapshot_state_is_not_object() {
        let merged = merge_batch_generation_runtime_state(
            Some(json!(["stale-array-state"])),
            json!({"phase": "pending"}),
        );

        assert_eq!(merged, json!({"phase": "pending"}));
    }

    #[test]
    fn should_replace_runtime_state_when_incoming_snapshot_state_is_not_object() {
        let merged = merge_batch_generation_runtime_state(
            Some(json!({"phase": "generating", "progress": 45})),
            json!(["terminal-array-state"]),
        );

        assert_eq!(merged, json!(["terminal-array-state"]));
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
    fn should_prepare_cancel_persistence_plan_from_runtime_owner() {
        let mut task = build_task("running");
        task.id = "task-cancel-owner-1".to_string();
        task.project_id = "project-cancel-1".to_string();
        task.total_chapters = 2;
        task.completed_chapters = 1;
        task.current_chapter_id = Some("chapter-2".to_string());
        task.current_chapter_number = Some(2);

        let snapshot = build_snapshot(
            None,
            None,
            Some(json!({
                "progress": 55,
                "phase": "generating",
                "status": "running"
            })),
        );

        let persistence_plan =
            prepare_batch_generation_cancel_persistence_plan(&task, Some(&snapshot))
                .expect("running task should prepare cancel persistence plan");
        let payload = persistence_plan.response_payload_for_test(batch_generation_task::Model {
            status: "cancelled".to_string(),
            ..task
        });

        assert_eq!(payload["batch_id"], "task-cancel-owner-1");
        assert_eq!(payload["status"], "cancelled");
        assert_eq!(payload["message"], "Batch generation cancelled");
        assert_eq!(payload["checkpoint"]["phase"], "cancelled");
    }

    #[test]
    fn should_reject_terminal_status_inside_cancel_prepare_runtime_owner() {
        let mut task = build_task("cancelled");
        task.id = "task-cancel-owner-2".to_string();

        let error = prepare_batch_generation_cancel_persistence_plan(&task, None)
            .expect_err("cancelled task should fail cancel preparation");

        assert!(matches!(
            error,
            PrepareBatchGenerationCancelPersistenceError::InvalidStatus(ref status)
                if status == "cancelled"
        ));
        assert_eq!(
            error.detail_message(),
            "Cannot cancel task in status cancelled"
        );
    }

    async fn setup_cancel_runtime_owner_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = db.get_database_backend();
        let schema = Schema::new(builder);

        db.execute(builder.build(&schema.create_table_from_entity(batch_generation_task::Entity)))
            .await
            .expect("create batch_generation_task table");
        db.execute(
            builder.build(&schema.create_table_from_entity(batch_generation_snapshot::Entity)),
        )
        .await
        .expect("create batch_generation_snapshot table");

        db
    }

    async fn seed_cancel_runtime_owner_fixture(db: &DatabaseConnection) {
        let now = chrono::Utc::now().naive_utc();

        batch_generation_task::ActiveModel {
            id: Set("batch-cancel-db-smoke".to_string()),
            project_id: Set("project-cancel-db-smoke".to_string()),
            user_id: Set("user-cancel-db-smoke".to_string()),
            start_chapter_number: Set(2),
            chapter_count: Set(3),
            chapter_ids: Set(json!([
                "chapter-cancel-2",
                "chapter-cancel-3",
                "chapter-cancel-4"
            ])),
            style_id: Set(None),
            target_word_count: Set(2800),
            enable_analysis: Set(true),
            status: Set("running".to_string()),
            total_chapters: Set(3),
            completed_chapters: Set(1),
            failed_chapters: Set(json!([])),
            current_chapter_id: Set(Some("chapter-cancel-3".to_string())),
            current_chapter_number: Set(Some(3)),
            current_retry_count: Set(0),
            max_retries: Set(3),
            created_at: Set(Some(now)),
            started_at: Set(Some(now)),
            completed_at: Set(None),
            error_message: Set(None),
        }
        .insert(db)
        .await
        .expect("insert cancel db smoke task");

        batch_generation_snapshot::ActiveModel {
            id: Set("snapshot-cancel-db-smoke".to_string()),
            batch_task_id: Set("batch-cancel-db-smoke".to_string()),
            latest_quality_metrics: Set(Some(json!({
                "overall_score": 89.0,
                "source": "cancel-db-smoke"
            }))),
            quality_metrics_history: Set(None),
            quality_metrics_summary: Set(Some(json!({
                "chapter_count": 1,
                "avg_score": 89.0
            }))),
            workflow_runtime_state: Set(Some(json!({
                "phase": "generating",
                "progress": 48,
                "last_event": "selected_candidate",
                "last_message": "Rust cancel smoke before cancellation",
                "candidate_gateway": {
                    "execution_path": "rust_candidate_executor",
                    "fallback_applied": false,
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "rust_error": null
                },
                "active_story_repair_payload": {
                    "scope": "batch",
                    "mode": "cancel-db-smoke"
                }
            }))),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .expect("insert cancel db smoke snapshot");
    }

    #[tokio::test]
    async fn should_persist_db_backed_cancelled_batch_generation_from_runtime_command_owner() {
        let db = setup_cancel_runtime_owner_db().await;
        seed_cancel_runtime_owner_fixture(&db).await;

        let payload = super::cancel_owned_batch_generation_runtime_command(
            &db,
            "batch-cancel-db-smoke",
            "user-cancel-db-smoke",
        )
        .await
        .expect("db-backed cancel payload");
        let updated_task = batch_generation_task::Entity::find_by_id("batch-cancel-db-smoke")
            .one(&db)
            .await
            .expect("load cancelled task")
            .expect("cancelled task exists");
        let updated_snapshot = batch_generation_snapshot::Entity::find()
            .filter(batch_generation_snapshot::Column::BatchTaskId.eq("batch-cancel-db-smoke"))
            .one(&db)
            .await
            .expect("load cancelled snapshot")
            .expect("cancelled snapshot exists");
        let runtime_state = updated_snapshot
            .workflow_runtime_state
            .expect("cancelled runtime state");

        assert_eq!(payload["batch_id"], "batch-cancel-db-smoke");
        assert_eq!(payload["status"], "cancelled");
        assert_eq!(payload["message"], "Batch generation cancelled");
        assert_eq!(payload["checkpoint"]["phase"], "cancelled");
        assert_eq!(payload["checkpoint"]["status"], "cancelled");
        assert_eq!(payload["checkpoint"]["progress"], 100);
        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["mode"],
            "cancel-db-smoke"
        );
        assert_eq!(payload["terminal_reason"], "cancelled");
        assert_eq!(payload["can_resume"], true);

        assert_eq!(updated_task.status, "cancelled");
        assert!(updated_task.completed_at.is_some());
        assert_eq!(runtime_state["phase"], "cancelled");
        assert_eq!(runtime_state["status"], "cancelled");
        assert_eq!(runtime_state["progress"], 100);
        assert_eq!(runtime_state["last_event"], "cancelled");
        assert_eq!(runtime_state["last_message"], "批量生成已取消");
        assert_eq!(
            runtime_state["candidate_gateway"]["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            runtime_state["active_story_repair_payload"]["mode"],
            "cancel-db-smoke"
        );
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
                provider_payload: crate::services::chapter_generation_prompt_service::build_placeholder_prompt_context_provider_payload(),
            },
            test_candidate_gateway_config(),
        );

        assert_eq!(input.user_id, "user-10");
        assert_eq!(input.chapter_ids, vec!["chapter-3".to_string()]);
        assert_eq!(input.target_word_count, 2800);
        assert_eq!(input.ai_config.provider, AIConfig::default().provider);
        assert!(input.candidate_gateway_config.rust_executor_enabled);
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
                provider_payload: crate::services::chapter_generation_prompt_service::build_placeholder_prompt_context_provider_payload(),
            },
            test_candidate_gateway_config(),
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

        let (startup_snapshot_plan, startup_input) =
            super::build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed(
                "user-10".to_string(),
                vec!["chapter-3".to_string(), "chapter-4".to_string()],
                2,
                3200,
                runtime_state_seed,
                PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: crate::services::chapter_generation_prompt_service::build_placeholder_prompt_context_provider_payload(),
                },
                test_candidate_gateway_config(),
            );

        assert_eq!(
            startup_snapshot_plan.runtime_state()["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["candidate_gateway"]["rollback_boundary"],
            "test_batch_candidate_gateway"
        );
        assert!(startup_input.candidate_gateway_config.rust_executor_enabled);
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
                candidate_gateway_config: test_candidate_gateway_config(),
            });

        assert_eq!(session.user_id, "user-10");
        assert_eq!(session.target_word_count, 2800);
        assert_eq!(session.total_chapters, 2);
        assert!(session.candidate_gateway_config.rust_executor_enabled);
        assert!(!session.candidate_gateway_config.fallback_on_rust_error);
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
    fn should_build_selected_candidate_event_snapshot_from_generated_result_owner() {
        let snapshot = super::build_batch_generation_selected_candidate_event_snapshot(
            &GeneratedChapterResult {
                chapter_id: "chapter-3".to_string(),
                chapter_number: 3,
                title: "第三章".to_string(),
                content: "正文".to_string(),
                word_count: 1200,
                selected_candidate_event_source: Some(json!({
                    "full_content": "候选正文",
                    "candidate_chunks": ["候选", "正文"],
                    "candidate_index": 1,
                    "candidate_count": 2,
                    "winner_candidate_index": 1,
                    "generation_path": "rust_candidate_executor",
                    "quality_gate_plan": {"action": "continue"}
                })),
                ..Default::default()
            },
            true,
        )
        .expect("selected candidate event snapshot");

        assert_eq!(snapshot["last_event"], "selected_candidate");
        assert_eq!(snapshot["selected_candidate_events"][0]["type"], "progress");
        assert_eq!(
            snapshot["selected_candidate_events"][0]["generation_path"],
            "rust_candidate_executor"
        );
        assert_eq!(snapshot["selected_candidate_events"][1]["type"], "chunk");
        assert_eq!(snapshot["selected_candidate_events"][2]["content"], "正文");
    }

    #[test]
    fn should_skip_selected_candidate_chunks_for_retry_gate_snapshot() {
        let snapshot = super::build_batch_generation_selected_candidate_event_snapshot(
            &GeneratedChapterResult {
                chapter_id: "chapter-3".to_string(),
                chapter_number: 3,
                title: "第三章".to_string(),
                content: "正文".to_string(),
                word_count: 1200,
                selected_candidate_event_source: Some(json!({
                    "full_content": "候选正文",
                    "candidate_chunks": ["候选", "正文"],
                    "quality_gate_plan": {"action": "retry"}
                })),
                ..Default::default()
            },
            true,
        )
        .expect("selected candidate event snapshot");

        let events = snapshot["selected_candidate_events"]
            .as_array()
            .expect("selected candidate events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "progress");
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
        let patch = build_quality_gate_blocked_runtime_state_patch_from_workflow_state(
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
        let patch = build_quality_gate_blocked_runtime_state_patch_from_workflow_state(
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
            candidate_gateway_config: test_candidate_gateway_config(),
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
            crate::services::chapter_generation_prompt_service::PromptContextProviderPayload {
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
            ..Default::default()
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
        assert_eq!(
            BatchGenerationPostWriteGuardPlan::resolve(true, false),
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
    fn should_continue_post_write_guard_only_when_task_exists_and_chapter_content_written() {
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

    #[tokio::test]
    async fn should_fail_post_write_guard_when_generated_chapter_content_is_empty() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = db.get_database_backend();
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(batch_generation_task::Entity)))
            .await
            .expect("create batch_generation_task table");
        db.execute(builder.build(&schema.create_table_from_entity(chapter::Entity)))
            .await
            .expect("create chapter table");

        let now = chrono::Utc::now().naive_utc();
        batch_generation_task::ActiveModel {
            id: Set("batch-empty-content-guard".to_string()),
            project_id: Set("project-empty-content-guard".to_string()),
            user_id: Set("user-empty-content-guard".to_string()),
            start_chapter_number: Set(2),
            chapter_count: Set(2),
            chapter_ids: Set(json!([
                "chapter-empty-content-2",
                "chapter-empty-content-3"
            ])),
            style_id: Set(None),
            target_word_count: Set(2800),
            enable_analysis: Set(true),
            status: Set("running".to_string()),
            total_chapters: Set(2),
            completed_chapters: Set(0),
            failed_chapters: Set(json!([])),
            current_chapter_id: Set(Some("chapter-empty-content-2".to_string())),
            current_chapter_number: Set(Some(2)),
            current_retry_count: Set(0),
            max_retries: Set(3),
            created_at: Set(Some(now)),
            started_at: Set(Some(now)),
            completed_at: Set(None),
            error_message: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert batch task");

        chapter::ActiveModel {
            id: Set("chapter-empty-content-2".to_string()),
            project_id: Set("project-empty-content-guard".to_string()),
            chapter_number: Set(2),
            title: Set("第二章".to_string()),
            content: Set(Some("   ".to_string())),
            summary: Set(None),
            word_count: Set(0),
            status: Set("draft".to_string()),
            outline_id: Set(None),
            sub_index: Set(0),
            expansion_plan: Set(None),
            created_at: Set(now),
            updated_at: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert empty chapter");

        let error =
            super::BatchGenerationPostWriteGuardPlan::for_chapter("chapter-empty-content-2")
                .execute(&db, "batch-empty-content-guard")
                .await
                .expect_err("empty generated content should route to retry failure");

        assert_eq!(error, "章节生成完成后正文未写入");
    }

    #[test]
    fn should_not_project_manual_review_generated_result_into_quality_gate_runtime_state() {
        let runtime_state = super::build_non_applied_generated_result_quality_runtime_state(
            &GeneratedChapterResult {
                chapter_id: "chapter-manual-review".to_string(),
                chapter_number: 2,
                title: "第二章".to_string(),
                content: "候选正文".to_string(),
                word_count: 1200,
                content_applied: false,
                provisional_draft_saved: false,
                quality_gate_action: Some("manual_review".to_string()),
                quality_gate_message: Some("建议继续修复".to_string()),
                quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "manual_review",
                        "label": "建议继续修复",
                        "summary": "质量不足"
                    }
                })),
                candidate_draft: Some(json!({
                    "repair_payload": {
                        "quality_gate_decision": "manual_review",
                        "quality_gate_label": "建议继续修复",
                        "phase": "quality_blocked"
                    }
                })),
                ..Default::default()
            },
        );

        assert!(runtime_state.get("active_story_repair_payload").is_none());
        assert_eq!(
            runtime_state["latest_quality_metrics"]["quality_gate"]["decision"],
            "manual_review"
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
            BatchGenerationFailureKind::GenerationError,
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
            "第7章质量门禁未通过，建议继续修复: 自动修复预算已耗尽",
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
                "第7章质量门禁未通过，建议继续修复: 自动修复预算已耗尽".to_string(),
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
            Some("第7章质量门禁未通过，建议继续修复: 自动修复预算已耗尽")
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
        super::apply_manual_review_terminal_fields(&mut payload, "建议继续修复");
        assert_eq!(payload["quality_gate_decision"], "manual_review");
        assert_eq!(payload["quality_gate_label"], "建议继续修复");
        assert_eq!(payload["phase"], "quality_blocked");

        let mut gate = json!({});
        crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::normalize_terminal_quality_gate_payload(
            &mut gate,
            "建议继续修复",
        );
        assert_eq!(gate["quality_gate"]["status"], "failed");
        assert_eq!(gate["quality_gate"]["decision"], "manual_review");
        assert_eq!(gate["quality_gate"]["label"], "建议继续修复");
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
        shared_normalize_terminal_quality_history(&mut history, "建议继续修复");
        assert_eq!(history[0]["quality_gate"]["status"], "failed");
        assert_eq!(history[0]["quality_gate"]["decision"], "manual_review");
        assert_eq!(history[0]["quality_gate"]["label"], "建议继续修复");

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
        shared_normalize_terminal_quality_history_context(&mut context, "建议继续修复");
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
            "建议继续修复"
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
        apply_terminal_quality_runtime_patch_contract(
            &mut payload,
            Some(&runtime_state),
            runtime_state.get("active_story_repair_payload"),
            "建议继续修复",
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

        let mut payload = build_manual_review_terminal_runtime_patch_contract(7, "建议继续修复");
        apply_terminal_quality_runtime_patch_contract(
            &mut payload,
            Some(&runtime_state),
            runtime_state.get("active_story_repair_payload"),
            "建议继续修复",
        );

        assert_eq!(
            payload["analysis_task_message"],
            "第 7 章质量门禁未通过，建议继续修复"
        );
        assert_eq!(payload["analysis_task_progress"], 100);
        assert!(payload["analysis_last_error"].is_null());
        assert_eq!(payload["quality_gate_decision"], "manual_review");
        assert_eq!(payload["quality_gate_label"], "建议继续修复");
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
            "建议继续修复"
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
                "quality_gate_label": "建议继续修复",
                "phase": "quality_blocked"
            }
        });
        let payload = build_retry_quality_runtime_patch_contract_from_workflow_state(
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
                "quality_gate_label": "建议继续修复",
                "phase": "quality_blocked"
            }
        });

        let payload = build_retry_quality_runtime_patch_contract_from_workflow_state(
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
    fn should_not_resolve_manual_review_terminal_semantics_from_current_quality_runtime_state() {
        let current_quality_runtime_state = json!({
            "quality_metrics_summary": {
                "quality_gate": {
                    "status": "failed",
                    "decision": "manual_review",
                    "label": "建议继续修复"
                }
            },
            "latest_quality_metrics": {
                "quality_gate": {
                    "status": "failed",
                    "decision": "manual_review",
                    "label": "建议继续修复"
                }
            },
            "active_story_repair_payload": {
                "summary": "继续补强冲突",
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "建议继续修复",
                "phase": "quality_blocked"
            }
        });

        let semantics = super::resolve_batch_generation_quality_gate_terminal_semantics(
            None,
            Some(&current_quality_runtime_state),
            3,
            3,
        )
        .expect("fallback terminal semantics");

        assert_eq!(semantics.kind, BatchGenerationFailedTerminalKind::Error);
        assert_eq!(semantics.reason, "error");
        assert_eq!(semantics.label, "执行失败");
        assert!(!semantics.review_required);
        assert!(semantics.can_resume);
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
    fn should_not_route_quality_gate_manual_review_to_stop_owner() {
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
                    "quality_gate_label": "建议继续修复",
                    "phase": "quality_blocked"
                }
            })),
            3,
            3,
            BatchGenerationFailedTerminalSemantics {
                kind: BatchGenerationFailedTerminalKind::ManualReview,
                reason: "manual_review",
                label: "建议继续修复".to_string(),
                review_required: true,
                can_resume: false,
            },
        );
        assert_eq!(plan, None);
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
                candidate_gateway_config: test_candidate_gateway_config(),
            });

        let _ =
            BatchGenerationFollowUpAnalysisPlan::from_generated_result(&GeneratedChapterResult {
                chapter_id: "chapter-3".to_string(),
                chapter_number: 3,
                title: "第三章".to_string(),
                content: "正文".to_string(),
                word_count: 2,
                ..Default::default()
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
                candidate_gateway_config: test_candidate_gateway_config(),
            },
        );
    }
}
