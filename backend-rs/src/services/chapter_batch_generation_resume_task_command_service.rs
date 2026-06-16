pub(crate) mod resume_launch_owner;
pub(crate) mod resume_task_command_owner;

pub(crate) use self::resume_launch_owner::build_batch_generation_resume_launch_owner_contract;
#[cfg(test)]
use self::resume_launch_owner::map_single_chapter_resume_validation_error;
use self::resume_launch_owner::BatchGenerationResumeLaunchPersistencePlan;
#[cfg(test)]
use self::resume_launch_owner::{
    ResumeExecutionDispatchPlan, ResumeExecutionEligibilityPlan, ValidatedResumeExecutionPlan,
};
pub(crate) use self::resume_task_command_owner::{
    build_batch_generation_resume_task_command_owner_contract,
    resume_owned_batch_generation_task_command,
};
#[cfg(test)]
use self::resume_task_command_owner::{
    prepare_batch_generation_resume, prepare_owned_batch_generation_resume,
    resolve_resume_active_story_repair_payload_from_runtime_context,
    restore_resume_compat_options_from_runtime_context, restored_resume_quality_runtime_context,
};
use crate::services::chapter_batch_generation_read_context_service::LoadOwnedBatchGenerationTaskError;
use crate::services::chapter_batch_generation_runtime_state_service::ResolveResumeExecutionSelectionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResumeBatchGenerationDomainError {
    InvalidStatus,
    ManualReviewBlocked,
    NoResumableChaptersFound,
    NoChaptersLeftToResume,
    SingleChapterUnavailable(String),
    ChaptersUnavailable,
    PrerequisitesBlocked(String),
    Internal(String),
}

impl ResumeBatchGenerationDomainError {
    pub(crate) fn detail_message(&self) -> String {
        match self {
            Self::InvalidStatus => "Only failed or cancelled tasks can be resumed".to_string(),
            Self::ManualReviewBlocked => {
                "Manual review blocked tasks cannot be resumed".to_string()
            }
            Self::NoResumableChaptersFound => "No resumable chapters found".to_string(),
            Self::NoChaptersLeftToResume => "No chapters left to resume".to_string(),
            Self::SingleChapterUnavailable(detail) => detail.clone(),
            Self::ChaptersUnavailable => "Some chapters no longer exist".to_string(),
            Self::PrerequisitesBlocked(detail) => {
                format!("Resume blocked by prerequisites: {detail}")
            }
            Self::Internal(detail) => detail.clone(),
        }
    }

