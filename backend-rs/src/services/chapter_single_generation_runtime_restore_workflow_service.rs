use serde_json::{json, Value};

use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract;
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract;

#[cfg(test)]
pub(crate) use super::chapter_single_generation_background_launch_service::SingleGenerationStartupSnapshotPlan;
pub(crate) use super::chapter_single_generation_background_launch_service::{
    build_single_generation_background_launch_owner_contract,
    build_single_generation_startup_snapshot_owner_contract,
};
use super::chapter_single_generation_existing_background_task_service::build_single_generation_existing_background_task_owner_contract;
use super::chapter_single_generation_prepare_service::build_single_generation_prepare_owner_contract;
use super::chapter_single_generation_runtime_seed_service::build_single_generation_runtime_seed_owner_contract;
use super::chapter_single_generation_runtime_state_service::build_single_generation_runtime_state_owner_contract;

pub(crate) mod restore_owner;
pub(crate) mod write_workflow_owner;
pub(crate) use restore_owner::PreparedSingleChapterGenerationRestoredRuntimeLaunch;
pub(crate) use write_workflow_owner::{
    build_single_generation_write_workflow_runtime_owner_contract,
    SingleGenerationBackgroundWriteWorkflowEntry,
};

pub(crate) fn build_single_generation_write_workflow_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_runtime_restore_workflow_service",
        "scope": "single_generation_background_write_existing_task_launch_persist_dispatch_and_response_payload",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/api/chapter_generation_routes.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "background_entrypoints": [
                "SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload",
                "SingleGenerationBackgroundWriteWorkflowEntry::prepare",
                "SingleGenerationBackgroundWriteWorkflowEntry::persist_and_dispatch"
            ],
            "existing_task_read_path": [
                "load_active_single_generation_background_tasks",
                "recover_generation_task_if_needed",
                "load_chapter_generation_snapshot_map",
                "build_single_generation_existing_background_task_payload"
            ],
            "launch_path": [
                "PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_background_launch_parts_from_route_target",
                "PreparedSingleGenerationBackgroundLaunchParts::persist_and_dispatch"
            ],
            "response_payload_fields": [
                "task_id",
                "chapter_id",
                "status",
                "message",
                "estimated_time_minutes",
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_metrics_summary",
                "quality_history_context",
                "active_story_repair_payload",
                "candidate_gateway"
            ],
            "gateway_config": [
                "ChapterCandidateRouteGatewayConfig",
                "persist_and_dispatch receives route/AppConfig supplied gateway config"
            ]
        },
        "active_consumers": [
            "chapter_generation_routes::generate_chapter_background",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter_single_generation_runtime_state_service"
        ],
        "write_workflow_runtime_owner_contract": build_single_generation_write_workflow_runtime_owner_contract(),
        "existing_background_task_owner_contract": build_single_generation_existing_background_task_owner_contract(),
        "prepare_owner_contract": build_single_generation_prepare_owner_contract(),
        "runtime_state_owner_contract": build_single_generation_runtime_state_owner_contract(),
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "background_launch_owner_contract": build_single_generation_background_launch_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-single-generation-owner",
            "db_backed_persistence_smoke": "should_persist_db_backed_single_generation_background_task_and_snapshot_from_rust_owner",
            "manifest_probe_count": 6,
            "rust_manifest_probe_count": 6,
            "python_fallback_probe_count": 0,
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "single-generation final frozen rollback shell retained; reopen only if bootstrap rollback policy changes",
            "status": "rust_single_generation_write_workflow_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "runtime_knobs": [
                "legacy_single_generation_direct_ai",
                "python_candidate_executor_fallback"
            ],
            "source_map_policy": "single_generation_write_workflow_owner_is_rust_only_and_background_entry_shell_source_map_is_deleted",
            "python_fallback_removal_ready": true,
            "rollback_files": []
        }
    })
}

