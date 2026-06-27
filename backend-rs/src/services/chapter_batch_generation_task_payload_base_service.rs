use serde_json::{json, Value};

pub(crate) mod metadata_owner;
pub(crate) mod quality_terminal_status_owner;
pub(crate) mod task_view_payload_owner;

#[cfg(test)]
pub(crate) use self::metadata_owner::task_kind;
pub(crate) use self::metadata_owner::{
    batch_generation_stage_code, batch_generation_task_kind, batch_generation_task_type,
    build_batch_generation_command_summary_payload,
    build_batch_generation_payload_metadata_owner_contract, checkpoint_with_runtime_metadata,
    estimated_task_minutes, task_execution_mode, task_type, to_iso,
    BatchGenerationCommandProgressSummary, BatchGenerationTaskKind,
};
pub(crate) use self::quality_terminal_status_owner::{
    build_batch_generation_quality_terminal_status_owner_contract,
    insert_batch_generation_terminal_status_payload, resolve_failed_terminal_semantics,
    resolve_failed_terminal_semantics_from_sources, BatchGenerationFailedTerminalKind,
    BatchGenerationFailedTerminalSemantics, BatchGenerationQualityStatusContext,
};
#[allow(unused_imports)]
pub(crate) use self::task_view_payload_owner::{
    apply_batch_generation_loading_stage_fields,
    build_batch_generation_status_task_payload_from_task_and_snapshot_projection,
    build_batch_generation_status_task_payload_with_quality_context,
    build_batch_generation_task_response_payload_from_runtime_parts,
    build_batch_generation_task_runtime_payload,
    build_batch_generation_task_runtime_payload_from_task_state,
    build_batch_generation_task_view_payload_from_task_state,
    build_batch_generation_task_view_payload_owner_contract,
    build_batch_generation_task_view_payload_with_quality_context,
};

use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    build_generation_quality_runtime_owner_contract, BatchGenerationQualityRuntimeContext,
    GenerationQualityRuntimeContext,
};

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

