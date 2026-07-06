use serde_json::{json, Value};

use crate::services::chapter_analysis_runtime_service::build_chapter_analysis_runtime_owner_contract;
use crate::services::chapter_generation_history_persistence_service::build_chapter_generation_history_persistence_owner_contract;
use crate::services::chapter_generation_runtime_service::build_single_generation_candidate_runtime_owner_contract;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract;
use crate::services::chapter_single_generation_result_lifecycle_service::build_single_generation_result_lifecycle_owner_contract;

use super::chapter_single_generation_prepare_service::build_single_generation_prepare_owner_contract;
use super::chapter_single_generation_runtime_restore_workflow_service::build_single_generation_runtime_restore_owner_contract;
use super::chapter_single_generation_runtime_state_service::build_single_generation_runtime_state_owner_contract;
mod lifecycle_owner;
mod success_owner;
pub(crate) use lifecycle_owner::build_single_generation_stream_lifecycle_owner_contract;
pub(crate) use lifecycle_owner::create_owned_single_generation_stream;
#[cfg(test)]
pub(crate) use lifecycle_owner::SingleGenerationStreamLifecyclePlan;
pub(crate) use success_owner::SingleGenerationStreamSuccessArtifacts;
#[cfg(test)]
pub(crate) use success_owner::{
    attach_single_generation_stream_story_runtime_contract,
    build_single_generation_stream_story_runtime_contract,
    build_single_generation_stream_story_runtime_contract_with_metrics,
    map_single_generation_stream_quality_gate_action,
    persist_single_generation_stream_followup_candidate_draft, SingleGenerationStreamEmissionStep,
    SingleGenerationStreamSuccessEventPayload,
};

pub(crate) fn build_single_generation_stream_workflow_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_stream_workflow_service",
        "scope": "single_generation_sse_stream_lifecycle_success_payload_quality_gate_and_story_runtime_contract",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/api/chapter_generation_routes.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_single_generation_result_lifecycle_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "stream_entrypoints": [
                "create_owned_single_generation_stream",
                "SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config",
                "SingleGenerationStreamLifecyclePlan::spawn"
            ],
            "runtime_gateway_path": [
                "SingleGenerationRuntimeLaunchInput::execute_generation_with_gateway_config",
                "ChapterCandidateRouteGatewayConfig",
                "candidate_gateway metadata copied into stream response payload"
            ],
            "sse_success_order": [
                "tracker.complete",
                "quality_metrics_event",
                "quality_gate_event_when_blocked",
                "sse_result_response_payload",
                "analysis_started_event",
                "sse_done"
            ],
            "response_payload_fields": [
                "chapter_id",
                "chapter_number",
                "title",
                "content",
                "word_count",
                "saved_word_count",
                "chapter_status",
                "content_applied",
                "content_source",
                "analysis_task_id",
                "quality_metrics",
                "quality_gate_action",
                "quality_gate_message",
                "hard_gate_blocked",
                "story_runtime_contract",
                "candidate_draft",
                "candidate_gateway",
                "active_story_repair_payload"
            ],
            "quality_gate_actions": [
                "continue",
                "retry",
                "manual_review"
            ],
            "story_runtime_contract": [
                "build_single_generation_stream_story_runtime_contract",
                "attach_single_generation_stream_story_runtime_contract",
                "request_overrides",
                "blueprint"
            ],
            "analysis_followup": [
                "prepare_chapter_analysis_execution",
                "analyze_generated_chapter_follow_up",
                "analysis_started_event"
            ]
        },
        "active_consumers": [
            "chapter_generation_routes::generate_chapter_stream",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter_single_generation_runtime_state_service",
            "chapter_generation_runtime_service",
            "chapter_generation_runtime_service::story_repair_quality_context_owner"
        ],
        "lifecycle_owner_contract": build_single_generation_stream_lifecycle_owner_contract(),
        "prepare_owner_contract": build_single_generation_prepare_owner_contract(),
        "runtime_restore_owner_contract": build_single_generation_runtime_restore_owner_contract(),
        "runtime_state_owner_contract": build_single_generation_runtime_state_owner_contract(),
        "draft_persistence_owner_contract": build_chapter_generation_history_persistence_owner_contract(),
        "result_lifecycle_owner_contract": build_single_generation_result_lifecycle_owner_contract(),
        "shared_candidate_runtime_owner_contract": build_single_generation_candidate_runtime_owner_contract(),
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "story_repair_quality_context_owner_contract": build_story_repair_quality_context_owner_contract(),
        "analysis_runtime_owner_contract": build_chapter_analysis_runtime_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_single_generation_stream_workflow_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-single-generation-owner",
            "stream_business_probe": "chapter-single-generation-stream-business-rust",
            "background_business_probe": "chapter-single-generation-background-business-rust",
            "manifest_probe_count": 6,
            "rust_manifest_probe_count": 6,
            "python_fallback_probe_count": 0,
            "source_map_closeout_ready": true,
            "remaining_cutover_gate": "single-generation stream orchestration source-map package deleted; surviving Python closeout work is now limited to separate prepare and shared runtime/candidate source-map packages",
            "status": "rust_stream_workflow_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "runtime_knobs": [
                "legacy_single_generation_direct_ai",
                "python_candidate_executor_fallback"
            ],
            "source_map_policy": "python_stream_orchestration_shell_deleted_after_test_seam_migration",
            "python_fallback_removal_ready": true,
            "rollback_files": []
        }
    })
}