    fn from_execution_selection_error(error: ResolveResumeExecutionSelectionError) -> Self {
        match error {
            ResolveResumeExecutionSelectionError::NoResumableChaptersFound => {
                Self::NoResumableChaptersFound
            }
            ResolveResumeExecutionSelectionError::NoChaptersLeftToResume => {
                Self::NoChaptersLeftToResume
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrepareOwnedBatchGenerationResumeError {
    Task(LoadOwnedBatchGenerationTaskError),
    Domain(ResumeBatchGenerationDomainError),
    Config(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResumeBatchGenerationTaskCommandError {
    Task(LoadOwnedBatchGenerationTaskError),
    Domain(ResumeBatchGenerationDomainError),
    Config(String),
}

impl From<PrepareOwnedBatchGenerationResumeError> for ResumeBatchGenerationTaskCommandError {
    fn from(error: PrepareOwnedBatchGenerationResumeError) -> Self {
        match error {
            PrepareOwnedBatchGenerationResumeError::Task(error) => Self::Task(error),
            PrepareOwnedBatchGenerationResumeError::Domain(error) => Self::Domain(error),
            PrepareOwnedBatchGenerationResumeError::Config(error) => Self::Config(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
        IntoActiveModel, Schema, Set,
    };

    use crate::models::batch_generation_snapshot;
    use crate::models::batch_generation_task;
    use crate::models::career;
    use crate::models::chapter;
    use crate::models::character;
    use crate::models::character_career;
    use crate::models::foreshadow;
    use crate::models::project;
    use crate::models::settings;
    use crate::models::story_memory;
    use crate::services::chapter_access_service::LoadAccessibleChapterForGenerationError;
    use crate::services::chapter_batch_generation_read_context_service::LoadOwnedBatchGenerationTaskError;
    use crate::services::chapter_batch_generation_resume_task_command_service::ResumeBatchGenerationTaskCommandError;
    use crate::services::chapter_batch_generation_runtime_state_service::prepare_batch_generation_resume_restored_runtime_state;
    use crate::services::chapter_batch_generation_runtime_state_service::{
        build_batch_generation_execution_input, build_pending_batch_generation_runtime_checkpoint,
        BatchGenerationResumeResetPersistencePlan, RestoredResumeRuntimeStateProjection,
        ResumeBatchGenerationCommandState, ResumeExecutionSelection, ResumeResetSemantics,
    };
    use crate::services::chapter_batch_generation_task_payload_base_service::BatchGenerationTaskKind;
    use crate::services::chapter_batch_generation_task_payload_base_service::{
        build_batch_generation_command_summary_payload, BatchGenerationCommandProgressSummary,
        BatchGenerationQualityStatusContext,
    };
    use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
    use crate::services::chapter_generation_execution_contract_service::normalize_chapter_generation_target_word_count;
    use crate::services::chapter_generation_execution_contract_service::BatchGenerationRequestRuntimeState;
    use crate::services::chapter_generation_execution_contract_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::manual_review_label;
    use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::{
        extract_quality_gate_object, extract_repair_guidance_object,
        restore_story_repair_compat_options_from_active_snapshot,
    };
    use crate::services::chapter_single_generation_prepare_service::{
        PrepareSingleChapterGenerationRequestError, SingleChapterGenerationTarget,
    };
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;
    use serde_json::{json, Value};

    use super::{
        build_batch_generation_resume_task_command_owner_contract,
        map_single_chapter_resume_validation_error,
        resolve_resume_active_story_repair_payload_from_runtime_context,
        restore_resume_compat_options_from_runtime_context,
        restored_resume_quality_runtime_context, BatchGenerationResumeLaunchPersistencePlan,
        ResumeBatchGenerationDomainError, ResumeExecutionDispatchPlan,
        ResumeExecutionEligibilityPlan, ValidatedResumeExecutionPlan,
    };

    fn test_single_generation_gateway_config() -> ChapterCandidateRouteGatewayConfig {
        ChapterCandidateRouteGatewayConfig {
            rust_executor_enabled: true,
            fallback_on_rust_error: false,
            disabled_reason: Some("test resume single-generation explicit gateway".to_string()),
            rollback_boundary: "test_single_generation_resume_gateway".to_string(),
        }
    }

    fn build_default_execution_config(
    ) -> crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig
    {
        crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
            ai_config: crate::ai::AIConfig::default(),
            provider_payload: crate::services::chapter_generation_prompt_service::build_placeholder_prompt_context_provider_payload(),
        }
    }

    fn build_test_resume_launch_persistence_plan(
        command_state: ResumeBatchGenerationCommandState,
        user_id: &str,
        chapter_ids: Vec<String>,
        enable_analysis: bool,
    ) -> BatchGenerationResumeLaunchPersistencePlan {
        let dispatch_plan = ResumeExecutionDispatchPlan::Batch {
            runtime_input: build_batch_generation_execution_input(
                user_id.to_string(),
                chapter_ids,
                command_state.target_word_count,
                SingleChapterGenerationCompatOptions {
                    enable_analysis,
                    ..Default::default()
                },
                build_default_execution_config(),
                test_single_generation_gateway_config(),
            ),
        };
        let reset_persistence_plan =
            BatchGenerationResumeResetPersistencePlan::from_resume_task(&command_state, None);

        BatchGenerationResumeLaunchPersistencePlan::from_contract_for_test(
            command_state,
            dispatch_plan,
            reset_persistence_plan,
            test_single_generation_gateway_config(),
        )
    }

    #[test]
    fn should_publish_batch_generation_resume_task_command_owner_contract() {
        let contract = build_batch_generation_resume_task_command_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_resume_task_command_service::resume_task_command_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/services/batch_generation/resume_service.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["command_entrypoints"][0],
            "prepare_owned_batch_generation_resume"
        );
        assert_eq!(
            contract["behavior_contract"]["command_entrypoints"][1],
            "resume_owned_batch_generation_task_command"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_restore"][0],
            "prepare_batch_generation_resume_restored_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["gateway_config"][1],
            "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter_batch_generation_resume_task_command_service::resume_owned_batch_generation_task_command"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_knob"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
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
            contract["service_runtime_closeout_status"]["task_command_owner"],
            "resume_owned_batch_generation_task_command"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["command_owner"],
            "prepare_owned_batch_generation_resume"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["execution_selection_owner"],
            "ResumeBatchGenerationCommandState::resolve_execution_selection"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["persist_and_dispatch_owner"],
            "BatchGenerationResumeLaunchPersistencePlan::persist_and_dispatch"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["runtime_restore_owner"],
            "prepare_batch_generation_resume_restored_runtime_state"
        );
        assert_eq!(
            contract["resume_launch_owner_contract"]["owner"],
            "chapter_batch_generation_resume_task_command_service::resume_launch_owner"
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
            "rust_batch_generation_resume_task_command_owner_ready_for_source_map_closeout_review"
        );
    }

    #[test]
    fn should_publish_batch_generation_resume_launch_owner_contract() {
        let contract = super::build_batch_generation_resume_launch_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_resume_task_command_service::resume_launch_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["resume_launch_entrypoints"][3],
            "BatchGenerationResumeLaunchPersistencePlan::persist_and_dispatch"
        );
        assert_eq!(
            contract["behavior_contract"]["execution_selection_entrypoints"][2],
            "ValidatedResumeExecutionPlan::from_command_state"
        );
        assert_eq!(
            contract["behavior_contract"]["execution_selection_entrypoints"][4],
            "ResumeExecutionDispatchPlan::dispatch"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_restore_entrypoints"][3],
            "resume_owned_batch_generation_task_command"
        );
        assert_eq!(
            contract["behavior_contract"]["response_projection_entrypoints"][0],
            "BatchGenerationResumeResetPersistencePlan::into_resume_response_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["dispatch_contract"]["batch_dispatch_owner"],
            "dispatch_batch_generation_runtime"
        );
        assert_eq!(
            contract["behavior_contract"]["domain_error_surface"][6],
            "PrerequisitesBlocked"
        );
        assert_eq!(
            contract["active_consumers"][4],
            "chapter_batch_generation_active_gateway_smoke_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_state_keys"][7],
            "candidate_gateway"
        );
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
        latest_quality_metrics: Option<serde_json::Value>,
        quality_metrics_summary: Option<serde_json::Value>,
    ) -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics,
            quality_metrics_history: None,
            quality_metrics_summary,
            workflow_runtime_state: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn build_snapshot_with_runtime_state(
        workflow_runtime_state: serde_json::Value,
    ) -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            workflow_runtime_state: Some(workflow_runtime_state),
            ..build_snapshot(None, None)
        }
    }

    async fn setup_resume_settings_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(settings::Entity)))
            .await
            .expect("create settings table");
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
        db.execute(builder.build(&schema.create_table_from_entity(character::Entity)))
            .await
            .expect("create characters table");
        db.execute(builder.build(&schema.create_table_from_entity(career::Entity)))
            .await
            .expect("create careers table");
        db.execute(builder.build(&schema.create_table_from_entity(character_career::Entity)))
            .await
            .expect("create character careers table");
        db.execute(builder.build(&schema.create_table_from_entity(story_memory::Entity)))
            .await
            .expect("create story memories table");
        db.execute(builder.build(&schema.create_table_from_entity(foreshadow::Entity)))
            .await
            .expect("create foreshadows table");
        db
    }

    async fn seed_resume_settings(db: &DatabaseConnection, user_id: &str) {
        let now = Utc::now().naive_utc();
        let settings = settings::ActiveModel {
            id: Set("settings-1".to_string()),
            user_id: Set(user_id.to_string()),
            api_provider: Set("openai".to_string()),
            api_key: Set("sk-resume-owner".to_string()),
            api_base_url: Set("https://api.example.com/v1".to_string()),
            api_backup_urls: Set(None),
            provider_type: Set("openai".to_string()),
            fallback_strategy: Set("manual".to_string()),
            azure_api_version: Set(None),
            llm_model: Set("stored-model".to_string()),
            temperature: Set(0.6),
            max_tokens: Set(2048),
            system_prompt: Set(Some("resume-owner-prompt".to_string())),
            preferences: Set(Some("{}".to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        };
        settings.insert(db).await.expect("insert settings");
    }

    async fn seed_resume_project_and_chapters(db: &DatabaseConnection, user_id: &str) {
        let now = Utc::now().naive_utc();
        let project = project::ActiveModel {
            id: Set("project-1".to_string()),
            user_id: Set(user_id.to_string()),
            title: Set("Resume Project".to_string()),
            description: Set(None),
            theme: Set(None),
            genre: Set(None),
            target_words: Set(5000),
            current_words: Set(0),
            status: Set("active".to_string()),
            wizard_status: Set("completed".to_string()),
            wizard_step: Set(0),
            outline_mode: Set("simple".to_string()),
            world_time_period: Set(None),
            world_location: Set(None),
            world_atmosphere: Set(None),
            world_rules: Set(None),
            chapter_count: Set(Some(2)),
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
        };
        project.insert(db).await.expect("insert project");

        for (chapter_id, chapter_number, title) in
            [("chapter-1", 1, "第一章"), ("chapter-2", 2, "第二章")]
        {
            let chapter = chapter::ActiveModel {
                id: Set(chapter_id.to_string()),
                project_id: Set("project-1".to_string()),
                chapter_number: Set(chapter_number),
                title: Set(title.to_string()),
                content: Set(None),
                summary: Set(None),
                word_count: Set(0),
                status: Set("draft".to_string()),
                outline_id: Set(None),
                sub_index: Set(1),
                expansion_plan: Set(None),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            };
            chapter.insert(db).await.expect("insert chapter");
        }
    }

    async fn seed_owned_resume_task_and_snapshot(db: &DatabaseConnection, user_id: &str) {
        let now = Utc::now().naive_utc();

        batch_generation_task::ActiveModel {
            id: Set("batch-resume-db-smoke".to_string()),
            project_id: Set("project-1".to_string()),
            user_id: Set(user_id.to_string()),
            start_chapter_number: Set(1),
            chapter_count: Set(2),
            chapter_ids: Set(json!(["chapter-1", "chapter-2"])),
            style_id: Set(None),
            target_word_count: Set(0),
            enable_analysis: Set(true),
            status: Set("failed".to_string()),
            total_chapters: Set(2),
            completed_chapters: Set(1),
            failed_chapters: Set(json!([{
                "chapter_id": "chapter-2",
                "phase": "generation_failed",
                "error": "provider interrupted"
            }])),
            current_chapter_id: Set(Some("chapter-2".to_string())),
            current_chapter_number: Set(Some(2)),
            current_retry_count: Set(1),
            max_retries: Set(3),
            created_at: Set(Some(now)),
            started_at: Set(Some(now)),
            completed_at: Set(Some(now)),
            error_message: Set(Some("provider interrupted".to_string())),
        }
        .insert(db)
        .await
        .expect("insert resume db smoke task");

        batch_generation_snapshot::ActiveModel {
            id: Set("snapshot-resume-db-smoke".to_string()),
            batch_task_id: Set("batch-resume-db-smoke".to_string()),
            latest_quality_metrics: Set(Some(json!({
                "overall_score": 87.0,
                "source": "resume-db-smoke"
            }))),
            quality_metrics_history: Set(Some(json!([{
                "overall_score": 84.0,
                "source": "resume-db-smoke-history"
            }]))),
            quality_metrics_summary: Set(Some(json!({
                "chapter_count": 1,
                "avg_score": 87.0,
                "quality_gate": {
                    "decision": "allow_resume"
                }
            }))),
            workflow_runtime_state: Set(Some(json!({
                "phase": "failed",
                "progress": 50,
                "last_event": "failed",
                "last_message": "DB backed resume smoke failed before resume",
                "batch_request_runtime_state": {
                    "compat_options": {
                        "style_id": null,
                        "enable_analysis": true,
                        "enable_mcp": false,
                        "web_research_enabled": false,
                        "web_research_query": null,
                        "narrative_perspective": null,
                        "creative_mode": null,
                        "story_focus": null,
                        "plot_stage": null,
                        "story_creation_brief": null,
                        "quality_preset": null,
                        "quality_notes": null,
                        "story_repair_summary": "沿用 DB-backed resume 修复摘要",
                        "story_repair_targets": ["节奏修复"],
                        "story_preserve_strengths": []
                    },
                    "model_override": "resume-db-model"
                },
                "active_story_repair_payload": {
                    "scope": "batch",
                    "mode": "resume-db-smoke",
                    "summary": "沿用 DB-backed resume 修复摘要",
                    "repair_targets": ["节奏修复"]
                },
                "candidate_gateway": {
                    "execution_path": "rust_candidate_executor",
                    "fallback_applied": false,
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "rust_error": null
                }
            }))),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .expect("insert resume db smoke snapshot");
    }

    #[test]
    fn should_build_resume_execution_selection_for_single_and_batch_tasks() {
        let mut single = build_task("failed");
        single.chapter_count = 1;
        single.chapter_ids = json!(["chapter-1"]);
        single.current_chapter_id = Some("chapter-1".to_string());

        let single_state = ResumeBatchGenerationCommandState::from_task(&single);
        let single_selection = single_state
            .resolve_execution_selection()
            .expect("single selection should exist");
        assert!(matches!(
            single_selection,
            crate::services::chapter_batch_generation_runtime_state_service::ResumeExecutionSelection::SingleChapter {
                chapter_id,
            } if chapter_id == "chapter-1"
        ));

        let mut batch = build_task("cancelled");
        batch.chapter_count = 2;
        batch.chapter_ids = json!(["chapter-1", {"id": "chapter-2"}]);
        batch.current_chapter_id = None;

        let batch_state = ResumeBatchGenerationCommandState::from_task(&batch);
        let batch_selection = batch_state
            .resolve_execution_selection()
            .expect("batch selection should exist");
        assert!(matches!(
            batch_selection,
            crate::services::chapter_batch_generation_runtime_state_service::ResumeExecutionSelection::Batch {
                chapter_ids,
            } if chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[test]
    fn should_detect_quality_blocked_failed_chapter_as_manual_review_blocker() {
        assert_eq!(
            manual_review_label(Some(&json!([{
                "phase": "quality_blocked"
            }]))),
            Some("需人工复核".to_string())
        );
    }

    #[test]
    fn should_detect_exhausted_auto_repair_quality_context_as_manual_review_blocker() {
        assert_eq!(
            crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::manual_review_label_from_quality_context_with_retry_budget(
                None,
                Some(&json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复预算已耗尽"
                    }
                })),
                None,
                3,
                3,
            ),
            Some("自动修复预算已耗尽".to_string())
        );
    }

    #[test]
    fn should_fail_malformed_single_resume_execution_selection_with_shared_batch_fallback() {
        let mut malformed_single = build_task("failed");
        malformed_single.chapter_count = 1;
        malformed_single.chapter_ids = json!({"chapter_id": "chapter-1"});
        malformed_single.current_chapter_id = Some("chapter-1".to_string());

        let malformed_state = ResumeBatchGenerationCommandState::from_task(&malformed_single);
        let error = malformed_state
            .resolve_execution_selection()
            .map(|_| panic!("malformed single should fallback to batch error"))
            .map_err(ResumeBatchGenerationDomainError::from_execution_selection_error)
            .unwrap_or(ResumeBatchGenerationDomainError::NoResumableChaptersFound);

        assert_eq!(
            error,
            ResumeBatchGenerationDomainError::NoResumableChaptersFound
        );
        assert_eq!(error.detail_message(), "No resumable chapters found");
    }

    #[test]
    fn should_keep_resume_domain_error_detail_messages_stable() {
        assert_eq!(
            ResumeBatchGenerationDomainError::InvalidStatus.detail_message(),
            "Only failed or cancelled tasks can be resumed"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::ManualReviewBlocked.detail_message(),
            "Manual review blocked tasks cannot be resumed"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::NoResumableChaptersFound.detail_message(),
            "No resumable chapters found"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::NoChaptersLeftToResume.detail_message(),
            "No chapters left to resume"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::SingleChapterUnavailable(
                "Chapter not found or access denied".to_string()
            )
            .detail_message(),
            "Chapter not found or access denied"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::ChaptersUnavailable.detail_message(),
            "Some chapters no longer exist"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::Internal("boom".to_string()).detail_message(),
            "boom"
        );
    }

    #[test]
    fn should_keep_resume_batch_generation_task_command_config_error_shape() {
        let error = ResumeBatchGenerationTaskCommandError::Config("model missing".to_string());

        assert!(matches!(
            error,
            ResumeBatchGenerationTaskCommandError::Config(detail) if detail == "model missing"
        ));
    }

    #[test]
    fn should_keep_resume_batch_generation_task_command_task_error_shape() {
        let error = ResumeBatchGenerationTaskCommandError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        );

        assert!(matches!(
            error,
            ResumeBatchGenerationTaskCommandError::Task(
                LoadOwnedBatchGenerationTaskError::TaskNotFound
            )
        ));
    }

    #[test]
    fn should_map_single_chapter_prepare_errors_into_stable_resume_messages() {
        assert_eq!(
            map_single_chapter_resume_validation_error(
                PrepareSingleChapterGenerationRequestError::Chapter(
                    LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
                ),
            )
            .detail_message(),
            "Chapter not found or access denied"
        );
        assert_eq!(
            map_single_chapter_resume_validation_error(
                PrepareSingleChapterGenerationRequestError::PrerequisitesBlocked(
                    "缺少章节大纲".to_string(),
                ),
            )
            .detail_message(),
            "Resume blocked by prerequisites: 缺少章节大纲"
        );
    }

    #[test]
    fn should_detect_manual_review_resume_blocker_from_shared_quality_semantics() {
        assert_eq!(
            manual_review_label(Some(&json!([{
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "needs review"
            }]))),
            Some("needs review".to_string())
        );
        assert_eq!(
            manual_review_label(Some(&json!([{
                "quality_gate_decision": "manual_review"
            }]))),
            Some("需人工复核".to_string())
        );
        assert!(manual_review_label(Some(&json!([{
            "quality_gate_decision": "passed"
        }])))
        .is_none());
    }

    #[tokio::test]
    async fn should_block_resume_when_runtime_active_story_repair_payload_requires_manual_review() {
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let workflow_runtime_state = json!({
            "active_story_repair_payload": {
                "summary": "需要人工处理",
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核"
            }
        });

        let result = super::prepare_batch_generation_resume(
            &sea_orm::DatabaseConnection::Disconnected,
            command_state,
            "user-1",
            Some(&build_snapshot_with_runtime_state(workflow_runtime_state)),
            test_single_generation_gateway_config(),
        )
        .await;

        match result {
            Err(ResumeBatchGenerationDomainError::ManualReviewBlocked) => {}
            other => panic!("expected ManualReviewBlocked, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn should_block_resume_when_quality_summary_requires_manual_review_even_without_failed_chapter_label(
    ) {
        let mut task = build_task("failed");
        task.failed_chapters = json!([]);
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "等待人工处理"
                },
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "质量闸门要求人工复核"
                }
            })),
        );

        let result = super::prepare_batch_generation_resume(
            &sea_orm::DatabaseConnection::Disconnected,
            command_state,
            "user-1",
            Some(&snapshot),
            test_single_generation_gateway_config(),
        )
        .await;

        match result {
            Err(ResumeBatchGenerationDomainError::ManualReviewBlocked) => {}
            other => panic!("expected ManualReviewBlocked, got {:?}", other),
        }
    }

    #[test]
    fn should_prepare_resume_launch_with_restored_request_runtime_state_owner() {
        let mut task = build_task("failed");
        task.project_id = "project-7".to_string();
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let restored_request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("沿用运行态修复建议".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                story_preserve_strengths: vec!["结尾钩子".to_string()],
                ..Default::default()
            },
            Some("model-x".to_string()),
        );
        let reset_persistence_plan = BatchGenerationResumeResetPersistencePlan::from_resume_task(
            &command_state,
            Some(json!({
                "resume_from_batch_id": "task-1",
                "current_retry_count": 0,
                "max_retries": 3,
                "current_chapter_id": "chapter-1",
                "current_chapter_number": 1,
                "phase": "pending",
                "progress": 0,
                "status": "pending",
                "last_event": "resume"
            })),
        );
        let dispatch_plan = ResumeExecutionDispatchPlan::SingleChapter {
            runtime_input: SingleGenerationRuntimeLaunchInput {
                chapter_id: "chapter-1".to_string(),
                user_id: "user-1".to_string(),
                execution_input: SingleChapterGenerationExecutionInput {
                    target_word_count: normalize_chapter_generation_target_word_count(Some(
                        command_state.target_word_count,
                    )),
                    compat_options: restored_request_runtime_state.compat_options.clone(),
                    execution_config: build_default_execution_config(),
                },
            },
        };
        let persistence_plan = BatchGenerationResumeLaunchPersistencePlan::new(
            command_state.clone(),
            dispatch_plan,
            reset_persistence_plan.clone(),
            test_single_generation_gateway_config(),
        );
        assert_eq!(
            persistence_plan
                .single_generation_gateway_config()
                .rollback_boundary,
            "test_single_generation_resume_gateway"
        );

        match persistence_plan.dispatch_plan() {
            ResumeExecutionDispatchPlan::SingleChapter { runtime_input } => {
                assert_eq!(runtime_input.user_id, "user-1");
                assert_eq!(runtime_input.chapter_id, "chapter-1");
                assert_eq!(runtime_input.execution_input.target_word_count, 3000);
                assert_eq!(
                    runtime_input
                        .execution_input
                        .compat_options
                        .story_repair_summary(),
                    "沿用运行态修复建议"
                );
                assert_eq!(
                    runtime_input
                        .execution_input
                        .compat_options
                        .story_repair_targets(),
                    &["压缩说明".to_string()]
                );
                assert_eq!(
                    runtime_input
                        .execution_input
                        .compat_options
                        .story_preserve_strengths(),
                    &["结尾钩子".to_string()]
                );
                assert_eq!(
                    runtime_input
                        .execution_input
                        .execution_config
                        .ai_config
                        .provider,
                    crate::ai::AIConfig::default().provider
                );
            }
            ResumeExecutionDispatchPlan::Batch { .. } => {
                panic!("expected single chapter dispatch plan");
            }
        }
        let response_payload = persistence_plan.response_payload();
        assert_eq!(response_payload["message"], "Task resumed and queued");
        assert_eq!(response_payload["resumed_from_batch_id"], "task-1");
        let reset_checkpoint = reset_persistence_plan.checkpoint();
        assert_eq!(
            response_payload["checkpoint"]["resume_from_batch_id"],
            reset_checkpoint["resume_from_batch_id"]
        );
        assert_eq!(
            response_payload["checkpoint"]["current_retry_count"],
            reset_checkpoint["current_retry_count"]
        );
        assert_eq!(
            response_payload["checkpoint"]["max_retries"],
            reset_checkpoint["max_retries"]
        );
        assert_eq!(
            response_payload["checkpoint"]["current_chapter_id"],
            reset_checkpoint["current_chapter_id"]
        );
        assert_eq!(
            response_payload["checkpoint"]["current_chapter_number"],
            reset_checkpoint["current_chapter_number"]
        );
        assert_eq!(
            response_payload["checkpoint"]["phase"],
            reset_checkpoint["phase"]
        );
        assert_eq!(
            response_payload["checkpoint"]["progress"],
            reset_checkpoint["progress"]
        );
        assert_eq!(
            response_payload["checkpoint"]["status"],
            reset_checkpoint["status"]
        );
        assert_eq!(
            response_payload["checkpoint"]["last_event"],
            reset_checkpoint["last_event"]
        );
        assert_eq!(response_payload["stage_code"], "6.writing.loading");
        assert_eq!(response_payload["execution_mode"], "interactive");
        assert_eq!(
            response_payload["checkpoint"]["stage_code"],
            "6.writing.loading"
        );
        assert_eq!(response_payload["checkpoint"]["progress_phase"], "loading");
    }

    #[test]
    fn should_build_resume_persistence_plan_with_shared_reset_and_dispatch_owner() {
        let mut task = build_task("failed");
        task.project_id = "project-8".to_string();
        task.target_word_count = 0;
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                enable_analysis: true,
                ..Default::default()
            },
            Some("model-8".to_string()),
        );
        let reset_persistence_plan = BatchGenerationResumeResetPersistencePlan::from_resume_task(
            &command_state,
            Some(json!({
                "resume_from_batch_id": "task-1",
                "current_retry_count": 0,
                "max_retries": 3
            })),
        );
        let dispatch_plan = ResumeExecutionDispatchPlan::Batch {
            runtime_input: build_batch_generation_execution_input(
                "user-8".to_string(),
                vec!["chapter-1".to_string(), "chapter-2".to_string()],
                normalize_chapter_generation_target_word_count(Some(
                    command_state.target_word_count,
                )),
                request_runtime_state.compat_options.clone(),
                build_default_execution_config(),
                test_single_generation_gateway_config(),
            ),
        };
        let persistence_plan = BatchGenerationResumeLaunchPersistencePlan::new(
            command_state.clone(),
            dispatch_plan,
            reset_persistence_plan.clone(),
            test_single_generation_gateway_config(),
        );

        match persistence_plan.dispatch_plan() {
            ResumeExecutionDispatchPlan::Batch { runtime_input } => {
                assert_eq!(runtime_input.user_id, "user-8");
                assert_eq!(
                    runtime_input.chapter_ids,
                    vec!["chapter-1".to_string(), "chapter-2".to_string()]
                );
                assert_eq!(runtime_input.target_word_count, 1);
                assert!(runtime_input.compat_options.enable_analysis);
                assert_eq!(
                    runtime_input.ai_config.provider,
                    crate::ai::AIConfig::default().provider
                );
            }
            ResumeExecutionDispatchPlan::SingleChapter { .. } => {
                panic!("expected batch dispatch plan");
            }
        }
        let response_payload = persistence_plan.response_payload();
        assert_eq!(response_payload["resumed_from_batch_id"], "task-1");
        assert_eq!(response_payload["checkpoint"]["status"], "pending");
        let reset_checkpoint = reset_persistence_plan.checkpoint();
        assert_eq!(
            response_payload["checkpoint"]["resume_from_batch_id"],
            reset_checkpoint["resume_from_batch_id"]
        );
        assert_eq!(
            response_payload["checkpoint"]["current_retry_count"],
            reset_checkpoint["current_retry_count"]
        );
        assert_eq!(
            response_payload["checkpoint"]["max_retries"],
            reset_checkpoint["max_retries"]
        );
        assert_eq!(
            response_payload["checkpoint"]["phase"],
            reset_checkpoint["phase"]
        );
        assert_eq!(
            response_payload["checkpoint"]["progress"],
            reset_checkpoint["progress"]
        );
        assert_eq!(
            response_payload["checkpoint"]["status"],
            reset_checkpoint["status"]
        );
        assert_eq!(
            response_payload["checkpoint"]["last_event"],
            reset_checkpoint["last_event"]
        );
        assert_eq!(response_payload["stage_code"], "6.writing.loading");
        assert_eq!(response_payload["execution_mode"], "interactive");
        assert_eq!(
            response_payload["checkpoint"]["stage_code"],
            "6.writing.loading"
        );
        assert_eq!(response_payload["checkpoint"]["progress_phase"], "loading");
    }

    #[tokio::test]
    async fn should_keep_resume_persistence_plan_restored_runtime_state_owner_contract() {
        let db = setup_resume_settings_db().await;
        seed_resume_settings(&db, "user-8").await;

        let mut task = build_task("failed");
        task.project_id = "project-8".to_string();
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let execution_plan = ValidatedResumeExecutionPlan::Batch {
            chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
        };
        let restored_runtime_state = RestoredResumeRuntimeStateProjection {
            quality_status_context: BatchGenerationQualityStatusContext::default(),
            request_runtime_state: BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    story_repair_summary: Some("沿用恢复态摘要".to_string()),
                    ..Default::default()
                },
                Some("owner-model".to_string()),
            ),
            runtime_state_seed: Some(json!({
                "resume_from_batch_id": "task-1",
                "current_retry_count": 0,
                "max_retries": 3,
                "current_chapter_id": "chapter-1",
                "current_chapter_number": 1,
                "phase": "pending",
                "progress": 0,
                "status": "pending",
                "last_event": "resume"
            })),
        };

        let persistence_plan =
            BatchGenerationResumeLaunchPersistencePlan::prepare_from_validated_execution(
                &db,
                command_state,
                execution_plan,
                restored_runtime_state,
                Some(json!({
                    "phase": "failed",
                    "last_event": "error",
                    "quality_metrics_history": [{"overall_score": 88}]
                })),
                "user-8",
                test_single_generation_gateway_config(),
            )
            .await
            .expect("resume persistence plan");

        match persistence_plan.dispatch_plan() {
            ResumeExecutionDispatchPlan::Batch { runtime_input } => {
                assert_eq!(runtime_input.user_id, "user-8");
                assert_eq!(
                    runtime_input.chapter_ids,
                    vec!["chapter-1".to_string(), "chapter-2".to_string()]
                );
                assert_eq!(runtime_input.target_word_count, 3000);
                assert!(runtime_input.compat_options.enable_analysis);
                assert_eq!(
                    runtime_input.compat_options.story_repair_summary(),
                    "沿用恢复态摘要"
                );
                assert_eq!(runtime_input.ai_config.model, "owner-model");
            }
            ResumeExecutionDispatchPlan::SingleChapter { .. } => {
                panic!("expected batch dispatch plan");
            }
        }

        let response_payload = persistence_plan.response_payload();
        assert_eq!(response_payload["resumed_from_batch_id"], "task-1");
        assert_eq!(response_payload["completed_chapters"], 0);
        assert_eq!(response_payload["total_chapters"], 1);
        assert_eq!(
            response_payload["checkpoint"]["resume_from_batch_id"],
            "task-1"
        );
        assert_eq!(response_payload["checkpoint"]["current_retry_count"], 0);
        assert_eq!(response_payload["checkpoint"]["max_retries"], 3);
        assert_eq!(response_payload["checkpoint"]["status"], "pending");
        assert_eq!(response_payload["checkpoint"]["last_event"], "resume");
        assert_eq!(
            persistence_plan
                .reset_persistence_plan()
                .resume_snapshot_plan()
                .runtime_state()["quality_metrics_history"][0]["overall_score"],
            88
        );
    }

    #[test]
    fn should_keep_resume_launch_restored_state_contract() {
        let mut task = build_task("failed");
        task.id = "task-resume-owner-1".to_string();
        task.current_retry_count = 2;
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let snapshot = build_snapshot_with_runtime_state(json!({
            "phase": "failed",
            "last_event": "error",
            "quality_metrics_history": [{"overall_score": 88}],
            "active_story_repair_payload": {
                "summary": "继续沿用"
            }
        }));

        let (restored_runtime_state, existing_workflow_runtime_state) =
            prepare_batch_generation_resume_restored_runtime_state(&command_state, Some(&snapshot))
                .expect("resume launch restored state");

        assert_eq!(command_state.batch_id, "task-resume-owner-1");
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
    fn should_build_pending_runtime_checkpoint_for_queued_batch_task() {
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
        assert!(checkpoint["chapter_id"].is_null());
    }

    #[test]
    fn should_build_resume_payload_from_updated_task_projection() {
        let mut task = build_task("pending");
        task.project_id = "project-9".to_string();
        task.current_chapter_id = Some("chapter-2".to_string());
        task.total_chapters = 3;
        task.completed_chapters = 1;

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let payload = serde_json::json!({
            "batch_id": command_state.batch_id,
            "message": "Task resumed and queued",
            "project_id": command_state.project_id,
            "task_type": "chapter_single_generate",
            "status": "pending",
            "stage_code": "6.writing.loading",
            "execution_mode": "interactive",
            "current_chapter_id": "chapter-2",
            "created_at": null,
            "checkpoint": {
                "stage_code": "6.writing.loading",
                "execution_mode": "interactive",
                "chapter_id": "chapter-2",
                "progress_phase": "loading"
            },
            "completed_chapters": 1,
            "total_chapters": 3
        });
        let summary_payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: "task-1".to_string(),
                total_chapters: 3,
                completed_chapters: 1,
            },
            "Task resumed and queued",
        );
        assert_eq!(summary_payload["total_chapters"], 3);
        assert_eq!(summary_payload["completed_chapters"], 1);
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["project_id"], "project-9");
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-2");
        assert_eq!(payload["checkpoint"]["progress_phase"], "loading");
    }

    #[test]
    fn should_build_resume_payload_from_shared_command_projection_owner() {
        let mut task = build_task("failed");
        task.id = "task-7".to_string();
        task.project_id = "project-7".to_string();
        task.total_chapters = 4;
        task.completed_chapters = 2;
        task.current_chapter_id = Some("chapter-4".to_string());

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let summary_payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: command_state.batch_id,
                total_chapters: command_state.total_chapters,
                completed_chapters: command_state.completed_chapters,
            },
            "Task resumed and queued",
        );
        assert_eq!(summary_payload["total_chapters"], 4);
        assert_eq!(summary_payload["completed_chapters"], 2);
    }

    #[test]
    fn should_build_resume_payload_from_reset_single_task_projection() {
        let mut task = build_task("failed");
        task.project_id = "project-9".to_string();
        task.current_chapter_id = Some("chapter-2".to_string());
        task.total_chapters = 3;
        task.completed_chapters = 2;

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let reset = command_state.resolve_reset_semantics();
        let summary_payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: command_state.batch_id,
                total_chapters: command_state.total_chapters,
                completed_chapters: reset.completed_chapters,
            },
            "Task resumed and queued",
        );
        assert_eq!(summary_payload["total_chapters"], 3);
        assert_eq!(summary_payload["completed_chapters"], 0);
        assert_eq!(reset.status, "pending");
        assert_eq!(reset.current_chapter_id.as_deref(), Some("chapter-2"));
    }

    #[test]
    fn should_build_resume_payload_from_reset_batch_task_projection() {
        let mut task = build_task("cancelled");
        task.chapter_count = 2;
        task.chapter_ids = json!(["chapter-1", "chapter-2"]);
        task.current_chapter_id = Some("chapter-2".to_string());
        task.current_chapter_number = Some(2);
        task.total_chapters = 2;
        task.completed_chapters = 1;

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let reset = command_state.resolve_reset_semantics();
        let summary_payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: command_state.batch_id,
                total_chapters: command_state.total_chapters,
                completed_chapters: reset.completed_chapters,
            },
            "Task resumed and queued",
        );
        assert_eq!(summary_payload["total_chapters"], 2);
        assert_eq!(summary_payload["completed_chapters"], 0);
        assert_eq!(reset.status, "pending");
        assert!(reset.current_chapter_id.is_none());
    }

    #[test]
    fn should_build_resume_response_payload_from_owner() {
        let mut task = build_task("failed");
        task.project_id = "project-9".to_string();
        task.current_chapter_id = Some("chapter-2".to_string());
        task.current_chapter_number = Some(2);
        task.total_chapters = 3;
        task.completed_chapters = 2;
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let checkpoint = crate::services::chapter_batch_generation_runtime_state_service::build_batch_generation_resume_runtime_checkpoint(
            &command_state,
            Some(json!({
                "resume_from_batch_id": "task-1",
                "current_retry_count": 0,
                "max_retries": 3,
                "latest_quality_metrics": {
                    "overall_score": 84,
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    }
                },
                "quality_metrics_history": [
                    {
                        "overall_score": 88,
                        "quality_gate": {
                            "decision": "continue"
                        }
                    },
                    {
                        "overall_score": 84,
                        "quality_gate": {
                            "decision": "auto_repair"
                        }
                    }
                ],
                "quality_metrics_summary_state": {
                    "scope": "chapter",
                    "chapter_count": 2,
                    "first_overall_score": 88.0,
                    "last_overall_score": 84.0
                },
                "quality_metrics_summary": {
                    "overall_score": 84.0,
                    "repair_guidance": {
                        "summary": "压缩说明段落"
                    },
                    "quality_runtime_context": {
                        "recent_metrics": [
                            {
                                "overall_score": 84,
                                "quality_gate": {
                                    "decision": "auto_repair"
                                }
                            }
                        ],
                        "history_scope": "chapter"
                    }
                },
                "quality_history_context": {
                    "recent_metrics": [
                        {
                            "overall_score": 84
                        }
                    ],
                    "history_scope": "chapter",
                    "source": "resume_checkpoint"
                },
                "active_story_repair_payload": {
                    "summary": "沿用上一轮修复建议",
                    "repair_targets": ["压缩说明"],
                    "preserve_strengths": ["结尾钩子"],
                    "source": "recent_history_summary",
                    "scope": "chapter"
                }
            })),
        );
        let reset_persistence_plan =
            BatchGenerationResumeResetPersistencePlan::from_contract_for_test(
                command_state.batch_id.clone(),
                command_state.total_chapters,
                command_state.resolve_reset_semantics(),
                checkpoint,
            );
        let payload = reset_persistence_plan
            .clone()
            .into_resume_response_payload(&command_state);

        assert_eq!(payload["message"], "Task resumed and queued");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["project_id"], "project-9");
        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["stage_code"], "6.writing.loading");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.loading");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-2");
        assert_eq!(payload["checkpoint"]["resume_from_batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["current_retry_count"], 0);
        assert_eq!(payload["checkpoint"]["max_retries"], 3);
        assert_eq!(payload["checkpoint"]["current_chapter_id"], "chapter-2");
        assert_eq!(payload["checkpoint"]["current_chapter_number"], 2);
        assert_eq!(payload["checkpoint"]["progress_phase"], "loading");
        assert_eq!(payload["checkpoint"]["phase"], "pending");
        assert_eq!(payload["checkpoint"]["progress"], 0);
        assert_eq!(payload["checkpoint"]["status"], "pending");
        assert_eq!(payload["checkpoint"]["last_event"], "resume");
        assert_eq!(payload["completed_chapters"], 0);
        assert_eq!(payload["total_chapters"], 3);
        assert_eq!(payload["resumed_from_batch_id"], "task-1");
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(
            payload["latest_quality_metrics"]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(
            payload["quality_metrics_history"][1]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 84.0);
        assert_eq!(
            payload["quality_metrics_summary"]["repair_guidance"]["summary"],
            "压缩说明段落"
        );
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_history_context"]["source"],
            "resume_checkpoint"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "沿用上一轮修复建议"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["压缩说明"])
        );
    }

    #[test]
    fn should_keep_resume_response_payload_reset_persistence_owner_contract() {
        let mut task = build_task("failed");
        task.id = "task-owner".to_string();
        task.project_id = "project-owner".to_string();
        task.current_chapter_id = Some("chapter-command".to_string());
        task.current_chapter_number = Some(2);
        task.total_chapters = 3;
        task.completed_chapters = 2;
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let reset_persistence_plan =
            BatchGenerationResumeResetPersistencePlan::from_contract_for_test(
                command_state.batch_id.clone(),
                7,
                ResumeResetSemantics {
                    status: "pending",
                    current_chapter_id: Some("chapter-owner".to_string()),
                    current_chapter_number: Some(9),
                    include_progress_totals: false,
                    completed_chapters: 0,
                    failed_chapters: json!([]),
                    current_retry_count: 0,
                },
                json!({
                    "resume_from_batch_id": "task-owner",
                    "current_retry_count": 0,
                    "max_retries": 3,
                    "current_chapter_id": "chapter-owner",
                    "current_chapter_number": 9,
                    "phase": "pending",
                    "progress": 0,
                    "status": "pending",
                    "last_event": "resume"
                }),
            );

        let payload = reset_persistence_plan
            .clone()
            .into_resume_response_payload(&command_state);

        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["completed_chapters"], 0);
        assert_eq!(payload["total_chapters"], 7);
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-owner");
        assert_eq!(payload["checkpoint"]["current_chapter_id"], "chapter-owner");
        assert_eq!(payload["checkpoint"]["current_chapter_number"], 9);
        assert_eq!(payload["checkpoint"]["status"], "pending");
    }

    #[test]
    fn should_keep_resume_execution_selection_contract_for_dispatch_owner() {
        let single_execution = ResumeExecutionSelection::SingleChapter {
            chapter_id: "chapter-1".to_string(),
        };
        let batch_execution = ResumeExecutionSelection::Batch {
            chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
        };

        assert!(matches!(
            single_execution,
            ResumeExecutionSelection::SingleChapter {
                chapter_id,
            } if chapter_id == "chapter-1"
        ));
        assert!(matches!(
            batch_execution,
            ResumeExecutionSelection::Batch {
                chapter_ids,
            } if chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[test]
    fn should_keep_resume_dispatch_owner_contract_explicit() {
        let dispatch_owner = ResumeExecutionDispatchPlan::dispatch;
        let dispatch_plan = ResumeExecutionDispatchPlan::Batch {
            runtime_input: build_batch_generation_execution_input(
                "user-1".to_string(),
                vec!["chapter-1".to_string(), "chapter-2".to_string()],
                3200,
                SingleChapterGenerationCompatOptions::default(),
                crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: crate::ai::AIConfig::default(),
                    provider_payload: crate::services::chapter_generation_prompt_service::build_placeholder_prompt_context_provider_payload(),
                },
                test_single_generation_gateway_config(),
            ),
        };

        let _ = dispatch_owner;
        assert!(matches!(
            dispatch_plan,
            ResumeExecutionDispatchPlan::Batch {
                runtime_input,
            } if runtime_input.chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[test]
    fn should_build_single_chapter_resume_dispatch_plan() {
        let dispatch_plan = ResumeExecutionDispatchPlan::SingleChapter {
            runtime_input: SingleGenerationRuntimeLaunchInput {
                chapter_id: "chapter-9".to_string(),
                user_id: "user-9".to_string(),
                execution_input: SingleChapterGenerationExecutionInput {
                    target_word_count: 2800,
                    compat_options: SingleChapterGenerationCompatOptions {
                        enable_analysis: true,
                        ..Default::default()
                    },
                    execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                        ai_config: crate::ai::AIConfig::default(),
                        provider_payload: crate::services::chapter_generation_prompt_service::build_placeholder_prompt_context_provider_payload(),
                    },
                },
            },
        };

        assert!(matches!(
            dispatch_plan,
            ResumeExecutionDispatchPlan::SingleChapter { runtime_input }
                if runtime_input.chapter_id == "chapter-9"
                    && runtime_input.user_id == "user-9"
                    && runtime_input.execution_input.target_word_count == 2800
                    && runtime_input.execution_input.compat_options.enable_analysis
        ));
    }

    #[test]
    fn should_build_resume_execution_eligibility_plan_for_single_and_batch_selection() {
        let single_plan = ResumeExecutionEligibilityPlan::from_execution_selection(
            ResumeExecutionSelection::SingleChapter {
                chapter_id: "chapter-1".to_string(),
            },
        )
        .expect("single eligibility plan");
        let batch_plan = ResumeExecutionEligibilityPlan::from_execution_selection(
            ResumeExecutionSelection::Batch {
                chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
            },
        )
        .expect("batch eligibility plan");

        assert!(matches!(
            single_plan,
            ResumeExecutionEligibilityPlan::SingleChapter { chapter_id }
                if chapter_id == "chapter-1"
        ));
        assert!(matches!(
            batch_plan,
            ResumeExecutionEligibilityPlan::Batch { chapter_ids }
                if chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[test]
    fn should_build_resume_execution_eligibility_plan_from_command_state_owner() {
        let mut single = build_task("failed");
        single.chapter_count = 1;
        single.chapter_ids = json!(["chapter-1"]);
        single.current_chapter_id = Some("chapter-1".to_string());

        let mut batch = build_task("cancelled");
        batch.chapter_count = 2;
        batch.chapter_ids = json!(["chapter-1", {"id": "chapter-2"}]);
        batch.current_chapter_id = None;

        let single_plan = ResumeExecutionEligibilityPlan::from_command_state(
            &ResumeBatchGenerationCommandState::from_task(&single),
        )
        .expect("single eligibility plan from command state");
        let batch_plan = ResumeExecutionEligibilityPlan::from_command_state(
            &ResumeBatchGenerationCommandState::from_task(&batch),
        )
        .expect("batch eligibility plan from command state");

        assert!(matches!(
            single_plan,
            ResumeExecutionEligibilityPlan::SingleChapter { chapter_id }
                if chapter_id == "chapter-1"
        ));
        assert!(matches!(
            batch_plan,
            ResumeExecutionEligibilityPlan::Batch { chapter_ids }
                if chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[test]
    fn should_keep_validated_resume_execution_plan_contract_explicit() {
        let validated_single_plan = ValidatedResumeExecutionPlan::SingleChapter {
            validated_single_chapter_target: SingleChapterGenerationTarget {
                project_id: "project-1".to_string(),
                chapter_id: "chapter-1".to_string(),
                chapter_number: 1,
                title: "第一章".to_string(),
            },
        };
        let validated_batch_plan = ValidatedResumeExecutionPlan::Batch {
            chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
        };

        assert!(matches!(
            validated_single_plan,
            ValidatedResumeExecutionPlan::SingleChapter {
                validated_single_chapter_target,
            } if validated_single_chapter_target.chapter_id == "chapter-1"
                && validated_single_chapter_target.chapter_number == 1
        ));
        assert!(matches!(
            validated_batch_plan,
            ValidatedResumeExecutionPlan::Batch { chapter_ids }
                if chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[test]
    fn should_fail_resume_execution_eligibility_plan_when_batch_selection_empty() {
        let error = ResumeExecutionEligibilityPlan::from_execution_selection(
            ResumeExecutionSelection::Batch {
                chapter_ids: Vec::new(),
            },
        )
        .expect_err("empty batch selection should fail");

        assert_eq!(
            error,
            ResumeBatchGenerationDomainError::NoResumableChaptersFound
        );
    }

    #[tokio::test]
    async fn should_build_validated_resume_execution_plan_from_command_state_owner() {
        let db = setup_resume_settings_db().await;
        seed_resume_settings(&db, "user-8").await;
        seed_resume_project_and_chapters(&db, "user-8").await;

        let mut task = build_task("failed");
        task.chapter_count = 2;
        task.chapter_ids = json!(["chapter-1", {"id": "chapter-2"}]);
        task.current_chapter_id = None;

        let execution_plan = ValidatedResumeExecutionPlan::from_command_state(
            &db,
            "user-8",
            &ResumeBatchGenerationCommandState::from_task(&task),
        )
        .await
        .expect("validated execution plan");

        assert!(matches!(
            execution_plan,
            ValidatedResumeExecutionPlan::Batch { chapter_ids }
                if chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[tokio::test]
    async fn should_prepare_db_backed_owned_resume_launch_from_rust_owner() {
        let db = setup_resume_settings_db().await;
        seed_resume_settings(&db, "user-resume-db-smoke").await;
        seed_resume_project_and_chapters(&db, "user-resume-db-smoke").await;
        let mut prerequisite_chapter = chapter::Entity::find_by_id("chapter-1")
            .one(&db)
            .await
            .expect("load prerequisite chapter")
            .expect("prerequisite chapter exists")
            .into_active_model();
        prerequisite_chapter.status = Set("completed".to_string());
        prerequisite_chapter.content = Set(Some("前置章节已完成正文".to_string()));
        prerequisite_chapter.summary = Set(Some("前置章节概要".to_string()));
        prerequisite_chapter.word_count = Set(1000);
        prerequisite_chapter
            .update(&db)
            .await
            .expect("mark prerequisite chapter completed");
        seed_owned_resume_task_and_snapshot(&db, "user-resume-db-smoke").await;

        let plan = super::prepare_owned_batch_generation_resume(
            &db,
            "batch-resume-db-smoke",
            "user-resume-db-smoke",
            test_single_generation_gateway_config(),
        )
        .await
        .expect("db-backed owned resume plan");
        let response_payload = plan.response_payload();

        let ResumeExecutionDispatchPlan::Batch { runtime_input } = plan.dispatch_plan() else {
            panic!("db-backed resume should dispatch through batch runtime owner");
        };
        assert_eq!(runtime_input.user_id, "user-resume-db-smoke");
        assert_eq!(runtime_input.chapter_ids, vec!["chapter-2".to_string()]);
        assert_eq!(runtime_input.target_word_count, 1);
        assert!(runtime_input.compat_options.enable_analysis);
        assert_eq!(
            runtime_input.compat_options.story_repair_summary(),
            "沿用 DB-backed resume 修复摘要"
        );
        assert_eq!(runtime_input.ai_config.model, "resume-db-model");
        assert_eq!(
            runtime_input.candidate_gateway_config.rollback_boundary,
            "test_single_generation_resume_gateway"
        );
        assert_eq!(
            plan.single_generation_gateway_config().rollback_boundary,
            "test_single_generation_resume_gateway"
        );
        assert_eq!(response_payload["batch_id"], "batch-resume-db-smoke");
        assert_eq!(response_payload["status"], "pending");
        assert_eq!(response_payload["message"], "Task resumed and queued");
        assert_eq!(
            response_payload["checkpoint"]["resume_from_batch_id"],
            "batch-resume-db-smoke"
        );
        assert_eq!(response_payload["checkpoint"]["last_event"], "resume");
        assert_eq!(
            response_payload["checkpoint"]["active_story_repair_payload"]["mode"],
            "resume-db-smoke"
        );
        assert_eq!(
            response_payload["active_story_repair_payload"]["summary"],
            "沿用 DB-backed resume 修复摘要"
        );
        assert_eq!(
            response_payload["latest_quality_metrics"]["source"],
            "resume-db-smoke"
        );
        assert_eq!(
            response_payload["quality_metrics_summary"]["quality_gate"]["decision"],
            "allow_resume"
        );
    }

    #[tokio::test]
    async fn should_build_resume_dispatch_plan_from_validated_execution_and_restored_runtime_owner()
    {
        let db = setup_resume_settings_db().await;
        seed_resume_settings(&db, "user-8").await;
        seed_resume_project_and_chapters(&db, "user-8").await;

        let mut task = build_task("failed");
        task.project_id = "project-1".to_string();
        task.chapter_count = 2;
        task.chapter_ids = json!(["chapter-1", {"id": "chapter-2"}]);
        task.current_chapter_id = None;
        task.target_word_count = 0;

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let execution_plan =
            ValidatedResumeExecutionPlan::from_command_state(&db, "user-8", &command_state)
                .await
                .expect("validated execution plan");
        let restored_runtime_state = RestoredResumeRuntimeStateProjection {
            quality_status_context: BatchGenerationQualityStatusContext::default(),
            request_runtime_state: BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    story_repair_summary: Some("沿用恢复态摘要".to_string()),
                    ..Default::default()
                },
                Some("owner-model".to_string()),
            ),
            runtime_state_seed: None,
        };

        let (dispatch_plan, runtime_state_seed) =
            ResumeExecutionDispatchPlan::from_validated_execution(
                &db,
                "user-8",
                execution_plan,
                restored_runtime_state,
                normalize_chapter_generation_target_word_count(Some(
                    command_state.target_word_count,
                )),
                test_single_generation_gateway_config(),
            )
            .await
            .expect("dispatch plan from validated execution owner");

        assert!(runtime_state_seed.is_none());

        assert!(matches!(
            dispatch_plan,
            ResumeExecutionDispatchPlan::Batch { runtime_input }
                if runtime_input.user_id == "user-8"
                    && runtime_input.chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
                    && runtime_input.target_word_count == 1
                    && runtime_input.compat_options.enable_analysis
                    && runtime_input.ai_config.model == "owner-model"
        ));
    }

    #[tokio::test]
    async fn should_build_single_chapter_resume_dispatch_plan_from_validated_execution_and_restored_runtime_owner(
    ) {
        let db = setup_resume_settings_db().await;
        seed_resume_settings(&db, "user-8").await;
        seed_resume_project_and_chapters(&db, "user-8").await;

        let mut task = build_task("failed");
        task.project_id = "project-1".to_string();
        task.chapter_count = 1;
        task.chapter_ids = json!(["chapter-1"]);
        task.current_chapter_id = Some("chapter-1".to_string());
        task.target_word_count = 2800;

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let execution_plan =
            ValidatedResumeExecutionPlan::from_command_state(&db, "user-8", &command_state)
                .await
                .expect("validated single execution plan");
        let restored_runtime_state = RestoredResumeRuntimeStateProjection {
            quality_status_context: BatchGenerationQualityStatusContext::default(),
            request_runtime_state: BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    story_repair_summary: Some("沿用恢复态摘要".to_string()),
                    story_repair_targets: vec!["压缩说明".to_string()],
                    ..Default::default()
                },
                Some("owner-model".to_string()),
            ),
            runtime_state_seed: None,
        };

        let (dispatch_plan, runtime_state_seed) =
            ResumeExecutionDispatchPlan::from_validated_execution(
                &db,
                "user-8",
                execution_plan,
                restored_runtime_state,
                normalize_chapter_generation_target_word_count(Some(
                    command_state.target_word_count,
                )),
                test_single_generation_gateway_config(),
            )
            .await
            .expect("single chapter dispatch plan from validated execution owner");

        assert!(runtime_state_seed.is_none());

        assert!(matches!(
            dispatch_plan,
            ResumeExecutionDispatchPlan::SingleChapter { runtime_input }
                if runtime_input.user_id == "user-8"
                    && runtime_input.chapter_id == "chapter-1"
                    && runtime_input.execution_input.target_word_count == 2800
                    && runtime_input.execution_input.compat_options.enable_analysis
                    && runtime_input.execution_input.compat_options.story_repair_summary() == "沿用恢复态摘要"
                    && runtime_input.execution_input.compat_options.story_repair_targets() == ["压缩说明".to_string()]
                    && runtime_input.execution_input.execution_config.ai_config.model == "owner-model"
        ));
    }

    #[tokio::test]
    async fn should_fail_validated_resume_execution_plan_from_command_state_with_stable_domain_error(
    ) {
        let db = setup_resume_settings_db().await;
        let mut malformed_single = build_task("failed");
        malformed_single.chapter_count = 1;
        malformed_single.chapter_ids = json!({"chapter_id": "chapter-1"});
        malformed_single.current_chapter_id = Some("chapter-1".to_string());

        let error = ValidatedResumeExecutionPlan::from_command_state(
            &db,
            "user-8",
            &ResumeBatchGenerationCommandState::from_task(&malformed_single),
        )
        .await
        .expect_err("malformed single should fail through validated owner entrypoint");

        assert_eq!(
            error,
            ResumeBatchGenerationDomainError::NoResumableChaptersFound
        );
    }

    #[test]
    fn should_fail_resume_execution_eligibility_plan_from_command_state_with_stable_domain_error() {
        let mut malformed_single = build_task("failed");
        malformed_single.chapter_count = 1;
        malformed_single.chapter_ids = json!({"chapter_id": "chapter-1"});
        malformed_single.current_chapter_id = Some("chapter-1".to_string());

        let error = ResumeExecutionEligibilityPlan::from_command_state(
            &ResumeBatchGenerationCommandState::from_task(&malformed_single),
        )
        .expect_err("malformed single should fail through shared command-state owner");

        assert_eq!(
            error,
            ResumeBatchGenerationDomainError::NoResumableChaptersFound
        );
    }

    #[test]
    fn should_build_resume_reset_persistence_plan_from_shared_checkpoint_owner() {
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let persistence_plan =
            crate::services::chapter_batch_generation_runtime_state_service::BatchGenerationResumeResetPersistencePlan::from_resume_task(
                &command_state,
                Some(json!({
                    "resume_from_batch_id": "task-1",
                    "current_retry_count": 0,
                    "max_retries": 3
                })),
            );

        assert_eq!(persistence_plan.checkpoint()["phase"], "pending");
        assert_eq!(persistence_plan.checkpoint()["status"], "pending");
        assert_eq!(persistence_plan.checkpoint()["last_event"], "resume");
        assert_eq!(
            persistence_plan.checkpoint()["resume_from_batch_id"],
            "task-1"
        );
        assert_eq!(persistence_plan.checkpoint()["current_retry_count"], 0);
        assert_eq!(persistence_plan.checkpoint()["max_retries"], 3);
    }

    #[test]
    fn should_project_restored_resume_runtime_state_into_launch_parts_owner() {
        let runtime_state = json!({
            "quality_metrics_history": [
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
                },
                {
                    "overall_score": 84,
                    "repair_guidance": {
                        "summary": "当前章需要压缩说明",
                        "repair_targets": ["压缩说明", "提前冲突"],
                        "preserve_strengths": ["角色张力"]
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ]
        });
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            Some("model-2".to_string()),
        );
        let restored_runtime_state = RestoredResumeRuntimeStateProjection::from_sources(
            BatchGenerationTaskKind::SingleChapter,
            "task-2",
            5,
            Some(&runtime_state),
            None,
            &request_runtime_state,
        );
        let launch_parts = restored_runtime_state.into_launch_parts();
        let restored_request_runtime_state = launch_parts.request_runtime_state;
        let seed = launch_parts
            .runtime_state_seed
            .expect("resume runtime state seed");

        assert_eq!(
            restored_request_runtime_state
                .compat_options
                .story_repair_summary(),
            "当前章需要压缩说明"
        );
        assert_eq!(
            restored_request_runtime_state
                .compat_options
                .story_repair_targets(),
            &["压缩说明".to_string(), "提前冲突".to_string()]
        );
        assert_eq!(
            restored_request_runtime_state.model_override.as_deref(),
            Some("model-2")
        );
        assert_eq!(seed["resume_from_batch_id"], "task-2");
        assert_eq!(seed["max_retries"], 5);
        assert_eq!(
            seed["active_story_repair_payload"]["source"],
            "recent_history_summary"
        );
        assert_eq!(seed["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(seed["quality_history_context"]["scope"], "chapter");
    }

    #[tokio::test]
    async fn should_prepare_batch_resume_runtime_launch_from_restored_state_owner() {
        let db = setup_resume_settings_db().await;
        seed_resume_settings(&db, "user-8").await;

        let restored_runtime_state = RestoredResumeRuntimeStateProjection {
            quality_status_context: BatchGenerationQualityStatusContext::default(),
            request_runtime_state: BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    story_repair_summary: Some("沿用恢复态摘要".to_string()),
                    story_repair_targets: vec!["压缩说明".to_string()],
                    ..Default::default()
                },
                Some("owner-model".to_string()),
            ),
            runtime_state_seed: Some(json!({
                "resume_from_batch_id": "task-restore",
                "current_retry_count": 0,
                "max_retries": 3
            })),
        };

        let prepared_launch = restored_runtime_state
            .prepare_batch_runtime_launch(
                &db,
                "user-8",
                vec!["chapter-1".to_string(), "chapter-2".to_string()],
                3200,
                test_single_generation_gateway_config(),
            )
            .await
            .expect("prepared batch resume runtime launch");

        assert_eq!(prepared_launch.runtime_input.user_id, "user-8");
        assert_eq!(
            prepared_launch.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(prepared_launch.runtime_input.target_word_count, 3200);
        assert!(prepared_launch.runtime_input.compat_options.enable_analysis);
        assert_eq!(
            prepared_launch
                .runtime_input
                .compat_options
                .story_repair_summary(),
            "沿用恢复态摘要"
        );
        assert_eq!(
            prepared_launch
                .runtime_input
                .compat_options
                .story_repair_targets(),
            &["压缩说明".to_string()]
        );
        assert_eq!(prepared_launch.runtime_input.ai_config.model, "owner-model");
        assert_eq!(
            prepared_launch
                .runtime_state_seed
                .as_ref()
                .expect("runtime seed")["resume_from_batch_id"],
            "task-restore"
        );
    }

    #[tokio::test]
    async fn should_prepare_single_resume_runtime_launch_from_restored_state_owner() {
        let db = setup_resume_settings_db().await;
        seed_resume_settings(&db, "user-10").await;

        let restored_runtime_state = RestoredResumeRuntimeStateProjection {
            quality_status_context: BatchGenerationQualityStatusContext::default(),
            request_runtime_state: BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions {
                    story_repair_summary: Some("沿用单章恢复态摘要".to_string()),
                    story_repair_targets: vec!["压缩说明".to_string()],
                    ..Default::default()
                },
                Some("owner-model-single".to_string()),
            ),
            runtime_state_seed: Some(json!({
                "resume_from_batch_id": "task-single-restore",
                "current_retry_count": 0,
                "max_retries": 3
            })),
        };

        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-10".to_string(),
            chapter_id: "chapter-10".to_string(),
            chapter_number: 10,
            title: "第十章".to_string(),
        };

        let prepared_launch = restored_runtime_state
            .prepare_single_chapter_runtime_launch(&db, "user-10", &chapter_target, 2800)
            .await
            .expect("prepared single resume runtime launch");

        assert_eq!(prepared_launch.runtime_input.user_id, "user-10");
        assert_eq!(prepared_launch.runtime_input.chapter_id, "chapter-10");
        assert_eq!(
            prepared_launch
                .runtime_input
                .execution_input
                .target_word_count,
            2800
        );
        assert_eq!(
            prepared_launch
                .runtime_input
                .execution_input
                .compat_options
                .story_repair_summary(),
            "沿用单章恢复态摘要"
        );
        assert_eq!(
            prepared_launch
                .runtime_input
                .execution_input
                .compat_options
                .story_repair_targets(),
            &["压缩说明".to_string()]
        );
        assert_eq!(
            prepared_launch
                .runtime_input
                .execution_input
                .execution_config
                .ai_config
                .model,
            "owner-model-single"
        );
        assert_eq!(
            prepared_launch
                .runtime_state_seed
                .as_ref()
                .expect("runtime seed")["resume_from_batch_id"],
            "task-single-restore"
        );
    }

    #[test]
    fn should_restore_resume_runtime_state_from_shared_persisted_runtime_context_owner() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            Some("gpt-4.1".to_string()),
        );
        let restored_runtime_state = RestoredResumeRuntimeStateProjection::from_sources(
            BatchGenerationTaskKind::SingleChapter,
            "task-9",
            4,
            Some(&json!({
                "batch_request_runtime_state": BatchGenerationRequestRuntimeState::new(
                    SingleChapterGenerationCompatOptions::default(),
                    Some("gpt-4o-mini".to_string())
                ),
                "quality_metrics_summary_state": {
                    "scope": "chapter",
                    "chapter_count": 2
                },
                "quality_metrics_summary": {
                    "overall_score": 77,
                    "repair_guidance": {
                        "summary": "来自运行态"
                    }
                },
                "latest_quality_metrics": {
                    "overall_score": 76
                }
            })),
            Some(&batch_generation_snapshot::Model {
                latest_quality_metrics: Some(json!({
                    "overall_score": 91,
                    "quality_gate": {
                        "decision": "auto_repair"
                    }
                })),
                quality_metrics_history: Some(json!([
                    {"overall_score": 88},
                    {"overall_score": 91}
                ])),
                quality_metrics_summary: Some(json!({
                    "overall_score": 91,
                    "repair_guidance": {
                        "summary": "来自快照摘要"
                    },
                    "quality_runtime_context": {
                        "scope": "chapter",
                        "recent_metrics": [{"overall_score": 91}]
                    }
                })),
                ..build_snapshot(None, None)
            }),
            &request_runtime_state,
        );
        let seed = restored_runtime_state
            .runtime_state_seed
            .expect("resume seed from shared persisted owner");

        assert_eq!(
            restored_runtime_state
                .request_runtime_state
                .model_override
                .as_deref(),
            Some("gpt-4o-mini")
        );
        assert_eq!(seed["latest_quality_metrics"]["overall_score"], 91);
        assert_eq!(seed["quality_metrics_history"][1]["overall_score"], 91);
        assert_eq!(seed["quality_metrics_summary"]["overall_score"], 91);
        assert_eq!(
            seed["quality_metrics_summary"]["repair_guidance"]["summary"],
            "来自快照摘要"
        );
        assert_eq!(seed["quality_history_context"]["scope"], "chapter");
    }

    #[test]
    fn should_keep_resume_execution_and_payload_contract_explicit() {
        let dispatch_plan = ResumeExecutionDispatchPlan::SingleChapter {
            runtime_input: SingleGenerationRuntimeLaunchInput {
                chapter_id: "chapter-9".to_string(),
                user_id: "user-9".to_string(),
                execution_input: SingleChapterGenerationExecutionInput {
                    target_word_count: 2800,
                    compat_options: SingleChapterGenerationCompatOptions {
                        story_repair_summary: Some("补强冲突".to_string()),
                        ..Default::default()
                    },
                    execution_config: build_default_execution_config(),
                },
            },
        };
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let reset_persistence_plan =
            BatchGenerationResumeResetPersistencePlan::from_resume_task(&command_state, None);
        let persistence_plan = BatchGenerationResumeLaunchPersistencePlan::new(
            command_state,
            dispatch_plan,
            reset_persistence_plan,
            test_single_generation_gateway_config(),
        );

        match persistence_plan.dispatch_plan() {
            ResumeExecutionDispatchPlan::SingleChapter { runtime_input } => {
                assert_eq!(runtime_input.user_id, "user-9");
                assert_eq!(runtime_input.chapter_id, "chapter-9");
                assert_eq!(runtime_input.execution_input.target_word_count, 2800);
                assert_eq!(
                    runtime_input
                        .execution_input
                        .compat_options
                        .story_repair_summary(),
                    "补强冲突"
                );
                assert_eq!(
                    runtime_input
                        .execution_input
                        .execution_config
                        .ai_config
                        .provider,
                    crate::ai::AIConfig::default().provider
                );
            }
            ResumeExecutionDispatchPlan::Batch { .. } => {
                panic!("expected single chapter dispatch plan");
            }
        }
    }

    #[test]
    fn should_build_dispatch_plan_from_resume_persistence_plan_owner() {
        let dispatch_plan = ResumeExecutionDispatchPlan::Batch {
            runtime_input: build_batch_generation_execution_input(
                "user-7".to_string(),
                vec!["chapter-1".to_string(), "chapter-2".to_string()],
                3100,
                SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    ..Default::default()
                },
                build_default_execution_config(),
                test_single_generation_gateway_config(),
            ),
        };
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let reset_persistence_plan =
            BatchGenerationResumeResetPersistencePlan::from_resume_task(&command_state, None);
        let persistence_plan = BatchGenerationResumeLaunchPersistencePlan::new(
            command_state,
            dispatch_plan,
            reset_persistence_plan,
            test_single_generation_gateway_config(),
        );
        let dispatch_plan = persistence_plan.dispatch_plan().clone();

        assert!(matches!(
            dispatch_plan,
            ResumeExecutionDispatchPlan::Batch { runtime_input }
                if runtime_input.user_id == "user-7"
                    && runtime_input.chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
                    && runtime_input.target_word_count == 3100
                    && runtime_input.compat_options.enable_analysis
        ));
    }

    #[test]
    fn should_keep_batch_generation_resume_task_command_dispatch_plan_contract() {
        let mut task = build_task("failed");
        task.user_id = "user-7".to_string();
        task.target_word_count = 3100;
        task.enable_analysis = true;
        task.chapter_ids = json!(["chapter-1", "chapter-2"]);

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let persistence_plan = build_test_resume_launch_persistence_plan(
            command_state.clone(),
            "user-7",
            vec!["chapter-1".to_string(), "chapter-2".to_string()],
            true,
        );

        match persistence_plan.dispatch_plan() {
            ResumeExecutionDispatchPlan::Batch { runtime_input } => {
                assert_eq!(runtime_input.user_id, "user-7");
                assert_eq!(
                    runtime_input.chapter_ids,
                    vec!["chapter-1".to_string(), "chapter-2".to_string()]
                );
                assert_eq!(runtime_input.target_word_count, 3100);
                assert!(runtime_input.compat_options.enable_analysis);
            }
            ResumeExecutionDispatchPlan::SingleChapter { .. } => {
                panic!("expected batch dispatch plan");
            }
        }

        assert_eq!(
            persistence_plan.response_payload()["batch_id"],
            command_state.batch_id
        );
    }

    #[test]
    fn should_keep_batch_generation_resume_task_command_response_payload_owner_contract() {
        let mut task = build_task("cancelled");
        task.chapter_count = 2;
        task.total_chapters = 2;
        task.target_word_count = 2800;
        task.chapter_ids = json!(["chapter-3", "chapter-4"]);

        let persistence_plan = build_test_resume_launch_persistence_plan(
            ResumeBatchGenerationCommandState::from_task(&task),
            "user-1",
            vec!["chapter-3".to_string(), "chapter-4".to_string()],
            false,
        );

        assert_eq!(persistence_plan.response_payload()["status"], "pending");
        assert_eq!(
            persistence_plan.response_payload()["message"],
            "Task resumed and queued"
        );
        assert_eq!(
            persistence_plan.response_payload()["checkpoint"]["phase"],
            "pending"
        );
    }

    #[test]
    fn should_keep_batch_generation_resume_task_command_launch_start_owner_contract() {
        let mut task = build_task("failed");
        task.user_id = "user-41".to_string();
        task.target_word_count = 2900;
        task.chapter_ids = json!(["chapter-41", "chapter-42"]);

        let persistence_plan = build_test_resume_launch_persistence_plan(
            ResumeBatchGenerationCommandState::from_task(&task),
            "user-41",
            vec!["chapter-41".to_string(), "chapter-42".to_string()],
            true,
        );

        match persistence_plan.dispatch_plan() {
            ResumeExecutionDispatchPlan::Batch { runtime_input } => {
                assert_eq!(runtime_input.user_id, "user-41");
                assert_eq!(
                    runtime_input.chapter_ids,
                    vec!["chapter-41".to_string(), "chapter-42".to_string()]
                );
                assert_eq!(runtime_input.target_word_count, 2900);
                assert!(runtime_input.compat_options.enable_analysis);
            }
            ResumeExecutionDispatchPlan::SingleChapter { .. } => {
                panic!("expected batch dispatch plan");
            }
        }

        assert_eq!(persistence_plan.response_payload()["batch_id"], "task-1");
        assert_eq!(persistence_plan.response_payload()["status"], "pending");
    }

    #[test]
    fn should_keep_resume_batch_generation_owned_prepare_owner_contract_explicit() {
        let task = batch_generation_task::Model {
            id: "task-5".to_string(),
            project_id: "project-5".to_string(),
            user_id: "user-5".to_string(),
            start_chapter_number: 5,
            chapter_count: 2,
            chapter_ids: json!(["chapter-5", "chapter-6"]),
            style_id: None,
            target_word_count: 3400,
            enable_analysis: true,
            status: "failed".to_string(),
            total_chapters: 2,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-5".to_string()),
            current_chapter_number: Some(5),
            current_retry_count: 1,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let workflow_runtime_state = json!({
            "batch_request_runtime_state": {
                "compat_options": {
                    "enable_analysis": true,
                    "enable_mcp": true,
                    "web_research_enabled": false,
                    "story_repair_targets": [],
                    "story_preserve_strengths": []
                },
                "model_override": null
            }
        });
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-5".to_string(),
            batch_task_id: "task-5".to_string(),
            workflow_runtime_state: Some(workflow_runtime_state.clone()),
            latest_quality_metrics: None,
            quality_metrics_history: None,
            quality_metrics_summary: None,
            created_at: Some(Utc::now().naive_utc()),
            updated_at: Some(Utc::now().naive_utc()),
        };
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);

        assert_eq!(command_state.batch_id, "task-5");
        assert_eq!(command_state.chapter_ids, json!(["chapter-5", "chapter-6"]));
        assert_eq!(
            snapshot
                .workflow_runtime_state
                .as_ref()
                .and_then(|state| state.get("batch_request_runtime_state"))
                .and_then(|state| state.get("model_override")),
            Some(&Value::Null)
        );
    }

    #[test]
    fn should_prefer_existing_active_story_repair_payload_for_resume_seed() {
        let workflow_runtime_state = json!({
            "active_story_repair_payload": {
                "summary": "来自运行态",
                "source": "current_chapter_quality"
            }
        });
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("来自请求".to_string()),
                story_repair_targets: vec!["请求目标".to_string()],
                ..Default::default()
            },
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));

        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            Some(&workflow_runtime_state),
            None,
            &request_runtime_state,
        )
        .runtime_state_seed
        .expect("resume runtime state seed");

        assert_eq!(seed["resume_from_batch_id"], "task-1");
        assert_eq!(seed["current_retry_count"], 0);
        assert_eq!(seed["max_retries"], 3);
        assert_eq!(seed["active_story_repair_payload"]["summary"], "来自运行态");
        assert_eq!(
            seed["active_story_repair_payload"]["source"],
            "current_chapter_quality"
        );
    }

    #[test]
    fn should_rehydrate_manual_story_repair_payload_for_resume_seed_when_snapshot_missing() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("补强前章伏笔".to_string()),
                story_repair_targets: vec!["伏笔回收".to_string()],
                story_preserve_strengths: vec!["尾声氛围".to_string()],
                ..Default::default()
            },
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));

        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            None,
            None,
            &request_runtime_state,
        )
        .runtime_state_seed
        .expect("resume seed");

        assert_eq!(seed["resume_from_batch_id"], "task-1");
        assert_eq!(seed["current_retry_count"], 0);
        assert_eq!(seed["max_retries"], 3);
        assert_eq!(
            seed["active_story_repair_payload"]["summary"],
            "补强前章伏笔"
        );
        assert_eq!(
            seed["active_story_repair_payload"]["repair_targets"],
            json!(["伏笔回收"])
        );
        assert_eq!(
            seed["active_story_repair_payload"]["preserve_strengths"],
            json!(["尾声氛围"])
        );
        assert_eq!(
            seed["active_story_repair_payload"]["source"],
            "manual_request"
        );
    }

    #[test]
    fn should_skip_resume_runtime_state_seed_without_story_repair_payload() {
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            None,
            None,
            &BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions::default(),
                None,
            ),
        )
        .runtime_state_seed
        .expect("resume seed without repair payload");

        assert_eq!(seed["resume_from_batch_id"], "task-1");
        assert_eq!(seed["current_retry_count"], 0);
        assert_eq!(seed["max_retries"], 3);
        assert!(seed.get("active_story_repair_payload").is_none());
    }

    #[test]
    fn should_restore_story_repair_compat_options_from_active_snapshot_when_request_empty() {
        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &SingleChapterGenerationCompatOptions::default(),
            Some(&json!({
                "summary": "补强前章伏笔",
                "repair_targets": ["回收悬念", "压缩说明"],
                "preserve_strengths": ["角色张力", "结尾钩子"]
            })),
            None,
            None,
        );

        assert_eq!(restored.story_repair_summary(), "补强前章伏笔");
        assert_eq!(
            restored.story_repair_targets(),
            &["回收悬念".to_string(), "压缩说明".to_string()]
        );
        assert_eq!(
            restored.story_preserve_strengths(),
            &["角色张力".to_string(), "结尾钩子".to_string()]
        );
    }

    #[test]
    fn should_keep_explicit_story_repair_compat_options_over_active_snapshot() {
        let compat = SingleChapterGenerationCompatOptions {
            story_repair_summary: Some("来自请求".to_string()),
            story_repair_targets: vec!["请求目标".to_string()],
            story_preserve_strengths: vec!["请求长板".to_string()],
            ..Default::default()
        };
        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &compat,
            Some(&json!({
                "summary": "来自快照",
                "repair_targets": ["快照目标"],
                "preserve_strengths": ["快照长板"]
            })),
            None,
            None,
        );

        assert_eq!(restored.story_repair_summary(), "来自请求");
        assert_eq!(restored.story_repair_targets(), &["请求目标".to_string()]);
        assert_eq!(
            restored.story_preserve_strengths(),
            &["请求长板".to_string()]
        );
    }

    #[test]
    fn should_restore_story_repair_compat_options_from_quality_metrics_summary_when_active_snapshot_missing(
    ) {
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "根据质量摘要补强中段冲突",
                    "repair_targets": ["提前引爆冲突", "减少重复说明"],
                    "preserve_strengths": ["人物张力", "结尾钩子"]
                }
            })),
        );

        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &SingleChapterGenerationCompatOptions::default(),
            None,
            snapshot.quality_metrics_summary.as_ref(),
            snapshot.latest_quality_metrics.as_ref(),
        );

        assert_eq!(restored.story_repair_summary(), "根据质量摘要补强中段冲突");
        assert_eq!(
            restored.story_repair_targets(),
            &["提前引爆冲突".to_string(), "减少重复说明".to_string()]
        );
        assert_eq!(
            restored.story_preserve_strengths(),
            &["人物张力".to_string(), "结尾钩子".to_string()]
        );
    }

    #[test]
    fn should_restore_active_story_repair_payload_from_quality_context() {
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "根据批量质量摘要补强冲突密度",
                    "repair_targets": ["提前爆点", "压缩说明"],
                    "preserve_strengths": ["角色压迫感"],
                    "focus_areas": ["节奏", "冲突", "", "节奏", "信息密度"],
                    "weakest_metric_key": "pacing",
                    "weakest_metric_label": "节奏",
                    "weakest_metric_value": 63.5
                },
                "quality_gate": {
                    "status": "failed",
                    "decision": "auto_repair",
                    "label": "需要修复",
                    "summary": "中段说明偏多",
                    "failed_metrics": [
                        {"label": "节奏"},
                        {"label": "信息密度"},
                        {"name": "ignored"}
                    ]
                }
            })),
        );

        let payload =
            crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::restore_active_story_repair_payload_from_quality_context(
                snapshot.quality_metrics_summary.as_ref(),
                snapshot.latest_quality_metrics.as_ref(),
                "batch",
                "recent_history_summary",
                "Recent history summary",
            )
            .expect("active story repair payload");

        assert_eq!(payload["summary"], "根据批量质量摘要补强冲突密度");
        assert_eq!(payload["repair_targets"], json!(["提前爆点", "压缩说明"]));
        assert_eq!(payload["preserve_strengths"], json!(["角色压迫感"]));
        assert_eq!(payload["focus_areas"], json!(["节奏", "冲突", "信息密度"]));
        assert_eq!(payload["weakest_metric_key"], "pacing");
        assert_eq!(payload["weakest_metric_label"], "节奏");
        assert_eq!(payload["weakest_metric_value"], 63.5);
        assert_eq!(payload["quality_gate_status"], "failed");
        assert_eq!(payload["quality_gate_decision"], "auto_repair");
        assert_eq!(payload["quality_gate_label"], "需要修复");
        assert_eq!(payload["quality_gate_summary"], "中段说明偏多");
        assert_eq!(
            payload["quality_gate_failed_metrics"],
            json!(["节奏", "信息密度"])
        );
        assert_eq!(payload["source"], "recent_history_summary");
        assert_eq!(payload["source_label"], "Recent history summary");
        assert_eq!(payload["scope"], "batch");
        assert!(payload["updated_at"].is_null());
    }

    #[test]
    fn should_prefer_quality_context_active_story_repair_payload_for_resume_seed_when_runtime_payload_missing(
    ) {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "沿用批量摘要修复建议",
                    "repair_targets": ["压缩说明"],
                    "preserve_strengths": ["钩子"]
                },
                "quality_gate": {
                    "status": "failed",
                    "decision": "auto_repair",
                    "label": "需要修复",
                    "summary": "存在节奏问题",
                    "failed_metrics": [{"label": "节奏"}]
                }
            })),
        );

        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            None,
            Some(&snapshot),
            &request_runtime_state,
        )
        .runtime_state_seed
        .expect("resume seed with quality context");

        assert_eq!(
            seed["active_story_repair_payload"]["summary"],
            "沿用批量摘要修复建议"
        );
        assert_eq!(
            seed["active_story_repair_payload"]["repair_targets"],
            json!(["压缩说明"])
        );
        assert_eq!(
            seed["active_story_repair_payload"]["preserve_strengths"],
            json!(["钩子"])
        );
        assert_eq!(
            seed["active_story_repair_payload"]["quality_gate_decision"],
            "auto_repair"
        );
        assert_eq!(
            seed["active_story_repair_payload"]["quality_gate_failed_metrics"],
            json!(["节奏"])
        );
        assert_eq!(
            seed["active_story_repair_payload"]["source"],
            "recent_history_summary"
        );
        assert_eq!(
            seed["quality_metrics_summary"]["repair_guidance"]["summary"],
            "沿用批量摘要修复建议"
        );
    }

    #[test]
    fn should_prefer_runtime_active_story_repair_payload_over_quality_context_for_resume_seed() {
        let workflow_runtime_state = json!({
            "active_story_repair_payload": {
                "summary": "来自运行态",
                "source": "current_chapter_quality"
            }
        });
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "来自质量摘要"
                }
            })),
        );

        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            Some(&workflow_runtime_state),
            Some(&snapshot),
            &request_runtime_state,
        )
        .runtime_state_seed
        .expect("resume seed");

        assert_eq!(seed["active_story_repair_payload"]["summary"], "来自运行态");
        assert_eq!(
            seed["active_story_repair_payload"]["source"],
            "current_chapter_quality"
        );
        assert_eq!(
            seed["quality_metrics_summary"]["repair_guidance"]["summary"],
            "来自质量摘要"
        );
    }

    #[test]
    fn should_restore_quality_history_context_into_resume_seed_from_quality_summary() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "来自质量摘要"
                },
                "quality_runtime_context": {
                    "recent_metrics": [{"overall_score": 87}],
                    "history_scope": "batch"
                }
            })),
        );

        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            None,
            Some(&snapshot),
            &request_runtime_state,
        )
        .runtime_state_seed
        .expect("resume seed");

        assert_eq!(
            seed["quality_history_context"],
            json!({
                "recent_metrics": [{"overall_score": 87}],
                "history_scope": "batch"
            })
        );
    }

    #[test]
    fn should_restore_quality_summary_state_and_history_into_resume_seed() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let workflow_runtime_state = json!({
            "quality_metrics_summary_state": {
                "scope": "batch",
                "chapter_count": 2,
                "first_overall_score": 88.0,
                "last_overall_score": 84.0
            }
        });
        let snapshot = batch_generation_snapshot::Model {
            quality_metrics_history: Some(json!([
                {"overall_score": 88},
                {"overall_score": 84}
            ])),
            quality_metrics_summary: Some(json!({
                "overall_score": 84.0,
                "quality_runtime_context": {
                    "recent_metrics": [{"overall_score": 84}]
                }
            })),
            ..build_snapshot(None, None)
        };

        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            Some(&workflow_runtime_state),
            Some(&snapshot),
            &request_runtime_state,
        )
        .runtime_state_seed
        .expect("resume seed");

        assert_eq!(
            seed["quality_metrics_history"],
            json!([
                {"overall_score": 88},
                {"overall_score": 84}
            ])
        );
        assert_eq!(seed["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            seed["quality_metrics_summary_state"]["first_overall_score"],
            88.0
        );
        assert_eq!(
            seed["quality_metrics_summary_state"]["last_overall_score"],
            84.0
        );
    }

    #[test]
    fn should_restore_latest_quality_metrics_into_resume_seed() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = batch_generation_snapshot::Model {
            latest_quality_metrics: Some(json!({
                "overall_score": 84,
                "quality_gate": {
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            })),
            ..build_snapshot(None, None)
        };

        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            None,
            Some(&snapshot),
            &request_runtime_state,
        )
        .runtime_state_seed
        .expect("resume seed");

        assert_eq!(seed["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(
            seed["latest_quality_metrics"]["quality_gate"]["decision"],
            "auto_repair"
        );
    }

    #[test]
    fn should_rebuild_quality_summary_state_from_history_when_runtime_state_missing() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = batch_generation_snapshot::Model {
            quality_metrics_history: Some(json!([
                {
                    "overall_score": 88,
                    "pacing_score": 8.3,
                    "quality_gate": {
                        "status": "passed",
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 84,
                    "pacing_score": 7.5,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ])),
            quality_metrics_summary: Some(json!({
                "overall_score": 84.0,
                "quality_runtime_context": {
                    "recent_metrics": [{"overall_score": 84}]
                }
            })),
            ..build_snapshot(None, None)
        };

        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            None,
            Some(&snapshot),
            &request_runtime_state,
        )
        .runtime_state_seed
        .expect("resume seed");

        assert_eq!(seed["quality_metrics_summary_state"]["scope"], "chapter");
        assert_eq!(seed["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            seed["quality_metrics_summary_state"]["first_overall_score"],
            88.0
        );
        assert_eq!(
            seed["quality_metrics_summary_state"]["last_overall_score"],
            84.0
        );
        assert_eq!(
            seed["quality_metrics_summary_state"]["pacing_score_total"],
            15.8
        );
        assert_eq!(
            seed["quality_metrics_summary_state"]["pacing_score_count"],
            2
        );
        assert_eq!(
            seed["quality_metrics_summary_state"]["recent_history"][1]["quality_gate"]["decision"],
            "auto_repair"
        );
    }

    #[test]
    fn should_rebuild_quality_summary_and_history_context_from_history_when_summary_missing() {
        let runtime_state = json!({
            "quality_metrics_history": [
                {
                    "overall_score": 88,
                    "engagement_score": 8.8,
                    "coherence_score": 8.4,
                    "pacing_score": 8.3,
                    "repair_guidance": {
                        "summary": "上一章总体稳定"
                    },
                    "quality_gate": {
                        "status": "passed",
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 84,
                    "engagement_score": 8.1,
                    "coherence_score": 8.0,
                    "pacing_score": 7.5,
                    "repair_guidance": {
                        "summary": "当前章需要压缩说明"
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ]
        });

        let restored = restored_resume_quality_runtime_context(
            BatchGenerationTaskKind::Batch,
            None,
            Some(&runtime_state),
        );

        assert_eq!(
            restored
                .quality_metrics_summary
                .as_ref()
                .and_then(|summary| summary.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            restored
                .quality_metrics_summary
                .as_ref()
                .and_then(|summary| summary.get("overall_score")),
            Some(&json!(84.0))
        );
        assert_eq!(
            restored
                .quality_history_context
                .as_ref()
                .and_then(|context| context.get("scope")),
            Some(&json!("batch"))
        );
        assert_eq!(
            restored
                .quality_history_context
                .as_ref()
                .and_then(|context| context.get("recent_metrics"))
                .and_then(Value::as_array)
                .map(|items| items.len()),
            Some(2)
        );
    }

    #[test]
    fn should_restore_single_resume_quality_runtime_context_with_chapter_scope_from_history() {
        let runtime_state = json!({
            "quality_metrics_history": [
                {
                    "overall_score": 88,
                    "engagement_score": 8.8,
                    "coherence_score": 8.4,
                    "pacing_score": 8.3,
                    "repair_guidance": {
                        "summary": "上一章总体稳定"
                    },
                    "quality_gate": {
                        "status": "passed",
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 84,
                    "engagement_score": 8.1,
                    "coherence_score": 8.0,
                    "pacing_score": 7.5,
                    "repair_guidance": {
                        "summary": "当前章需要压缩说明"
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ]
        });

        let restored = restored_resume_quality_runtime_context(
            BatchGenerationTaskKind::SingleChapter,
            None,
            Some(&runtime_state),
        );

        assert_eq!(
            restored
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("scope")),
            Some(&json!("chapter"))
        );
        assert_eq!(
            restored
                .quality_metrics_summary
                .as_ref()
                .and_then(|summary| summary.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            restored
                .quality_history_context
                .as_ref()
                .and_then(|context| context.get("scope")),
            Some(&json!("chapter"))
        );
    }

    #[test]
    fn should_rebuild_resume_seed_quality_summary_from_runtime_history_when_snapshot_summary_missing(
    ) {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let runtime_state = json!({
            "quality_metrics_history": [
                {
                    "overall_score": 88,
                    "engagement_score": 8.8,
                    "coherence_score": 8.4,
                    "pacing_score": 8.3,
                    "repair_guidance": {
                        "summary": "上一章总体稳定"
                    },
                    "quality_gate": {
                        "status": "passed",
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 84,
                    "engagement_score": 8.1,
                    "coherence_score": 8.0,
                    "pacing_score": 7.5,
                    "repair_guidance": {
                        "summary": "当前章需要压缩说明"
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ]
        });

        let seed = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            Some(&runtime_state),
            None,
            &request_runtime_state,
        )
        .runtime_state_seed
        .expect("resume seed");

        assert_eq!(seed["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(seed["quality_metrics_summary"]["overall_score"], 84.0);
        assert_eq!(seed["quality_history_context"]["scope"], "chapter");
        assert_eq!(seed["quality_metrics_summary_state"]["chapter_count"], 2);
    }

    #[test]
    fn should_restore_single_resume_seed_from_summary_only_snapshot_quality_context() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "overall_score": 84,
                "repair_guidance": {
                    "summary": "当前章需要压缩说明",
                    "repair_targets": ["压缩说明", "提前冲突"],
                    "preserve_strengths": ["角色张力"]
                },
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议修复"
                },
                "quality_runtime_context": {
                    "scope": "chapter",
                    "recent_metrics": [
                        {
                            "overall_score": 84,
                            "quality_gate": {
                                "status": "warning",
                                "decision": "auto_repair",
                                "label": "建议修复"
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
            })),
        );

        let restored = RestoredResumeRuntimeStateProjection::from_sources(
            command_state.task_kind(),
            &command_state.batch_id,
            command_state.max_retries,
            None,
            Some(&snapshot),
            &request_runtime_state,
        );
        let seed = restored.runtime_state_seed.expect("resume seed");

        assert_eq!(
            restored
                .request_runtime_state
                .compat_options
                .story_repair_summary(),
            "当前章需要压缩说明"
        );
        assert_eq!(
            restored
                .request_runtime_state
                .compat_options
                .story_repair_targets(),
            &["压缩说明".to_string(), "提前冲突".to_string()]
        );
        assert_eq!(
            seed["active_story_repair_payload"]["source"],
            "recent_history_summary"
        );
        assert_eq!(seed["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(seed["quality_metrics_history"][0]["overall_score"], 88);
        assert_eq!(seed["quality_metrics_history"][1]["overall_score"], 84);
        assert_eq!(seed["quality_metrics_summary_state"]["scope"], "chapter");
        assert_eq!(seed["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(seed["quality_history_context"]["scope"], "chapter");
        assert_eq!(seed["quality_metrics_summary"]["overall_score"], 84);
    }

    #[test]
    fn should_extract_quality_gate_object_from_summary_or_raw_shape() {
        let direct = extract_quality_gate_object(Some(&json!({
            "quality_gate": {
                "decision": "auto_repair"
            }
        })))
        .expect("direct gate");
        let raw = extract_quality_gate_object(Some(&json!({
            "raw": {
                "quality_gate": {
                    "decision": "manual_review"
                }
            }
        })))
        .expect("raw gate");

        assert_eq!(direct["decision"], "auto_repair");
        assert_eq!(raw["decision"], "manual_review");
        assert!(extract_quality_gate_object(Some(&json!({"foo": "bar"}))).is_none());
    }

    #[test]
    fn should_restore_story_repair_compat_options_from_latest_quality_metrics_when_summary_missing()
    {
        let snapshot = build_snapshot(
            Some(json!({
                "repair_guidance": {
                    "summary": "根据最新质量指标压缩解释段",
                    "repair_targets": ["压缩说明"],
                    "preserve_strengths": ["氛围描写"]
                }
            })),
            None,
        );

        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &SingleChapterGenerationCompatOptions::default(),
            None,
            snapshot.quality_metrics_summary.as_ref(),
            snapshot.latest_quality_metrics.as_ref(),
        );

        assert_eq!(
            restored.story_repair_summary(),
            "根据最新质量指标压缩解释段"
        );
        assert_eq!(restored.story_repair_targets(), &["压缩说明".to_string()]);
        assert_eq!(
            restored.story_preserve_strengths(),
            &["氛围描写".to_string()]
        );
    }

    #[test]
    fn should_restore_story_repair_compat_options_from_raw_quality_metrics_summary_when_needed() {
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "raw": {
                    "repair_guidance": {
                        "summary": "从 raw 质量摘要恢复补强建议",
                        "repair_targets": ["强化转折"],
                        "preserve_strengths": ["对白节奏"]
                    }
                }
            })),
        );

        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &SingleChapterGenerationCompatOptions::default(),
            None,
            snapshot.quality_metrics_summary.as_ref(),
            snapshot.latest_quality_metrics.as_ref(),
        );

        assert_eq!(
            restored.story_repair_summary(),
            "从 raw 质量摘要恢复补强建议"
        );
        assert_eq!(restored.story_repair_targets(), &["强化转折".to_string()]);
        assert_eq!(
            restored.story_preserve_strengths(),
            &["对白节奏".to_string()]
        );
    }

    #[test]
    fn should_restore_single_resume_compat_options_from_restored_history_only_quality_context() {
        let runtime_state = json!({
            "quality_metrics_history": [
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
                },
                {
                    "overall_score": 84,
                    "repair_guidance": {
                        "summary": "当前章需要压缩说明",
                        "repair_targets": ["压缩说明", "提前冲突"],
                        "preserve_strengths": ["角色张力"]
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ]
        });
        let restored_quality_context = restored_resume_quality_runtime_context(
            BatchGenerationTaskKind::SingleChapter,
            None,
            Some(&runtime_state),
        );

        let restored = restore_resume_compat_options_from_runtime_context(
            &BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions::default(),
                None,
            ),
            None,
            &restored_quality_context,
        );

        assert_eq!(restored.story_repair_summary(), "当前章需要压缩说明");
        assert_eq!(
            restored.story_repair_targets(),
            &["压缩说明".to_string(), "提前冲突".to_string()]
        );
        assert_eq!(
            restored.story_preserve_strengths(),
            &["角色张力".to_string()]
        );
    }

    #[test]
    fn should_restore_single_resume_active_story_repair_payload_from_restored_history_only_quality_context(
    ) {
        let runtime_state = json!({
            "quality_metrics_history": [
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
                },
                {
                    "overall_score": 84,
                    "repair_guidance": {
                        "summary": "当前章需要压缩说明",
                        "repair_targets": ["压缩说明", "提前冲突"],
                        "preserve_strengths": ["角色张力"]
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复",
                        "failed_metrics": [{"label": "节奏"}]
                    }
                }
            ]
        });
        let restored_quality_context = restored_resume_quality_runtime_context(
            BatchGenerationTaskKind::SingleChapter,
            None,
            Some(&runtime_state),
        );

        let restored = resolve_resume_active_story_repair_payload_from_runtime_context(
            None,
            None,
            &restored_quality_context,
            "chapter",
        )
        .expect("single resume active story repair payload");

        assert_eq!(restored["summary"], "当前章需要压缩说明");
        assert_eq!(restored["repair_targets"], json!(["压缩说明", "提前冲突"]));
        assert_eq!(restored["preserve_strengths"], json!(["角色张力"]));
        assert_eq!(restored["quality_gate_decision"], "auto_repair");
        assert_eq!(restored["quality_gate_failed_metrics"], json!(["节奏"]));
        assert_eq!(restored["scope"], "chapter");
        assert_eq!(restored["source"], "recent_history_summary");
    }

    #[test]
    fn should_prefer_latest_metrics_guidance_and_merge_summary_fallbacks() {
        let snapshot = build_snapshot(
            Some(json!({
                "repair_guidance": {
                    "summary": "来自 latest",
                    "repair_targets": ["latest target"],
                    "preserve_strengths": ["latest strength"]
                }
            })),
            Some(json!({
                "repair_guidance": {
                    "summary": "来自 summary",
                    "repair_targets": ["summary target"],
                    "preserve_strengths": ["summary strength"]
                }
            })),
        );

        let guidance = crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::quality_repair_guidance_from_quality_context(
            snapshot.quality_metrics_summary.as_ref(),
            snapshot.latest_quality_metrics.as_ref(),
        )
            .expect("quality guidance should be resolved");

        let merged_guidance = crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::merged_story_repair_guidance_from_quality_context(
            snapshot.quality_metrics_summary.as_ref(),
            snapshot.latest_quality_metrics.as_ref(),
        )
            .expect("merged quality guidance should be resolved");

        assert_eq!(guidance.get("summary"), Some(&json!("来自 latest")));
        assert_eq!(
            merged_guidance.get("repair_targets"),
            Some(&json!(["latest target", "summary target"]))
        );
        assert_eq!(
            merged_guidance.get("preserve_strengths"),
            Some(&json!(["latest strength", "summary strength"]))
        );
    }

    #[test]
    fn should_extract_repair_guidance_object_from_summary_or_raw_shape() {
        let direct = extract_repair_guidance_object(Some(&json!({
            "repair_guidance": {
                "summary": "direct"
            }
        })))
        .expect("direct guidance");
        let raw = extract_repair_guidance_object(Some(&json!({
            "raw": {
                "repair_guidance": {
                    "summary": "raw"
                }
            }
        })))
        .expect("raw guidance");

        assert_eq!(direct.get("summary"), Some(&json!("direct")));
        assert_eq!(raw.get("summary"), Some(&json!("raw")));
        assert!(extract_repair_guidance_object(Some(&json!({"foo": "bar"}))).is_none());
    }
}