pub(crate) fn build_chapter_batch_generation_task_payload_base_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_task_payload_base_service",
        "scope": "batch_generation_task_view_payload_checkpoint_and_terminal_semantics_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service/task_view_payload_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "batch_generation_task_kind",
                "task_kind",
                "batch_generation_task_type",
                "task_type",
                "build_batch_generation_task_view_payload_owner_contract",
                "checkpoint_with_runtime_metadata",
                "build_batch_generation_task_runtime_payload",
                "build_batch_generation_task_response_payload_from_runtime_parts",
                "build_batch_generation_task_runtime_payload_from_task_state",
                "build_batch_generation_task_view_payload_from_task_state",
                "build_batch_generation_task_view_payload_with_quality_context",
                "build_batch_generation_status_task_payload_with_quality_context",
                "build_batch_generation_status_task_payload_from_task_and_snapshot_projection",
                "insert_batch_generation_terminal_status_payload",
                "resolve_failed_terminal_semantics_from_sources"
            ],
            "checkpoint_fields": [
                "progress",
                "progress_phase",
                "stage_code",
                "execution_mode",
                "candidate_gateway",
                "rust_runtime",
                "rust_runtime_owner"
            ],
            "task_payload_fields": [
                "task_id",
                "project_id",
                "status",
                "progress",
                "completed_chapters",
                "total_chapters",
                "checkpoint",
                "terminal_reason",
                "terminal_label",
                "review_required",
                "can_resume",
                "active_story_repair_payload"
            ],
            "terminal_status_policy": [
                "completed -> terminal_reason completed and can_resume false",
                "cancelled -> terminal_reason cancelled and can_resume true",
                "failed manual_review -> review_required true and can_resume false",
                "failed retry_or_error -> execution_failed_label and can_resume true",
                "non_terminal -> null terminal fields and can_resume false"
            ],
            "quality_context_policy": "BatchGenerationQualityStatusContext projects snapshot and runtime quality fields into task/status payloads",
            "candidate_gateway_policy": "runtime candidate gateway metadata is projected only when workflow runtime state contains a valid object",
            "python_compat_policy": "checkpoint and status payload fields keep Python query snapshot compatible keys while Rust owns projection",
            "loading_stage_policy": "loading stage fields stay available for payload builders that need to surface in-progress write work"
        },
        "metadata_owner_contract": build_batch_generation_payload_metadata_owner_contract(),
        "quality_terminal_status_owner_contract": build_batch_generation_quality_terminal_status_owner_contract(),
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "task_view_payload_owner_contract": build_batch_generation_task_view_payload_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_task_payload_base_service",
            "cargo test api::health",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "task_payload_owner": "chapter_batch_generation_task_payload_base_service",
            "checkpoint_projection_owner": "chapter_batch_generation_task_payload_base_service::task_view_payload_owner",
            "quality_status_projection_owner": "BatchGenerationQualityStatusContext",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation task-payload direct source-map deleted; surviving Python closeout work for this aggregate owner is now limited to shared batch-generation-snapshot schema/runtime/database/API-test hold and shared batch-generation-task schema/runtime/API/test-support packages",
            "status": "rust_batch_generation_task_payload_owner_direct_source_map_deleted"
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
        "rollback_boundary": "batch_generation_task_payload_python_source_map"
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        apply_batch_generation_loading_stage_fields, batch_generation_stage_code,
        batch_generation_task_kind, batch_generation_task_type,
        build_batch_generation_command_summary_payload,
        build_batch_generation_status_task_payload_from_task_and_snapshot_projection,
        build_batch_generation_status_task_payload_with_quality_context,
        build_batch_generation_task_response_payload_from_runtime_parts,
        build_batch_generation_task_runtime_payload,
        build_batch_generation_task_runtime_payload_from_task_state,
        build_batch_generation_task_view_payload_from_task_state,
        build_batch_generation_task_view_payload_with_quality_context,
        checkpoint_with_runtime_metadata, insert_batch_generation_terminal_status_payload,
        resolve_failed_terminal_semantics, resolve_failed_terminal_semantics_from_sources,
        task_execution_mode, task_kind, task_type, BatchGenerationCommandProgressSummary,
        BatchGenerationFailedTerminalKind, BatchGenerationQualityStatusContext,
        BatchGenerationTaskKind, BatchGenerationTaskResponsePayloadOptions,
        BatchGenerationTaskResponseQualityPayload, BatchGenerationTaskViewPayloadVariant,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_generation_execution_contract_service::active_story_repair_payload_from_runtime_state;
    use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::BatchGenerationQualityRuntimeContext;

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
    fn should_publish_chapter_batch_generation_task_payload_base_owner_contract() {
        let contract = super::build_chapter_batch_generation_task_payload_base_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_task_payload_base_service"
        );
        assert_eq!(
            contract["scope"],
            "batch_generation_task_view_payload_checkpoint_and_terminal_semantics_owner"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs"
        );
        assert_eq!(contract["behavior_contract"]["entrypoints"][1], "task_kind");
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][2],
            "batch_generation_task_type"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][6],
            "build_batch_generation_task_runtime_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][10],
            "build_batch_generation_task_view_payload_with_quality_context"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][11],
            "build_batch_generation_status_task_payload_with_quality_context"
        );
        assert_eq!(
            contract["rust_owner_map"][7],
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["checkpoint_fields"][3],
            "execution_mode"
        );
        assert_eq!(
            contract["behavior_contract"]["terminal_status_policy"][2],
            "failed manual_review -> review_required true and can_resume false"
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
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
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
            "rust_batch_generation_task_payload_owner_direct_source_map_deleted"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "batch-generation task-payload direct source-map deleted; surviving Python closeout work for this aggregate owner is now limited to shared batch-generation-snapshot schema/runtime/database/API-test hold and shared batch-generation-task schema/runtime/API/test-support packages"
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["batch_generation_task_model"],
            "shared_python_runtime_api_and_test_support_reference"
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["default_python_module_consumers"],
            json!([
                "backend/tests/test_support/database_test_support.py",
                "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
            ])
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["dedicated_python_regression_surfaces"],
            json!([
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_batch_status_resume.py"
            ])
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["test_support_consumers"],
            json!([
                "backend/tests/test_support/batch_generation_status_read_owner_test_adapter.py",
                "backend/tests/test_support/batch_generation_orchestration_test_adapter.py",
                "backend/tests/test_support/batch_generation_route_test_adapter.py"
            ])
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["physical_closeout_ready"],
            json!(false)
        );
        assert_eq!(
            contract["validation_boundary"][0],
            "cargo test chapter_batch_generation_task_payload_base_service"
        );
        assert_eq!(
            contract["quality_terminal_status_owner_contract"]["python_source_map"],
            json!([])
        );
        assert_eq!(
            crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract()["source_map_closeout_status"]["shared_schema_hold_status"]["physical_closeout_ready"],
            json!(false)
        );
        assert_eq!(contract["validation_boundary"][1], "cargo test api::health");
        assert_eq!(
            contract["quality_terminal_status_owner_contract"]["owner"],
            "chapter_batch_generation_task_payload_base_service::quality_terminal_status_owner"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            "chapter_generation_runtime_service::quality_runtime_context_owner"
        );
        assert_eq!(
            contract["metadata_owner_contract"]["request_runtime_state_owner_contract"]["owner"],
            "chapter_generation_execution_contract_service::request_runtime_state"
        );
        assert_eq!(
            contract["metadata_owner_contract"]["candidate_runtime_state_owner_contract"]["owner"],
            "chapter_candidate_runtime_state_service"
        );
        assert_eq!(
            contract["metadata_owner_contract"]["python_source_map"],
            json!([])
        );
        assert_eq!(
            contract["metadata_owner_contract"]["service_runtime_closeout_status"]
                ["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["metadata_owner_contract"]["service_runtime_closeout_status"]["status"],
            "rust_batch_generation_payload_metadata_owner_source_map_deleted"
        );
        assert_eq!(
            contract["metadata_owner_contract"]["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "batch-generation payload metadata source-map package deleted; surviving Python closeout work is now limited to shared batch-generation-task schema/runtime/API/test-support packages"
        );
        assert_eq!(
            contract["metadata_owner_contract"]["rollback_boundary"]["source_map_policy"],
            "batch_generation_task_payload_metadata_owner_is_rust_only_and_surviving_python_schema_runtime_surfaces_are_tracked_by_shared_task_contracts"
        );
        assert_eq!(
            contract["metadata_owner_contract"]["shared_schema_hold_status"]
                ["batch_generation_task_model"],
            "shared_python_runtime_api_and_test_support_reference"
        );
        assert_eq!(
            contract["metadata_owner_contract"]["shared_schema_hold_status"]
                ["default_python_module_consumers"],
            json!([
                "backend/tests/test_support/database_test_support.py",
                "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
            ])
        );
        assert_eq!(
            contract["metadata_owner_contract"]["shared_schema_hold_status"]
                ["dedicated_python_regression_surfaces"],
            json!([
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_batch_status_resume.py"
            ])
        );
        assert_eq!(
            contract["metadata_owner_contract"]["shared_schema_hold_status"]
                ["test_support_consumers"],
            json!([
                "backend/tests/test_support/batch_generation_status_read_owner_test_adapter.py",
                "backend/tests/test_support/batch_generation_orchestration_test_adapter.py",
                "backend/tests/test_support/batch_generation_route_test_adapter.py"
            ])
        );
        assert_eq!(
            contract["metadata_owner_contract"]["shared_schema_hold_status"]
                ["physical_closeout_ready"],
            false
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["python_source_map"],
            json!([])
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["service_runtime_closeout_status"]
                ["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["service_runtime_closeout_status"]
                ["status"],
            "rust_batch_generation_task_view_payload_owner_direct_source_map_deleted"
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "batch-generation task-view payload direct source-map deleted; surviving Python closeout work for this owner is now limited to shared batch-generation-snapshot schema/runtime/database/API-test hold and shared batch-generation-task schema/runtime/API/test-support packages"
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["rollback_boundary"]["source_map_policy"],
            "batch_generation_task_view_payload_owner_is_rust_only_and_surviving_python_schema_runtime_surfaces_are_tracked_by_shared_task_contracts"
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["shared_schema_hold_status"]
                ["batch_generation_task_model"],
            "shared_python_runtime_api_and_test_support_reference"
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["shared_schema_hold_status"]
                ["default_python_module_consumers"],
            json!([
                "backend/tests/test_support/database_test_support.py",
                "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
            ])
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["shared_schema_hold_status"]
                ["dedicated_python_regression_surfaces"],
            json!([
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_batch_status_resume.py"
            ])
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["shared_schema_hold_status"]
                ["test_support_consumers"],
            json!([
                "backend/tests/test_support/batch_generation_status_read_owner_test_adapter.py",
                "backend/tests/test_support/batch_generation_orchestration_test_adapter.py",
                "backend/tests/test_support/batch_generation_route_test_adapter.py"
            ])
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["shared_schema_hold_status"]
                ["physical_closeout_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"],
            "batch_generation_task_payload_python_source_map"
        );
    }

    #[test]
    fn should_publish_batch_generation_quality_terminal_status_owner_contract() {
        let contract = super::build_batch_generation_quality_terminal_status_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_task_payload_base_service::quality_terminal_status_owner"
        );
        assert_eq!(
            contract["scope"],
            "batch_generation_quality_status_context_failed_terminal_semantics_and_status_payload_projection"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["quality_status_context_entrypoints"][0],
            "BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["failed_terminal_semantics_entrypoints"][2],
            "resolve_failed_terminal_semantics_from_sources"
        );
        assert_eq!(
            contract["behavior_contract"]["status_payload_projection_entrypoints"][2],
            "build_batch_generation_status_task_payload_from_task_and_snapshot_projection"
        );
        assert_eq!(
            contract["behavior_contract"]["projected_fields"][6],
            "terminal_reason"
        );
        assert_eq!(
            contract["validation_boundary"][0],
            "cargo test chapter_batch_generation_task_payload_base_service"
        );
        assert_eq!(
            contract["validation_boundary"][2],
            "cargo check --manifest-path backend-rs/Cargo.toml"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_batch_generation_quality_terminal_status_owner_direct_source_map_deleted"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "batch-generation quality-terminal-status direct source-map deleted; surviving Python closeout work for this owner is now limited to shared batch-generation-snapshot schema/runtime/database/API-test hold and shared batch-generation-task schema/runtime/API/test-support packages"
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["batch_generation_task_model"],
            "shared_python_runtime_api_and_test_support_reference"
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["default_python_module_consumers"],
            json!([
                "backend/tests/test_support/database_test_support.py",
                "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
            ])
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["dedicated_python_regression_surfaces"],
            json!([
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_batch_status_resume.py"
            ])
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["test_support_consumers"],
            json!([
                "backend/tests/test_support/batch_generation_status_read_owner_test_adapter.py",
                "backend/tests/test_support/batch_generation_orchestration_test_adapter.py",
                "backend/tests/test_support/batch_generation_route_test_adapter.py"
            ])
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["physical_closeout_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_quality_terminal_status_owner_is_rust_only_and_surviving_python_schema_runtime_surfaces_are_tracked_by_shared_task_contracts"
        );
    }

    #[test]
    fn should_resolve_batch_generation_task_type_semantics_inside_payload_owner() {
        let single = build_task_shape("pending", 1, json!(["chapter-1"]), 1);
        let batch = build_task_shape("pending", 2, json!(["chapter-1", "chapter-2"]), 2);
        let malformed_single =
            build_task_shape("pending", 1, json!({"chapter_id": "chapter-1"}), 1);

        assert_eq!(
            batch_generation_task_type(BatchGenerationTaskKind::SingleChapter),
            "chapter_single_generate"
        );
        assert_eq!(
            batch_generation_task_type(BatchGenerationTaskKind::Batch),
            "chapters_batch_generate"
        );
        assert_eq!(
            batch_generation_task_kind(1, &json!(["chapter-1"])),
            BatchGenerationTaskKind::SingleChapter
        );
        assert_eq!(
            batch_generation_task_kind(2, &json!(["chapter-1", "chapter-2"])),
            BatchGenerationTaskKind::Batch
        );
        assert_eq!(task_kind(&single), BatchGenerationTaskKind::SingleChapter);
        assert_eq!(task_kind(&batch), BatchGenerationTaskKind::Batch);
        assert_eq!(task_kind(&malformed_single), BatchGenerationTaskKind::Batch);
        assert_eq!(task_type(&single), "chapter_single_generate");
        assert_eq!(task_type(&batch), "chapters_batch_generate");
        assert_eq!(task_type(&malformed_single), "chapters_batch_generate");
    }

    #[test]
    fn should_preserve_python_status_response_builder_task_type_contract_inside_payload_owner() {
        let python_single_shape =
            build_task_shape("pending", 1, json!(["chapter-python-compat"]), 1);
        let python_batch_shape = build_task_shape(
            "running",
            3,
            json!(["chapter-1", "chapter-2", "chapter-3"]),
            3,
        );
        let python_empty_single_shape = build_task_shape("pending", 1, json!([]), 1);
        let python_object_single_shape =
            build_task_shape("pending", 1, json!({"id": "chapter-python-compat"}), 1);

        assert_eq!(task_type(&python_single_shape), "chapter_single_generate");
        assert_eq!(task_type(&python_batch_shape), "chapters_batch_generate");
        assert_eq!(
            task_type(&python_empty_single_shape),
            "chapters_batch_generate"
        );
        assert_eq!(
            task_type(&python_object_single_shape),
            "chapters_batch_generate"
        );
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
    fn should_build_batch_generation_task_response_payload_from_runtime_parts() {
        let payload = build_batch_generation_task_response_payload_from_runtime_parts(
            "task-9",
            "chapters_batch_generate",
            "project-9",
            "pending",
            Some("chapter-7"),
            None,
            Some(&json!({
                "progress": 42,
                "candidate_gateway": {
                    "execution_path": "rust_candidate_executor",
                    "fallback_applied": false,
                    "rollback_boundary": "python_candidate_executor_fallback"
                }
            })),
            BatchGenerationTaskResponsePayloadOptions {
                checkpoint_override: Some(("chapter_id".to_string(), json!("chapter-8"))),
                ..Default::default()
            },
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
        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(payload["candidate_gateway"]["fallback_applied"], false);
        assert_eq!(
            payload["checkpoint"]["candidate_gateway"],
            payload["candidate_gateway"]
        );
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
    fn should_project_candidate_gateway_from_runtime_state_to_task_payload() {
        let task = build_task("completed");
        let runtime_state = json!({
            "progress": 100,
            "candidate_gateway": {
                "execution_path": "rust_candidate_executor",
                "fallback_applied": false,
                "fallback_reason": "rust executor completed",
                "rollback_boundary": "python_candidate_executor_fallback",
                "rust_error": null
            }
        });

        let payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&runtime_state),
        );

        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(payload["candidate_gateway"]["fallback_applied"], false);
        assert_eq!(
            payload["candidate_gateway"]["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            payload["checkpoint"]["candidate_gateway"],
            payload["candidate_gateway"]
        );
    }

    #[test]
    fn should_not_invent_candidate_gateway_for_invalid_runtime_metadata() {
        let task = build_task("running");
        let runtime_state = json!({
            "progress": 42,
            "candidate_gateway": "not-an-object"
        });

        let payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&runtime_state),
        );

        assert!(payload.get("candidate_gateway").is_none());
        assert_eq!(payload["checkpoint"]["candidate_gateway"], "not-an-object");
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
    fn should_build_status_task_payload_from_task_and_snapshot_projection_owner() {
        let mut snapshot = snapshot_with_quality_fields();
        snapshot.workflow_runtime_state = Some(json!({
            "progress": 60,
            "candidate_gateway": {
                "execution_path": "rust_candidate_executor",
                "fallback_applied": false,
                "fallback_reason": "rust executor completed",
                "rollback_boundary": "python_candidate_executor_fallback",
                "rust_error": null
            },
            "active_story_repair_payload": {
                "mode": "repair"
            }
        }));
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .as_ref()
            .expect("snapshot runtime state");
        let payload = build_batch_generation_status_task_payload_from_task_and_snapshot_projection(
            &build_task("completed"),
            Some(&snapshot),
            Some(workflow_runtime_state),
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["current_retry_count"], 2);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(payload["terminal_reason"], "completed");
        assert_eq!(payload["review_required"], false);
        assert_eq!(payload["can_resume"], false);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            payload["checkpoint"]["candidate_gateway"],
            payload["candidate_gateway"]
        );
    }

    #[test]
    fn should_build_status_task_payload_from_quality_context_owner() {
        let payload = build_batch_generation_status_task_payload_with_quality_context(
            &build_task("failed"),
            Some(&json!({"progress": 80})),
            &BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({"score": 88})),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: Some(json!({"summary": "good"})),
                quality_history_context: None,
                active_story_repair_payload: Some(json!({"mode": "repair"})),
            },
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 80);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.failed");
        assert_eq!(payload["current_retry_count"], 2);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(payload["latest_quality_metrics"]["score"], 88);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "good");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_batch_generation_task_response_payload_from_runtime_parts_with_checkpoint_override(
    ) {
        let task = build_task("running");

        let payload = build_batch_generation_task_response_payload_from_runtime_parts(
            &task.id,
            task_type(&task),
            &task.project_id,
            &task.status,
            task.current_chapter_id.as_deref(),
            task.created_at,
            Some(&json!({"progress": 42})),
            BatchGenerationTaskResponsePayloadOptions {
                checkpoint_override: Some(("chapter_id".to_string(), json!("chapter-9"))),
                ..Default::default()
            },
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
        let mut payload = build_batch_generation_task_response_payload_from_runtime_parts(
            "task-9",
            "chapters_batch_generate",
            "project-9",
            "pending",
            Some("chapter-7"),
            None,
            Some(&json!({"progress": 42})),
            BatchGenerationTaskResponsePayloadOptions {
                checkpoint_override: Some(("chapter_id".to_string(), json!("chapter-8"))),
                ..Default::default()
            },
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

        let payload = active_story_repair_payload_from_runtime_state(Some(&runtime_state));

        assert_eq!(payload, Some(json!({"mode": "repair", "attempt": 2})));
    }

    #[test]
    fn should_ignore_non_object_active_story_repair_payload() {
        let runtime_state = json!({
            "active_story_repair_payload": "not-an-object"
        });

        assert_eq!(
            active_story_repair_payload_from_runtime_state(Some(&runtime_state)),
            None
        );
        assert_eq!(active_story_repair_payload_from_runtime_state(None), None);
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