#[cfg(test)]
mod tests {
    use sea_orm::Set;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait};
    use serde_json::json;

    use super::{
        attach_single_generation_stream_story_runtime_contract,
        build_chapter_analysis_runtime_owner_contract,
        build_generation_quality_runtime_owner_contract,
        build_single_generation_candidate_runtime_owner_contract,
        build_single_generation_prepare_owner_contract,
        build_single_generation_runtime_restore_owner_contract,
        build_single_generation_runtime_state_owner_contract,
        build_single_generation_stream_story_runtime_contract,
        build_single_generation_stream_story_runtime_contract_with_metrics,
        build_single_generation_stream_workflow_owner_contract,
        build_story_repair_quality_context_owner_contract,
        map_single_generation_stream_quality_gate_action,
        persist_single_generation_stream_followup_candidate_draft,
        SingleGenerationStreamEmissionStep, SingleGenerationStreamLifecyclePlan,
        SingleGenerationStreamSuccessArtifacts, SingleGenerationStreamSuccessEventPayload,
    };
    use crate::ai::AIConfig;
    use crate::models::{chapter, chapter_draft_attempt};
    use crate::services::chapter_access_service::LoadAccessibleChapterForGenerationError;
    use crate::services::chapter_batch_generation_write_workflow_service::build_batch_generation_task_active_model;
    use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
    use crate::services::chapter_generation_execution_contract_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_prompt_service::PromptContextProviderPayload;
    use crate::services::chapter_single_generation_prepare_service::{
        PrepareSingleChapterGenerationRequestError, SingleChapterGenerationRouteRequest,
    };
    use crate::services::chapter_single_generation_runtime_restore_workflow_service::PreparedSingleChapterGenerationRestoredRuntimeLaunch;
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;

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

    fn test_single_generation_stream_gateway_config() -> ChapterCandidateRouteGatewayConfig {
        ChapterCandidateRouteGatewayConfig {
            rust_executor_enabled: true,
            fallback_on_rust_error: false,
            disabled_reason: Some("test stream route explicit gateway".to_string()),
            rollback_boundary: "test_single_generation_stream_gateway".to_string(),
        }
    }

    #[test]
    fn should_publish_single_generation_stream_workflow_owner_contract() {
        let contract = build_single_generation_stream_workflow_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_stream_workflow_service"
        );
        assert!(!contract["python_source_map"]
            .as_array()
            .expect("python source map array")
            .iter()
            .any(|path| path
                .as_str()
                .unwrap_or_default()
                .ends_with("entry_service.py")));
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["stream_entrypoints"][0],
            "create_owned_single_generation_stream"
        );
        assert_eq!(
            contract["behavior_contract"]["sse_success_order"][3],
            "sse_result_response_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_fields"][16],
            "candidate_gateway"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["prepare_owner_contract"]["owner"],
            build_single_generation_prepare_owner_contract()["owner"]
        );
        assert_eq!(
            contract["runtime_restore_owner_contract"]["owner"],
            build_single_generation_runtime_restore_owner_contract()["owner"]
        );
        assert_eq!(
            contract["runtime_state_owner_contract"]["owner"],
            build_single_generation_runtime_state_owner_contract()["owner"]
        );
        assert_eq!(
            contract["lifecycle_owner_contract"]["python_source_map"],
            json!([])
        );
        assert_eq!(
            contract["shared_candidate_runtime_owner_contract"]["owner"],
            build_single_generation_candidate_runtime_owner_contract()["owner"]
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
            contract["analysis_runtime_owner_contract"]["owner"],
            build_chapter_analysis_runtime_owner_contract()["owner"]
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_knobs"][1],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(contract["rollback_boundary"]["rollback_files"], json!([]));
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["stream_business_probe"],
            "chapter-single-generation-stream-business-rust"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["background_business_probe"],
            "chapter-single-generation-background-business-rust"
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
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "single-generation stream orchestration source-map package deleted; surviving Python closeout work is now limited to separate prepare and shared runtime/candidate source-map packages"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_stream_workflow_owner_source_map_deleted"
        );
    }

    #[test]
    fn should_keep_background_workflow_error_contract_shape() {
        let chapter_error = PrepareSingleChapterGenerationRequestError::Chapter(
            LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
        );
        let config_error =
            PrepareSingleChapterGenerationRequestError::Config("model missing".to_string());
        let internal_error =
            PrepareSingleChapterGenerationRequestError::Internal("db failed".to_string());

        assert!(matches!(
            chapter_error,
            PrepareSingleChapterGenerationRequestError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied
            )
        ));
        assert!(matches!(
            config_error,
            PrepareSingleChapterGenerationRequestError::Config(detail) if detail == "model missing"
        ));
        assert!(matches!(
            internal_error,
            PrepareSingleChapterGenerationRequestError::Chapter(_)
                | PrepareSingleChapterGenerationRequestError::Config(_)
                | PrepareSingleChapterGenerationRequestError::Internal(_)
        ));
    }

    #[test]
    fn should_build_single_generation_task_chapter_payload_from_parts() {
        let payload = build_batch_generation_task_active_model(
            "task-2".to_string(),
            "project-2".to_string(),
            "user-2".to_string(),
            8,
            1,
            json!([{
                "id": "chapter-2",
                "chapter_number": 8,
                "title": "第八章",
            }]),
            None,
            2100,
            false,
            1,
            Some("chapter-2".to_string()),
            Some(8),
            0,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(
            payload.chapter_ids,
            Set(json!([{
                "id": "chapter-2",
                "chapter_number": 8,
                "title": "第八章",
            }]))
        );
    }

    #[test]
    fn should_build_single_generation_background_runtime_input_contract() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-9".to_string(),
            user_id: "user-42".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2400,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
                },
            },
        };

        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-42");
        assert_eq!(runtime_input.execution_input.target_word_count, 2400);
        assert_eq!(
            runtime_input
                .execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
    }

    #[test]
    fn should_keep_single_generation_stream_lifecycle_owner_contract() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-stream".to_string(),
            user_id: "user-stream".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2500,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: false,
                    ..empty_compat_options()
                },
                execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
                },
            },
        };

        let lifecycle =
            SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config(
                launch.clone(),
                test_single_generation_stream_gateway_config(),
            );

        assert_eq!(lifecycle.runtime_input.chapter_id, launch.chapter_id);
        assert_eq!(lifecycle.runtime_input.user_id, launch.user_id);
        assert_eq!(lifecycle.target_word_count, 2500);
        assert!(!lifecycle.enable_analysis);
    }

    #[test]
    fn should_keep_single_generation_stream_lifecycle_gateway_config_from_route() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-stream-gateway".to_string(),
            user_id: "user-stream-gateway".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2600,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
                },
            },
        };
        let gateway_config = ChapterCandidateRouteGatewayConfig {
            rust_executor_enabled: true,
            fallback_on_rust_error: false,
            disabled_reason: Some("route supplied rust owner".to_string()),
            rollback_boundary: "route_supplied_gateway".to_string(),
        };

        let lifecycle =
            SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config(
                launch,
                gateway_config.clone(),
            );

        assert_eq!(lifecycle.candidate_gateway_config, gateway_config);
    }

    #[test]
    fn should_build_single_generation_stream_terminal_failure_event() {
        let error = Err::<
            crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
            _,
        >("generation failed".to_string())
        .expect_err("expected failure");

        assert_eq!(error, "generation failed");
    }

    #[tokio::test]
    async fn should_build_single_generation_stream_even_when_runtime_will_fail_later() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-1".to_string(),
            user_id: "user-1".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2000,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
                },
            },
        };
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let _stream = SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config(
            launch,
            test_single_generation_stream_gateway_config(),
        )
        .spawn(db);
    }

    #[tokio::test]
    async fn should_build_single_generation_stream_when_follow_up_analysis_disabled() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-2".to_string(),
            user_id: "user-2".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 1600,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: false,
                    ..empty_compat_options()
                },
                execution_config:
                    crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                        ai_config: AIConfig::default(),
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
                    },
            },
        };
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let _stream = SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config(
            launch,
            test_single_generation_stream_gateway_config(),
        )
        .spawn(db);
    }

    #[tokio::test]
    async fn should_keep_single_generation_stream_lifecycle_spawn_owner_contract() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-3".to_string(),
            user_id: "user-3".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 1900,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
                },
            },
        };
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let _stream = SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config(
            launch,
            test_single_generation_stream_gateway_config(),
        )
        .spawn(db);
    }

    #[tokio::test]
    async fn should_preserve_single_generation_stream_route_request_error_boundary() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let result =
            PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_runtime_launch_input_from_route_request(
            &db,
            "missing-chapter",
                "user-route",
            SingleChapterGenerationRouteRequest {
                target_word_count: Some(1800),
                model: Some("gpt-test".to_string()),
                enable_analysis: Some(true),
                enable_mcp: Some(true),
                enable_web_research: Some(false),
                web_research_query: None,
                style_id: None,
                narrative_perspective: None,
                creative_mode: None,
                story_focus: None,
                plot_stage: None,
                story_creation_brief: None,
                quality_preset: None,
                quality_notes: None,
                story_repair_summary: None,
                story_repair_targets: Some(vec!["target-a".to_string()]),
                story_preserve_strengths: Some(vec!["strength-a".to_string()]),
            },
        )
        .await;

        assert!(result.is_err());
    }

    #[test]
    fn should_build_single_generation_stream_terminal_success_payload() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
            content: "content".to_string(),
            word_count: 2600,
            saved_word_count: 2600,
            chapter_status: "completed".to_string(),
            content_applied: true,
            candidate_gateway_metadata: Some(json!({
                "execution_path": "rust_candidate_executor",
                "fallback_applied": false,
                "rollback_boundary": "python_candidate_executor_fallback"
            })),
            ..Default::default()
        };

        let analysis = SingleGenerationStreamSuccessArtifacts::from_quality_metrics(
            Some("analysis-task-1".to_string()),
            Some(json!({
                "overall_score": 9.1,
                "quality_gate": {
                    "decision": "passed",
                    "summary": "当前章节通过"
                }
            })),
            Some(json!({
                "guidance": {
                    "creative_mode": "hook"
                },
                "blueprint": {
                    "current_chapter_number": 7,
                    "target_word_count": 2600
                }
            })),
        );

        let response_payload = analysis.response_payload(&result);
        let success_payloads = analysis.ordered_success_event_payloads(&result);
        let emission_plan = analysis.build_success_emission_plan(&result);

        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(response_payload["chapter_number"], 7);
        assert_eq!(response_payload["quality_gate_action"], "continue");
        assert_eq!(response_payload["quality_gate_message"], "当前章节通过");
        assert_eq!(response_payload["hard_gate_blocked"], false);
        assert_eq!(
            response_payload["latest_quality_metrics"]["overall_score"],
            9.1
        );
        assert_eq!(
            response_payload["story_runtime_contract"]["guidance"]["creative_mode"],
            "hook"
        );
        assert_eq!(
            response_payload["quality_metrics"]["story_runtime_contract"]["blueprint"]
                ["current_chapter_number"],
            7
        );
        assert_eq!(
            response_payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            response_payload["candidate_gateway"]["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(success_payloads.len(), 3);
        assert!(matches!(
            &success_payloads[0],
            SingleGenerationStreamSuccessEventPayload::Json(payload)
                if payload["type"] == "quality_metrics"
                    && payload["story_runtime_contract"]["blueprint"]["target_word_count"] == 2600
        ));
        assert!(matches!(
            &success_payloads[1],
            SingleGenerationStreamSuccessEventPayload::Result(payload)
                if payload["candidate_gateway"]["fallback_applied"] == false
                    && payload["quality_gate_action"] == "continue"
        ));
        assert!(matches!(
            &success_payloads[2],
            SingleGenerationStreamSuccessEventPayload::Json(payload)
                if payload["type"] == "analysis_started"
                    && payload["message"] == "章节分析任务已启动"
        ));
        assert_eq!(emission_plan.len(), 5);
        assert!(matches!(
            &emission_plan[0],
            SingleGenerationStreamEmissionStep::Complete(message)
                if message == "章节生成完成"
        ));
        assert!(matches!(
            &emission_plan[4],
            SingleGenerationStreamEmissionStep::Done
        ));
    }

    #[test]
    fn should_build_single_generation_stream_quality_events_for_retry_follow_up() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-8".to_string(),
            chapter_number: 8,
            title: "第八章".to_string(),
            content: "content".to_string(),
            word_count: 2800,
            saved_word_count: 2800,
            chapter_status: "draft".to_string(),
            content_applied: false,
            provisional_draft_saved: true,
            attempt_state: "retry".to_string(),
            ..Default::default()
        };
        let analysis = SingleGenerationStreamSuccessArtifacts::from_quality_metrics(
            Some("analysis-task-8".to_string()),
            Some(json!({
                "overall_score": 7.2,
                "repair_guidance": {
                    "summary": "建议收紧中段说明"
                },
                "quality_gate": {
                    "decision": "auto_repair",
                    "label": "建议继续修复",
                    "summary": "建议收紧中段说明"
                }
            })),
            Some(json!({
                "guidance": {
                    "story_focus": "advance_plot"
                },
                "blueprint": {
                    "current_chapter_number": 8,
                    "target_word_count": 2800
                }
            })),
        );

        let quality_gate_event = analysis
            .quality_gate_event(&result)
            .expect("quality gate event");
        let analysis_started_event = analysis
            .analysis_started_event()
            .expect("analysis started event");
        let response_payload = analysis.response_payload(&result);

        assert_eq!(quality_gate_event["type"], "quality_gate_retry");
        assert_eq!(quality_gate_event["message"], "建议收紧中段说明");
        assert_eq!(quality_gate_event["progress"], 88);
        assert_eq!(analysis_started_event["message"], "质量修复分析任务已启动");
        assert_eq!(response_payload["quality_gate_action"], "retry");
        assert_eq!(response_payload["hard_gate_blocked"], true);
        assert_eq!(
            response_payload["active_story_repair_payload"]["summary"],
            "建议收紧中段说明"
        );
    }

    #[test]
    fn should_project_single_generation_stream_success_event_order_from_analysis_owner() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-14".to_string(),
            chapter_number: 14,
            title: "第十四章".to_string(),
            content: "content".to_string(),
            word_count: 3600,
            chapter_status: "draft".to_string(),
            content_applied: false,
            attempt_state: "manual_review".to_string(),
            ..Default::default()
        };
        let analysis = SingleGenerationStreamSuccessArtifacts::from_quality_metrics(
            Some("analysis-task-14".to_string()),
            Some(json!({
                "overall_score": 6.8,
                "quality_gate": {
                    "decision": "manual_review",
                    "summary": "需要人工复核"
                }
            })),
            Some(json!({
                "guidance": {
                    "creative_mode": "suspense"
                }
            })),
        );

        let payloads = analysis.ordered_success_event_payloads(&result);

        assert_eq!(analysis.completion_message(), "章节生成完成");
        assert_eq!(analysis.quality_gate_event(&result), None);
        assert_eq!(payloads.len(), 3);
        assert!(matches!(
            payloads[0],
            SingleGenerationStreamSuccessEventPayload::Json(_)
        ));
        assert!(matches!(
            payloads[1],
            SingleGenerationStreamSuccessEventPayload::Result(_)
        ));
        assert!(matches!(
            payloads[2],
            SingleGenerationStreamSuccessEventPayload::Json(_)
        ));
    }

    #[test]
    fn should_build_single_generation_stream_emission_plan_from_response_owner() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-15".to_string(),
            chapter_number: 15,
            title: "第十五章".to_string(),
            content: "content".to_string(),
            word_count: 3700,
            saved_word_count: 3700,
            chapter_status: "draft".to_string(),
            content_applied: false,
            provisional_draft_saved: true,
            attempt_state: "retry".to_string(),
            ..Default::default()
        };
        let analysis = SingleGenerationStreamSuccessArtifacts::from_quality_metrics(
            Some("analysis-task-15".to_string()),
            Some(json!({
                "overall_score": 6.5,
                "quality_gate": {
                    "decision": "auto_repair",
                    "summary": "建议自动修复"
                }
            })),
            Some(json!({
                "guidance": {
                    "story_focus": "escalate_conflict"
                }
            })),
        );

        let plan = analysis.build_success_emission_plan(&result);

        assert_eq!(plan.len(), 6);
        assert!(matches!(
            &plan[0],
            SingleGenerationStreamEmissionStep::Complete(message)
                if message == "章节生成完成，已转入质量修复"
        ));
        assert!(matches!(
            &plan[2],
            SingleGenerationStreamEmissionStep::Payload(
                SingleGenerationStreamSuccessEventPayload::Json(payload)
            ) if payload["type"] == "quality_gate_retry"
        ));
        assert!(matches!(&plan[5], SingleGenerationStreamEmissionStep::Done));
    }

    #[test]
    fn should_build_single_generation_stream_story_runtime_contract_from_compat_options() {
        let compat = SingleChapterGenerationCompatOptions {
            creative_mode: Some("suspense".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("climax".to_string()),
            story_creation_brief: Some("让冲突快速升级".to_string()),
            quality_preset: Some("immersive".to_string()),
            quality_notes: Some("减少旁白解释".to_string()),
            ..empty_compat_options()
        };

        let contract = build_single_generation_stream_story_runtime_contract(8, 3200, &compat)
            .expect("story runtime contract");

        assert_eq!(contract["version"], 1);
        assert_eq!(contract["blueprint"]["current_chapter_number"], 8);
        assert_eq!(contract["blueprint"]["target_word_count"], 3200);
        assert_eq!(contract["guidance"]["creative_mode"], "suspense");
        assert_eq!(contract["request_overrides"]["quality_preset"], "immersive");
    }

    #[test]
    fn should_restore_single_generation_story_runtime_contract_blueprint_from_quality_runtime_context(
    ) {
        let compat = SingleChapterGenerationCompatOptions {
            creative_mode: Some("suspense".to_string()),
            story_focus: Some("advance_plot".to_string()),
            ..empty_compat_options()
        };

        let contract = build_single_generation_stream_story_runtime_contract_with_metrics(
            8,
            3200,
            &compat,
            Some(&json!({
                "quality_runtime_context": {
                    "creative_mode": "hook",
                    "plot_stage": "climax",
                    "story_long_term_goal": "追回失落线索",
                    "chapter_count": 12,
                    "current_chapter_number": 5,
                    "target_word_count": 2600,
                    "character_focus": ["沈砚", "苏槿"],
                    "foreshadow_payoff_plan": ["回收暗号"],
                    "character_state_ledger": [{"label": "沈砚", "summary": "情绪收紧"}],
                    "relationship_state_ledger": [{"label": "沈砚/苏槿", "summary": "互相试探"}],
                    "foreshadow_state_ledger": [{"label": "暗号", "summary": "等待兑现"}],
                    "organization_state_ledger": [{"label": "夜巡司", "summary": "开始施压"}],
                    "career_state_ledger": [{"label": "沈砚/夜巡人", "summary": "晋升受阻"}]
                }
            })),
        )
        .expect("story runtime contract");

        assert_eq!(contract["guidance"]["creative_mode"], "hook");
        assert_eq!(contract["guidance"]["story_focus"], "advance_plot");
        assert_eq!(contract["guidance"]["plot_stage"], "climax");
        assert_eq!(contract["blueprint"]["long_term_goal"], "追回失落线索");
        assert_eq!(contract["blueprint"]["chapter_count"], 12);
        assert_eq!(contract["blueprint"]["current_chapter_number"], 5);
        assert_eq!(contract["blueprint"]["target_word_count"], 2600);
        assert_eq!(
            contract["blueprint"]["character_focus_names"],
            json!(["沈砚", "苏槿"])
        );
        assert_eq!(
            contract["blueprint"]["foreshadow_payoff_plan"],
            json!(["回收暗号"])
        );
        assert_eq!(
            contract["blueprint"]["organization_state_ledger"][0]["label"],
            "夜巡司"
        );
        assert_eq!(
            contract["blueprint"]["career_state_ledger"][0]["summary"],
            "晋升受阻"
        );
    }

    #[test]
    fn should_attach_single_generation_stream_story_runtime_contract_when_quality_metrics_exist() {
        let payload = attach_single_generation_stream_story_runtime_contract(
            Some(json!({
                "overall_score": 92
            })),
            Some(&json!({
                "version": 1,
                "source": "chapter-generation-intent"
            })),
        )
        .expect("attached payload");

        assert_eq!(payload["overall_score"], 92);
        assert_eq!(payload["story_runtime_contract"]["version"], 1);
        assert_eq!(
            payload["story_runtime_contract"]["source"],
            "chapter-generation-intent"
        );
    }

    #[test]
    fn should_map_single_generation_stream_quality_gate_actions_to_runtime_contract() {
        assert_eq!(
            map_single_generation_stream_quality_gate_action(Some(&json!({
                "decision": "passed"
            })))
            .as_deref(),
            Some("continue")
        );
        assert_eq!(
            map_single_generation_stream_quality_gate_action(Some(&json!({
                "decision": "retry"
            })))
            .as_deref(),
            Some("retry")
        );
        assert_eq!(
            map_single_generation_stream_quality_gate_action(Some(&json!({
                "decision": "manual_review"
            })))
            .as_deref(),
            Some("continue")
        );
    }

    #[tokio::test]
    async fn should_persist_single_generation_stream_followup_candidate_draft_with_shared_lifecycle_owner(
    ) {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let schema = sea_orm::Schema::new(sea_orm::DatabaseBackend::Sqlite);
        let builder = db.get_database_backend();

        db.execute(builder.build(&schema.create_table_from_entity(chapter::Entity)))
            .await
            .expect("create chapter table");
        db.execute(builder.build(&schema.create_table_from_entity(chapter_draft_attempt::Entity)))
            .await
            .expect("create chapter draft attempt table");

        chapter::ActiveModel {
            id: Set("chapter-stream-1".to_string()),
            project_id: Set("project-stream-1".to_string()),
            chapter_number: Set(3),
            title: Set("第三章".to_string()),
            content: Set(Some("上一版正文".to_string())),
            word_count: Set(12),
            status: Set("writing".to_string()),
            summary: Set(None),
            outline_id: Set(None),
            sub_index: Set(0),
            expansion_plan: Set(None),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(Some(chrono::Utc::now().naive_utc())),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert chapter");

        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-stream-1".to_string(),
            chapter_number: 3,
            title: "第三章".to_string(),
            content: "候选正文需要继续修复".to_string(),
            word_count: 18,
            quality_metrics: Some(json!({
                "quality_gate": {
                    "decision": "auto_repair",
                    "summary": "建议继续修复"
                }
            })),
            ..Default::default()
        };

        let payload = persist_single_generation_stream_followup_candidate_draft(
            &db,
            &result,
            Some("retry"),
            None,
        )
        .await
        .expect("persist follow-up draft");

        assert_eq!(payload["quality_gate_action"], "retry");
        assert_eq!(payload["attempt_state"], "retry");

        let attempts = chapter_draft_attempt::Entity::find()
            .all(&db)
            .await
            .expect("load attempts");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].attempt_state, "retry");
        assert_eq!(attempts[0].quality_gate_action.as_deref(), Some("retry"));
    }
}
