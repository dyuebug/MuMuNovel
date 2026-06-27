mod prompt_block_owner;
mod prompt_runtime_owner;
mod provider_payload_owner;
mod quality_profile_owner;
mod story_card_owner;
mod template_render_owner;

pub(crate) use prompt_block_owner::{
    build_creative_mode_block, build_external_assets_block, build_optional_instruction_block,
    build_quality_contract_block, build_quality_generation_protocol_block,
    build_quality_json_protocol_block, build_quality_preference_block,
    build_quality_profile_payload, build_repair_diagnostic_block, build_repair_target_block,
    build_story_focus_block, build_web_research_block, creative_mode_spec, normalize_creative_mode,
    normalize_plot_stage, normalize_prompt_list, normalize_story_focus, plot_stage_label,
    resolve_prompt_preference, story_focus_spec, QUALITY_RUNTIME_TRACKING_TAG,
};
pub(crate) use prompt_runtime_owner::{
    build_previous_chapter_prompt_context, build_prompt_params_with_provider_payload,
    PreviousChapterPromptContext,
};
#[cfg(test)]
pub(crate) use provider_payload_owner::PROMPT_CONTEXT_PROVIDER_FIELD_KEYS;
pub(crate) use provider_payload_owner::{
    build_placeholder_prompt_context_provider_payload,
    build_prompt_context_provider_owner_contract, PromptContextProviderPayload,
};
pub(crate) use quality_profile_owner::{
    build_novel_quality_prompt_blocks, build_quality_profile_owner_contract,
    resolve_adaptive_quality_gate_profile, resolve_metric_threshold_adjustments,
    resolve_quality_weight_profile,
};
pub(crate) use story_card_owner::{
    build_narrative_blueprint_block as build_narrative_blueprint_block_owner,
    build_story_acceptance_card_block as build_story_acceptance_card_block_owner,
    build_story_action_rendering_card_block as build_story_action_rendering_card_block_owner,
    build_story_card_owner_contract,
    build_story_character_arc_card_block as build_story_character_arc_card_block_owner,
    build_story_cliffhanger_card_block as build_story_cliffhanger_card_block_owner,
    build_story_dialogue_advancement_card_block as build_story_dialogue_advancement_card_block_owner,
    build_story_emotion_landing_card_block as build_story_emotion_landing_card_block_owner,
    build_story_execution_checklist_block as build_story_execution_checklist_block_owner,
    build_story_information_release_card_block as build_story_information_release_card_block_owner,
    build_story_objective_card_block as build_story_objective_card_block_owner,
    build_story_opening_hook_card_block as build_story_opening_hook_card_block_owner,
    build_story_payoff_chain_card_block as build_story_payoff_chain_card_block_owner,
    build_story_repetition_control_card_block as build_story_repetition_control_card_block_owner,
    build_story_repetition_risk_block as build_story_repetition_risk_block_owner,
    build_story_result_card_block as build_story_result_card_block_owner,
    build_story_rule_grounding_card_block as build_story_rule_grounding_card_block_owner,
    build_story_scene_anchor_card_block as build_story_scene_anchor_card_block_owner,
    build_story_scene_density_card_block as build_story_scene_density_card_block_owner,
    build_story_summary_tone_control_card_block as build_story_summary_tone_control_card_block_owner,
    build_story_viewpoint_discipline_card_block as build_story_viewpoint_discipline_card_block_owner,
};
#[cfg(test)]
pub(crate) use template_render_owner::chapter_template_key;
pub(crate) use template_render_owner::{
    build_chapter_generation_prompt_owner_contract, build_prompt_with_provider_payload,
    prompt_block_text, ChapterGenerationPromptOverrides,
};

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{
        build_chapter_generation_prompt_owner_contract,
        build_placeholder_prompt_context_provider_payload, build_previous_chapter_prompt_context,
        build_prompt_context_provider_owner_contract, build_prompt_params_with_provider_payload,
        build_prompt_with_provider_payload, chapter_template_key, ChapterGenerationPromptOverrides,
        PROMPT_CONTEXT_PROVIDER_FIELD_KEYS,
    };
    use crate::models::{chapter, project};
    use crate::services::chapter_generation_prompt_service::PromptContextProviderPayload;

    fn build_project(outline_mode: &str) -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "项目标题".to_string(),
            genre: Some("奇幻".to_string()),
            description: None,
            theme: None,
            target_words: 120000,
            current_words: 0,
            status: "active".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 0,
            outline_mode: outline_mode.to_string(),
            narrative_perspective: None,
            world_time_period: Some("近未来".to_string()),
            world_location: Some("浮空城".to_string()),
            world_atmosphere: Some("压抑".to_string()),
            world_rules: Some("魔力守恒".to_string()),
            chapter_count: Some(3),
            character_count: 0,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn build_chapter(
        chapter_number: i32,
        title: &str,
        expansion_plan: Option<&str>,
        content: Option<&str>,
        summary: Option<&str>,
    ) -> chapter::Model {
        chapter::Model {
            id: format!("chapter-{chapter_number}"),
            project_id: "project-1".to_string(),
            title: title.to_string(),
            chapter_number,
            content: content.map(str::to_string),
            summary: summary.map(str::to_string),
            expansion_plan: expansion_plan.map(str::to_string),
            status: "pending".to_string(),
            word_count: 0,
            outline_id: None,
            sub_index: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_publish_chapter_generation_prompt_owner_contract() {
        let contract = build_chapter_generation_prompt_owner_contract();

        assert_eq!(contract["owner"], "chapter_generation_prompt_service");
        assert_eq!(
            contract["scope"],
            "shared_generation_prompt_template_and_runtime_block_owner"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("python source map")
                .len(),
            0
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_generation_prompt_service.rs"
        );
        assert_eq!(
            contract["rust_owner_map"][1],
            "backend-rs/src/services/chapter_generation_prompt_service/provider_payload_owner.rs"
        );
        assert_eq!(
            contract["rust_owner_map"][2],
            "backend-rs/src/services/chapter_generation_prompt_service/template_render_owner.rs"
        );
        assert_eq!(
            contract["rust_owner_map"][3],
            "backend-rs/src/services/chapter_generation_prompt_service/story_card_owner.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][2],
            "build_prompt_params_with_provider_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["prompt_context_provider_owner_contract"]["owner"],
            "chapter_generation_prompt_service"
        );
        assert_eq!(
            contract["behavior_contract"]["prompt_context_provider_owner_contract"]["scope"],
            "provider_payload_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["prompt_context_provider_owner_contract"]
                ["behavior_contract"]["prompt_param_bridge"],
            "PromptContextProviderPayload::into_prompt_params"
        );
        assert_eq!(
            contract["behavior_contract"]["template_key_policy"][1],
            "one-to-many_with_previous -> CHAPTER_GENERATION_ONE_TO_MANY_NEXT"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_blocks"][7],
            "quality_contract_block"
        );
        assert_eq!(
            contract["behavior_contract"]["story_card_owner_contract"]["owner"],
            "chapter_generation_prompt_service::story_card_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["story_card_owner_contract"]["behavior_contract"]
                ["entrypoints"][0],
            "build_narrative_blueprint_block"
        );
        assert_eq!(
            contract["behavior_contract"]["quality_profile_owner_contract"]["owner"],
            "chapter_generation_prompt_service::quality_profile_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["quality_profile_owner_contract"]["python_source_map"]
                .as_array()
                .expect("quality python source map")
                .len(),
            0
        );
        assert_eq!(
            contract["behavior_contract"]["quality_profile_owner_contract"]["behavior_contract"]
                ["entrypoints"][0],
            "build_novel_quality_prompt_blocks"
        );
        assert_eq!(
            contract["behavior_contract"]["quality_profile_owner_contract"]["behavior_contract"]
                ["external_asset_policy"][0],
            "summary_only_assets"
        );
        assert_eq!(
            contract["behavior_contract"]["quality_profile_owner_contract"]["rollback_boundary"]
                ["source_map_policy"],
            "production_python_quality_profile_source_map_deleted_after_rust_owner_validation"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            6
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["regeneration_manifest_probe_count"],
            13
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
            contract["service_runtime_closeout_status"]
                ["python_story_prompt_block_service_deleted"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["python_prompt_template_facade_lazy_source_map_import"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["python_prompt_template_facade_service_deleted"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["python_prompt_service_lazy_source_map_import"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_prompt_service_deleted"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["python_story_packet_lazy_source_map_import"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_story_packet_service_deleted"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["python_story_packet_lazy_continuity_ledger_import"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["python_story_packet_continuity_ledger_proxy_retired"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["python_story_continuity_ledger_service_deleted"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["production_promptservice_default_importers_cleared"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "prompt Python production services physically closed; historical Python prompt fixtures live under backend/tests/test_support"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_service_runtime_owner_with_prompt_python_production_services_deleted"
        );
        assert_eq!(
            contract["rollback_boundary"],
            "prompt rollback is now Rust owner plus backend/tests/test_support historical fixtures; no backend/app prompt Python service remains"
        );
    }

    #[test]
    fn should_build_placeholder_prompt_context_provider_payload() {
        let payload = build_placeholder_prompt_context_provider_payload();

        assert_eq!(payload.characters_info, "[]");
        assert_eq!(payload.chapter_careers, "[]");
        assert_eq!(payload.recent_chapters_context, "");
        assert_eq!(payload.previous_chapter_summary, "");
        assert_eq!(payload.foreshadow_reminders, "[]");
        assert_eq!(payload.relevant_memories, "[]");
        assert_eq!(payload.research_query, "");
        assert_eq!(payload.research_assets, "[]");
        assert_eq!(payload.external_assets, "[]");
        assert_eq!(payload.reference_assets, "[]");
        assert_eq!(payload.mcp_references, "");
    }

    #[test]
    fn should_convert_provider_payload_into_prompt_params() {
        let params = build_placeholder_prompt_context_provider_payload().into_prompt_params();

        assert_eq!(params["characters_info"], "[]");
        assert_eq!(params["chapter_careers"], "[]");
        assert_eq!(params["recent_chapters_context"], "");
        assert_eq!(params["previous_chapter_summary"], "");
        assert_eq!(params["foreshadow_reminders"], "[]");
        assert_eq!(params["relevant_memories"], "[]");
        assert_eq!(params["research_query"], "");
        assert_eq!(params["research_assets"], "[]");
        assert_eq!(params["external_assets"], "[]");
        assert_eq!(params["reference_assets"], "[]");
        assert_eq!(params["mcp_references"], "");
    }

    #[test]
    fn should_publish_prompt_context_provider_owner_contract() {
        let contract = build_prompt_context_provider_owner_contract();

        assert_eq!(contract["owner"], "chapter_generation_prompt_service");
        assert_eq!(contract["scope"], "provider_payload_owner");
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("python source map")
                .len(),
            0
        );
        assert_eq!(
            contract["rust_target_map"][0],
            "backend-rs/src/services/chapter_generation_prompt_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["provider_payload_fields"]
                .as_array()
                .expect("provider payload fields")
                .len(),
            PROMPT_CONTEXT_PROVIDER_FIELD_KEYS.len()
        );
        assert_eq!(
            contract["behavior_contract"]["prompt_param_bridge"],
            "PromptContextProviderPayload::into_prompt_params"
        );
        assert_eq!(
            contract["behavior_contract"]["prompt_render_consumer"],
            "build_prompt_params_with_provider_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["asset_prompt_visibility"][2],
            "mcp_references"
        );
        assert!(!contract["python_source_map"]
            .as_array()
            .expect("python source map")
            .iter()
            .any(|item| item == "backend/app/api/chapters.py"));
        assert_eq!(
            contract["behavior_contract"]["mcp_references_preserved"],
            true
        );
        assert_eq!(
            contract["active_consumers"][5],
            "chapter_batch_generation_runtime_state_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "production_python_prompt_builders_deleted_test_fixtures_only"
        );
    }

    #[test]
    fn should_select_template_keys_for_outline_mode_and_previous_chapter_state() {
        assert_eq!(
            chapter_template_key("one-to-many", false),
            "CHAPTER_GENERATION_ONE_TO_MANY"
        );
        assert_eq!(
            chapter_template_key("one-to-many", true),
            "CHAPTER_GENERATION_ONE_TO_MANY_NEXT"
        );
        assert_eq!(
            chapter_template_key("one-to-one", false),
            "CHAPTER_GENERATION_ONE_TO_ONE"
        );
        assert_eq!(
            chapter_template_key("custom-mode", true),
            "CHAPTER_GENERATION_ONE_TO_ONE_NEXT"
        );
    }

    #[test]
    fn should_inject_defaults_when_optional_prompt_fields_are_missing() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(3, "第三章", None, None, None);

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3200,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        assert!(prompt.contains("项目标题"));
        assert!(prompt.contains("第三章"));
        assert!(prompt.contains("3200"));
        assert!(prompt.contains("第三人称"));
        assert!(prompt.contains("暂无大纲"));
    }

    #[test]
    fn should_include_previous_chapter_context_and_continuation_excerpt() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(4, "第四章", Some("推进主线"), None, None);
        let previous_content = format!("{}{}", "甲".repeat(120), "乙".repeat(500));
        let previous_summary = "上一章总结";
        let previous_chapter = build_chapter(
            3,
            "第三章",
            Some("旧大纲"),
            Some(previous_content.as_str()),
            Some(previous_summary),
        );

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(Some(&previous_chapter)),
            true,
            3600,
            PromptContextProviderPayload {
                previous_chapter_summary: previous_summary.to_string(),
                ..build_placeholder_prompt_context_provider_payload()
            },
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        assert!(prompt.contains(previous_summary));
        assert!(prompt.contains(&"乙".repeat(500)));
        assert!(!prompt.contains(&"甲".repeat(120)));
    }

    #[test]
    fn should_build_prompt_with_injected_provider_payload() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(2, "第二章", Some("推进冲突"), None, None);
        let provider_payload = PromptContextProviderPayload {
            recent_chapters_context: String::new(),
            previous_chapter_summary: "上一章总结".to_string(),
            chapter_careers: "[]".to_string(),
            characters_info: "[角色甲]".to_string(),
            foreshadow_reminders: "[伏笔甲]".to_string(),
            relevant_memories: "[记忆甲]".to_string(),
            research_query: String::new(),
            research_assets: "[]".to_string(),
            external_assets: "[]".to_string(),
            reference_assets: "[]".to_string(),
            mcp_references: String::new(),
        };

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            provider_payload,
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        assert!(prompt.contains("[角色甲]"));
        assert!(prompt.contains("[伏笔甲]"));
        assert!(prompt.contains("[记忆甲]"));
    }

    #[test]
    fn should_build_prompt_params_with_defaults_and_provider_context() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(2, "第二章", None, None, None);
        let provider_payload = PromptContextProviderPayload {
            recent_chapters_context: String::new(),
            previous_chapter_summary: "上一章总结".to_string(),
            chapter_careers: "[]".to_string(),
            characters_info: "[角色甲]".to_string(),
            foreshadow_reminders: "[伏笔甲]".to_string(),
            relevant_memories: "[记忆甲]".to_string(),
            research_query: String::new(),
            research_assets: "[]".to_string(),
            external_assets: "[]".to_string(),
            reference_assets: "[]".to_string(),
            mcp_references: String::new(),
        };

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            provider_payload,
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("project_title").map(String::as_str),
            Some("项目标题")
        );
        assert_eq!(
            params.get("chapter_title").map(String::as_str),
            Some("第二章")
        );
        assert_eq!(
            params.get("target_word_count").map(String::as_str),
            Some("2800")
        );
        assert_eq!(
            params.get("narrative_perspective").map(String::as_str),
            Some("第三人称")
        );
        assert_eq!(
            params.get("chapter_outline").map(String::as_str),
            Some("暂无大纲")
        );
        assert_eq!(
            params.get("characters_info").map(String::as_str),
            Some("[角色甲]")
        );
        assert_eq!(
            params.get("previous_chapter_summary").map(String::as_str),
            Some("上一章总结")
        );
        assert_eq!(
            params.get("external_assets").map(String::as_str),
            Some("[]")
        );
    }

    #[test]
    fn should_apply_prompt_overrides_before_project_defaults() {
        let mut project_model = build_project("one-to-one");
        project_model.narrative_perspective = Some("第三人称".to_string());
        project_model.default_creative_mode = Some("balanced".to_string());
        project_model.default_story_focus = Some("advance_plot".to_string());
        project_model.default_plot_stage = Some("development".to_string());
        project_model.default_story_creation_brief = Some("项目默认总控".to_string());
        project_model.default_quality_preset = Some("balanced".to_string());
        project_model.default_quality_notes = Some("项目默认质量要求".to_string());
        let chapter_model = build_chapter(5, "第五章", Some("推进高潮"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3200,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                narrative_perspective: Some("第一人称".to_string()),
                creative_mode: Some("suspense".to_string()),
                story_focus: Some("reveal_mystery".to_string()),
                plot_stage: Some("climax".to_string()),
                story_creation_brief: Some("本章主打谜团揭晓前夜".to_string()),
                quality_preset: Some("immersive".to_string()),
                quality_notes: Some("压缩解释，强化临场感".to_string()),
                web_research_enabled: false,
                web_research_query: None,
                story_repair_summary: None,
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
            },
        );

        assert_eq!(
            params.get("narrative_perspective").map(String::as_str),
            Some("第一人称")
        );
        assert_eq!(
            params.get("creative_mode").map(String::as_str),
            Some("suspense")
        );
        assert_eq!(
            params.get("story_focus").map(String::as_str),
            Some("reveal_mystery")
        );
        assert_eq!(params.get("plot_stage").map(String::as_str), Some("climax"));
        assert_eq!(
            params.get("story_creation_brief").map(String::as_str),
            Some("本章主打谜团揭晓前夜")
        );
        assert_eq!(
            params.get("quality_preset").map(String::as_str),
            Some("immersive")
        );
        assert_eq!(
            params.get("quality_notes").map(String::as_str),
            Some("压缩解释，强化临场感")
        );
        assert!(params["creative_mode_block"].contains("创作模式"));
        assert!(params["creative_mode_block"].contains("悬念拉满"));
        assert!(params["story_focus_block"].contains("谜团揭示"));
        assert!(params["narrative_blueprint_block"].contains("悬念拉满 / 谜团揭示 / 高潮阶段"));
        assert!(params["narrative_blueprint_block"].contains("当前阶段要让核心矛盾正面碰撞"));
        assert!(params["story_creation_brief_block"].contains("本章主打谜团揭晓前夜"));
    }

    #[test]
    fn should_build_chapter_story_runtime_blocks_from_chinese_aliases() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(6, "第六章", Some("冲突加压"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                creative_mode: Some("钩子".to_string()),
                story_focus: Some("冲突".to_string()),
                plot_stage: Some("高潮".to_string()),
                ..ChapterGenerationPromptOverrides::default()
            },
        );

        assert!(params["creative_mode_block"].contains("【创作模式】当前采用“钩子优先”"));
        assert!(params["creative_mode_block"].contains("开场尽快抛出异常、任务或危险"));
        assert!(params["story_focus_block"].contains("【结构侧重点】当前优先“冲突升级”"));
        assert!(params["story_focus_block"].contains("优先写出目标受阻、局面恶化、选择更难的过程"));
        assert!(params["narrative_blueprint_block"]
            .contains("【结构蓝图】本轮按“钩子优先 / 冲突升级 / 高潮阶段”组织章节节拍"));
        assert!(params["narrative_blueprint_block"].contains("尾段优先保留信息缺口"));
        assert!(params["narrative_blueprint_block"].contains("重点避免：不要只堆钩子和异常"));
        assert!(params["story_objective_card_block"].contains("【章节目标卡】"));
        assert!(params["story_objective_card_block"].contains("阻力要逼近正面碰撞"));
        assert!(params["story_objective_card_block"].contains("转折要接近核心碰撞点"));
        assert!(params["story_result_card_block"].contains("【章节结果卡】"));
        assert!(params["story_result_card_block"].contains("逼近或触发正面碰撞"));
        assert!(params["story_payoff_chain_card_block"].contains("【章节爽点回收卡】"));
        assert!(
            params["story_payoff_chain_card_block"].contains("高潮阶段优先回收最值钱的承诺和冲突")
        );
        assert!(params["story_rule_grounding_card_block"].contains("【章节设定落地卡】"));
        assert!(params["story_rule_grounding_card_block"]
            .contains("规则的代价、限制或反噬要把冲突抬高"));
        assert!(params["story_information_release_card_block"].contains("【章节信息投放卡】"));
        assert!(params["story_information_release_card_block"]
            .contains("不要在高潮关键碰撞前后连续长讲设定"));
        assert!(params["story_emotion_landing_card_block"].contains("【章节情绪落点卡】"));
        assert!(params["story_emotion_landing_card_block"].contains("高潮阶段情绪要跟着碰撞一起爆"));
        assert!(params["story_action_rendering_card_block"].contains("【章节动作显影卡】"));
        assert!(params["story_action_rendering_card_block"].contains("让最该爆的地方直接哑火"));
        assert!(params["story_summary_tone_control_card_block"].contains("【章节总结腔抑制卡】"));
        assert!(
            params["story_summary_tone_control_card_block"].contains("把现场冲击改写成作者感悟")
        );
        assert!(params["story_repetition_control_card_block"].contains("【章节重复压缩卡】"));
        assert!(
            params["story_repetition_control_card_block"].contains("高潮阶段少复盘、少重复解释")
        );
        assert!(params["story_viewpoint_discipline_card_block"].contains("【章节视角纪律卡】"));
        assert!(
            params["story_viewpoint_discipline_card_block"].contains("不要在高潮现场频繁切镜头")
        );
        assert!(params["story_dialogue_advancement_card_block"].contains("【章节对白推进卡】"));
        assert!(params["story_dialogue_advancement_card_block"]
            .contains("不要在高潮对白里长篇复盘前情"));
        assert!(params["story_opening_hook_card_block"].contains("【章节开篇抓力卡】"));
        assert!(params["story_opening_hook_card_block"].contains("高潮阶段的开篇要延续既有高压"));
        assert!(params["story_opening_hook_card_block"].contains("开篇前 20%-25%"));
        assert!(params["story_execution_checklist_block"].contains("【章节执行清单】"));
        assert!(params["story_execution_checklist_block"]
            .contains("高潮阶段开场尽快把人物推到主碰撞现场"));
        assert!(params["story_scene_anchor_card_block"].contains("【章节场景调度卡】"));
        assert!(
            params["story_scene_anchor_card_block"].contains("高潮阶段镜头尽量贴近最核心的碰撞点")
        );
        assert!(params["story_scene_density_card_block"].contains("【章节场景密度卡】"));
        assert!(params["story_scene_density_card_block"].contains("高潮阶段要提高现场化比例"));
        assert!(params["story_repetition_risk_block"].contains("【章节重复风险卡】"));
        assert!(params["story_repetition_risk_block"].contains("高潮阶段不要反复假装要碰撞"));
        assert!(params["story_acceptance_card_block"].contains("【章节验收卡】"));
        assert!(params["story_acceptance_card_block"].contains("高潮阶段验收重点"));
        assert!(params["story_cliffhanger_card_block"].contains("【章节结尾悬停卡】"));
        assert!(params["story_cliffhanger_card_block"].contains("高潮阶段的结尾要保持冲击余震"));
        assert!(params["story_character_arc_card_block"].contains("【章节角色弧光卡】"));
        assert!(params["story_character_arc_card_block"].contains("高潮阶段要逼出人物真正底线"));

        let contract = &params["quality_contract_block"];
        let creative_index = contract.find("【创作模式】").expect("creative block");
        let story_index = contract.find("【结构侧重点】").expect("story focus block");
        let blueprint_index = contract.find("【结构蓝图】").expect("blueprint block");
        let objective_index = contract.find("【章节目标卡】").expect("objective card");
        let result_index = contract.find("【章节结果卡】").expect("result card");
        let payoff_index = contract.find("【章节爽点回收卡】").expect("payoff card");
        let rule_index = contract
            .find("【章节设定落地卡】")
            .expect("rule grounding card");
        let information_index = contract
            .find("【章节信息投放卡】")
            .expect("information release card");
        let emotion_index = contract
            .find("【章节情绪落点卡】")
            .expect("emotion landing card");
        let action_index = contract
            .find("【章节动作显影卡】")
            .expect("action rendering card");
        let summary_tone_index = contract
            .find("【章节总结腔抑制卡】")
            .expect("summary tone control card");
        let repetition_index = contract
            .find("【章节重复压缩卡】")
            .expect("repetition control card");
        let viewpoint_index = contract
            .find("【章节视角纪律卡】")
            .expect("viewpoint discipline card");
        let dialogue_index = contract
            .find("【章节对白推进卡】")
            .expect("dialogue advancement card");
        let opening_index = contract
            .find("【章节开篇抓力卡】")
            .expect("opening hook card");
        let execution_index = contract
            .find("【章节执行清单】")
            .expect("execution checklist block");
        let scene_anchor_index = contract
            .find("【章节场景调度卡】")
            .expect("scene anchor card");
        let scene_density_index = contract
            .find("【章节场景密度卡】")
            .expect("scene density card");
        let repetition_risk_index = contract
            .find("【章节重复风险卡】")
            .expect("repetition risk block");
        let acceptance_index = contract.find("【章节验收卡】").expect("acceptance card");
        let cliffhanger_index = contract
            .find("【章节结尾悬停卡】")
            .expect("cliffhanger card");
        let character_arc_index = contract
            .find("【章节角色弧光卡】")
            .expect("character arc card");
        assert!(creative_index < story_index);
        assert!(story_index < blueprint_index);
        assert!(blueprint_index < objective_index);
        assert!(objective_index < result_index);
        assert!(result_index < payoff_index);
        assert!(payoff_index < rule_index);
        assert!(rule_index < information_index);
        assert!(information_index < emotion_index);
        assert!(emotion_index < action_index);
        assert!(action_index < summary_tone_index);
        assert!(summary_tone_index < repetition_index);
        assert!(repetition_index < viewpoint_index);
        assert!(viewpoint_index < dialogue_index);
        assert!(dialogue_index < opening_index);
        assert!(opening_index < execution_index);
        assert!(execution_index < scene_anchor_index);
        assert!(scene_anchor_index < scene_density_index);
        assert!(scene_density_index < repetition_risk_index);
        assert!(repetition_risk_index < acceptance_index);
        assert!(acceptance_index < cliffhanger_index);
        assert!(cliffhanger_index < character_arc_index);
    }

    #[test]
    fn should_fallback_to_project_prompt_defaults_when_overrides_are_missing() {
        let mut project_model = build_project("one-to-many");
        project_model.narrative_perspective = Some("全知视角".to_string());
        project_model.default_creative_mode = Some("hook".to_string());
        project_model.default_story_focus = Some("escalate_conflict".to_string());
        project_model.default_plot_stage = Some("development".to_string());
        project_model.default_story_creation_brief = Some("项目默认总控".to_string());
        project_model.default_quality_preset = Some("plot_drive".to_string());
        project_model.default_quality_notes = Some("强调推进".to_string());
        let chapter_model = build_chapter(6, "第六章", Some("冲突加压"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("narrative_perspective").map(String::as_str),
            Some("全知视角")
        );
        assert_eq!(
            params.get("creative_mode").map(String::as_str),
            Some("hook")
        );
        assert_eq!(
            params.get("story_focus").map(String::as_str),
            Some("escalate_conflict")
        );
        assert_eq!(
            params.get("plot_stage").map(String::as_str),
            Some("development")
        );
        assert_eq!(
            params.get("story_creation_brief").map(String::as_str),
            Some("项目默认总控")
        );
        assert_eq!(
            params.get("quality_preset").map(String::as_str),
            Some("plot_drive")
        );
        assert_eq!(
            params.get("quality_notes").map(String::as_str),
            Some("强调推进")
        );
        assert!(params["creative_mode_block"].contains("钩子优先"));
        assert!(params["story_focus_block"].contains("冲突升级"));
        assert!(params["narrative_blueprint_block"].contains("发展阶段"));
    }

    #[test]
    fn should_keep_repair_blocks_empty_when_repair_inputs_are_missing() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(7, "第七章", Some("修复节奏"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2600,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("story_repair_summary").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_repair_targets").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_preserve_strengths").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_repair_target_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_repair_diagnostic_block")
                .map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn should_build_repair_blocks_from_prompt_overrides() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(8, "第八章", Some("修复支线"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3000,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                narrative_perspective: None,
                creative_mode: None,
                story_focus: None,
                plot_stage: None,
                story_creation_brief: None,
                quality_preset: None,
                quality_notes: None,
                web_research_enabled: false,
                web_research_query: None,
                story_repair_summary: Some("上一章中段节奏拖慢，需要重新压缩".to_string()),
                story_repair_targets: vec!["缩短铺垫".to_string(), "提前冲突触发".to_string()],
                story_preserve_strengths: vec!["角色声音".to_string(), "悬念尾钩".to_string()],
            },
        );

        assert_eq!(
            params.get("story_repair_summary").map(String::as_str),
            Some("上一章中段节奏拖慢，需要重新压缩")
        );
        assert_eq!(
            params.get("story_repair_targets").map(String::as_str),
            Some("缩短铺垫；提前冲突触发")
        );
        assert_eq!(
            params.get("story_preserve_strengths").map(String::as_str),
            Some("角色声音；悬念尾钩")
        );
        assert!(params["story_repair_target_block"].contains("需要修复：缩短铺垫；提前冲突触发"));
        assert!(params["story_repair_target_block"].contains("必须保留：角色声音；悬念尾钩"));
        assert!(
            params["story_repair_diagnostic_block"].contains("上一章中段节奏拖慢，需要重新压缩")
        );
        assert!(
            params["story_repair_diagnostic_block"].contains("本章修复项：缩短铺垫；提前冲突触发")
        );
        assert!(params["story_repair_diagnostic_block"].contains("保留优势：角色声音；悬念尾钩"));
    }

    #[test]
    fn should_keep_web_research_block_empty_when_not_enabled() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(9, "第九章", Some("推进调查"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2600,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("web_research_query").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("web_research_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_creation_brief_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_objective_card_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_result_card_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_payoff_chain_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_rule_grounding_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_information_release_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_emotion_landing_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_action_rendering_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_summary_tone_control_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_repetition_control_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_viewpoint_discipline_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_dialogue_advancement_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_opening_hook_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_execution_checklist_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_scene_anchor_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_scene_density_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_repetition_risk_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_acceptance_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_cliffhanger_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_character_arc_card_block")
                .map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn should_build_web_research_block_when_enabled() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(10, "第十章", Some("收束线索"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                narrative_perspective: None,
                creative_mode: None,
                story_focus: None,
                plot_stage: None,
                story_creation_brief: None,
                quality_preset: None,
                quality_notes: None,
                web_research_enabled: true,
                web_research_query: Some("晚清漕运与江南水路行会".to_string()),
                story_repair_summary: None,
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
            },
        );

        assert_eq!(
            params.get("web_research_query").map(String::as_str),
            Some("晚清漕运与江南水路行会")
        );
        assert!(params["web_research_block"].contains("已请求联网检索"));
        assert!(params["web_research_block"].contains("晚清漕运与江南水路行会"));
        assert!(params["story_creation_brief_block"].contains("晚清漕运与江南水路行会"));
    }

    #[test]
    fn should_surface_external_research_assets_from_provider_payload() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(11, "第十一章", Some("追查账册"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            PromptContextProviderPayload {
                recent_chapters_context: String::new(),
                previous_chapter_summary: String::new(),
                chapter_careers: "[]".to_string(),
                characters_info: "[]".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: "[]".to_string(),
                research_query: "晚清漕运夜航避税路线".to_string(),
                research_assets: "[]".to_string(),
                external_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"晚清漕运夜航避税路线\"}]"
                        .to_string(),
                reference_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"晚清漕运夜航避税路线\"}]"
                        .to_string(),
                mcp_references: "[]".to_string(),
            },
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("research_query").map(String::as_str),
            Some("晚清漕运夜航避税路线")
        );
        assert!(params["quality_external_assets_block"].contains("晚清漕运夜航避税路线"));
        assert!(params["quality_generation_block"].contains("章节生成质量基线"));
        assert!(params["quality_checker_block"].contains("章节质检口径"));
        assert!(params["quality_reviser_block"].contains("章节修订口径"));
        assert!(params["quality_mcp_guard_block"].contains("summary_only=true"));
        assert!(params["reference_assets"].contains("web_research_query"));
    }

    #[test]
    fn should_build_rust_quality_runtime_contract_from_prompt_params() {
        let mut project_model = build_project("one-to-one");
        project_model.default_quality_preset = Some("plot_drive".to_string());
        project_model.default_quality_notes =
            Some("提前冲突触发；压缩解释\n- 提前冲突触发".to_string());
        let chapter_model = build_chapter(12, "第十二章", Some("夜航追账"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            PromptContextProviderPayload {
                recent_chapters_context: String::new(),
                previous_chapter_summary: String::new(),
                chapter_careers: "[]".to_string(),
                characters_info: "[]".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: "[]".to_string(),
                research_query: String::new(),
                research_assets: "[]".to_string(),
                external_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"漕运夜航税卡绕行线\"}]"
                        .to_string(),
                reference_assets: "[]".to_string(),
                mcp_references: "MCP 摘要能力：只作参考".to_string(),
            },
            &ChapterGenerationPromptOverrides::default(),
        );

        assert!(params["quality_generation_protocol_block"].contains("rule_v3_quality_block"));
        assert!(params["quality_json_protocol_block"].contains("统一JSON协议护栏"));
        assert!(params["quality_preference_block"].contains("强情节回报"));
        assert!(params["quality_preference_block"].contains("补充偏好："));
        assert!(params["quality_preference_block"].contains("提前冲突触发"));
        assert_eq!(
            params["quality_preference_block"]
                .matches("提前冲突触发")
                .count(),
            1
        );
        assert!(params["quality_contract_block"].contains("<quality_contract priority=\"P0\">"));
        assert!(params["quality_contract_block"].contains("章节生成质量基线"));
        assert!(params["quality_contract_block"].contains("统一协议护栏"));
        assert!(params["quality_contract_block"].contains("漕运夜航税卡绕行线"));
        assert!(params["quality_mcp_references_block"].contains("MCP 摘要能力"));
    }

    #[test]
    fn should_inject_quality_contract_into_rendered_chapter_prompt() {
        let mut project_model = build_project("one-to-one");
        project_model.default_quality_preset = Some("immersive".to_string());
        let chapter_model = build_chapter(13, "第十三章", Some("潜入税卡"), None, None);

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3000,
            PromptContextProviderPayload {
                recent_chapters_context: String::new(),
                previous_chapter_summary: String::new(),
                chapter_careers: "[]".to_string(),
                characters_info: "[]".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: "[]".to_string(),
                research_query: String::new(),
                research_assets: "[]".to_string(),
                external_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"水路税卡换班规律\"}]"
                        .to_string(),
                reference_assets: "[]".to_string(),
                mcp_references: String::new(),
            },
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        let fusion_contract_index = prompt
            .find("</fusion_contract>")
            .expect("chapter template should keep fusion contract");
        let quality_contract_index = prompt
            .find("<quality_contract priority=\"P0\">")
            .expect("quality contract should be injected");

        assert!(quality_contract_index > fusion_contract_index);
        assert!(prompt.contains("章节生成质量基线"));
        assert!(prompt.contains("统一协议护栏"));
        assert!(prompt.contains("沉浸场景感"));
        assert!(prompt.contains("水路税卡换班规律"));
    }
}
