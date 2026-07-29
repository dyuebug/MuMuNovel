use serde_json::Value;

pub(crate) mod contract_prepare_owner;
pub(crate) mod prompt_prepare_owner;
pub(crate) mod request_prepare_owner;
#[cfg(test)]
pub(crate) use self::prompt_prepare_owner::{
    build_regeneration_prompt, prepare_partial_regeneration_input, PreparedPartialRegenerationInput,
};
pub(crate) use self::prompt_prepare_owner::{
    prepare_chapter_regeneration_stream, prepare_partial_regeneration_stream,
    FullChapterRegenerationStreamInput, PartialChapterRegenerationStreamInput,
};
pub(crate) use self::request_prepare_owner::{
    build_full_chapter_regeneration_stream_request_from_route_payload,
    build_partial_regeneration_stream_workflow_request_from_route_payload,
    validate_full_chapter_regeneration_stream_request_bounds,
    validate_partial_regeneration_stream_request_bounds, BuildRegenerationAiServiceError,
    FullChapterRegenerationStreamRequest, FullChapterRegenerationStreamRouteRequest,
    PartialRegenerationStreamRouteRequest, PartialRegenerationStreamWorkflowRequest,
    PreparePartialRegenerationError, PreparePartialRegenerationStreamError,
};
#[cfg(test)]
pub(crate) use self::request_prepare_owner::{
    build_partial_length_requirement, calculate_partial_target_words, PartialRegenerationLengthMode,
};

const MIN_REGENERATION_TARGET_WORD_COUNT: i64 = 500;
const MAX_REGENERATION_TARGET_WORD_COUNT: i64 = 10_000;
const MAX_REGENERATION_STORY_CREATION_BRIEF_LENGTH: usize = 1200;
const MAX_REGENERATION_QUALITY_NOTES_LENGTH: usize = 600;
const MAX_REGENERATION_WEB_RESEARCH_QUERY_LENGTH: usize = 500;
const REGENERATION_CREATIVE_MODE_VALUES: &[&str] = &[
    "balanced",
    "hook",
    "emotion",
    "suspense",
    "relationship",
    "payoff",
];
const REGENERATION_STORY_FOCUS_VALUES: &[&str] = &[
    "advance_plot",
    "deepen_character",
    "escalate_conflict",
    "reveal_mystery",
    "relationship_shift",
    "foreshadow_payoff",
];
const REGENERATION_PLOT_STAGE_VALUES: &[&str] = &["development", "climax", "ending"];
const REGENERATION_QUALITY_PRESET_VALUES: &[&str] = &[
    "balanced",
    "plot_drive",
    "immersive",
    "emotion_drama",
    "clean_prose",
];
const MIN_PARTIAL_REGENERATION_CONTEXT_CHARS: usize = 100;
const MAX_PARTIAL_REGENERATION_CONTEXT_CHARS: usize = 2000;
const MIN_PARTIAL_REGENERATION_TARGET_WORD_COUNT: usize = 10;
const MAX_PARTIAL_REGENERATION_TARGET_WORD_COUNT: usize = 5000;
const MAX_PARTIAL_REGENERATION_USER_INSTRUCTIONS_LENGTH: usize = 1000;
const MAX_PARTIAL_REGENERATION_WEB_RESEARCH_QUERY_LENGTH: usize = 500;

