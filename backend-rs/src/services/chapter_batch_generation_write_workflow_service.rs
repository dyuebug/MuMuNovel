#[cfg(test)]
use crate::services::chapter_batch_generation_runtime_state_service::BatchGenerationQueuedSnapshotPlan;

pub(crate) mod create_launch_owner;
pub(crate) mod request_prepare_owner;
pub(crate) mod write_workflow_owner;

pub(crate) use self::create_launch_owner::{
    build_batch_generation_create_launch_owner_contract,
    start_owned_batch_generation_create_launch_from_route_payload,
};
pub(crate) use self::request_prepare_owner::{
    build_batch_generation_create_workflow_request_from_route_payload,
    build_batch_generation_request_prepare_owner_contract, BatchGenerationCreateChapterTarget,
    BatchGenerationCreateRouteRequest, BatchGenerationCreateTaskSpec,
    BatchGenerationCreateWorkflowRequest, PrepareBatchGenerationCreateRequestError,
};
pub(crate) use self::write_workflow_owner::{
    build_batch_generation_write_workflow_owner_contract,
    start_owned_batch_generation_write_workflow, CreateBatchGenerationWriteWorkflowError,
};

#[cfg(test)]
pub(crate) use self::create_launch_owner::{
    build_batch_generation_runtime_state_payload_from_parts,
    build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload,
    build_batch_generation_task_active_model, select_batch_generation_create_effective_style_id,
    BatchGenerationCreateLaunchPersistencePlan, BatchGenerationCreateRuntimeSeed,
    BatchGenerationCreateStartupRuntimeState, BatchGenerationCreateStartupSeedSource,
    BatchGenerationTaskPersistenceSeed, PreparedBatchGenerationCreateWorkflowLaunch,
};

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend,
        EntityTrait, QueryFilter, Schema, Set,
    };
    use serde_json::{json, Value};

    use super::{
        build_batch_generation_create_workflow_request_from_route_payload,
        build_batch_generation_runtime_state_payload_from_parts,
        build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload,
        BatchGenerationCreateChapterTarget, BatchGenerationCreateLaunchPersistencePlan,
        BatchGenerationCreateRouteRequest, BatchGenerationCreateRuntimeSeed,
        BatchGenerationCreateStartupRuntimeState, BatchGenerationCreateStartupSeedSource,
        BatchGenerationCreateTaskSpec, BatchGenerationCreateWorkflowRequest,
        BatchGenerationTaskPersistenceSeed, CreateBatchGenerationWriteWorkflowError,
        PrepareBatchGenerationCreateRequestError, PreparedBatchGenerationCreateWorkflowLaunch,
    };
    use crate::models::chapter;
    use crate::models::{
        batch_generation_snapshot, batch_generation_task, generation_history, project,
        project_default_style, settings,
    };
    use crate::services::chapter_batch_generation_runtime_state_service::BatchGenerationQueuedCreateResponseChapter;
    use crate::services::chapter_batch_generation_task_payload_base_service::estimated_task_minutes;
    use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
    use crate::services::chapter_generation_execution_contract_service::normalize_chapter_generation_target_word_count;
    use crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_execution_contract_service::{
        batch_generation_request_runtime_state_payload, BatchGenerationRequestRuntimeState,
    };
    use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::aggregate_story_repair_quality_summaries;
    use crate::services::project_service::ProjectAccessQueryError;

    #[test]
    fn should_publish_batch_generation_write_workflow_owner_contract() {
        let contract = super::build_batch_generation_write_workflow_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_write_workflow_service"
        );
        assert_eq!(
            contract["scope"],
            "batch_generation_create_write_workflow_persist_dispatch_and_response_payload"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/api/chapter_batch_generation_routes.py"
        );
        assert_eq!(
            contract["rust_owner_map"][1],
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["create_entrypoints"][2],
            "start_owned_batch_generation_write_workflow"
        );
        assert_eq!(
            contract["behavior_contract"]["persistence_contract"][2],
            "BatchGenerationQueuedSnapshotPlan::persist"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_dispatch"][2],
            "ChapterCandidateRouteGatewayConfig"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_state_seed_entrypoints"][0],
            "BatchGenerationCreateStartupRuntimeState::prepare"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_state_seed_entrypoints"][3],
            "build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_entrypoints"][0],
            "BatchGenerationQueuedSnapshotPlan::into_create_response_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_fields"][11],
            "candidate_gateway"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["response_payload_owner"],
            "BatchGenerationQueuedSnapshotPlan::into_create_response_payload"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter_batch_generation_active_gateway_smoke_service"
        );
        assert_eq!(
            contract["runtime_state_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service"
        );
        assert_eq!(
            contract["task_payload_owner_contract"]["owner"],
            "chapter_batch_generation_task_payload_base_service"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["owner"],
            "chapter_generation_runtime_service::story_repair_quality_context_owner"
        );
        assert_eq!(
            contract["generation_execution_config_owner_contract"]["owner"],
            "chapter_generation_execution_contract_service::execution_config"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_knob"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_bootstrap_status"],
            "lazy_imported_and_registered_for_explicit_gateway_rollback_only"
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
            contract["service_runtime_closeout_status"]["create_workflow_owner"],
            "start_owned_batch_generation_write_workflow"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["runtime_dispatch_owner"],
            "dispatch_batch_generation_runtime"
        );
        assert_eq!(
            contract["create_launch_owner_contract"]["owner"],
            "chapter_batch_generation_write_workflow_service::create_launch_startup_seed_and_persistence"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_batch_generation_write_workflow_owner_ready_for_source_map_closeout_review"
        );
    }

    #[test]
    fn should_publish_batch_generation_create_launch_owner_contract() {
        let contract = super::build_batch_generation_create_launch_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_write_workflow_service::create_launch_startup_seed_and_persistence"
        );
        assert_eq!(
            contract["behavior_contract"]["startup_seed_entrypoints"][0],
            "BatchGenerationCreateStartupRuntimeState::prepare"
        );
        assert_eq!(
            contract["behavior_contract"]["startup_seed_entrypoints"][5],
            "BatchGenerationCreateRuntimeSeed::into_workflow_launch_parts"
        );
        assert_eq!(
            contract["behavior_contract"]["launch_projection_entrypoints"][4],
            "BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch"
        );
        assert_eq!(
            contract["behavior_contract"]["persistence_and_dispatch_entrypoints"][3],
            "dispatch_batch_generation_runtime"
        );
        assert_eq!(
            contract["behavior_contract"]["response_projection_entrypoints"][0],
            "BatchGenerationQueuedSnapshotPlan::into_create_response_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["response_projection_fields"][6],
            "candidate_gateway"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_seed_dependencies"][3],
            "prepare_generation_execution_config"
        );
        assert_eq!(
            contract["active_consumers"][3],
            "chapter_batch_generation_active_gateway_smoke_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_state_keys"][7],
            "candidate_gateway"
        );
    }

    async fn setup_batch_write_owner_db() -> DatabaseConnection {
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
        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create projects table");
        db.execute(builder.build(&schema.create_table_from_entity(chapter::Entity)))
            .await
            .expect("create chapters table");
        db.execute(builder.build(&schema.create_table_from_entity(settings::Entity)))
            .await
            .expect("create settings table");
        db.execute(builder.build(&schema.create_table_from_entity(project_default_style::Entity)))
            .await
            .expect("create project default styles table");
        db.execute(builder.build(&schema.create_table_from_entity(generation_history::Entity)))
            .await
            .expect("create generation history table");

        db
    }

    async fn seed_create_write_owner_fixture(db: &DatabaseConnection) {
        let now = Utc::now().naive_utc();

        project::ActiveModel {
            id: Set("project-create-db-smoke".to_string()),
            user_id: Set("user-create-db-smoke".to_string()),
            title: Set("Batch Create DB Smoke".to_string()),
            description: Set(None),
            theme: Set(None),
            genre: Set(None),
            target_words: Set(8000),
            current_words: Set(1200),
            status: Set("active".to_string()),
            wizard_status: Set("completed".to_string()),
            wizard_step: Set(0),
            outline_mode: Set("simple".to_string()),
            world_time_period: Set(None),
            world_location: Set(None),
            world_atmosphere: Set(None),
            world_rules: Set(None),
            chapter_count: Set(Some(3)),
            narrative_perspective: Set(None),
            character_count: Set(0),
            default_creative_mode: Set(None),
            default_story_focus: Set(None),
            default_plot_stage: Set(None),
            default_story_creation_brief: Set(None),
            default_quality_preset: Set(None),
            default_quality_notes: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .expect("insert create db smoke project");

        for (chapter_id, chapter_number, status, content, summary, word_count) in [
            (
                "chapter-create-1",
                1,
                "completed",
                Some("已完成前置章节正文"),
                Some("已完成前置章节概要"),
                1200,
            ),
            ("chapter-create-2", 2, "draft", None, None, 0),
            ("chapter-create-3", 3, "draft", None, None, 0),
        ] {
            chapter::ActiveModel {
                id: Set(chapter_id.to_string()),
                project_id: Set("project-create-db-smoke".to_string()),
                chapter_number: Set(chapter_number),
                title: Set(format!("第{chapter_number}章")),
                content: Set(content.map(str::to_string)),
                summary: Set(summary.map(str::to_string)),
                word_count: Set(word_count),
                status: Set(status.to_string()),
                outline_id: Set(None),
                sub_index: Set(1),
                expansion_plan: Set(None),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            }
            .insert(db)
            .await
            .expect("insert create db smoke chapter");
        }

        settings::ActiveModel {
            id: Set("settings-create-db-smoke".to_string()),
            user_id: Set("user-create-db-smoke".to_string()),
            api_provider: Set("openai".to_string()),
            api_key: Set("sk-create-owner".to_string()),
            api_base_url: Set("https://api.example.com/v1".to_string()),
            api_backup_urls: Set(None),
            provider_type: Set("openai".to_string()),
            fallback_strategy: Set("manual".to_string()),
            azure_api_version: Set(None),
            llm_model: Set("stored-create-model".to_string()),
            temperature: Set(0.4),
            max_tokens: Set(4096),
            system_prompt: Set(Some("create-owner-prompt".to_string())),
            preferences: Set(Some(
                json!({
                    "web_research": {
                        "web_research_enabled": true
                    }
                })
                .to_string(),
            )),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .expect("insert create db smoke settings");
    }

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
            content: Some("正文".to_string()),
            summary: Some("摘要".to_string()),
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn chapter_model_with_number(chapter_number: i32) -> chapter::Model {
        chapter::Model {
            id: format!("chapter-{chapter_number}"),
            chapter_number,
            title: format!("第{chapter_number}章"),
            ..chapter_model()
        }
    }

    fn build_chapter_target(
        id: &str,
        chapter_number: i32,
        title: &str,
    ) -> BatchGenerationCreateChapterTarget {
        BatchGenerationCreateChapterTarget {
            id: id.to_string(),
            chapter_number,
            title: title.to_string(),
        }
    }

    fn build_test_generation_execution_config() -> PreparedGenerationExecutionConfig {
        PreparedGenerationExecutionConfig {
            ai_config: crate::ai::AIConfig::default(),
            provider_payload:
                crate::services::chapter_generation_prompt_service::PromptContextProviderPayload {
                    recent_chapters_context: String::new(),
                    previous_chapter_summary: String::new(),
                    chapter_careers: "[]".to_string(),
                    characters_info: "[]".to_string(),
                    foreshadow_reminders: "[]".to_string(),
                    relevant_memories: String::new(),
                    research_query: String::new(),
                    research_assets: "[]".to_string(),
                    external_assets: "[]".to_string(),
                    reference_assets: "[]".to_string(),
                    mcp_references: String::new(),
                },
        }
    }

    fn build_test_batch_generation_create_workflow_launch(
        task_spec: BatchGenerationCreateTaskSpec,
        normalized_target_word_count: i32,
        chapters_to_generate: Vec<BatchGenerationCreateChapterTarget>,
        user_id: &str,
        runtime_seed: BatchGenerationCreateRuntimeSeed,
    ) -> PreparedBatchGenerationCreateWorkflowLaunch {
        super::PreparedBatchGenerationCreateWorkflowLaunch::from_runtime_seed(
            task_spec,
            normalized_target_word_count,
            chapters_to_generate,
            user_id,
            runtime_seed,
            build_test_generation_execution_config(),
            test_single_generation_gateway_config(),
        )
    }

    fn build_test_batch_generation_create_workflow_entry(
        task_id: &str,
        project_id: &str,
        task_spec: BatchGenerationCreateTaskSpec,
        normalized_target_word_count: i32,
        chapters_to_generate: Vec<BatchGenerationCreateChapterTarget>,
        user_id: &str,
        runtime_seed: BatchGenerationCreateRuntimeSeed,
    ) -> BatchGenerationCreateLaunchPersistencePlan {
        BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            task_id.to_string(),
            project_id.to_string(),
            build_test_batch_generation_create_workflow_launch(
                task_spec,
                normalized_target_word_count,
                chapters_to_generate,
                user_id,
                runtime_seed,
            ),
        )
    }

    fn test_single_generation_gateway_config() -> ChapterCandidateRouteGatewayConfig {
        ChapterCandidateRouteGatewayConfig {
            rust_executor_enabled: true,
            fallback_on_rust_error: false,
            disabled_reason: Some("test batch resume single-generation gateway".to_string()),
            rollback_boundary: "test_batch_resume_single_generation_gateway".to_string(),
        }
    }

    #[test]
    fn should_normalize_batch_generation_target_word_count() {
        assert_eq!(normalize_chapter_generation_target_word_count(None), 3000);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(-100)),
            1
        );
        assert_eq!(normalize_chapter_generation_target_word_count(Some(0)), 1);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(2500)),
            2500
        );
    }

    #[test]
    fn should_build_batch_generation_create_chapter_target_projection() {
        let target = BatchGenerationCreateChapterTarget::from_model(&chapter_model());

        assert_eq!(target.id, "chapter-7");
        assert_eq!(target.chapter_number, 7);
        assert_eq!(target.title, "第七章");
    }

    #[test]
    fn should_reject_unknown_batch_generation_create_route_fields_like_python_schema() {
        let error = serde_json::from_value::<BatchGenerationCreateRouteRequest>(json!({
            "start_chapter_number": 1,
            "count": 2,
            "unexpected_field": true
        }))
        .expect_err("python BatchGenerateRequest forbids extra fields");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn should_accept_known_batch_generation_create_route_fields_with_strict_schema() {
        let request = serde_json::from_value::<BatchGenerationCreateRouteRequest>(json!({
            "start_chapter_number": 1,
            "count": 2,
            "target_word_count": 3000,
            "creative_mode": "hook",
            "quality_notes": "keep pacing tight"
        }))
        .expect("known python BatchGenerateRequest fields should parse");

        assert_eq!(request.start_chapter_number, 1);
        assert_eq!(request.count, 2);
        assert_eq!(request.target_word_count, Some(3000));
        assert_eq!(request.creative_mode.as_deref(), Some("hook"));
        assert_eq!(request.quality_notes.as_deref(), Some("keep pacing tight"));
    }

    #[test]
    fn should_reject_batch_generation_create_route_null_for_non_nullable_python_default_fields() {
        for (field_name, payload) in [
            (
                "enable_analysis",
                json!({
                    "start_chapter_number": 1,
                    "count": 2,
                    "enable_analysis": null
                }),
            ),
            (
                "enable_mcp",
                json!({
                    "start_chapter_number": 1,
                    "count": 2,
                    "enable_mcp": null
                }),
            ),
            (
                "max_retries",
                json!({
                    "start_chapter_number": 1,
                    "count": 2,
                    "max_retries": null
                }),
            ),
        ] {
            let error =
                serde_json::from_value::<BatchGenerationCreateRouteRequest>(payload).unwrap_err();

            assert!(
                error.to_string().contains("invalid type: null"),
                "{field_name} should reject explicit null like Python defaulted fields"
            );
        }
    }

    #[test]
    fn should_keep_batch_generation_create_route_nullable_fields_accepting_null() {
        let request = serde_json::from_value::<BatchGenerationCreateRouteRequest>(json!({
            "start_chapter_number": 1,
            "count": 2,
            "target_word_count": null,
            "enable_web_research": null
        }))
        .expect("Python Optional fields should keep accepting explicit null");

        assert_eq!(request.target_word_count, None);
        assert_eq!(request.enable_web_research, None);
    }

    #[test]
    fn should_apply_batch_generation_create_python_defaults_when_fields_are_missing() {
        let route_request = serde_json::from_value::<BatchGenerationCreateRouteRequest>(json!({
            "start_chapter_number": 1,
            "count": 2
        }))
        .expect("missing defaulted route fields should parse");
        assert_eq!(route_request.enable_analysis, None);
        assert_eq!(route_request.enable_mcp, None);
        assert_eq!(route_request.max_retries, None);

        let request = BatchGenerationCreateWorkflowRequest::from_route_request(route_request);
        let compat = request.compat_options_with_web_research_default(false);

        assert!(!request.enable_analysis);
        assert_eq!(request.max_retries, 3);
        assert!(compat.enable_mcp());
        assert!(!compat.web_research_enabled());
    }

    #[test]
    fn should_keep_batch_generation_create_route_payload_request_contract() {
        let request = build_batch_generation_create_workflow_request_from_route_payload(
            BatchGenerationCreateRouteRequest {
                start_chapter_number: 5,
                count: 3,
                style_id: Some(9),
                target_word_count: Some(3200),
                enable_analysis: Some(true),
                enable_mcp: Some(true),
                enable_web_research: Some(false),
                web_research_query: Some("ignored".to_string()),
                max_retries: Some(6),
                model: Some("gpt-4.1-mini".to_string()),
                creative_mode: Some("hook".to_string()),
                story_focus: Some("advance_plot".to_string()),
                plot_stage: Some("climax".to_string()),
                story_creation_brief: Some("brief".to_string()),
                quality_preset: Some("plot_drive".to_string()),
                quality_notes: Some("notes".to_string()),
                story_repair_summary: Some("repair".to_string()),
                story_repair_targets: Some(vec!["target-a".to_string()]),
                story_preserve_strengths: Some(vec!["strength-a".to_string()]),
            },
        );

        assert_eq!(request.start_chapter_number, 5);
        assert_eq!(request.count, 3);
        assert_eq!(request.style_id, Some(9));
        assert_eq!(request.target_word_count, Some(3200));
        assert!(request.enable_analysis);
        assert_eq!(request.enable_mcp, Some(true));
        assert_eq!(request.enable_web_research, Some(false));
        assert_eq!(request.max_retries, 6);
        assert_eq!(request.model_override.as_deref(), Some("gpt-4.1-mini"));
        assert_eq!(request.creative_mode.as_deref(), Some("hook"));
        assert_eq!(request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(request.plot_stage.as_deref(), Some("climax"));
        assert_eq!(request.quality_preset.as_deref(), Some("plot_drive"));
    }

    #[test]
    fn should_normalize_batch_generation_create_generation_fields_like_python_schema() {
        let request = BatchGenerationCreateWorkflowRequest::from_route_request(
            BatchGenerationCreateRouteRequest {
                start_chapter_number: 1,
                count: 2,
                creative_mode: Some(" hook ".to_string()),
                story_focus: Some(" advance_plot ".to_string()),
                plot_stage: Some(" development ".to_string()),
                story_creation_brief: Some(" 本轮强化开场钩子 ".to_string()),
                quality_preset: Some(" plot_drive ".to_string()),
                quality_notes: Some(" 压缩说明段 ".to_string()),
                story_repair_summary: Some(" 修复中段节奏 ".to_string()),
                ..Default::default()
            },
        );

        assert_eq!(request.creative_mode.as_deref(), Some("hook"));
        assert_eq!(request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(request.plot_stage.as_deref(), Some("development"));
        assert_eq!(
            request.story_creation_brief.as_deref(),
            Some("本轮强化开场钩子")
        );
        assert_eq!(request.quality_preset.as_deref(), Some("plot_drive"));
        assert_eq!(request.quality_notes.as_deref(), Some("压缩说明段"));
        assert_eq!(
            request.story_repair_summary.as_deref(),
            Some("修复中段节奏")
        );
    }

    #[test]
    fn should_convert_blank_batch_generation_create_generation_fields_to_none() {
        let request = BatchGenerationCreateWorkflowRequest::from_route_request(
            BatchGenerationCreateRouteRequest {
                start_chapter_number: 1,
                count: 2,
                creative_mode: Some("   ".to_string()),
                story_focus: Some("\t".to_string()),
                plot_stage: Some("\n".to_string()),
                story_creation_brief: Some("   ".to_string()),
                quality_preset: Some("   ".to_string()),
                quality_notes: Some("   ".to_string()),
                story_repair_summary: Some("   ".to_string()),
                ..Default::default()
            },
        );

        assert!(request.creative_mode.is_none());
        assert!(request.story_focus.is_none());
        assert!(request.plot_stage.is_none());
        assert!(request.story_creation_brief.is_none());
        assert!(request.quality_preset.is_none());
        assert!(request.quality_notes.is_none());
        assert!(request.story_repair_summary.is_none());
    }

    #[test]
    fn should_seed_batch_runtime_state_with_normalized_generation_fields() {
        let request = BatchGenerationCreateWorkflowRequest::from_route_request(
            BatchGenerationCreateRouteRequest {
                start_chapter_number: 1,
                count: 2,
                creative_mode: Some(" hook ".to_string()),
                story_focus: Some(" advance_plot ".to_string()),
                plot_stage: Some(" development ".to_string()),
                story_creation_brief: Some(" 强化开场悬念 ".to_string()),
                quality_preset: Some(" plot_drive ".to_string()),
                quality_notes: Some(" 保持短句推进 ".to_string()),
                story_repair_summary: Some(" 修复伏笔衔接 ".to_string()),
                ..Default::default()
            },
        );

        let runtime_state = request.into_request_runtime_state(false);
        let payload = batch_generation_request_runtime_state_payload(&runtime_state);
        let compat_payload = &payload["batch_request_runtime_state"]["compat_options"];

        assert_eq!(compat_payload["creative_mode"], "hook");
        assert_eq!(compat_payload["story_focus"], "advance_plot");
        assert_eq!(compat_payload["plot_stage"], "development");
        assert_eq!(compat_payload["story_creation_brief"], "强化开场悬念");
        assert_eq!(compat_payload["quality_preset"], "plot_drive");
        assert_eq!(compat_payload["quality_notes"], "保持短句推进");
        assert_eq!(compat_payload["story_repair_summary"], "修复伏笔衔接");
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "修复伏笔衔接"
        );
    }

    #[test]
    fn should_project_batch_generation_create_targets_directly() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        assert_eq!(
            chapters_to_generate
                .iter()
                .map(|target| target.id.clone())
                .collect::<Vec<_>>(),
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        let chapter_id_payload = Value::Array(
            chapters_to_generate
                .iter()
                .map(|target| target.id.clone())
                .into_iter()
                .map(|chapter_id| json!(chapter_id))
                .collect(),
        );
        assert_eq!(chapter_id_payload, json!(["chapter-1", "chapter-2"]));
        let chapters_to_generate_payload = chapters_to_generate
            .iter()
            .map(|target| {
                json!({
                    "id": target.id,
                    "chapter_number": target.chapter_number,
                    "title": target.title,
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(chapters_to_generate_payload[0]["id"], "chapter-1");
        assert_eq!(chapters_to_generate_payload[1]["title"], "Second");
        assert_eq!(chapters_to_generate.len() as i32, 2);
    }

    #[test]
    fn should_select_batch_generation_create_range_from_project_chapters() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 2,
            count: 2,
            ..Default::default()
        };

        let selected = request
            .select_chapters_for_generation_range(vec![
                chapter_model_with_number(1),
                chapter_model_with_number(2),
                chapter_model_with_number(3),
                chapter_model_with_number(4),
            ])
            .expect("selected chapters");

        assert_eq!(
            selected
                .iter()
                .map(|chapter| chapter.chapter_number)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[test]
    fn should_distinguish_empty_project_from_empty_batch_generation_range() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 5,
            count: 2,
            ..Default::default()
        };

        let empty_project_error = request
            .select_chapters_for_generation_range(Vec::new())
            .expect_err("empty project should fail");
        let empty_range_error = request
            .select_chapters_for_generation_range(vec![
                chapter_model_with_number(1),
                chapter_model_with_number(2),
            ])
            .expect_err("empty range should fail");

        assert!(matches!(
            empty_project_error,
            PrepareBatchGenerationCreateRequestError::ProjectHasNoChapters
        ));
        assert!(matches!(
            empty_range_error,
            PrepareBatchGenerationCreateRequestError::ChaptersNotFound
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_count_above_python_limit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 21,
            max_retries: 3,
            ..Default::default()
        };

        let error = request
            .validate_request_bounds()
            .expect_err("count above python limit should fail");

        assert!(matches!(
            error,
            PrepareBatchGenerationCreateRequestError::InvalidCountTooLarge
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_target_word_count_below_python_limit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            target_word_count: Some(499),
            max_retries: 3,
            ..Default::default()
        };

        let error = request
            .validate_request_bounds()
            .expect_err("target word count below python limit should fail");

        assert!(matches!(
            error,
            PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooSmall
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_target_word_count_above_python_limit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            target_word_count: Some(10_001),
            max_retries: 3,
            ..Default::default()
        };

        let error = request
            .validate_request_bounds()
            .expect_err("target word count above python limit should fail");

        assert!(matches!(
            error,
            PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooLarge
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_max_retries_outside_python_bounds() {
        let too_low = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            max_retries: -1,
            ..Default::default()
        };
        let too_high = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            max_retries: 6,
            ..Default::default()
        };

        assert!(matches!(
            too_low
                .validate_request_bounds()
                .expect_err("negative max_retries should fail"),
            PrepareBatchGenerationCreateRequestError::InvalidMaxRetries
        ));
        assert!(matches!(
            too_high
                .validate_request_bounds()
                .expect_err("max_retries above python limit should fail"),
            PrepareBatchGenerationCreateRequestError::InvalidMaxRetries
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_invalid_generation_choice_fields() {
        let cases = [
            (
                BatchGenerationCreateWorkflowRequest {
                    start_chapter_number: 1,
                    count: 2,
                    max_retries: 3,
                    creative_mode: Some("too_fancy".to_string()),
                    ..Default::default()
                },
                PrepareBatchGenerationCreateRequestError::InvalidCreativeMode,
            ),
            (
                BatchGenerationCreateWorkflowRequest {
                    start_chapter_number: 1,
                    count: 2,
                    max_retries: 3,
                    story_focus: Some("too_broad".to_string()),
                    ..Default::default()
                },
                PrepareBatchGenerationCreateRequestError::InvalidStoryFocus,
            ),
            (
                BatchGenerationCreateWorkflowRequest {
                    start_chapter_number: 1,
                    count: 2,
                    max_retries: 3,
                    plot_stage: Some("middle".to_string()),
                    ..Default::default()
                },
                PrepareBatchGenerationCreateRequestError::InvalidPlotStage,
            ),
            (
                BatchGenerationCreateWorkflowRequest {
                    start_chapter_number: 1,
                    count: 2,
                    max_retries: 3,
                    quality_preset: Some("max_quality".to_string()),
                    ..Default::default()
                },
                PrepareBatchGenerationCreateRequestError::InvalidQualityPreset,
            ),
        ];

        for (request, expected_error) in cases {
            assert_eq!(
                request
                    .validate_request_bounds()
                    .expect_err("invalid generation choice should fail"),
                expected_error
            );
        }
    }

    #[test]
    fn should_reject_batch_generation_create_generation_text_fields_above_python_limits() {
        let long_brief = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            max_retries: 3,
            story_creation_brief: Some("a".repeat(1201)),
            ..Default::default()
        };
        let long_quality_notes = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            max_retries: 3,
            quality_notes: Some("b".repeat(601)),
            ..Default::default()
        };

        assert_eq!(
            long_brief
                .validate_request_bounds()
                .expect_err("story_creation_brief above python limit should fail"),
            PrepareBatchGenerationCreateRequestError::StoryCreationBriefTooLong
        );
        assert_eq!(
            long_quality_notes
                .validate_request_bounds()
                .expect_err("quality_notes above python limit should fail"),
            PrepareBatchGenerationCreateRequestError::QualityNotesTooLong
        );
    }

    #[test]
    fn should_accept_batch_generation_create_python_request_bounds() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 20,
            target_word_count: Some(10_000),
            max_retries: 5,
            ..Default::default()
        };
        let lower_bound_request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 1,
            target_word_count: Some(500),
            max_retries: 0,
            ..Default::default()
        };
        let default_target_request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 1,
            target_word_count: None,
            max_retries: 3,
            ..Default::default()
        };
        let choice_and_text_request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 1,
            target_word_count: Some(3000),
            max_retries: 3,
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            quality_preset: Some("plot_drive".to_string()),
            story_creation_brief: Some("a".repeat(1200)),
            quality_notes: Some("b".repeat(600)),
            ..Default::default()
        };
        let blank_choice_and_text_request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 1,
            max_retries: 3,
            creative_mode: Some("   ".to_string()),
            story_focus: Some("   ".to_string()),
            plot_stage: Some("   ".to_string()),
            quality_preset: Some("   ".to_string()),
            story_creation_brief: Some("   ".to_string()),
            quality_notes: Some("   ".to_string()),
            ..Default::default()
        };

        request
            .validate_request_bounds()
            .expect("python upper bounds should pass");
        lower_bound_request
            .validate_request_bounds()
            .expect("python lower bounds should pass");
        default_target_request
            .validate_request_bounds()
            .expect("default target word count should pass");
        choice_and_text_request
            .validate_request_bounds()
            .expect("valid python generation choices and text lengths should pass");
        blank_choice_and_text_request
            .validate_request_bounds()
            .expect("blank choices and texts normalize to None in python");
    }

    #[test]
    fn should_keep_batch_generation_route_write_workflow_project_access_error_shape() {
        let error = CreateBatchGenerationWriteWorkflowError::ProjectAccess(
            ProjectAccessQueryError::NotFoundOrAccessDenied,
        );

        assert!(matches!(
            error,
            CreateBatchGenerationWriteWorkflowError::ProjectAccess(
                ProjectAccessQueryError::NotFoundOrAccessDenied
            )
        ));
    }

    #[test]
    fn should_keep_batch_generation_route_write_workflow_error_shape() {
        let error = CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidCount,
        );

        assert!(matches!(
            error,
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::InvalidCount
            )
        ));
    }

    #[test]
    fn should_apply_effective_style_id_to_batch_generation_create_task_spec() {
        let task_spec = BatchGenerationCreateTaskSpec {
            start_chapter_number: 1,
            style_id: None,
            enable_analysis: false,
            max_retries: 3,
        }
        .with_effective_style_id(Some(12));

        assert_eq!(task_spec.style_id, Some(12));
        assert_eq!(task_spec.start_chapter_number, 1);
        assert!(!task_spec.enable_analysis);
        assert_eq!(task_spec.max_retries, 3);
    }

    #[test]
    fn should_select_explicit_batch_generation_create_style_before_default_style() {
        assert_eq!(
            super::select_batch_generation_create_effective_style_id(Some(9), Some(12)),
            Some(9)
        );
        assert_eq!(
            super::select_batch_generation_create_effective_style_id(None, Some(12)),
            Some(12)
        );
        assert_eq!(
            super::select_batch_generation_create_effective_style_id(None, None),
            None
        );
    }

    #[test]
    fn should_keep_batch_generation_create_prerequisite_error_shape() {
        let error = CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::PrerequisitesBlocked(
                "前置章节尚未完成: 2 章".to_string(),
            ),
        );

        assert!(matches!(
            error,
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::PrerequisitesBlocked(detail)
            ) if detail == "前置章节尚未完成: 2 章"
        ));
    }

    #[test]
    fn should_keep_batch_generation_route_write_workflow_config_error_shape() {
        let error = CreateBatchGenerationWriteWorkflowError::Config("model missing".to_string());

        assert!(matches!(
            error,
            CreateBatchGenerationWriteWorkflowError::Config(detail) if detail == "model missing"
        ));
    }

    #[tokio::test]
    async fn should_persist_db_backed_created_batch_generation_from_rust_write_owner() {
        let db = setup_batch_write_owner_db().await;
        seed_create_write_owner_fixture(&db).await;

        let payload = super::start_owned_batch_generation_write_workflow(
            &db,
            "project-create-db-smoke",
            "user-create-db-smoke",
            BatchGenerationCreateRouteRequest {
                start_chapter_number: 2,
                count: 2,
                style_id: None,
                target_word_count: Some(2500),
                enable_analysis: Some(true),
                enable_mcp: Some(false),
                enable_web_research: None,
                web_research_query: Some("旧都城线索".to_string()),
                max_retries: Some(4),
                model: Some("create-db-model".to_string()),
                creative_mode: Some("balanced".to_string()),
                story_focus: Some("advance_plot".to_string()),
                plot_stage: Some("development".to_string()),
                story_creation_brief: Some("推进第二卷主线".to_string()),
                quality_preset: Some("plot_drive".to_string()),
                quality_notes: Some("保持节奏紧凑".to_string()),
                story_repair_summary: Some("补强前章伏笔".to_string()),
                story_repair_targets: Some(vec!["伏笔回收".to_string()]),
                story_preserve_strengths: Some(vec!["角色张力".to_string()]),
            },
            test_single_generation_gateway_config(),
        )
        .await
        .expect("db-backed create payload");
        let batch_id = payload["batch_id"]
            .as_str()
            .expect("created batch id")
            .to_string();
        let created_task = batch_generation_task::Entity::find_by_id(&batch_id)
            .one(&db)
            .await
            .expect("load created task")
            .expect("created task exists");
        let created_snapshot = batch_generation_snapshot::Entity::find()
            .filter(batch_generation_snapshot::Column::BatchTaskId.eq(&batch_id))
            .one(&db)
            .await
            .expect("load created snapshot")
            .expect("created snapshot exists");
        let runtime_state = created_snapshot
            .workflow_runtime_state
            .expect("created runtime state");

        assert_eq!(payload["project_id"], "project-create-db-smoke");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["message"], "已创建批量生成任务，共 2 章");
        assert_eq!(payload["chapters_to_generate"][0]["id"], "chapter-create-2");
        assert_eq!(payload["chapters_to_generate"][1]["id"], "chapter-create-3");
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "补强前章伏笔"
        );

        assert_eq!(created_task.project_id, "project-create-db-smoke");
        assert_eq!(created_task.user_id, "user-create-db-smoke");
        assert_eq!(created_task.start_chapter_number, 2);
        assert_eq!(created_task.chapter_count, 2);
        assert_eq!(
            created_task.chapter_ids,
            json!(["chapter-create-2", "chapter-create-3"])
        );
        assert_eq!(created_task.target_word_count, 2500);
        assert!(created_task.enable_analysis);
        assert_eq!(created_task.status, "pending");
        assert_eq!(created_task.total_chapters, 2);
        assert_eq!(created_task.max_retries, 4);

        assert_eq!(runtime_state["phase"], "pending");
        assert_eq!(runtime_state["status"], "pending");
        assert_eq!(runtime_state["last_event"], "queued");
        assert_eq!(
            runtime_state["batch_request_runtime_state"]["model_override"],
            "create-db-model"
        );
        assert_eq!(
            runtime_state["batch_request_runtime_state"]["compat_options"]["web_research_enabled"],
            true
        );
        assert_eq!(
            runtime_state["batch_request_runtime_state"]["compat_options"]["story_repair_summary"],
            "补强前章伏笔"
        );
        assert_eq!(
            runtime_state["active_story_repair_payload"]["summary"],
            "补强前章伏笔"
        );
    }

    #[test]
    fn should_keep_batch_generation_write_workflow_request_contract_transport_free() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 3,
            count: 2,
            style_id: Some(7),
            target_word_count: Some(2800),
            enable_analysis: false,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 3,
            model_override: Some("gpt-4.1".to_string()),
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

        assert_eq!(request.start_chapter_number, 3);
        assert_eq!(request.count, 2);
        assert_eq!(request.style_id, Some(7));
        assert_eq!(request.target_word_count, Some(2800));
        assert!(!request.enable_analysis);
        assert_eq!(request.enable_mcp, None);
        assert_eq!(request.max_retries, 3);
        assert_eq!(request.model_override.as_deref(), Some("gpt-4.1"));
    }

    #[test]
    fn should_estimate_batch_generation_task_minutes_with_minimum_floor() {
        assert_eq!(estimated_task_minutes(0, 3000, false), 1);
        assert_eq!(estimated_task_minutes(1, 3000, false), 2);
        assert_eq!(estimated_task_minutes(1, 6000, false), 4);
        assert_eq!(estimated_task_minutes(3, 3000, true), 9);
        assert_eq!(estimated_task_minutes(2, 2800, true), 5);
    }

    #[test]
    fn should_build_batch_generation_create_response_payload() {
        let chapters = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let queued_runtime_state =
            super::BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
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
                        "source": "create_response_test"
                    },
                    "candidate_gateway": {
                        "execution_path": "rust_candidate_executor",
                        "fallback_applied": false,
                        "rollback_boundary": "test_batch_candidate_gateway"
                    },
                    "active_story_repair_payload": {
                        "summary": "沿用批量修复建议",
                        "repair_targets": ["压缩说明"],
                        "source": "recent_history_summary",
                        "scope": "batch"
                    }
                })),
            );
        let response_chapters = chapters
            .iter()
            .map(|target| BatchGenerationQueuedCreateResponseChapter {
                id: target.id.clone(),
                chapter_number: target.chapter_number,
                title: target.title.clone(),
            })
            .collect::<Vec<_>>();
        let payload = queued_runtime_state.into_create_response_payload(
            "task-1",
            "project-1",
            &response_chapters,
            3000,
            false,
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["task_type"], "chapters_batch_generate");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["last_event"], "queued");
        assert_eq!(payload["checkpoint"]["phase"], "pending");
        assert_eq!(payload["checkpoint"]["total"], 2);
        assert_eq!(payload["message"], "已创建批量生成任务，共 2 章");
        assert_eq!(payload["chapters_to_generate"][0]["id"], "chapter-1");
        assert_eq!(payload["chapters_to_generate"][1]["title"], "Second");
        assert_eq!(payload["estimated_time_minutes"], 4);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 86);
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_history_context"]["source"],
            "create_response_test"
        );
        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            payload["checkpoint"]["candidate_gateway"],
            payload["candidate_gateway"]
        );
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "沿用批量修复建议"
        );
    }

    #[test]
    fn should_build_batch_generation_create_launch_persistence_plan_from_create_parts() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            style_id: Some(9),
            target_word_count: Some(2800),
            enable_analysis: true,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 5,
            model_override: Some("gpt-4.1".to_string()),
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
        let normalized_target_word_count = 2800;
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default(),
            Some("gpt-4.1".to_string()),
        );
        let runtime_state_payload = json!({
            "batch_request_runtime_state": request_runtime_state.clone(),
            "quality_metrics_summary": {
                "chapter_count": 2,
                "overall_score": 86.0,
                "quality_runtime_context": {
                    "recent_metrics": [{"overall_score": 86}],
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
                "source": "plan_response"
            },
            "active_story_repair_payload": {
                "summary": "沿用批量修复建议",
                "repair_targets": ["压缩说明"],
                "source": "recent_history_summary",
                "scope": "batch"
            }
        });
        let runtime_seed =
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(runtime_state_payload);
        let plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-1".to_string(),
            "project-1".to_string(),
            build_test_batch_generation_create_workflow_launch(
                request.task_spec(),
                normalized_target_word_count,
                chapters_to_generate,
                "user-1",
                runtime_seed,
            ),
        );
        let response_payload = plan.response_payload();

        assert_eq!(response_payload["batch_id"], "task-1");
        assert_eq!(response_payload["message"], "已创建批量生成任务，共 2 章");
        assert_eq!(response_payload["project_id"], "project-1");
        assert_eq!(response_payload["task_type"], "chapters_batch_generate");
        assert_eq!(response_payload["status"], "pending");
        assert_eq!(response_payload["checkpoint"]["last_event"], "queued");
        assert_eq!(response_payload["checkpoint"]["total"], 2);
        assert_eq!(
            response_payload["latest_quality_metrics"]["overall_score"],
            86
        );
        assert_eq!(
            response_payload["quality_metrics_summary_state"]["chapter_count"],
            2
        );
        assert_eq!(
            response_payload["quality_history_context"]["source"],
            "plan_response"
        );
        assert_eq!(
            response_payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            response_payload["checkpoint"]["candidate_gateway"],
            response_payload["candidate_gateway"]
        );
        assert_eq!(
            response_payload["active_story_repair_payload"]["summary"],
            "沿用批量修复建议"
        );
        assert_eq!(
            response_payload["chapters_to_generate"][0]["id"],
            "chapter-1"
        );
        assert_eq!(plan.runtime_input.user_id, "user-1");
        assert_eq!(
            plan.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(plan.runtime_input.target_word_count, 2800);
        assert_eq!(
            plan.runtime_input.ai_config.provider,
            crate::ai::AIConfig::default().provider
        );
    }

    #[test]
    fn should_build_batch_generation_create_launch_task_from_create_parts() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            style_id: Some(9),
            target_word_count: Some(2800),
            enable_analysis: true,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 5,
            model_override: Some("gpt-4.1".to_string()),
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
        let now = NaiveDate::from_ymd_opt(2026, 5, 28)
            .expect("valid date")
            .and_hms_opt(22, 20, 0)
            .expect("valid time");
        let normalized_target_word_count = 2800;
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
            json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
        );
        let plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-1".to_string(),
            "project-1".to_string(),
            build_test_batch_generation_create_workflow_launch(
                request.task_spec(),
                normalized_target_word_count,
                chapters_to_generate,
                "user-1",
                runtime_seed,
            ),
        );
        let response_payload = plan.response_payload();
        let task = plan.background_task_active_model(now);

        assert_eq!(plan.task_seed.total_chapters, 2);
        assert_eq!(response_payload["batch_id"], "task-1");
        assert_eq!(response_payload["message"], "已创建批量生成任务，共 2 章");
        assert_eq!(task.id, sea_orm::Set("task-1".to_string()));
        assert_eq!(task.total_chapters, sea_orm::Set(2));
        assert_eq!(
            plan.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(normalized_target_word_count, 2800);
    }

    #[test]
    fn should_keep_batch_generation_create_task_spec_owner_contract_explicit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 9,
            count: 2,
            style_id: Some(4),
            target_word_count: Some(3200),
            enable_analysis: true,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 4,
            model_override: Some("gpt-4.1".to_string()),
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
        let task_spec = request.task_spec();

        assert_eq!(
            task_spec,
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 9,
                style_id: Some(4),
                enable_analysis: true,
                max_retries: 4,
            }
        );
    }

    #[test]
    fn should_keep_batch_generation_create_persistence_plan_owner_contract_explicit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 9,
            count: 2,
            style_id: Some(4),
            target_word_count: Some(3200),
            enable_analysis: true,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 4,
            model_override: Some("gpt-4.1".to_string()),
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
        let persistence_plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-9".to_string(),
            "project-9".to_string(),
            build_test_batch_generation_create_workflow_launch(
                request.task_spec(),
                3200,
                vec![
                    build_chapter_target("chapter-9", 9, "Ninth"),
                    build_chapter_target("chapter-10", 10, "Tenth"),
                ],
                "user-9",
                BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                    json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
                ),
            ),
        );

        assert_eq!(persistence_plan.task_seed.id, "task-9");
        assert_eq!(persistence_plan.task_seed.project_id, "project-9");
        assert_eq!(persistence_plan.task_seed.total_chapters, 2);
        assert_eq!(persistence_plan.runtime_input.target_word_count, 3200);
        assert_eq!(
            persistence_plan.task_seed.chapter_ids,
            json!(["chapter-9", "chapter-10"])
        );
        assert_eq!(persistence_plan.task_seed.start_chapter_number, 9);
        assert_eq!(persistence_plan.task_seed.style_id, Some(4));
        assert!(persistence_plan.task_seed.enable_analysis);
        assert_eq!(persistence_plan.task_seed.max_retries, 4);
    }

    #[test]
    fn should_keep_batch_generation_create_persistence_plan_contract_from_create_launch_owner() {
        let persistence_plan = build_test_batch_generation_create_workflow_entry(
            "task-11",
            "project-11",
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 11,
                style_id: Some(6),
                enable_analysis: true,
                max_retries: 4,
            },
            3100,
            vec![
                build_chapter_target("chapter-11", 11, "Eleventh"),
                build_chapter_target("chapter-12", 12, "Twelfth"),
            ],
            "user-11",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
            ),
        );

        assert_eq!(persistence_plan.task_seed.id, "task-11");
        assert_eq!(persistence_plan.task_seed.project_id, "project-11");
        assert_eq!(persistence_plan.task_seed.start_chapter_number, 11);
        assert_eq!(persistence_plan.task_seed.total_chapters, 2);
        assert_eq!(persistence_plan.runtime_input.user_id, "user-11");
        assert_eq!(persistence_plan.runtime_input.target_word_count, 3100);
        assert_eq!(
            persistence_plan.runtime_input.chapter_ids,
            vec!["chapter-11".to_string(), "chapter-12".to_string()]
        );
    }

    #[test]
    fn should_keep_batch_generation_create_persistence_plan_payload_owner_contract() {
        let persistence_plan = build_test_batch_generation_create_workflow_entry(
            "task-21",
            "project-21",
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 21,
                style_id: Some(8),
                enable_analysis: false,
                max_retries: 3,
            },
            2800,
            vec![
                build_chapter_target("chapter-21", 21, "Twenty-first"),
                build_chapter_target("chapter-22", 22, "Twenty-second"),
            ],
            "user-21",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
                "batch_request_runtime_state": {"model_override": "gpt-4.1"},
                "quality_metrics_summary": {"overall_score": 88}
            })),
        );

        assert_eq!(persistence_plan.response_payload()["batch_id"], "task-21");
        assert_eq!(
            persistence_plan.response_payload()["message"],
            "已创建批量生成任务，共 2 章"
        );
        assert_eq!(
            persistence_plan.response_payload()["quality_metrics_summary"]["overall_score"],
            88
        );
        assert_eq!(
            persistence_plan.task_seed.chapter_ids,
            json!(["chapter-21", "chapter-22"])
        );
    }

    #[test]
    fn should_keep_batch_generation_create_persistence_plan_start_owner_contract() {
        let persistence_plan = build_test_batch_generation_create_workflow_entry(
            "task-31",
            "project-31",
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 31,
                style_id: Some(5),
                enable_analysis: true,
                max_retries: 4,
            },
            3600,
            vec![
                build_chapter_target("chapter-31", 31, "Thirty-first"),
                build_chapter_target("chapter-32", 32, "Thirty-second"),
            ],
            "user-31",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
                "batch_request_runtime_state": {"model_override": "gpt-4.1"},
                "quality_metrics_summary": {"chapter_count": 2}
            })),
        );

        assert_eq!(persistence_plan.task_seed.id, "task-31");
        assert_eq!(persistence_plan.task_seed.project_id, "project-31");
        assert_eq!(persistence_plan.runtime_input.user_id, "user-31");
        assert_eq!(persistence_plan.runtime_input.target_word_count, 3600);
        assert_eq!(
            persistence_plan.response_payload()["quality_metrics_summary"]["chapter_count"],
            2
        );
    }

    #[test]
    fn should_keep_batch_generation_create_runtime_seed_contract() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("沿用历史修复建议".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                ..crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let runtime_state_payload = json!({
            "batch_request_runtime_state": request_runtime_state.clone(),
            "active_story_repair_payload": {
                "summary": "沿用历史修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            },
            "quality_metrics_summary": {
                "overall_score": 84
            }
        });

        let runtime_seed =
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(runtime_state_payload);
        let (runtime_state_payload, resolved_compat_options) = runtime_seed.into_parts();

        assert_eq!(
            runtime_state_payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            runtime_state_payload["quality_metrics_summary"]["overall_score"],
            84
        );
        assert_eq!(
            resolved_compat_options.story_repair_summary(),
            "沿用历史修复建议"
        );
        assert_eq!(
            resolved_compat_options.story_repair_targets(),
            &["压缩说明".to_string()]
        );
    }

    #[test]
    fn should_build_batch_generation_create_workflow_runtime_parts_from_runtime_seed() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("沿用历史修复建议".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                ..crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
            "batch_request_runtime_state": request_runtime_state,
            "active_story_repair_payload": {
                "summary": "沿用历史修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            },
            "quality_metrics_summary": {
                "overall_score": 84
            }
        }));

        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 1,
                style_id: Some(9),
                enable_analysis: true,
                max_retries: 5,
            },
            2800,
            chapters_to_generate,
            "user-1",
            runtime_seed,
        );

        assert_eq!(
            workflow_launch.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(workflow_launch.runtime_input.user_id, "user-1");
        assert_eq!(workflow_launch.runtime_input.target_word_count, 2800);
        assert_eq!(
            workflow_launch
                .runtime_input
                .compat_options
                .story_repair_summary(),
            "沿用历史修复建议"
        );
        assert_eq!(
            workflow_launch.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                ["overall_score"],
            84
        );
    }

    #[test]
    fn should_materialize_batch_generation_create_workflow_launch_parts_inside_runtime_seed_owner()
    {
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
            "batch_request_runtime_state": {
                "compat_options": {
                    "enable_analysis": true,
                    "story_repair_summary": "沿用历史修复建议",
                    "story_repair_targets": ["压缩说明"]
                },
                "model_override": "gpt-4.1"
            },
            "active_story_repair_payload": {
                "summary": "沿用历史修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            },
            "quality_metrics_summary": {
                "overall_score": 84
            }
        }));

        let (startup_snapshot_plan, runtime_input) = runtime_seed.into_workflow_launch_parts(
            "user-1".to_string(),
            vec!["chapter-1".to_string(), "chapter-2".to_string()],
            2,
            2800,
            build_test_generation_execution_config(),
            test_single_generation_gateway_config(),
        );

        assert_eq!(
            startup_snapshot_plan.runtime_state()["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["overall_score"],
            84
        );
        assert_eq!(runtime_input.user_id, "user-1");
        assert_eq!(runtime_input.target_word_count, 2800);
        assert_eq!(
            runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(
            runtime_input.compat_options.story_repair_summary(),
            "沿用历史修复建议"
        );
        assert_eq!(
            runtime_input.compat_options.story_repair_targets(),
            &["压缩说明".to_string()]
        );
    }

    #[test]
    fn should_build_batch_generation_create_workflow_launch_into_persistence_plan() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 1,
                style_id: Some(9),
                enable_analysis: true,
                max_retries: 5,
            },
            2800,
            chapters_to_generate.clone(),
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
            ),
        );

        let plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-1".to_string(),
            "project-1".to_string(),
            workflow_launch,
        );

        assert_eq!(plan.task_seed.id, "task-1");
        assert_eq!(plan.task_seed.project_id, "project-1");
        assert_eq!(plan.runtime_input.user_id, "user-1");
        assert_eq!(plan.task_seed.start_chapter_number, 1);
        assert_eq!(plan.task_seed.total_chapters, 2);
        assert_eq!(plan.runtime_input.target_word_count, 2800);
        assert_eq!(
            plan.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
    }

    #[test]
    fn should_materialize_batch_generation_create_persistence_payload_inside_persistence_plan_owner(
    ) {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 1,
                style_id: Some(9),
                enable_analysis: true,
                max_retries: 5,
            },
            2800,
            chapters_to_generate.clone(),
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
                "batch_request_runtime_state": {
                    "model_override": "gpt-4.1"
                },
                "quality_metrics_summary": {
                    "overall_score": 86
                }
            })),
        );

        let plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-1".to_string(),
            "project-1".to_string(),
            workflow_launch,
        );

        assert_eq!(plan.task_seed.id, "task-1");
        assert_eq!(plan.task_seed.project_id, "project-1");
        assert_eq!(plan.task_seed.start_chapter_number, 1);
        assert_eq!(plan.task_seed.total_chapters, 2);
        assert_eq!(plan.response_payload()["batch_id"], "task-1");
        assert_eq!(
            plan.response_payload()["message"],
            "已创建批量生成任务，共 2 章"
        );
        assert_eq!(
            plan.response_payload()["quality_metrics_summary"]["overall_score"],
            86
        );
        assert_eq!(plan.runtime_input.user_id, "user-1");
        assert_eq!(plan.runtime_input.target_word_count, 2800);
        assert_eq!(
            plan.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(
            plan.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["overall_score"],
            86
        );
    }

    #[test]
    fn should_materialize_batch_generation_create_task_seed_inside_persistence_plan_owner() {
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 3,
                style_id: Some(4),
                enable_analysis: true,
                max_retries: 4,
            },
            3200,
            vec![
                build_chapter_target("chapter-3", 3, "Third"),
                build_chapter_target("chapter-4", 4, "Fourth"),
            ],
            "user-3",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
            ),
        );

        let plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-3".to_string(),
            "project-3".to_string(),
            workflow_launch,
        );

        assert_eq!(
            plan.task_seed,
            BatchGenerationTaskPersistenceSeed {
                id: "task-3".to_string(),
                project_id: "project-3".to_string(),
                user_id: "user-3".to_string(),
                start_chapter_number: 3,
                chapter_count: 2,
                chapter_ids: json!(["chapter-3", "chapter-4"]),
                style_id: Some(4),
                target_word_count: 3200,
                enable_analysis: true,
                total_chapters: 2,
                current_chapter_id: None,
                current_chapter_number: None,
                max_retries: 4,
            }
        );
    }

    #[test]
    fn should_keep_batch_generation_create_workflow_launch_owner_contract_explicit() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 1,
                style_id: Some(9),
                enable_analysis: true,
                max_retries: 5,
            },
            2800,
            chapters_to_generate.clone(),
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
            ),
        );

        assert_eq!(workflow_launch.task_spec.start_chapter_number, 1);
        assert_eq!(workflow_launch.task_spec.style_id, Some(9));
        assert!(workflow_launch.task_spec.enable_analysis);
        assert_eq!(workflow_launch.task_spec.max_retries, 5);
        assert_eq!(
            workflow_launch.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(workflow_launch.runtime_input.target_word_count, 2800);
        assert_eq!(workflow_launch.runtime_input.user_id, "user-1");
        assert_eq!(workflow_launch.chapters_to_generate.len(), 2);
    }

    #[test]
    fn should_keep_batch_generation_create_workflow_launch_runtime_seed_owner_contract() {
        let workflow_launch = super::PreparedBatchGenerationCreateWorkflowLaunch::from_runtime_seed(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 3,
                style_id: Some(4),
                enable_analysis: true,
                max_retries: 4,
            },
            3200,
            vec![
                build_chapter_target("chapter-3", 3, "Third"),
                build_chapter_target("chapter-4", 4, "Fourth"),
            ],
            "user-3",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
                "batch_request_runtime_state": {
                    "model_override": "gpt-4.1"
                },
                "quality_metrics_summary": {
                    "overall_score": 86
                }
            })),
            build_test_generation_execution_config(),
            test_single_generation_gateway_config(),
        );

        let super::PreparedBatchGenerationCreateWorkflowLaunch {
            task_spec,
            chapters_to_generate,
            startup_snapshot_plan,
            runtime_input,
        } = workflow_launch;

        assert_eq!(task_spec.start_chapter_number, 3);
        assert_eq!(task_spec.style_id, Some(4));
        assert!(task_spec.enable_analysis);
        assert_eq!(task_spec.max_retries, 4);
        assert_eq!(chapters_to_generate.len(), 2);
        assert_eq!(
            startup_snapshot_plan.runtime_state()["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["overall_score"],
            86
        );
        assert_eq!(runtime_input.user_id, "user-3");
        assert_eq!(runtime_input.target_word_count, 3200);
        assert_eq!(
            runtime_input.chapter_ids,
            vec!["chapter-3".to_string(), "chapter-4".to_string()]
        );
    }

    #[test]
    fn should_keep_explicit_batch_generation_create_style_over_default_style() {
        let task_spec = BatchGenerationCreateTaskSpec {
            start_chapter_number: 1,
            style_id: Some(9),
            enable_analysis: false,
            max_retries: 3,
        };
        let effective_style_id =
            super::select_batch_generation_create_effective_style_id(task_spec.style_id, Some(12));
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            task_spec.with_effective_style_id(effective_style_id),
            2800,
            vec![build_chapter_target("chapter-1", 1, "First")],
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {}}),
            ),
        );

        assert_eq!(workflow_launch.task_spec.style_id, Some(9));
    }

    #[test]
    fn should_apply_project_default_style_to_batch_generation_create_workflow_launch() {
        let task_spec = BatchGenerationCreateTaskSpec {
            start_chapter_number: 1,
            style_id: None,
            enable_analysis: false,
            max_retries: 3,
        };
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            task_spec.with_effective_style_id(Some(12)),
            2800,
            vec![build_chapter_target("chapter-1", 1, "First")],
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {}}),
            ),
        );

        assert_eq!(workflow_launch.task_spec.style_id, Some(12));
    }

    #[test]
    fn should_build_batch_generation_create_persistence_plan_task_and_response_payload() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let now = NaiveDate::from_ymd_opt(2026, 5, 31)
            .expect("valid date")
            .and_hms_opt(21, 40, 0)
            .expect("valid time");
        let plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-1".to_string(),
            "project-1".to_string(),
            build_test_batch_generation_create_workflow_launch(
                BatchGenerationCreateTaskSpec {
                    start_chapter_number: 1,
                    style_id: Some(9),
                    enable_analysis: true,
                    max_retries: 5,
                },
                2800,
                chapters_to_generate.clone(),
                "user-1",
                BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                    json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
                ),
            ),
        );
        let response_payload = plan.response_payload();
        let task = plan.background_task_active_model(now);

        assert_eq!(plan.task_seed.id, "task-1");
        assert_eq!(response_payload["batch_id"], "task-1");
        assert_eq!(response_payload["estimated_time_minutes"], 5);
        assert_eq!(task.id, sea_orm::Set("task-1".to_string()));
        assert_eq!(plan.runtime_input.user_id, "user-1");
        assert_eq!(plan.runtime_input.target_word_count, 2800);
    }

    #[test]
    fn should_build_batch_generation_create_startup_runtime_state_from_recent_history_summary() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let recent_history_summary = json!({
            "repair_guidance": {
                "summary": "沿用最近三章修复建议",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["尾章钩子"],
                "focus_areas": ["pacing"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复"
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 83}]
            },
            "overall_score": 83
        });

        let startup_runtime_state =
            BatchGenerationCreateStartupRuntimeState::from_recent_history_summary(
                request_runtime_state.clone(),
                Some(recent_history_summary),
            );

        assert_eq!(
            startup_runtime_state.seed_source(),
            BatchGenerationCreateStartupSeedSource::RecentHistorySummary
        );
        assert_eq!(
            startup_runtime_state.request_runtime_state(),
            &request_runtime_state
        );
        assert_eq!(
            startup_runtime_state.runtime_state_payload()["active_story_repair_payload"]["summary"],
            "沿用最近三章修复建议"
        );
        assert_eq!(
            startup_runtime_state.runtime_state_payload()["quality_metrics_summary"]
                ["overall_score"],
            83.0
        );
    }

    #[test]
    fn should_build_batch_generation_create_startup_runtime_state_from_request_only() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("保留手工修复目标".to_string()),
                story_repair_targets: vec!["补强动机".to_string()],
                ..crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default()
            },
            Some("gpt-4.1".to_string()),
        );

        let startup_runtime_state =
            BatchGenerationCreateStartupRuntimeState::from_recent_history_summary(
                request_runtime_state.clone(),
                None,
            );

        assert_eq!(
            startup_runtime_state.seed_source(),
            BatchGenerationCreateStartupSeedSource::RequestOnly
        );
        assert_eq!(
            startup_runtime_state.request_runtime_state(),
            &request_runtime_state
        );
        assert_eq!(
            startup_runtime_state.runtime_state_payload()["active_story_repair_payload"]["summary"],
            "保留手工修复目标"
        );
        assert_eq!(
            startup_runtime_state.runtime_state_payload()["batch_request_runtime_state"]
                ["model_override"],
            "gpt-4.1"
        );
        assert!(startup_runtime_state.runtime_state_payload()["quality_metrics_summary"].is_null());
    }

    #[test]
    fn should_build_batch_generation_create_runtime_seed_inside_startup_owner() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("沿用最近三章修复建议".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                ..crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let startup_runtime_state =
            BatchGenerationCreateStartupRuntimeState::from_recent_history_summary(
                request_runtime_state.clone(),
                Some(json!({
                    "repair_guidance": {
                        "summary": "沿用最近三章修复建议",
                        "repair_targets": ["压缩说明"],
                        "preserve_strengths": ["尾章钩子"],
                        "focus_areas": ["pacing"]
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "repair",
                        "label": "需修复"
                    },
                    "overall_score": 83
                })),
            );

        let runtime_seed = startup_runtime_state.into_runtime_seed();
        let (runtime_state_payload, resolved_compat_options) = runtime_seed.into_parts();
        assert_eq!(
            runtime_state_payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            resolved_compat_options.story_repair_summary(),
            "沿用最近三章修复建议"
        );
        assert_eq!(
            resolved_compat_options.story_repair_targets(),
            &["压缩说明".to_string()]
        );
    }

    #[test]
    fn should_keep_batch_generation_create_runtime_seed_dispatch_ready_contract() {
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
            "batch_request_runtime_state": {
                "compat_options": {
                    "enable_analysis": true,
                    "enable_mcp": true,
                    "web_research_enabled": false,
                    "story_repair_summary": "沿用最近三章修复建议",
                    "story_repair_targets": ["压缩说明"],
                    "story_preserve_strengths": []
                },
                "model_override": "gpt-4.1"
            },
            "active_story_repair_payload": {
                "summary": "沿用最近三章修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            }
        }));
        let (runtime_state_payload, resolved_compat_options) = runtime_seed.into_parts();

        assert_eq!(
            runtime_state_payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            resolved_compat_options.story_repair_summary(),
            "沿用最近三章修复建议"
        );
        assert_eq!(
            resolved_compat_options.story_repair_targets(),
            &["压缩说明".to_string()]
        );
    }

    #[test]
    fn should_materialize_batch_generation_queued_snapshot_inside_runtime_seed_owner() {
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
            "batch_request_runtime_state": {
                "model_override": "gpt-4.1"
            },
            "quality_metrics_summary": {
                "overall_score": 86
            },
            "active_story_repair_payload": {
                "summary": "沿用最近三章修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            }
        }));

        let startup_snapshot_plan = runtime_seed.startup_snapshot_plan(2);

        assert_eq!(
            startup_snapshot_plan.runtime_state()["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["overall_score"],
            86
        );
        assert_eq!(
            startup_snapshot_plan.active_story_repair_payload(),
            Some(json!({
                "summary": "沿用最近三章修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            }))
        );
    }

    #[test]
    fn should_seed_manual_story_repair_payload_into_batch_runtime_state() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions {
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
                story_repair_summary: Some("中段节奏需要压缩".to_string()),
                story_repair_targets: vec!["提前冲突触发".to_string()],
                story_preserve_strengths: vec!["尾章钩子".to_string()],
            },
            Some("gpt-4.1".to_string()),
        );

        let payload = batch_generation_request_runtime_state_payload(&runtime_state);

        assert_eq!(
            payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "中段节奏需要压缩"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["提前冲突触发"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["尾章钩子"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "manual_request"
        );
        assert_eq!(payload["active_story_repair_payload"]["scope"], "batch");
    }

    #[test]
    fn should_skip_empty_manual_story_repair_payload_in_batch_runtime_state() {
        let payload = batch_generation_request_runtime_state_payload(
            &BatchGenerationRequestRuntimeState::new(
                crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default(),
                None,
            ),
        );

        assert!(payload.get("active_story_repair_payload").is_none());
    }

    #[test]
    fn should_merge_manual_and_recent_history_story_repair_state_into_create_runtime_seed() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions {
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
                story_repair_summary: Some("手工摘要".to_string()),
                story_repair_targets: vec!["手工目标".to_string(), "共同目标".to_string()],
                story_preserve_strengths: vec!["手工优点".to_string()],
            },
            Some("gpt-4.1".to_string()),
        );

        let quality_summary = json!({
            "repair_guidance": {
                "summary": "历史摘要",
                "repair_targets": ["共同目标", "历史目标"],
                "preserve_strengths": ["历史优点"],
                "focus_areas": ["历史焦点"],
                "weakest_metric_key": "continuity",
                "weakest_metric_label": "Continuity",
                "weakest_metric_value": 0.62
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "近期质量波动",
                "failed_metrics": [{"label": "Continuity"}]
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 85}],
                "history_scope": "batch"
            },
            "overall_score": 85
        });

        let payload = build_batch_generation_runtime_state_payload_from_parts(
            &runtime_state,
            Some(&quality_summary),
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "手工摘要"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["手工目标", "共同目标", "历史目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["手工优点", "历史优点"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["focus_areas"],
            json!(["历史焦点"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "manual_plus_recent_history_summary"
        );
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 85.0);
        assert_eq!(
            payload["quality_history_context"],
            json!({
                "scope": "batch",
                "recent_metrics": [{
                    "history_index": 0,
                    "overall_score": 85,
                    "repair_guidance": {
                        "summary": "历史摘要",
                        "repair_targets": ["共同目标", "历史目标"],
                        "preserve_strengths": ["历史优点"],
                        "focus_areas": ["历史焦点"],
                        "weakest_metric_key": "continuity",
                        "weakest_metric_label": "Continuity",
                        "weakest_metric_value": 0.62
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "repair",
                        "label": "需修复",
                        "summary": "近期质量波动",
                        "failed_metrics": [{"label": "Continuity"}]
                    }
                }],
                "history_scope": "batch"
            })
        );
    }

    #[test]
    fn should_write_recent_history_quality_state_into_create_runtime_seed_without_manual_input() {
        let quality_summary = json!({
            "repair_guidance": {
                "summary": "历史摘要",
                "repair_targets": ["历史目标"],
                "preserve_strengths": ["历史优点"],
                "focus_areas": ["历史焦点"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "近期质量波动",
                "failed_metrics": [{"label": "Continuity"}]
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 88}]
            },
            "overall_score": 88
        });

        let payload = build_batch_generation_runtime_state_payload_from_parts(
            &BatchGenerationRequestRuntimeState::new(
                crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default(),
                None,
            ),
            Some(&quality_summary),
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "历史摘要"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "recent_history_summary"
        );
        assert_eq!(
            payload["quality_history_context"],
            json!({
                "scope": "batch",
                "recent_metrics": [{
                    "history_index": 0,
                    "overall_score": 88,
                    "repair_guidance": {
                        "summary": "历史摘要",
                        "repair_targets": ["历史目标"],
                        "preserve_strengths": ["历史优点"],
                        "focus_areas": ["历史焦点"]
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "repair",
                        "label": "需修复",
                        "summary": "近期质量波动",
                        "failed_metrics": [{"label": "Continuity"}]
                    }
                }]
            })
        );
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 88.0);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 88);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 88);
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "batch");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 1);
    }

    #[test]
    fn should_seed_batch_runtime_state_with_latest_quality_metrics_context() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::default();
        let quality_summary = json!({
            "repair_guidance": {
                "summary": "历史摘要",
                "repair_targets": ["历史目标"],
                "preserve_strengths": ["历史优点"],
                "focus_areas": ["历史焦点"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "历史门禁"
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 88}]
            },
            "overall_score": 88
        });
        let latest_quality_metrics = json!({
            "repair_guidance": {
                "summary": "最新摘要",
                "repair_targets": ["最新目标"],
                "preserve_strengths": ["最新优点"],
                "focus_areas": ["最新焦点"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "auto_repair",
                "label": "最新门禁",
                "summary": "继续修复"
            },
            "overall_score": 81
        });

        let payload = build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
            &request_runtime_state,
            None,
            Some(&quality_summary),
            Some(&latest_quality_metrics),
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "最新摘要"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["最新目标", "历史目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["最新优点", "历史优点"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["quality_gate_label"],
            "最新门禁"
        );
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 81);
    }

    #[test]
    fn should_aggregate_recent_history_quality_summaries_before_seeding_batch_runtime_state() {
        let first_summary = json!({
            "overall_score": 86,
            "repair_guidance": {
                "summary": "先处理节奏拖沓",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["尾章钩子"],
                "focus_areas": ["pacing", "conflict"]
            },
            "quality_gate": {
                "decision": "repair",
                "failed_metrics": [{"label": "Pacing"}]
            }
        });
        let second_summary = json!({
            "overall_score": 81,
            "repair_guidance": {
                "summary": "补角色动机",
                "repair_targets": ["强化动机", "提前冲突"],
                "preserve_strengths": ["人物口吻"],
                "focus_areas": ["character", "pacing"]
            },
            "quality_gate": {
                "decision": "manual_review",
                "failed_metrics": [{"label": "Character"}]
            }
        });
        let aggregated =
            aggregate_story_repair_quality_summaries(&[first_summary, second_summary], "batch")
                .expect("aggregated batch summary");

        let payload = build_batch_generation_runtime_state_payload_from_parts(
            &BatchGenerationRequestRuntimeState::new(
                crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default(),
                None,
            ),
            Some(&aggregated),
        );
        let runtime_seed =
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(payload.clone());
        let (_, compat) = runtime_seed.into_parts();

        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary"]["recent_focus_areas"],
            json!(["pacing", "conflict", "character"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["压缩说明", "提前冲突", "强化动机"])
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            payload["quality_metrics_history"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 81);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 86);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 86);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary_state"]["first_overall_score"],
            81.0
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["last_overall_score"],
            86.0
        );
        assert_eq!(compat.story_repair_summary(), "先处理节奏拖沓");
        assert_eq!(
            compat.story_repair_targets(),
            &[
                "压缩说明".to_string(),
                "提前冲突".to_string(),
                "强化动机".to_string()
            ]
        );
    }

    #[test]
    fn should_restore_batch_runtime_compat_options_from_seeded_story_repair_payload() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let quality_summary = json!({
            "repair_guidance": {
                "summary": "沿用批量历史修复建议",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["尾章钩子"]
            }
        });

        let payload = build_batch_generation_runtime_state_payload_from_parts(
            &runtime_state,
            Some(&quality_summary),
        );
        let runtime_seed =
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(payload.clone());
        let (_, compat) = runtime_seed.into_parts();

        assert_eq!(compat.story_repair_summary(), "沿用批量历史修复建议");
        assert_eq!(
            compat.story_repair_targets(),
            &["压缩说明".to_string(), "提前冲突".to_string()]
        );
        assert_eq!(compat.story_preserve_strengths(), &["尾章钩子".to_string()]);
    }

    #[test]
    fn should_project_batch_generation_create_chapter_ids_in_order() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-5", 5, "Chapter 5"),
            build_chapter_target("chapter-6", 6, "Chapter 6"),
        ];
        let chapter_ids = chapters_to_generate
            .iter()
            .map(|target| target.id.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            chapter_ids,
            vec!["chapter-5".to_string(), "chapter-6".to_string()]
        );
    }

    #[test]
    fn should_build_batch_generation_task_chapter_id_payload_from_create_parts() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-5", 5, "Chapter 5"),
            build_chapter_target("chapter-6", 6, "Chapter 6"),
        ];
        let chapter_id_payload = Value::Array(
            chapters_to_generate
                .iter()
                .map(|target| target.id.clone())
                .into_iter()
                .map(|chapter_id| json!(chapter_id))
                .collect(),
        );

        assert_eq!(chapter_id_payload, json!(["chapter-5", "chapter-6"]));
    }

    #[test]
    fn should_build_batch_generation_create_response_chapters_to_generate_payload() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-5", 5, "Chapter 5"),
            build_chapter_target("chapter-6", 6, "Chapter 6"),
        ];
        let payload = chapters_to_generate
            .iter()
            .map(|target| {
                json!({
                    "id": target.id,
                    "chapter_number": target.chapter_number,
                    "title": target.title,
                })
            })
            .collect::<Vec<_>>();

        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0]["id"], "chapter-5");
        assert_eq!(payload[0]["chapter_number"], 5);
        assert_eq!(payload[1]["title"], "Chapter 6");
    }
}
