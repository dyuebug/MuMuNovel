use serde_json::{json, Value};

pub(crate) mod seed_owner;

#[cfg(test)]
pub(crate) use self::seed_owner::{
    build_single_generation_runtime_launch_input_from_request_runtime_state,
    RestoredSingleGenerationRuntimeState,
};
pub(crate) use self::seed_owner::{
    prepare_single_chapter_runtime_launch_input_from_request_runtime_state,
    prepare_single_generation_restored_runtime_seed_from_target,
};
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract;
use crate::services::chapter_single_generation_runtime_state_service::build_single_generation_runtime_state_owner_contract;

pub(crate) fn build_single_generation_runtime_seed_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_runtime_seed_service",
        "scope": "single_generation_runtime_state_seed_story_repair_restore_recent_history_and_launch_input_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_runtime_seed_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs",
            "backend-rs/src/services/chapter_quality_metrics_query_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_single_generation_runtime_launch_input_from_request_runtime_state",
                "RestoredSingleGenerationRuntimeState::from_quality_fragments",
                "RestoredSingleGenerationRuntimeState::into_startup_runtime_launch_parts",
                "restore_single_generation_runtime_state",
                "prepare_single_chapter_runtime_launch_input_from_request_runtime_state"
            ],
            "runtime_state_seed_contract": [
                "build_single_generation_runtime_state_payload_from_sources",
                "build_single_generation_runtime_state_payload_from_parts",
                "resolve_single_generation_runtime_compat_options_from_seed",
                "build_single_generation_runtime_launch_input"
            ],
            "story_repair_recent_history_contract": [
                "load_recent_single_generation_story_repair_quality_summary",
                "aggregate_story_repair_quality_summaries",
                "restore_story_repair_compat_options_from_active_snapshot"
            ],
            "seed_sources": [
                "current_chapter_quality",
                "recent_history_summary"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_single_generation_prepare_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_single_generation_active_gateway_smoke_service"
        ],
        "runtime_state_owner_contract": build_single_generation_runtime_state_owner_contract(),
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "story_repair_quality_context_owner_contract": build_story_repair_quality_context_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_seed_service",
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test api::health",
            "cargo check"
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_single_generation_runtime_launch_input_from_request_runtime_state,
        build_single_generation_runtime_seed_owner_contract, RestoredSingleGenerationRuntimeState,
    };
    use crate::services::chapter_generation_execution_contract_service::{
        BatchGenerationRequestRuntimeState, PreparedGenerationExecutionConfig,
        SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_prompt_service::PromptContextProviderPayload;
    use crate::services::chapter_quality_metrics_query_service::ChapterQualityMetricsFragments;
    use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationTarget;
    use serde_json::json;

    #[test]
    fn should_publish_single_generation_runtime_seed_owner_contract() {
        let contract = build_single_generation_runtime_seed_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_runtime_seed_service"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "build_single_generation_runtime_launch_input_from_request_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][3],
            "restore_single_generation_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["seed_sources"][1],
            "recent_history_summary"
        );
        assert_eq!(
            contract["python_source_map"].as_array().map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn should_build_single_generation_runtime_launch_input_from_request_runtime_state_owner() {
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-9".to_string(),
            chapter_number: 9,
            title: "第九章".to_string(),
        };
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions {
                enable_analysis: true,
                story_repair_summary: Some("沿用恢复态摘要".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                ..Default::default()
            },
            Some("owner-model".to_string()),
        );

        let runtime_input = build_single_generation_runtime_launch_input_from_request_runtime_state(
            &chapter_target,
            "user-9",
            2800,
            &request_runtime_state,
            PreparedGenerationExecutionConfig {
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
        );

        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-9");
        assert_eq!(runtime_input.execution_input.target_word_count, 2800);
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_summary(),
            "沿用恢复态摘要"
        );
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_targets(),
            &["压缩说明".to_string()]
        );
    }

    #[test]
    fn should_project_restored_single_generation_runtime_state_into_startup_and_runtime_launch_owner(
    ) {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("request summary".to_string()),
                story_repair_targets: vec!["request-target".to_string()],
                story_preserve_strengths: vec!["request-strength".to_string()],
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
