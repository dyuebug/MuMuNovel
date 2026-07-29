pub(crate) mod execution_config_owner;
pub(crate) mod request_runtime_state_owner;
pub(crate) mod single_generation_contract_owner;

pub(crate) use self::execution_config_owner::{
    build_generation_execution_config_owner_contract, prepare_generation_execution_config,
    prepare_generation_execution_config_with_provider_payload,
    prepare_role_aware_generation_execution_config,
    prepare_role_aware_generation_execution_config_with_provider_payload,
    PreparedGenerationExecutionConfig, PreparedRoleModelPolicyContext,
};
pub(crate) use self::request_runtime_state_owner::{
    active_story_repair_payload_from_runtime_state,
    active_story_repair_payload_ref_from_runtime_state,
    batch_generation_request_runtime_state_payload,
    build_batch_request_runtime_state_owner_contract, deserialize_optional_non_null,
    parse_batch_generation_request_runtime_state, BatchGenerationRequestRuntimeState,
};
pub(crate) use self::single_generation_contract_owner::{
    build_prompt_overrides_from_compat_options,
    build_single_generation_execution_contract_owner_contract,
    normalize_chapter_generation_target_word_count, SingleChapterGenerationCompatOptions,
    SingleChapterGenerationExecutionInput,
};

const BATCH_REQUEST_RUNTIME_STATE_KEY: &str = "batch_request_runtime_state";
pub(crate) const DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT: i32 = 3000;
pub(crate) const MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT: i32 = 1;

#[cfg(test)]
mod tests {
    use super::{
        active_story_repair_payload_from_runtime_state,
        active_story_repair_payload_ref_from_runtime_state,
        batch_generation_request_runtime_state_payload,
        build_batch_request_runtime_state_owner_contract,
        build_generation_execution_config_owner_contract,
        build_prompt_overrides_from_compat_options,
        build_single_generation_execution_contract_owner_contract,
        normalize_chapter_generation_target_word_count,
        parse_batch_generation_request_runtime_state, BatchGenerationRequestRuntimeState,
        SingleChapterGenerationCompatOptions, BATCH_REQUEST_RUNTIME_STATE_KEY,
        DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT, MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT,
    };
    use serde_json::json;