pub(crate) fn build_single_generation_runtime_restore_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_runtime_restore_workflow_service",
        "scope": "single_generation_runtime_restore_startup_snapshot_background_seed_persist_dispatch_and_response_payload",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs",
            "backend-rs/src/services/chapter_quality_metrics_query_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "runtime_restore_entrypoints": [
                "build_single_generation_runtime_launch_input_from_request_runtime_state",
                "RestoredSingleGenerationRuntimeState::from_quality_fragments",
                "RestoredSingleGenerationRuntimeState::into_startup_runtime_launch_parts",
                "PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_route_target"
            ],
            "startup_snapshot_entrypoints": [
                "build_single_generation_pending_checkpoint",
                "SingleGenerationStartupSnapshotPlan::from_pending_checkpoint",
                "SingleGenerationStartupSnapshotPlan::persist"
            ],
            "background_launch_entrypoints": [
                "PreparedSingleChapterGenerationRestoredRuntimeLaunch::into_background_launch_parts",
                "PreparedSingleGenerationBackgroundLaunchParts::persist_and_dispatch",
                "SingleGenerationBackgroundLaunchPersistenceDispatchPlan::persist_and_dispatch"
            ],
            "persistence_payloads": [
                "SingleGenerationTaskPersistenceSeed::into_active_model",
                "upsert_chapter_generation_runtime_snapshot",
                "build_single_generation_background_create_response_payload"
            ],
            "runtime_state_fields": [
                "batch_request_runtime_state",
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_metrics_summary",
                "quality_history_context",
                "active_story_repair_payload"
            ],
            "response_payload_fields": [
                "task_id",
                "chapter_id",
                "status",
                "message",
                "estimated_time_minutes",
                "checkpoint",
                "runtime_state",
                "candidate_gateway",
                "active_story_repair_payload"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_stream_workflow_service",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter_generation_routes::generate_chapter_background"
        ],
        "existing_background_task_owner_contract": build_single_generation_existing_background_task_owner_contract(),
        "runtime_seed_owner_contract": build_single_generation_runtime_seed_owner_contract(),
        "prepare_owner_contract": build_single_generation_prepare_owner_contract(),
        "runtime_state_owner_contract": build_single_generation_runtime_state_owner_contract(),
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "story_repair_quality_context_owner_contract": build_story_repair_quality_context_owner_contract(),
        "startup_snapshot_owner_contract": build_single_generation_startup_snapshot_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-single-generation-owner",
            "db_backed_persistence_smoke": "should_persist_db_backed_single_generation_background_task_and_snapshot_from_rust_owner",
            "manifest_probe_count": 6,
            "rust_manifest_probe_count": 6,
            "python_fallback_probe_count": 0,
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "single-generation final frozen rollback shell retained; reopen only if bootstrap rollback policy changes",
            "status": "rust_single_generation_runtime_restore_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "runtime_knobs": [
                "legacy_single_generation_direct_ai",
                "python_candidate_executor_fallback"
            ],
            "source_map_policy": "single_generation_runtime_restore_owner_is_rust_only_and_background_entry_shell_source_map_is_deleted",
            "python_fallback_removal_ready": true,
            "rollback_files": []
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_chapter_generation_snapshot_owner_contract,
        build_generation_quality_runtime_owner_contract,
        build_single_generation_background_launch_owner_contract,
        build_single_generation_existing_background_task_owner_contract,
        build_single_generation_prepare_owner_contract,
        build_single_generation_runtime_restore_owner_contract,
        build_single_generation_runtime_seed_owner_contract,
        build_single_generation_runtime_state_owner_contract,
        build_single_generation_startup_snapshot_owner_contract,
        build_single_generation_write_workflow_owner_contract,
        build_story_repair_quality_context_owner_contract,
        PreparedSingleChapterGenerationRestoredRuntimeLaunch,
        SingleGenerationBackgroundWriteWorkflowEntry, SingleGenerationStartupSnapshotPlan,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
    use crate::services::chapter_generation_execution_contract_service::BatchGenerationRequestRuntimeState;
    use crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_execution_contract_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_prompt_service::PromptContextProviderPayload;
    use crate::services::chapter_quality_metrics_query_service::ChapterQualityMetricsFragments;
    use crate::services::chapter_single_generation_background_launch_service::build_single_generation_pending_checkpoint;
    use crate::services::chapter_single_generation_background_launch_service::{
        build_single_generation_background_create_response_payload,
        build_single_generation_background_task_active_model,
        build_single_generation_background_task_persistence_seed,
        build_test_single_generation_background_response_payload,
        SingleGenerationBackgroundLaunchPersistenceDispatchPlan,
        SingleGenerationTaskPersistenceSeed,
    };
    use crate::services::chapter_single_generation_prepare_service::{
        PrepareSingleChapterGenerationRequestError, SingleChapterGenerationRequest,
        SingleChapterGenerationRouteRequest, SingleChapterGenerationTarget,
    };
    use crate::services::chapter_single_generation_runtime_seed_service::{
        build_single_generation_runtime_launch_input_from_request_runtime_state,
        RestoredSingleGenerationRuntimeState,
    };
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;
    use sea_orm::{
        ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
        QueryFilter, Schema,
    };
    use serde_json::json;

    fn empty_compat_options() -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: None,
            enable_analysis: true,
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
        }
    }

    #[test]
    fn should_publish_single_generation_runtime_restore_owner_contract() {
        let contract = build_single_generation_runtime_restore_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_runtime_restore_workflow_service"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_restore_entrypoints"][1],
            "RestoredSingleGenerationRuntimeState::from_quality_fragments"
        );
        assert_eq!(
            contract["behavior_contract"]["background_launch_entrypoints"][1],
            "PreparedSingleGenerationBackgroundLaunchParts::persist_and_dispatch"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_state_fields"][6],
            "active_story_repair_payload"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["existing_background_task_owner_contract"]["owner"],
            build_single_generation_existing_background_task_owner_contract()["owner"]
        );
        assert_eq!(
            contract["runtime_seed_owner_contract"]["owner"],
            build_single_generation_runtime_seed_owner_contract()["owner"]
        );
        assert_eq!(
            contract["prepare_owner_contract"]["owner"],
            build_single_generation_prepare_owner_contract()["owner"]
        );
        assert_eq!(
            contract["runtime_state_owner_contract"]["owner"],
            build_single_generation_runtime_state_owner_contract()["owner"]
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            build_chapter_generation_snapshot_owner_contract()["owner"]
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            build_generation_quality_runtime_owner_contract()["owner"]
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["owner"],
            build_story_repair_quality_context_owner_contract()["owner"]
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
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
            "single-generation final frozen rollback shell retained; reopen only if bootstrap rollback policy changes"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_single_generation_runtime_restore_owner_source_map_deleted"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "single_generation_runtime_restore_owner_is_rust_only_and_background_entry_shell_source_map_is_deleted"
        );
        assert_eq!(contract["rollback_boundary"]["rollback_files"], json!([]));
    }

    #[test]
    fn should_publish_single_generation_startup_snapshot_owner_contract() {
        let contract = build_single_generation_startup_snapshot_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_background_launch_service::launch_owner::startup_snapshot_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "build_single_generation_pending_checkpoint"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][2],
            "SingleGenerationStartupSnapshotPlan::persist"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["active_consumers"][2],
            "chapter_single_generation_active_gateway_smoke_service"
        );
    }

    #[test]
    fn should_publish_single_generation_write_workflow_owner_contract() {
        let contract = build_single_generation_write_workflow_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_runtime_restore_workflow_service"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["background_entrypoints"][0],
            "SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload"
        );
        assert_eq!(
            contract["write_workflow_runtime_owner_contract"]["owner"],
            "chapter_single_generation_runtime_restore_workflow_service::write_workflow_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_fields"][11],
            "candidate_gateway"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["existing_background_task_owner_contract"]["owner"],
            build_single_generation_existing_background_task_owner_contract()["owner"]
        );
        assert_eq!(
            contract["prepare_owner_contract"]["owner"],
            build_single_generation_prepare_owner_contract()["owner"]
        );
        assert_eq!(
            contract["runtime_state_owner_contract"]["owner"],
            build_single_generation_runtime_state_owner_contract()["owner"]
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            build_chapter_generation_snapshot_owner_contract()["owner"]
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            build_generation_quality_runtime_owner_contract()["owner"]
        );
        assert_eq!(
            contract["background_launch_owner_contract"]["owner"],
            build_single_generation_background_launch_owner_contract()["owner"]
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
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
            "single-generation final frozen rollback shell retained; reopen only if bootstrap rollback policy changes"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_single_generation_write_workflow_owner_source_map_deleted"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "single_generation_write_workflow_owner_is_rust_only_and_background_entry_shell_source_map_is_deleted"
        );
    }

    #[test]
    fn should_publish_single_generation_background_launch_owner_contract() {
        let contract = build_single_generation_background_launch_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_background_launch_service::launch_owner"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][1],
            "PreparedSingleGenerationBackgroundLaunchParts::persist_and_dispatch"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_entrypoints"][0],
            "build_single_generation_background_create_response_payload"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter_generation_routes::generate_chapter_background"
        );
    }

    #[test]
    fn should_keep_single_generation_background_workflow_existing_payload_owner_contract() {
        let entry =
            SingleGenerationBackgroundWriteWorkflowEntry::from_existing_task_payload(json!({
                "task_id": "task-11",
                "chapter_id": "chapter-11",
                "status": "running",
                "message": "已有后台生成任务正在执行"
            }));

        match entry {
            SingleGenerationBackgroundWriteWorkflowEntry::ExistingTaskPayload(payload) => {
                assert_eq!(payload["task_id"], "task-11");
                assert_eq!(payload["chapter_id"], "chapter-11");
                assert_eq!(payload["status"], "running");
                assert_eq!(payload["message"], "已有后台生成任务正在执行");
            }
            SingleGenerationBackgroundWriteWorkflowEntry::Launch(_) => {
                panic!("expected existing task payload branch")
            }
        }
    }

    #[test]
    fn should_keep_single_generation_background_workflow_launch_owner_contract() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-12".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "第十二章".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 3200,
            compat_options: empty_compat_options(),
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
                provider_payload: PromptContextProviderPayload {
                    recent_chapters_context: String::new(),
                    previous_chapter_summary: String::new(),
                    chapter_careers: "[]".to_string(),
                    characters_info: "[]".to_string(),
                    foreshadow_reminders: "[]".to_string(),
                    relevant_memories: "[]".to_string(),
                    research_query: String::new(),
                    research_assets: "[]".to_string(),
                    external_assets: "[]".to_string(),
                    reference_assets: "[]".to_string(),
                    mcp_references: String::new(),
                },
                role_policy_context: None,
            },
        };
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            execution_input.compat_options.clone(),
            Some("gpt-4.1".to_string()),
        );
        let runtime_state_payload = json!({
            "batch_request_runtime_state": request_runtime_state.clone(),
            "quality_metrics_summary": {"chapter_count": 1}
        });
        let entry = SingleGenerationBackgroundWriteWorkflowEntry::from_prepared_request(
            "task-12".to_string(),
            "user-1",
            chapter_target,
            execution_input,
            request_runtime_state,
            runtime_state_payload,
        );

        match entry {
            SingleGenerationBackgroundWriteWorkflowEntry::Launch(launch) => {
                let response_payload = launch.response_payload.clone();

                assert_eq!(response_payload["task_id"], "task-12");
                assert_eq!(response_payload["chapter_id"], "chapter-12");
                assert_eq!(response_payload["estimated_time_minutes"], 3);
                assert_eq!(
                    launch.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                        ["chapter_count"],
                    1
                );
            }
            SingleGenerationBackgroundWriteWorkflowEntry::ExistingTaskPayload(_) => {
                panic!("expected launch branch")
            }
        }
    }

    #[test]
    fn should_build_single_chapter_generation_target_background_launch_payloads_from_runtime_restore_owner(
    ) {
        let target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let checkpoint = build_single_generation_pending_checkpoint(&target);
        let persistence_seed = build_single_generation_background_task_persistence_seed(
            "task-1".to_string(),
            &target,
            "user-1".to_string(),
            2600,
            true,
        );
        let active_model = build_single_generation_background_task_active_model(
            "task-1".to_string(),
            &target,
            "user-1".to_string(),
            2600,
            true,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(checkpoint["chapter_id"], "chapter-7");
        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(
            persistence_seed,
            SingleGenerationTaskPersistenceSeed {
                id: "task-1".to_string(),
                project_id: "project-1".to_string(),
                user_id: "user-1".to_string(),
                start_chapter_number: 7,
                chapter_count: 1,
                chapter_ids: json!([{
                    "id": "chapter-7",
                    "chapter_number": 7,
                    "title": "Seven",
                }]),
                style_id: None,
                target_word_count: 2600,
                enable_analysis: true,
                total_chapters: 1,
                current_chapter_id: Some("chapter-7".to_string()),
                current_chapter_number: Some(7),
                max_retries: 3,
            }
        );
        assert_eq!(active_model.target_word_count, sea_orm::Set(2600));
        assert_eq!(active_model.status, sea_orm::Set("pending".to_string()));
        assert_eq!(active_model.completed_chapters, sea_orm::Set(0));
        assert_eq!(active_model.failed_chapters, sea_orm::Set(json!([])));
        assert_eq!(active_model.current_retry_count, sea_orm::Set(0));
        assert_eq!(active_model.enable_analysis, sea_orm::Set(true));
        assert_eq!(active_model.max_retries, sea_orm::Set(3));
        assert_eq!(
            active_model.chapter_ids,
            sea_orm::Set(json!([{
                "id": "chapter-7",
                "chapter_number": 7,
                "title": "Seven",
            }]))
        );
        assert_eq!(
            active_model.current_chapter_id,
            sea_orm::Set(Some("chapter-7".to_string()))
        );
    }

    #[test]
    fn should_build_single_chapter_generation_background_parts_from_runtime_restore_owner() {
        let target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let checkpoint = build_single_generation_pending_checkpoint(&target);
        let task = build_single_generation_background_task_active_model(
            "task-1".to_string(),
            &target,
            "user-1".to_string(),
            2600,
            true,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(checkpoint["chapter_id"], "chapter-7");
        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(task.target_word_count, sea_orm::Set(2600));
        assert_eq!(
            task.current_chapter_id,
            sea_orm::Set(Some("chapter-7".to_string()))
        );
    }

    #[test]
    fn should_keep_background_launch_owner_contract_from_restored_launch() {
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-12".to_string(),
            chapter_id: "chapter-12".to_string(),
            chapter_number: 12,
            title: "Twelve".to_string(),
        };
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                enable_analysis: true,
                ..Default::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 3200,
            compat_options: request_runtime_state.compat_options.clone(),
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
                provider_payload: PromptContextProviderPayload {
                    recent_chapters_context: String::new(),
                    previous_chapter_summary: String::new(),
                    chapter_careers: "[]".to_string(),
                    characters_info: "[]".to_string(),
                    foreshadow_reminders: "[]".to_string(),
                    relevant_memories: "[]".to_string(),
                    research_query: String::new(),
                    research_assets: "[]".to_string(),
                    external_assets: "[]".to_string(),
                    reference_assets: "[]".to_string(),
                    mcp_references: String::new(),
                },
                role_policy_context: None,
            },
        };
        let runtime_input = build_single_generation_runtime_launch_input_from_request_runtime_state(
            &chapter_target,
            "user-1",
            execution_input.target_word_count,
            &request_runtime_state,
            execution_input.execution_config.clone(),
        );
        let restored_launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
            chapter_target,
            json!({
                "quality_metrics_summary": {"chapter_count": 1},
                "active_story_repair_payload": {"mode": "repair"}
            }),
            runtime_input,
        );

        let launch_parts = restored_launch.into_background_launch_parts("task-12".to_string());

        assert_eq!(launch_parts.response_payload["task_id"], "task-12");
        assert_eq!(launch_parts.response_payload["chapter_id"], "chapter-12");
        assert_eq!(launch_parts.response_payload["estimated_time_minutes"], 3);
        assert_eq!(
            launch_parts.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                ["chapter_count"],
            1
        );
        assert_eq!(launch_parts.task_seed.max_retries, 3);
        assert_eq!(launch_parts.task_seed.enable_analysis, true);
    }

    #[test]
    fn should_project_background_launch_persistence_dispatch_plan_from_launch_parts_owner() {
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-33".to_string(),
            chapter_id: "chapter-33".to_string(),
            chapter_number: 33,
            title: "Thirty Three".to_string(),
        };
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-33".to_string(),
            user_id: "user-33".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 3300,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    ..Default::default()
                },
                execution_config: PreparedGenerationExecutionConfig {
                    ai_config: crate::ai::AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                    role_policy_context: None,
                },
            },
        };
        let restored_launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
            chapter_target,
            json!({
                "quality_metrics_summary": {"chapter_count": 1},
                "active_story_repair_payload": {"mode": "repair"}
            }),
            runtime_input,
        );
        let now = chrono::NaiveDate::from_ymd_opt(2026, 6, 10)
            .expect("valid date")
            .and_hms_opt(4, 30, 0)
            .expect("valid time");

        let plan = SingleGenerationBackgroundLaunchPersistenceDispatchPlan::from_launch_parts(
            restored_launch.into_background_launch_parts("task-33".to_string()),
            now,
        );

        assert_eq!(plan.task_id, "task-33");
        assert_eq!(plan.task.id, sea_orm::Set("task-33".to_string()));
        assert_eq!(plan.task.project_id, sea_orm::Set("project-33".to_string()));
        assert_eq!(plan.task.user_id, sea_orm::Set("user-33".to_string()));
        assert_eq!(plan.task.status, sea_orm::Set("pending".to_string()));
        assert_eq!(plan.task.created_at, sea_orm::Set(Some(now)));
        assert_eq!(plan.response_payload["task_id"], "task-33");
        assert_eq!(plan.response_payload["status"], "pending");
        assert_eq!(
            plan.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["chapter_count"],
            1
        );
        assert_eq!(plan.runtime_input.chapter_id, "chapter-33");
        assert_eq!(plan.runtime_input.user_id, "user-33");
        assert_eq!(plan.runtime_input.execution_input.target_word_count, 3300);
    }

    #[test]
    fn should_build_single_generation_background_response_compatibility_payload() {
        let target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let response_payload = build_test_single_generation_background_response_payload(
            "task-1",
            &target,
            2,
            Some(&json!({"mode": "repair"})),
        );

        assert_eq!(response_payload["task_id"], "task-1");
        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(response_payload["status"], "pending");
        assert_eq!(response_payload["message"], "单章后台生成任务已创建");
        assert_eq!(response_payload["estimated_time_minutes"], 2);
        assert_eq!(
            response_payload["active_story_repair_payload"]["mode"],
            "repair"
        );
    }

    #[test]
    fn should_keep_background_response_payload_quality_context_fields() {
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-7".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-7".to_string(),
            user_id: "user-7".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2400,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    ..Default::default()
                },
                execution_config: PreparedGenerationExecutionConfig {
                    ai_config: crate::ai::AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                    role_policy_context: None,
                },
            },
        };
        let startup_snapshot_plan = SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
            build_single_generation_pending_checkpoint(&chapter_target),
            json!({
                "latest_quality_metrics": {"overall_score": 88},
                "quality_metrics_summary": {"chapter_count": 2},
                "quality_metrics_history": [{"overall_score": 82}, {"overall_score": 88}],
                "quality_history_context": {"source": "history"},
                "active_story_repair_payload": {"mode": "repair"}
            }),
        );

        let payload = build_single_generation_background_create_response_payload(
            "task-7",
            &chapter_target,
            &startup_snapshot_plan,
            &runtime_input,
        );

        assert_eq!(payload["task_id"], "task-7");
        assert_eq!(payload["chapter_id"], "chapter-7");
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 88);
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_history_context"]["source"], "history");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[tokio::test]
    async fn should_prepare_single_chapter_generation_request_from_target_without_reloading_chapter(
    ) {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let request = SingleChapterGenerationRequest {
            target_word_count: Some(1800),
            ..SingleChapterGenerationRequest::default()
        };
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-9".to_string(),
            chapter_number: 9,
            title: "Nine".to_string(),
        };

        let error = PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_target(
            &db,
            "user-1",
            &request,
            chapter_target,
        )
        .await
        .expect_err("sqlite memory db should fail before any chapter reload path is needed");

        assert!(matches!(
            error,
            PrepareSingleChapterGenerationRequestError::Config(_)
                | PrepareSingleChapterGenerationRequestError::Internal(_)
        ));
    }

    async fn setup_single_generation_background_owner_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);

        db.execute(builder.build(&schema.create_table_from_entity(batch_generation_task::Entity)))
            .await
            .expect("create batch generation tasks table");
        db.execute(
            builder.build(&schema.create_table_from_entity(batch_generation_snapshot::Entity)),
        )
        .await
        .expect("create batch generation snapshots table");

        db
    }

    fn db_backed_single_generation_runtime_input(
        chapter_id: &str,
        user_id: &str,
    ) -> SingleGenerationRuntimeLaunchInput {
        SingleGenerationRuntimeLaunchInput {
            chapter_id: chapter_id.to_string(),
            user_id: user_id.to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2600,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    story_repair_summary: Some("db-backed repair summary".to_string()),
                    story_repair_targets: vec!["节奏".to_string()],
                    ..Default::default()
                },
                execution_config: PreparedGenerationExecutionConfig {
                    ai_config: crate::ai::AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: "db-backed previous context".to_string(),
                        previous_chapter_summary: "db-backed previous summary".to_string(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                    role_policy_context: None,
                },
            },
        }
    }

    #[tokio::test]
    async fn should_persist_db_backed_single_generation_background_task_and_snapshot_from_rust_owner(
    ) {
        let db = setup_single_generation_background_owner_db().await;
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-single-db-smoke".to_string(),
            chapter_id: "chapter-single-db-smoke".to_string(),
            chapter_number: 9,
            title: "Single DB Smoke".to_string(),
        };
        let runtime_input = db_backed_single_generation_runtime_input(
            &chapter_target.chapter_id,
            "user-single-db-smoke",
        );
        let restored_launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
            chapter_target,
            json!({
                "latest_quality_metrics": {
                    "overall_score": 89.0,
                    "source": "single-generation-db-backed-smoke"
                },
                "quality_metrics_summary": {
                    "chapter_count": 1,
                    "avg_score": 89.0
                },
                "quality_metrics_history": [{
                    "overall_score": 87.0,
                    "source": "single-generation-db-backed-history"
                }],
                "quality_history_context": {
                    "source": "single-generation-db-backed-smoke"
                },
                "active_story_repair_payload": {
                    "scope": "chapter",
                    "mode": "single-generation-db-backed-smoke"
                }
            }),
            runtime_input,
        );
        let response_payload = restored_launch
            .into_background_launch_parts("single-db-smoke-task".to_string())
            .persist_and_dispatch(
                &db,
                ChapterCandidateRouteGatewayConfig {
                    rust_executor_enabled: true,
                    fallback_on_rust_error: false,
                    disabled_reason: Some(
                        "single generation db-backed smoke fallback-freeze".to_string(),
                    ),
                    rollback_boundary: "legacy_single_generation_direct_ai".to_string(),
                },
                chrono::NaiveDate::from_ymd_opt(2026, 6, 10)
                    .expect("valid smoke date")
                    .and_hms_opt(3, 30, 0)
                    .expect("valid smoke time"),
            )
            .await
            .expect("persist single generation background launch");

        let persisted_task = batch_generation_task::Entity::find_by_id("single-db-smoke-task")
            .one(&db)
            .await
            .expect("load persisted single generation task")
            .expect("single generation task persisted");
        let persisted_snapshot = batch_generation_snapshot::Entity::find()
            .filter(batch_generation_snapshot::Column::BatchTaskId.eq("single-db-smoke-task"))
            .one(&db)
            .await
            .expect("load persisted single generation snapshot")
            .expect("single generation snapshot persisted");

        assert_eq!(response_payload["task_id"], "single-db-smoke-task");
        assert_eq!(response_payload["status"], "pending");
        assert_eq!(
            response_payload["active_story_repair_payload"]["mode"],
            "single-generation-db-backed-smoke"
        );
        assert_eq!(persisted_task.user_id, "user-single-db-smoke");
        assert_eq!(persisted_task.project_id, "project-single-db-smoke");
        assert_eq!(persisted_task.status, "pending");
        assert_eq!(persisted_task.chapter_count, 1);
        assert_eq!(persisted_task.total_chapters, 1);
        assert_eq!(persisted_task.completed_chapters, 0);
        assert_eq!(persisted_task.current_retry_count, 0);
        assert_eq!(persisted_task.max_retries, 3);
        assert_eq!(
            persisted_task.current_chapter_id.as_deref(),
            Some("chapter-single-db-smoke")
        );
        assert_eq!(
            persisted_task.chapter_ids[0]["id"],
            "chapter-single-db-smoke"
        );
        assert_eq!(
            persisted_snapshot
                .workflow_runtime_state
                .as_ref()
                .expect("workflow runtime state")["phase"],
            "pending"
        );
        assert_eq!(
            persisted_snapshot
                .workflow_runtime_state
                .as_ref()
                .expect("workflow runtime state")["active_story_repair_payload"]["mode"],
            "single-generation-db-backed-smoke"
        );
        assert_eq!(
            persisted_snapshot
                .workflow_runtime_state
                .as_ref()
                .expect("workflow runtime state")["latest_quality_metrics"]["source"],
            "single-generation-db-backed-smoke"
        );
        assert_eq!(
            persisted_snapshot
                .workflow_runtime_state
                .as_ref()
                .expect("workflow runtime state")["quality_metrics_summary"]["chapter_count"],
            1
        );
        assert!(persisted_snapshot.latest_quality_metrics.is_none());
        assert!(persisted_snapshot.quality_metrics_summary.is_none());
    }

    #[test]
    fn should_normalize_single_chapter_generation_compat_options_from_request_owner() {
        let request = SingleChapterGenerationRouteRequest {
            style_id: Some(9),
            target_word_count: Some(2800),
            model: None,
            enable_analysis: None,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: Some("hook".to_string()),
            story_focus: Some("reveal_mystery".to_string()),
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: Some("immersive".to_string()),
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: None,
            story_preserve_strengths: None,
        }
        .into_generation_request();

        let compat = request.compat_options_with_web_research_default(false);

        assert_eq!(compat.style_id(), Some(9));
        assert!(compat.enable_analysis());
        assert!(compat.enable_mcp());
        assert!(!compat.web_research_enabled());
        assert_eq!(compat.web_research_query(), None);
        assert_eq!(compat.creative_mode(), "hook");
        assert_eq!(compat.story_focus(), "reveal_mystery");
        assert_eq!(compat.quality_preset(), "immersive");
        assert_eq!(compat.story_repair_targets(), &[] as &[String]);
        assert_eq!(compat.story_preserve_strengths(), &[] as &[String]);
    }

    #[test]
    fn should_fallback_to_settings_default_for_single_generation_web_research() {
        let request = SingleChapterGenerationRouteRequest {
            style_id: None,
            target_word_count: Some(2800),
            model: None,
            enable_analysis: None,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: None,
            story_preserve_strengths: None,
        }
        .into_generation_request();

        let compat = request.compat_options_with_web_research_default(true);

        assert!(compat.web_research_enabled());
    }

    #[test]
    fn should_restore_runtime_launch_parts_from_quality_fragments_owner() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("request summary".to_string()),
                story_repair_targets: vec!["compress".to_string()],
                ..Default::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2400,
            compat_options: request_runtime_state.compat_options.clone(),
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
                provider_payload: PromptContextProviderPayload {
                    recent_chapters_context: String::new(),
                    previous_chapter_summary: String::new(),
                    chapter_careers: "[]".to_string(),
                    characters_info: "[]".to_string(),
                    foreshadow_reminders: "[]".to_string(),
                    relevant_memories: "[]".to_string(),
                    research_query: String::new(),
                    research_assets: "[]".to_string(),
                    external_assets: "[]".to_string(),
                    reference_assets: "[]".to_string(),
                    mcp_references: String::new(),
                },
                role_policy_context: None,
            },
        };
        let restored_runtime_state = RestoredSingleGenerationRuntimeState::from_quality_fragments(
            json!({
                "phase": "pending",
                "status": "pending",
                "chapter_id": "chapter-9"
            }),
            &request_runtime_state,
            ChapterQualityMetricsFragments {
                latest_quality_metrics: Some(json!({"overall_score": 84})),
                history_id: None,
                generated_at: None,
                quality_metrics_summary: Some(json!({
                    "chapter_count": 2,
                    "repair_guidance": {
                        "summary": "restored summary"
                    }
                })),
                quality_metrics_history: Some(json!([
                    {"overall_score": 80},
                    {"overall_score": 84}
                ])),
                quality_metrics_summary_state: Some(json!({"chapter_count": 2})),
            },
            None,
        );

        assert_eq!(
            restored_runtime_state.request_runtime_state(),
            &request_runtime_state
        );

        let (startup_snapshot_plan, runtime_input) = restored_runtime_state
            .into_startup_runtime_launch_parts(
                "chapter-9".to_string(),
                "user-9".to_string(),
                execution_input,
            );

        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["chapter_count"],
            2
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["latest_quality_metrics"]["overall_score"],
            84
        );
        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-9");
        assert_eq!(runtime_input.execution_input.target_word_count, 2400);
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_summary(),
            "request summary"
        );
    }
}