pub(crate) fn build_chapter_regeneration_prepare_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_regeneration_prepare_service",
        "scope": "full_and_partial_regeneration_request_prompt_prepare_owner",
        "python_source_map": [],
        "shared_owner_source_maps": {
            "prompt_owner": [],
            "research_payload_owner": []
        },
        "rust_owner_map": [
            "backend-rs/src/services/chapter_regeneration_prepare_service.rs",
            "backend-rs/src/services/chapter_regeneration_prepare_service/contract_prepare_owner.rs",
            "backend-rs/src/services/chapter_regeneration_prepare_service/prompt_prepare_owner.rs",
            "backend-rs/src/services/chapter_regeneration_prepare_service/request_prepare_owner.rs",
            "backend-rs/src/services/chapter_regeneration_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_generation_prompt_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service/research_payload_owner.rs",
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs",
            "backend-rs/src/services/settings_service.rs",
            "backend-rs/src/services/writing_style_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_full_chapter_regeneration_stream_request_from_route_payload",
                "validate_full_chapter_regeneration_stream_request_bounds",
                "build_partial_regeneration_stream_workflow_request_from_route_payload",
                "validate_partial_regeneration_stream_request_bounds",
                "build_full_chapter_regeneration_contract_snapshot",
                "build_partial_chapter_regeneration_contract_snapshot",
                "build_regeneration_prompt",
                "prepare_partial_regeneration_input",
                "prepare_chapter_regeneration_stream",
                "prepare_partial_regeneration_stream",
                "build_regeneration_ai_service",
                "build_role_aware_regeneration_execution_config",
                "load_partial_style_content"
            ],
            "generation_execution_policy": {
                "role": "writer",
                "intent_kinds": [
                    "chapter_regenerate",
                    "chapter_partial_regenerate"
                ],
                "config_owner": "prepare_role_aware_generation_execution_config_with_provider_payload",
                "provider_payload_precedence_preserved": true,
                "partial_max_tokens_override_preserved": true,
                "legacy_builder_preserved": "build_regeneration_ai_service"
            },
            "full_request_fields": [
                "target_word_count",
                "custom_instructions",
                "selected_suggestion_indices",
                "focus_areas",
                "story_creation_brief",
                "quality_notes",
                "story_repair_summary",
                "creative_mode",
                "story_focus",
                "plot_stage",
                "quality_preset",
                "enable_web_research",
                "web_research_query",
                "preserve_elements",
                "story_repair_targets",
                "story_preserve_strengths"
            ],
            "partial_request_fields": [
                "selected_text",
                "start_position",
                "end_position",
                "user_instructions",
                "context_chars",
                "style_id",
                "length_mode",
                "target_word_count",
                "enable_web_research",
                "web_research_query"
            ],
            "full_bounds": {
                "target_word_count_min": MIN_REGENERATION_TARGET_WORD_COUNT,
                "target_word_count_max": MAX_REGENERATION_TARGET_WORD_COUNT,
                "story_creation_brief_max": MAX_REGENERATION_STORY_CREATION_BRIEF_LENGTH,
                "quality_notes_max": MAX_REGENERATION_QUALITY_NOTES_LENGTH,
                "web_research_query_max": MAX_REGENERATION_WEB_RESEARCH_QUERY_LENGTH
            },
            "partial_bounds": {
                "context_chars_min": MIN_PARTIAL_REGENERATION_CONTEXT_CHARS,
                "context_chars_max": MAX_PARTIAL_REGENERATION_CONTEXT_CHARS,
                "target_word_count_min": MIN_PARTIAL_REGENERATION_TARGET_WORD_COUNT,
                "target_word_count_max": MAX_PARTIAL_REGENERATION_TARGET_WORD_COUNT,
                "user_instructions_max": MAX_PARTIAL_REGENERATION_USER_INSTRUCTIONS_LENGTH,
                "web_research_query_max": MAX_PARTIAL_REGENERATION_WEB_RESEARCH_QUERY_LENGTH
            },
            "choice_fields": {
                "creative_mode": REGENERATION_CREATIVE_MODE_VALUES,
                "story_focus": REGENERATION_STORY_FOCUS_VALUES,
                "plot_stage": REGENERATION_PLOT_STAGE_VALUES,
                "quality_preset": REGENERATION_QUALITY_PRESET_VALUES
            },
            "prompt_inputs": [
                "chapter content and summary",
                "prompt context provider payload",
                "previous chapter context",
                "custom instructions",
                "selected suggestions and focus areas",
                "story creation brief",
                "quality notes and story repair summary",
                "web research payload",
                "preserve elements"
            ],
            "partial_prepare_policy": [
                "selected text falls back to chapter slice",
                "context window is clamped around selected range",
                "length mode resolves default/shorten/expand/custom target words",
                "style content is optional",
                "max tokens are clamped between 500 and 8000",
                "web research payload is injected when available"
            ],
            "stream_prepare_policy": [
                "load settings-backed AI service",
                "build full regeneration prompt before stream launch",
                "load optional writing style for partial regeneration",
                "materialize PartialChapterRegenerationStreamInput"
            ],
            "error_contract": [
                "BuildRegenerationAiServiceError",
                "PreparePartialRegenerationError",
                "PreparePartialRegenerationStreamError"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_regeneration_prepare_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_regeneration_routes::regenerate_chapter_stream",
            "chapter_regeneration_routes::partial_regenerate_stream",
            "chapter_regeneration_stream_workflow_service",
            "chapter-regeneration-full-stream-business-rust",
            "chapter-regeneration-partial-stream-business-rust"
        ],
        "rollback_boundary": {
            "python_source_map": "chapter_regeneration_prepare_python_source_map",
            "python_fallback_removal_ready": true,
            "approval_required": "separate shared prompt or research owner closeout outside direct regeneration prepare package"
        },
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-chapter-regeneration-owner",
            "regeneration_manifest_probe_count": 13,
            "rust_manifest_probe_count": 13,
            "python_fallback_probe_count": 0,
            "full_request_owner": "build_full_chapter_regeneration_stream_request_from_route_payload",
            "full_bounds_owner": "validate_full_chapter_regeneration_stream_request_bounds",
            "partial_request_owner": "build_partial_regeneration_stream_workflow_request_from_route_payload",
            "partial_bounds_owner": "validate_partial_regeneration_stream_request_bounds",
            "prompt_owner": "build_regeneration_prompt",
            "partial_prepare_owner": "prepare_partial_regeneration_input",
            "full_stream_prepare_owner": "prepare_chapter_regeneration_stream",
            "partial_stream_prepare_owner": "prepare_partial_regeneration_stream",
            "ai_service_owner": "build_regeneration_ai_service",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "separate_shared_prompt_or_research_owner_closeout_outside_direct_regeneration_prepare_package",
            "status": "rust_chapter_regeneration_prepare_owner_direct_package_closed_out"
        }
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use serde_json::Value;

    use crate::models::chapter;
    use crate::services::chapter_generation_prompt_service::{
        build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
    };

    use super::{
        build_chapter_regeneration_prepare_owner_contract,
        build_full_chapter_regeneration_stream_request_from_route_payload,
        build_partial_length_requirement,
        build_partial_regeneration_stream_workflow_request_from_route_payload,
        build_regeneration_prompt, calculate_partial_target_words,
        prepare_partial_regeneration_input, BuildRegenerationAiServiceError,
        FullChapterRegenerationStreamRequest, FullChapterRegenerationStreamRouteRequest,
        PartialRegenerationLengthMode, PartialRegenerationStreamRouteRequest,
        PartialRegenerationStreamWorkflowRequest, PreparePartialRegenerationError,
        PreparedPartialRegenerationInput, MAX_PARTIAL_REGENERATION_CONTEXT_CHARS,
        MIN_REGENERATION_TARGET_WORD_COUNT,
    };

    fn chapter_with_content(content: &str) -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            title: "测试章节".to_string(),
            chapter_number: 1,
            content: Some(content.to_string()),
            summary: None,
            word_count: content.chars().count() as i32,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    fn valid_prepared_partial_input(
        result: Result<PreparedPartialRegenerationInput, PreparePartialRegenerationError>,
    ) -> PreparedPartialRegenerationInput {
        match result {
            Ok(prepared) => prepared,
            Err(_) => panic!("partial input should be valid"),
        }
    }

    fn valid_partial_regeneration_workflow_request() -> PartialRegenerationStreamWorkflowRequest {
        PartialRegenerationStreamWorkflowRequest {
            selected_text: "选中文本".to_string(),
            start_position: 1,
            end_position: 3,
            context_chars: Some(500),
            user_instructions: "有效指令".to_string(),
            length_mode: Some("similar".to_string()),
            target_word_count: Some(120),
            style_id: None,
            enable_web_research: None,
            web_research_query: None,
        }
    }

    fn regeneration_provider_payload() -> PromptContextProviderPayload {
        PromptContextProviderPayload {
            recent_chapters_context: "【最近章节规划】\n第三章追查漕运税卡".to_string(),
            previous_chapter_summary: "上一章发现账册缺页".to_string(),
            chapter_careers: "【职业】\n主职业: 漕帮账房".to_string(),
            characters_info: "【角色】\n沈三\n当前状态: 起疑".to_string(),
            foreshadow_reminders: "【伏笔提醒】\n- 夜航税卡".to_string(),
            relevant_memories: "【相关记忆】\n- 码头旧案".to_string(),
            research_query: String::new(),
            research_assets: "[]".to_string(),
            external_assets: "[{\"kind\":\"web\",\"summary\":\"夜航税卡协商\"}]".to_string(),
            reference_assets: "[{\"kind\":\"web\",\"summary\":\"夜航税卡协商\"}]".to_string(),
            mcp_references: String::new(),
        }
    }

    #[test]
    fn should_build_regeneration_prompt_with_default_fields() {
        let chapter = chapter_with_content("原始正文");
        let route_request = FullChapterRegenerationStreamRouteRequest::default();
        let request =
            build_full_chapter_regeneration_stream_request_from_route_payload(route_request);
        let prompt = build_regeneration_prompt(
            &chapter,
            &request,
            &build_placeholder_prompt_context_provider_payload(),
            None,
            None,
            None,
        );

        assert!(prompt.contains("章节标题：测试章节"));
        assert!(prompt.contains("章节编号：1"));
        assert!(prompt.contains("目标字数：3000"));
        assert!(prompt.contains("原章节内容：\n原始正文"));
        assert!(prompt.contains("保留结构：false"));
        assert!(prompt.contains("保留人物特征：true"));
    }

    #[test]
    fn should_keep_full_request_default_equivalent_to_route_default() {
        let route_default = build_full_chapter_regeneration_stream_request_from_route_payload(
            FullChapterRegenerationStreamRouteRequest::default(),
        );
        let request_default = FullChapterRegenerationStreamRequest::default();

        assert_eq!(request_default, route_default);
        assert!(request_default.preserve_character_traits());
        assert!(!request_default.preserve_structure());
        assert!(!request_default.auto_apply());
    }

    #[test]
    fn should_build_regeneration_prompt_with_explicit_fields() {
        let chapter = chapter_with_content("原始正文");
        let route_request = FullChapterRegenerationStreamRouteRequest {
            modification_source: None,
            target_word_count: Some(1800),
            custom_instructions: Some("强化冲突".to_string()),
            selected_suggestion_indices: vec![Value::from(1), Value::from("skip"), Value::from(3)],
            focus_areas: vec![Value::from("节奏"), Value::from(7), Value::from("人物")],
            story_creation_brief: Some("总控说明".to_string()),
            quality_notes: Some("质量偏好".to_string()),
            story_repair_summary: Some("修复摘要".to_string()),
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("climax".to_string()),
            quality_preset: Some("balanced".to_string()),
            enable_web_research: Some(true),
            web_research_query: Some("晚清漕运夜航与税卡协商".to_string()),
            preserve_elements: Some(serde_json::json!({
                "preserve_structure": true,
                "preserve_dialogues": ["对白A", "对白B"],
                "preserve_plot_points": ["转折A"],
                "preserve_character_traits": false
            })),
            story_repair_targets: vec![Value::from("目标A"), Value::from("目标B")],
            story_preserve_strengths: vec![Value::from("优势A")],
            style_id: None,
            version_note: None,
            auto_apply: None,
        };
        let request =
            build_full_chapter_regeneration_stream_request_from_route_payload(route_request);
        let prompt = build_regeneration_prompt(
            &chapter,
            &request,
            &build_placeholder_prompt_context_provider_payload(),
            None,
            None,
            None,
        );

        assert!(prompt.contains("目标字数：1800"));
        assert!(prompt.contains("用户修改要求：\n强化冲突"));
        assert!(prompt.contains("选中建议索引：1, 3"));
        assert!(prompt.contains("重点优化方向：节奏、人物"));
        assert!(prompt.contains("创作模式：hook"));
        assert!(prompt.contains("保留结构：true"));
        assert!(prompt.contains("保留对话：对白A、对白B"));
        assert!(prompt.contains("保留剧情点：转折A"));
        assert!(prompt.contains("保留人物特征：false"));
        assert!(prompt.contains("修复目标：目标A、目标B"));
        assert!(prompt.contains("保留优势：优势A"));
    }

    #[test]
    fn should_normalize_full_regeneration_fields_like_python_schema() {
        let request = build_full_chapter_regeneration_stream_request_from_route_payload(
            FullChapterRegenerationStreamRouteRequest {
                target_word_count: Some(2200),
                custom_instructions: Some(" 强化冲突 ".to_string()),
                story_creation_brief: Some(" 总控说明 ".to_string()),
                quality_notes: Some(" 质量偏好 ".to_string()),
                story_repair_summary: Some(" 修复摘要 ".to_string()),
                creative_mode: Some(" hook ".to_string()),
                story_focus: Some(" advance_plot ".to_string()),
                plot_stage: Some(" development ".to_string()),
                quality_preset: Some(" plot_drive ".to_string()),
                web_research_query: Some(" 晚清漕运 ".to_string()),
                ..Default::default()
            },
        );

        assert_eq!(request.target_word_count(), 2200);
        assert_eq!(request.custom_instructions(), "强化冲突");
        assert_eq!(request.story_creation_brief(), "总控说明");
        assert_eq!(request.quality_notes(), "质量偏好");
        assert_eq!(request.story_repair_summary(), "修复摘要");
        assert_eq!(request.creative_mode(), "hook");
        assert_eq!(request.story_focus(), "advance_plot");
        assert_eq!(request.quality_preset(), "plot_drive");
        assert_eq!(request.web_research_query(), Some("晚清漕运"));
        request
            .validate_request_bounds()
            .expect("normalized python regeneration request fields should pass");
    }

    #[test]
    fn should_convert_blank_full_regeneration_fields_to_none() {
        let request = build_full_chapter_regeneration_stream_request_from_route_payload(
            FullChapterRegenerationStreamRouteRequest {
                custom_instructions: Some("   ".to_string()),
                story_creation_brief: Some("\t".to_string()),
                quality_notes: Some("\n".to_string()),
                story_repair_summary: Some("   ".to_string()),
                creative_mode: Some("   ".to_string()),
                story_focus: Some("   ".to_string()),
                plot_stage: Some("   ".to_string()),
                quality_preset: Some("   ".to_string()),
                web_research_query: Some("   ".to_string()),
                ..Default::default()
            },
        );

        assert_eq!(request.custom_instructions(), "");
        assert_eq!(request.story_creation_brief(), "");
        assert_eq!(request.quality_notes(), "");
        assert_eq!(request.story_repair_summary(), "");
        assert_eq!(request.creative_mode(), "");
        assert_eq!(request.story_focus(), "");
        assert_eq!(request.quality_preset(), "");
        assert_eq!(request.web_research_query(), None);
        request
            .validate_request_bounds()
            .expect("blank python regeneration request fields normalize to None");
    }

    #[test]
    fn should_reject_full_regeneration_target_word_count_outside_python_bounds() {
        let too_low = FullChapterRegenerationStreamRequest {
            target_word_count: Some(499),
            ..Default::default()
        };
        let too_high = FullChapterRegenerationStreamRequest {
            target_word_count: Some(10_001),
            ..Default::default()
        };

        assert!(matches!(
            too_low
                .validate_request_bounds()
                .expect_err("target_word_count below python limit should fail"),
            BuildRegenerationAiServiceError::InvalidTargetWordCountTooSmall
        ));
        assert!(matches!(
            too_high
                .validate_request_bounds()
                .expect_err("target_word_count above python limit should fail"),
            BuildRegenerationAiServiceError::InvalidTargetWordCountTooLarge
        ));
    }

    #[test]
    fn should_reject_full_regeneration_invalid_choice_fields() {
        let cases = [
            (
                FullChapterRegenerationStreamRequest {
                    creative_mode: Some("too_fancy".to_string()),
                    ..Default::default()
                },
                BuildRegenerationAiServiceError::InvalidCreativeMode,
            ),
            (
                FullChapterRegenerationStreamRequest {
                    story_focus: Some("too_broad".to_string()),
                    ..Default::default()
                },
                BuildRegenerationAiServiceError::InvalidStoryFocus,
            ),
            (
                FullChapterRegenerationStreamRequest {
                    plot_stage: Some("middle".to_string()),
                    ..Default::default()
                },
                BuildRegenerationAiServiceError::InvalidPlotStage,
            ),
            (
                FullChapterRegenerationStreamRequest {
                    quality_preset: Some("max_quality".to_string()),
                    ..Default::default()
                },
                BuildRegenerationAiServiceError::InvalidQualityPreset,
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
    fn should_reject_full_regeneration_text_fields_above_python_limits() {
        let long_brief = FullChapterRegenerationStreamRequest {
            story_creation_brief: Some("a".repeat(1201)),
            ..Default::default()
        };
        let long_quality_notes = FullChapterRegenerationStreamRequest {
            quality_notes: Some("b".repeat(601)),
            ..Default::default()
        };
        let long_web_research_query = FullChapterRegenerationStreamRequest {
            web_research_query: Some("c".repeat(501)),
            ..Default::default()
        };

        assert!(matches!(
            long_brief
                .validate_request_bounds()
                .expect_err("story_creation_brief above python limit should fail"),
            BuildRegenerationAiServiceError::StoryCreationBriefTooLong
        ));
        assert!(matches!(
            long_quality_notes
                .validate_request_bounds()
                .expect_err("quality_notes above python limit should fail"),
            BuildRegenerationAiServiceError::QualityNotesTooLong
        ));
        assert!(matches!(
            long_web_research_query
                .validate_request_bounds()
                .expect_err("web_research_query above python limit should fail"),
            BuildRegenerationAiServiceError::WebResearchQueryTooLong
        ));
    }

    #[test]
    fn should_accept_full_regeneration_python_request_bounds() {
        let lower_bound_request = FullChapterRegenerationStreamRequest {
            target_word_count: Some(500),
            ..Default::default()
        };
        let upper_bound_request = FullChapterRegenerationStreamRequest {
            target_word_count: Some(10_000),
            ..Default::default()
        };
        let choice_and_text_request = FullChapterRegenerationStreamRequest {
            target_word_count: Some(3000),
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            quality_preset: Some("plot_drive".to_string()),
            story_creation_brief: Some("a".repeat(1200)),
            quality_notes: Some("b".repeat(600)),
            web_research_query: Some("c".repeat(500)),
            ..Default::default()
        };

        lower_bound_request
            .validate_request_bounds()
            .expect("python lower target word count should pass");
        upper_bound_request
            .validate_request_bounds()
            .expect("python upper target word count should pass");
        choice_and_text_request
            .validate_request_bounds()
            .expect("valid python regeneration choices and text lengths should pass");
    }

    #[test]
    fn should_build_regeneration_prompt_with_rust_owned_context_payload() {
        let chapter = chapter_with_content("原始正文");
        let request = FullChapterRegenerationStreamRequest::default();

        let prompt = build_regeneration_prompt(
            &chapter,
            &request,
            &regeneration_provider_payload(),
            Some("联网说明"),
            Some("[{\"kind\":\"web\",\"summary\":\"夜航税卡协商\"}]"),
            Some("[{\"kind\":\"web\",\"summary\":\"夜航税卡协商\"}]"),
        );

        assert!(prompt.contains("最近章节规划"));
        assert!(prompt.contains("第三章追查漕运税卡"));
        assert!(prompt.contains("上一章发现账册缺页"));
        assert!(prompt.contains("沈三"));
        assert!(prompt.contains("主职业: 漕帮账房"));
        assert!(prompt.contains("夜航税卡"));
        assert!(prompt.contains("码头旧案"));
    }

    #[test]
    fn should_build_partial_length_requirement_for_modes() {
        assert_eq!(
            build_partial_length_requirement(None, None, 100),
            "尽量保持与原文接近，原文约 100 字，目标 80-120 字"
        );
        assert_eq!(
            build_partial_length_requirement(Some("expand"), None, 100),
            "建议扩写至 120-200 字"
        );
        assert_eq!(
            build_partial_length_requirement(Some("custom"), Some(300), 100),
            "目标长度约 300 字，允许上下浮动 20%"
        );
    }

    #[test]
    fn should_calculate_partial_target_words_for_modes() {
        assert_eq!(calculate_partial_target_words(None, None, 100), 150);
        assert_eq!(
            calculate_partial_target_words(Some("expand"), None, 100),
            200
        );
        assert_eq!(
            calculate_partial_target_words(Some("custom"), Some(260), 100),
            260
        );
        assert_eq!(
            calculate_partial_target_words(Some("custom"), None, 100),
            150
        );
    }

    #[test]
    fn should_normalize_partial_regeneration_length_mode() {
        assert_eq!(
            PartialRegenerationLengthMode::normalize(None),
            PartialRegenerationLengthMode::Similar
        );
        assert_eq!(
            PartialRegenerationLengthMode::normalize(Some("expand")),
            PartialRegenerationLengthMode::Expand
        );
        assert_eq!(
            PartialRegenerationLengthMode::normalize(Some("condense")),
            PartialRegenerationLengthMode::Condense
        );
        assert_eq!(
            PartialRegenerationLengthMode::normalize(Some("custom")),
            PartialRegenerationLengthMode::Custom
        );
        assert_eq!(
            PartialRegenerationLengthMode::normalize(Some("unexpected")),
            PartialRegenerationLengthMode::Similar
        );
    }

    #[test]
    fn should_normalize_partial_regeneration_route_text_fields_like_python_schema() {
        let request = build_partial_regeneration_stream_workflow_request_from_route_payload(
            PartialRegenerationStreamRouteRequest {
                selected_text: "选中文本".to_string(),
                start_position: 1,
                end_position: 3,
                user_instructions: " 强化心理压迫 ".to_string(),
                context_chars: Some(500),
                style_id: None,
                length_mode: Some(" expand ".to_string()),
                target_word_count: Some(120),
                enable_web_research: Some(true),
                web_research_query: Some(" 晚清码头规约 ".to_string()),
            },
        );

        assert_eq!(request.user_instructions(), "强化心理压迫");
        assert_eq!(request.length_mode(), Some("expand"));
        assert_eq!(request.web_research_query(), Some("晚清码头规约"));
        request
            .validate_request_bounds()
            .expect("normalized python partial regeneration fields should pass");
    }

    #[test]
    fn should_convert_blank_partial_regeneration_optional_text_to_none() {
        let request = build_partial_regeneration_stream_workflow_request_from_route_payload(
            PartialRegenerationStreamRouteRequest {
                selected_text: "选中文本".to_string(),
                start_position: 1,
                end_position: 3,
                user_instructions: " 有效指令 ".to_string(),
                context_chars: None,
                style_id: None,
                length_mode: Some("   ".to_string()),
                target_word_count: None,
                enable_web_research: None,
                web_research_query: Some("\t".to_string()),
            },
        );

        assert_eq!(request.user_instructions(), "有效指令");
        assert_eq!(request.length_mode(), None);
        assert_eq!(request.web_research_query(), None);
        request
            .validate_request_bounds()
            .expect("blank optional partial regeneration fields should normalize to None");
    }

    #[test]
    fn should_reject_partial_regeneration_request_bounds_like_python_schema() {
        let cases = [
            (
                PartialRegenerationStreamWorkflowRequest {
                    start_position: 3,
                    end_position: 3,
                    ..valid_partial_regeneration_workflow_request()
                },
                PreparePartialRegenerationError::InvalidRange,
            ),
            (
                PartialRegenerationStreamWorkflowRequest {
                    user_instructions: String::new(),
                    ..valid_partial_regeneration_workflow_request()
                },
                PreparePartialRegenerationError::EmptyUserInstructions,
            ),
            (
                PartialRegenerationStreamWorkflowRequest {
                    user_instructions: "a".repeat(1001),
                    ..valid_partial_regeneration_workflow_request()
                },
                PreparePartialRegenerationError::UserInstructionsTooLong,
            ),
            (
                PartialRegenerationStreamWorkflowRequest {
                    context_chars: Some(99),
                    ..valid_partial_regeneration_workflow_request()
                },
                PreparePartialRegenerationError::ContextCharsTooSmall,
            ),
            (
                PartialRegenerationStreamWorkflowRequest {
                    context_chars: Some(2001),
                    ..valid_partial_regeneration_workflow_request()
                },
                PreparePartialRegenerationError::ContextCharsTooLarge,
            ),
            (
                PartialRegenerationStreamWorkflowRequest {
                    target_word_count: Some(9),
                    ..valid_partial_regeneration_workflow_request()
                },
                PreparePartialRegenerationError::TargetWordCountTooSmall,
            ),
            (
                PartialRegenerationStreamWorkflowRequest {
                    target_word_count: Some(5001),
                    ..valid_partial_regeneration_workflow_request()
                },
                PreparePartialRegenerationError::TargetWordCountTooLarge,
            ),
            (
                PartialRegenerationStreamWorkflowRequest {
                    web_research_query: Some("q".repeat(501)),
                    ..valid_partial_regeneration_workflow_request()
                },
                PreparePartialRegenerationError::WebResearchQueryTooLong,
            ),
        ];

        for (request, expected_error) in cases {
            assert_eq!(
                request
                    .validate_request_bounds()
                    .expect_err("invalid python partial regeneration boundary should fail"),
                expected_error
            );
        }
    }

    #[test]
    fn should_accept_partial_regeneration_python_request_bounds() {
        let lower_bound_request = PartialRegenerationStreamWorkflowRequest {
            context_chars: Some(100),
            target_word_count: Some(10),
            ..valid_partial_regeneration_workflow_request()
        };
        let upper_bound_request = PartialRegenerationStreamWorkflowRequest {
            context_chars: Some(2000),
            target_word_count: Some(5000),
            user_instructions: "a".repeat(1000),
            web_research_query: Some("q".repeat(500)),
            ..valid_partial_regeneration_workflow_request()
        };

        lower_bound_request
            .validate_request_bounds()
            .expect("python lower partial regeneration bounds should pass");
        upper_bound_request
            .validate_request_bounds()
            .expect("python upper partial regeneration bounds should pass");
    }

    #[test]
    fn should_resolve_partial_regeneration_length_plan_from_shared_owner() {
        let expand =
            PartialRegenerationLengthMode::normalize(Some("expand")).resolve_plan(None, 100);
        assert_eq!(expand.requirement, "建议扩写至 120-200 字");
        assert_eq!(expand.target_words, 200);

        let custom_fallback =
            PartialRegenerationLengthMode::normalize(Some("custom")).resolve_plan(None, 100);
        assert_eq!(
            custom_fallback.requirement,
            "默认按接近原文长度处理，原文约 100 字"
        );
        assert_eq!(custom_fallback.target_words, 150);
    }

    #[test]
    fn should_prepare_partial_regeneration_input_with_override_and_context() {
        let chapter = chapter_with_content("一二三四五六七八九十");

        let result = prepare_partial_regeneration_input(
            &chapter,
            "替换文本",
            2,
            5,
            2,
            "增强张力",
            Some("custom"),
            Some(120),
            Some("风格说明"),
            &regeneration_provider_payload(),
            Some("联网说明"),
            Some("[{\"title\":\"资料A\",\"summary\":\"夜航税卡协商\"}]"),
            Some("[{\"title\":\"资料A\",\"summary\":\"夜航税卡协商\"}]"),
        );
        let prepared = valid_prepared_partial_input(result);

        assert_eq!(prepared.original_word_count, 4);
        assert_eq!(prepared.target_words, 120);
        assert_eq!(prepared.selected_text, "替换文本");
        assert!(prepared.prompt.contains("原文选中片段：\n替换文本"));
        assert!(prepared.prompt.contains("前文上下文：\n一二"));
        assert!(prepared.prompt.contains("后文上下文：\n六七"));
        assert!(prepared.prompt.contains("风格说明"));
        assert!(prepared.prompt.contains("沈三"));
        assert!(prepared.prompt.contains("上一章发现账册缺页"));
        assert!(prepared.prompt.contains("联网说明"));
        assert!(prepared.prompt.contains("external_assets"));
        assert!(prepared.prompt.contains("夜航税卡协商"));
    }

    #[test]
    fn should_prepare_partial_regeneration_input_with_content_fallback_and_edge_context() {
        let chapter = chapter_with_content("一二三四五六七八九十");

        let result = prepare_partial_regeneration_input(
            &chapter,
            "  ",
            0,
            2,
            3,
            "",
            None,
            None,
            None,
            &build_placeholder_prompt_context_provider_payload(),
            None,
            None,
            None,
        );
        let prepared = valid_prepared_partial_input(result);

        assert_eq!(prepared.original_word_count, 2);
        assert_eq!(prepared.selected_text, "一二");
        assert!(prepared.prompt.contains("原文选中片段：\n一二"));
        assert!(prepared.prompt.contains("（无前文上下文）"));
        assert!(prepared.prompt.contains("后文上下文：\n三四五"));
        assert!(prepared.prompt.contains("（无额外要求）"));
    }

    #[test]
    fn should_clamp_partial_regeneration_max_tokens() {
        let chapter = chapter_with_content("一二三四五");

        let floor_result = prepare_partial_regeneration_input(
            &chapter,
            "",
            1,
            2,
            1,
            "",
            Some("custom"),
            Some(1),
            None,
            &build_placeholder_prompt_context_provider_payload(),
            None,
            None,
            None,
        );
        let floor_prepared = valid_prepared_partial_input(floor_result);

        let cap_result = prepare_partial_regeneration_input(
            &chapter,
            "",
            1,
            2,
            1,
            "",
            Some("custom"),
            Some(10_000),
            None,
            &build_placeholder_prompt_context_provider_payload(),
            None,
            None,
            None,
        );
        let cap_prepared = valid_prepared_partial_input(cap_result);

        assert_eq!(floor_prepared.target_words, 1);
        assert_eq!(floor_prepared.max_tokens, 500);
        assert_eq!(cap_prepared.target_words, 10_000);
        assert_eq!(cap_prepared.max_tokens, 8000);
    }

    #[test]
    fn should_reject_invalid_partial_regeneration_range() {
        let chapter = chapter_with_content("一二三");

        let result = prepare_partial_regeneration_input(
            &chapter,
            "",
            2,
            2,
            1,
            "",
            None,
            None,
            None,
            &build_placeholder_prompt_context_provider_payload(),
            None,
            None,
            None,
        );
        let error = match result {
            Ok(_) => panic!("empty range should be invalid"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            PreparePartialRegenerationError::InvalidRange
        ));
    }

    #[test]
    fn should_reject_empty_partial_regeneration_selection() {
        let chapter = chapter_with_content("   ");

        let result = prepare_partial_regeneration_input(
            &chapter,
            "",
            0,
            1,
            1,
            "",
            None,
            None,
            None,
            &build_placeholder_prompt_context_provider_payload(),
            None,
            None,
            None,
        );
        let error = match result {
            Ok(_) => panic!("blank selected text should be invalid"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            PreparePartialRegenerationError::EmptySelectedText
        ));
    }

    #[test]
    fn should_publish_chapter_regeneration_prepare_owner_contract() {
        let contract = build_chapter_regeneration_prepare_owner_contract();

        assert_eq!(contract["owner"], "chapter_regeneration_prepare_service");
        assert_eq!(
            contract["scope"],
            "full_and_partial_regeneration_request_prompt_prepare_owner"
        );
        assert_eq!(contract["python_source_map"].as_array().unwrap().len(), 0);
        assert_eq!(
            contract["shared_owner_source_maps"]["prompt_owner"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            contract["shared_owner_source_maps"]["research_payload_owner"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_regeneration_prepare_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][4],
            "build_full_chapter_regeneration_contract_snapshot"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][5],
            "build_partial_chapter_regeneration_contract_snapshot"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][9],
            "prepare_partial_regeneration_stream"
        );
        assert_eq!(
            contract["behavior_contract"]["generation_execution_policy"]["role"],
            "writer"
        );
        assert_eq!(
            contract["behavior_contract"]["generation_execution_policy"]["intent_kinds"],
            serde_json::json!(["chapter_regenerate", "chapter_partial_regenerate"])
        );
        assert_eq!(
            contract["behavior_contract"]["generation_execution_policy"]
                ["legacy_builder_preserved"],
            "build_regeneration_ai_service"
        );
        assert_eq!(
            contract["behavior_contract"]["full_bounds"]["target_word_count_min"],
            MIN_REGENERATION_TARGET_WORD_COUNT
        );
        assert_eq!(
            contract["behavior_contract"]["partial_bounds"]["context_chars_max"],
            MAX_PARTIAL_REGENERATION_CONTEXT_CHARS
        );
        assert_eq!(
            contract["behavior_contract"]["choice_fields"]["creative_mode"][0],
            "balanced"
        );
        assert_eq!(
            contract["behavior_contract"]["partial_prepare_policy"][4],
            "max tokens are clamped between 500 and 8000"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter_regeneration_routes::partial_regenerate_stream"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-chapter-regeneration-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["regeneration_manifest_probe_count"],
            13
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            13
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["full_request_owner"],
            "build_full_chapter_regeneration_stream_request_from_route_payload"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["partial_prepare_owner"],
            "prepare_partial_regeneration_input"
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
            "rust_chapter_regeneration_prepare_owner_direct_package_closed_out"
        );
    }
}