    #[test]
    fn should_publish_generation_execution_config_owner_contract_from_execution_contract_owner() {
        let contract = build_generation_execution_config_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_execution_contract_service::execution_config"
        );
        assert_eq!(
            contract["scope"],
            "shared_generation_execution_config_bridge"
        );
        assert_eq!(contract["python_source_map"].as_array().unwrap().len(), 0);
        assert_eq!(
            contract["historical_python_test_support"][0],
            "backend/tests/test_support/ai_gateway/ai_config.py"
        );
        assert_eq!(
            contract["rust_target_map"][0],
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs"
        );
        assert_eq!(
            contract["rust_owner_functions"][0],
            "prepare_generation_execution_config"
        );
        assert_eq!(
            contract["behavior_contract"]["ai_config_owner"],
            "SettingsService::build_ai_config"
        );
        assert_eq!(
            contract["behavior_contract"]["model_override_forwarded"],
            true
        );
        assert_eq!(
            contract["behavior_contract"]["provider_payload_passthrough"],
            true
        );
        assert_eq!(
            contract["active_consumers"][3],
            "chapter_batch_generation_runtime_state_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "production_python_ai_gateway_source_map_deleted_historical_fixtures_live_under_tests_test_support"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][1],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            json!(11)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            json!(17)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["execution_config_owner"],
            "prepare_generation_execution_config_with_provider_payload"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_shared_execution_config_owner_with_deleted_python_ai_gateway_source_map"
        );
    }

    #[test]
    fn should_publish_batch_request_runtime_state_owner_contract_from_execution_contract_owner() {
        let contract = build_batch_request_runtime_state_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_execution_contract_service::request_runtime_state"
        );
        assert_eq!(
            contract["scope"],
            "batch_request_runtime_state_payload_and_story_repair_projection"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_target_map"][0],
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["request_runtime_state_key"],
            BATCH_REQUEST_RUNTIME_STATE_KEY
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][2],
            "batch_generation_request_runtime_state_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["payload_extraction_policy"],
            "active_story_repair_payload_must_be_object"
        );
        assert_eq!(
            contract["active_consumers"][3],
            "chapter_batch_generation_resume_task_command_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_request_runtime_state_owner_is_rust_only_and_no_longer_tracks_direct_python_request_runtime_state_shell_source_maps"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"],
            json!(["phase5-batch-generation-owner"])
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            json!(11)
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
            contract["service_runtime_closeout_status"]["status"],
            "rust_batch_request_runtime_state_owner_direct_package_closed_out"
        );
    }

    #[test]
    fn should_skip_empty_prompt_override_values() {
        let compat = SingleChapterGenerationCompatOptions {
            creative_mode: Some("   ".to_string()),
            story_focus: Some("advance_plot".to_string()),
            web_research_enabled: false,
            ..Default::default()
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert_eq!(prompt_overrides.creative_mode, None);
        assert_eq!(
            prompt_overrides.story_focus.as_deref(),
            Some("advance_plot")
        );
    }

    #[test]
    fn should_include_web_research_fields_in_prompt_overrides() {
        let compat = SingleChapterGenerationCompatOptions {
            style_id: Some(3),
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: true,
            web_research_query: Some("民国报馆夜班排印流程".to_string()),
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

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert!(prompt_overrides.web_research_enabled);
        assert_eq!(
            prompt_overrides.web_research_query.as_deref(),
            Some("民国报馆夜班排印流程")
        );
    }

    #[test]
    fn should_preserve_story_repair_arrays_in_prompt_overrides() {
        let compat = SingleChapterGenerationCompatOptions {
            story_repair_summary: Some("保留悬念并压缩解释".to_string()),
            story_repair_targets: vec!["压缩说明".to_string(), "强化冲突".to_string()],
            story_preserve_strengths: vec!["人物动机清晰".to_string()],
            ..Default::default()
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert_eq!(
            prompt_overrides.story_repair_summary.as_deref(),
            Some("保留悬念并压缩解释")
        );
        assert_eq!(
            prompt_overrides.story_repair_targets,
            vec!["压缩说明".to_string(), "强化冲突".to_string()]
        );
        assert_eq!(
            prompt_overrides.story_preserve_strengths,
            vec!["人物动机清晰".to_string()]
        );
    }

    #[test]
    fn should_describe_execution_contract_owner_boundary() {
        let contract = build_single_generation_execution_contract_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_execution_contract_service::single_generation_contract_owner"
        );
        assert_eq!(
            contract["scope"],
            "single_generation_execution_input_and_compat_options"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["compat_option_fields"]
                .as_array()
                .expect("compat option fields")
                .len(),
            15
        );
        assert_eq!(
            contract["behavior_contract"]["execution_input_fields"],
            serde_json::json!(["target_word_count", "compat_options", "execution_config"])
        );
        assert_eq!(
            contract["behavior_contract"]["target_word_count_entrypoint"],
            "normalize_chapter_generation_target_word_count"
        );
        assert_eq!(
            contract["behavior_contract"]["default_target_word_count"],
            DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT
        );
        assert_eq!(
            contract["behavior_contract"]["minimum_target_word_count"],
            MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT
        );
        assert_eq!(
            contract["behavior_contract"]["request_runtime_state_key"],
            BATCH_REQUEST_RUNTIME_STATE_KEY
        );
        assert_eq!(
            contract["behavior_contract"]["generation_execution_config_owner_contract"]["owner"],
            "chapter_generation_execution_contract_service::execution_config"
        );
        assert_eq!(
            contract["behavior_contract"]["generation_execution_config_owner_contract"]
                ["behavior_contract"]["prepared_fields"],
            json!(["ai_config", "provider_payload", "role_policy_context"])
        );
        assert_eq!(
            contract["behavior_contract"]["generation_execution_config_owner_contract"]
                ["behavior_contract"]["role_policy_context_forwarded"],
            true
        );
        assert_eq!(
            contract["behavior_contract"]["request_runtime_state_owner_contract"]["owner"],
            "chapter_generation_execution_contract_service::request_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["request_runtime_state_owner_contract"]
                ["behavior_contract"]["entrypoints"][3],
            "parse_batch_generation_request_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["empty_string_prompt_overrides_skipped"],
            true
        );
        assert_eq!(
            contract["behavior_contract"]["web_research_fields_preserved"],
            true
        );
        assert_eq!(
            contract["behavior_contract"]["story_repair_arrays_preserved"],
            true
        );
        assert_eq!(
            contract["active_consumers"][9],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["active_consumers"][10],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
        assert_eq!(contract["active_consumers"][11], serde_json::Value::Null);
        assert_eq!(
            contract["rollback_boundary"]["runtime_knobs"][0],
            "SingleChapterGenerationCompatOptions"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "single_generation_execution_contract_owner_is_rust_only_and_shared_prompt_story_repair_python_surfaces_are_tracked_by_external_owner_contracts"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][1],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            json!(11)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            json!(17)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["compat_options_owner"],
            "SingleChapterGenerationCompatOptions"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["request_runtime_state_owner"],
            "BatchGenerationRequestRuntimeState"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "single-generation execution contract owner is rust-only; surviving Python exit work is tracked by shared prompt and shared runtime owner contracts outside this owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_shared_execution_contract_owner_direct_package_closed_out"
        );
    }

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
    fn should_normalize_chapter_generation_target_word_count_from_execution_contract_owner() {
        assert_eq!(
            normalize_chapter_generation_target_word_count(None),
            DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT
        );
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(-100)),
            MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT
        );
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(0)),
            MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT
        );
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(2500)),
            2500
        );
    }

    #[test]
    fn should_build_batch_request_runtime_state_payload_from_execution_contract_owner() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("强化旧伏笔".to_string()),
                story_repair_targets: vec!["伏笔".to_string()],
                story_preserve_strengths: vec!["氛围".to_string()],
                ..empty_compat_options()
            },
            Some("gpt-4.1".to_string()),
        );

        let payload = batch_generation_request_runtime_state_payload(&runtime_state);

        assert_eq!(
            payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(payload["active_story_repair_payload"]["scope"], "batch");
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"][0],
            "伏笔"
        );
    }

    #[test]
    fn should_parse_batch_request_runtime_state_from_execution_contract_owner() {
        let payload = json!({
            "batch_request_runtime_state": {
                "compat_options": {
                    "style_id": 7,
                    "enable_analysis": true,
                    "enable_mcp": false,
                    "web_research_enabled": true,
                    "web_research_query": "旧都城",
                    "narrative_perspective": null,
                    "creative_mode": null,
                    "story_focus": null,
                    "plot_stage": null,
                    "story_creation_brief": null,
                    "quality_preset": null,
                    "quality_notes": null,
                    "story_repair_summary": "强化冲突",
                    "story_repair_targets": ["冲突"],
                    "story_preserve_strengths": ["节奏"]
                },
                "model_override": "gpt-4.1"
            }
        });

        let runtime_state = parse_batch_generation_request_runtime_state(Some(&payload));

        assert_eq!(runtime_state.model_override.as_deref(), Some("gpt-4.1"));
        assert_eq!(
            runtime_state.compat_options.story_repair_summary.as_deref(),
            Some("强化冲突")
        );
        assert_eq!(
            runtime_state.compat_options.story_preserve_strengths,
            vec!["节奏".to_string()]
        );
    }

    #[test]
    fn should_extract_active_story_repair_payload_from_execution_contract_owner() {
        let payload = json!({
            "active_story_repair_payload": {
                "scope": "chapter",
                "summary": "修复节奏"
            }
        });

        let repair_payload =
            active_story_repair_payload_from_runtime_state(Some(&payload)).expect("repair payload");

        assert_eq!(repair_payload["scope"], "chapter");
        assert_eq!(repair_payload["summary"], "修复节奏");
    }

    #[test]
    fn should_borrow_active_story_repair_payload_from_execution_contract_owner() {
        let payload = json!({
            "active_story_repair_payload": {
                "scope": "batch",
                "summary": "继续补强冲突"
            }
        });

        let repair_payload = active_story_repair_payload_ref_from_runtime_state(Some(&payload))
            .expect("borrowed repair payload");

        assert_eq!(repair_payload["scope"], "batch");
        assert_eq!(repair_payload["summary"], "继续补强冲突");
    }
}
