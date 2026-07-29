use serde_json::{json, Value};

use crate::models::project;
use crate::services::chapter_generation_execution_contract_service::build_prompt_overrides_from_compat_options;
use crate::services::chapter_generation_runtime_service::runtime_execution_owner::build_chapter_generation_intent_overrides;
use crate::services::generation_contract_service::{
    apply_generation_intent_overrides, build_generation_contract_snapshot, GenerationContractError,
    GenerationContractSnapshotV1, GenerationIntentKind, GenerationIntentV1,
    GenerationRegenerationScope, GenerationSelection, GenerationTarget, GenerationTargetKind,
    StoryPacketV1,
};

use super::request_prepare_owner::{
    FullChapterRegenerationStreamRequest, PartialRegenerationLengthMode,
    PartialRegenerationStreamWorkflowRequest,
};

const FULL_REGENERATION_LEGACY_MODE: &str = "chapter_regenerate";
const PARTIAL_REGENERATION_LEGACY_MODE: &str = "chapter_partial_regenerate";

fn ensure_chapter_story_packet(
    project_model: &project::Model,
    story_packet: &StoryPacketV1,
) -> Result<(), GenerationContractError> {
    if story_packet.target.kind != GenerationTargetKind::Chapter {
        return Err(GenerationContractError::InvalidTarget(
            "chapter regeneration requires a chapter Story Packet target".to_string(),
        ));
    }
    if story_packet.project_id != project_model.id {
        return Err(GenerationContractError::ProjectMismatch {
            expected: story_packet.project_id.clone(),
            actual: project_model.id.clone(),
        });
    }
    Ok(())
}

fn positive_u32(value: usize, field: &str) -> Result<u32, GenerationContractError> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            GenerationContractError::InvalidTarget(format!("{field} must fit in a positive u32"))
        })
}

fn insert_non_empty_string(
    overrides: &mut std::collections::BTreeMap<String, Value>,
    key: &str,
    value: &str,
) {
    let value = value.trim();
    if !value.is_empty() {
        overrides.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn insert_non_empty_string_list(
    overrides: &mut std::collections::BTreeMap<String, Value>,
    key: &str,
    values: &[String],
) {
    if !values.is_empty() {
        overrides.insert(key.to_string(), json!(values));
    }
}

fn full_regeneration_preserve_constraints(
    request: &FullChapterRegenerationStreamRequest,
) -> Vec<String> {
    let mut constraints = Vec::new();
    if request.preserve_structure() {
        constraints.push("preserve_structure".to_string());
    }
    if !request.preserve_dialogues().is_empty() {
        constraints.push("preserve_dialogues".to_string());
    }
    if !request.preserve_plot_points().is_empty() {
        constraints.push("preserve_plot_points".to_string());
    }
    if request.preserve_character_traits() {
        constraints.push("preserve_character_traits".to_string());
    }
    if !request.story_preserve_strengths().is_empty() {
        constraints.push("preserve_story_strengths".to_string());
    }
    constraints
}

fn normalized_partial_length_mode(
    request: &PartialRegenerationStreamWorkflowRequest,
) -> &'static str {
    match PartialRegenerationLengthMode::normalize(request.length_mode()) {
        PartialRegenerationLengthMode::Similar => "similar",
        PartialRegenerationLengthMode::Expand => "expand",
        PartialRegenerationLengthMode::Condense => "condense",
        PartialRegenerationLengthMode::Custom => "custom",
    }
}

pub(crate) fn build_full_chapter_regeneration_contract_snapshot(
    project_model: &project::Model,
    story_packet: StoryPacketV1,
    request: &FullChapterRegenerationStreamRequest,
    web_research_default: bool,
) -> Result<GenerationContractSnapshotV1, GenerationContractError> {
    ensure_chapter_story_packet(project_model, &story_packet)?;
    let target_word_count =
        positive_u32(request.target_word_count() as usize, "target_word_count")?;
    let compat_options = request.compat_options_with_web_research_default(web_research_default);
    let prompt_overrides = build_prompt_overrides_from_compat_options(&compat_options);
    let mut intent_overrides = build_chapter_generation_intent_overrides(
        project_model,
        Some(target_word_count),
        &prompt_overrides,
        FULL_REGENERATION_LEGACY_MODE,
    );

    let opaque_overrides = &mut intent_overrides.creative_overrides.opaque_overrides;
    opaque_overrides.insert(
        "modification_source".to_string(),
        json!(request.modification_source()),
    );
    insert_non_empty_string_list(
        opaque_overrides,
        "selected_suggestion_indices",
        request.selected_suggestion_indices(),
    );
    insert_non_empty_string_list(opaque_overrides, "focus_areas", request.focus_areas());
    opaque_overrides.insert(
        "preserve_structure".to_string(),
        json!(request.preserve_structure()),
    );
    insert_non_empty_string_list(
        opaque_overrides,
        "preserve_dialogues",
        request.preserve_dialogues(),
    );
    insert_non_empty_string_list(
        opaque_overrides,
        "preserve_plot_points",
        request.preserve_plot_points(),
    );
    opaque_overrides.insert(
        "preserve_character_traits".to_string(),
        json!(request.preserve_character_traits()),
    );
    if let Some(style_id) = request.style_id() {
        opaque_overrides.insert("style_id".to_string(), json!(style_id));
    }

    let reason = (!request.custom_instructions().trim().is_empty())
        .then(|| request.custom_instructions().trim().to_string())
        .or_else(|| Some(request.modification_source().to_string()));
    intent_overrides.regeneration_scope = Some(GenerationRegenerationScope {
        selection: None,
        preserve_constraints: full_regeneration_preserve_constraints(request),
        reason,
    });

    let mut intent = GenerationIntentV1::new(
        GenerationIntentKind::ChapterRegenerate,
        story_packet.target.clone(),
    );
    apply_generation_intent_overrides(&mut intent, intent_overrides);
    build_generation_contract_snapshot(story_packet, intent)
}

pub(crate) fn build_partial_chapter_regeneration_contract_snapshot(
    project_model: &project::Model,
    story_packet: StoryPacketV1,
    request: &PartialRegenerationStreamWorkflowRequest,
    normalized_selected_text: String,
    normalized_target_words: usize,
    style_content: Option<&str>,
    web_research_default: bool,
) -> Result<GenerationContractSnapshotV1, GenerationContractError> {
    ensure_chapter_story_packet(project_model, &story_packet)?;
    let normalized_selected_text = normalized_selected_text.trim().to_string();
    if normalized_selected_text.is_empty() {
        return Err(GenerationContractError::InvalidTarget(
            "partial regeneration selection text must not be empty".to_string(),
        ));
    }
    let chapter_id = story_packet.target.chapter_id.clone().ok_or_else(|| {
        GenerationContractError::InvalidTarget("chapter_id is required".to_string())
    })?;
    let selection = GenerationSelection {
        start_index: request.start_position(),
        end_index: request.end_position(),
        selected_text: Some(normalized_selected_text),
    };
    let target = GenerationTarget::chapter_selection(
        story_packet.project_id.clone(),
        chapter_id,
        selection.clone(),
    );
    let target_word_count = positive_u32(normalized_target_words, "target_word_count")?;
    let compat_options = request.compat_options_with_web_research_default(web_research_default);
    let prompt_overrides = build_prompt_overrides_from_compat_options(&compat_options);
    let mut intent_overrides = build_chapter_generation_intent_overrides(
        project_model,
        Some(target_word_count),
        &prompt_overrides,
        PARTIAL_REGENERATION_LEGACY_MODE,
    );

    let opaque_overrides = &mut intent_overrides.creative_overrides.opaque_overrides;
    opaque_overrides.insert(
        "length_mode".to_string(),
        json!(normalized_partial_length_mode(request)),
    );
    opaque_overrides.insert("context_chars".to_string(), json!(request.context_chars()));
    if let Some(style_id) = request.style_id() {
        opaque_overrides.insert("style_id".to_string(), json!(style_id));
    }
    if let Some(style_content) = style_content {
        insert_non_empty_string(opaque_overrides, "style_prompt_content", style_content);
    }
    intent_overrides.regeneration_scope = Some(GenerationRegenerationScope {
        selection: Some(selection),
        preserve_constraints: vec![
            "preserve_content_outside_selection".to_string(),
            "preserve_character_and_setting_continuity".to_string(),
            "preserve_surrounding_context".to_string(),
        ],
        reason: Some(request.user_instructions().to_string()),
    });

    let mut intent =
        GenerationIntentV1::new(GenerationIntentKind::ChapterPartialRegenerate, target);
    apply_generation_intent_overrides(&mut intent, intent_overrides);
    build_generation_contract_snapshot(story_packet, intent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::chapter_regeneration_prepare_service::{
        build_full_chapter_regeneration_stream_request_from_route_payload,
        FullChapterRegenerationStreamRouteRequest,
    };
    use crate::services::generation_contract_service::validate_generation_contract_snapshot;

    fn project_model() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "测试项目".to_string(),
            description: None,
            theme: None,
            genre: Some("悬疑".to_string()),
            target_words: 100_000,
            current_words: 2_000,
            status: "chapter_generation".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 5,
            outline_mode: "standard".to_string(),
            world_time_period: None,
            world_location: None,
            world_atmosphere: None,
            world_rules: None,
            chapter_count: Some(20),
            narrative_perspective: Some("third_person".to_string()),
            character_count: 3,
            default_creative_mode: Some("balanced".to_string()),
            default_story_focus: Some("advance_plot".to_string()),
            default_plot_stage: Some("development".to_string()),
            default_story_creation_brief: Some("保持悬疑推进".to_string()),
            default_quality_preset: Some("plot_drive".to_string()),
            default_quality_notes: Some("避免信息重复".to_string()),
            created_at: Default::default(),
            updated_at: Some(Default::default()),
        }
    }

    fn chapter_story_packet() -> StoryPacketV1 {
        StoryPacketV1::new(
            "project-1",
            GenerationTarget::chapter("project-1", "chapter-1"),
        )
    }

    #[test]
    fn should_build_full_chapter_regeneration_contract_from_legacy_request() {
        let mut request = build_full_chapter_regeneration_stream_request_from_route_payload(
            FullChapterRegenerationStreamRouteRequest::default(),
        );
        request.target_word_count = Some(2400);
        request.custom_instructions = Some("强化冲突并保留关键反转".to_string());
        request.selected_suggestion_indices = vec!["2".to_string(), "4".to_string()];
        request.focus_areas = vec!["结构".to_string(), "情绪".to_string()];
        request.preserve_structure = true;
        request.preserve_dialogues = vec!["保留码头对话".to_string()];
        request.preserve_plot_points = vec!["账册反转".to_string()];
        request.story_preserve_strengths = vec!["悬念".to_string()];
        request.style_id = Some(9);

        let snapshot = build_full_chapter_regeneration_contract_snapshot(
            &project_model(),
            chapter_story_packet(),
            &request,
            false,
        )
        .expect("full regeneration contract should build");

        validate_generation_contract_snapshot(&snapshot)
            .expect("full regeneration contract should validate");
        assert_eq!(
            snapshot.generation_intent.kind,
            GenerationIntentKind::ChapterRegenerate
        );
        assert_eq!(
            snapshot.generation_intent.target.kind,
            GenerationTargetKind::Chapter
        );
        assert_eq!(snapshot.generation_intent.target_word_count, Some(2400));
        let scope = snapshot
            .generation_intent
            .regeneration_scope
            .as_ref()
            .expect("regeneration scope should exist");
        assert_eq!(scope.reason.as_deref(), Some("强化冲突并保留关键反转"));
        assert!(scope
            .preserve_constraints
            .contains(&"preserve_structure".to_string()));
        assert!(scope
            .preserve_constraints
            .contains(&"preserve_story_strengths".to_string()));
        let opaque = &snapshot
            .generation_intent
            .creative_overrides
            .opaque_overrides;
        assert_eq!(
            opaque.get("selected_suggestion_indices"),
            Some(&json!(["2", "4"]))
        );
        assert_eq!(opaque.get("focus_areas"), Some(&json!(["结构", "情绪"])));
        assert_eq!(opaque.get("style_id"), Some(&json!(9)));
        assert!(!opaque.contains_key("provider"));
        assert!(!opaque.contains_key("model"));
        assert!(!opaque.contains_key("version_note"));
        assert!(!opaque.contains_key("auto_apply"));
    }

    #[test]
    fn should_build_partial_regeneration_contract_with_typed_selection_and_constraints() {
        let request = PartialRegenerationStreamWorkflowRequest::new(
            String::new(),
            12,
            36,
            Some(600),
            "提升动作节奏".to_string(),
            Some("expand".to_string()),
            None,
            Some(7),
            Some(true),
            Some("检索码头装卸流程".to_string()),
        );

        let snapshot = build_partial_chapter_regeneration_contract_snapshot(
            &project_model(),
            chapter_story_packet(),
            &request,
            "选中的原始正文".to_string(),
            1800,
            Some("  冷峻克制的叙事风格  "),
            false,
        )
        .expect("partial regeneration contract should build");

        validate_generation_contract_snapshot(&snapshot)
            .expect("partial regeneration contract should validate");
        assert_eq!(
            snapshot.generation_intent.kind,
            GenerationIntentKind::ChapterPartialRegenerate
        );
        assert_eq!(
            snapshot.generation_intent.target.kind,
            GenerationTargetKind::ChapterSelection
        );
        let selection = snapshot
            .generation_intent
            .target
            .selection
            .as_ref()
            .expect("typed selection should exist");
        assert_eq!(selection.start_index, 12);
        assert_eq!(selection.end_index, 36);
        assert_eq!(selection.selected_text.as_deref(), Some("选中的原始正文"));
        let scope = snapshot
            .generation_intent
            .regeneration_scope
            .as_ref()
            .expect("partial regeneration scope should exist");
        assert_eq!(scope.selection.as_ref(), Some(selection));
        assert_eq!(scope.reason.as_deref(), Some("提升动作节奏"));
        assert_eq!(snapshot.generation_intent.target_word_count, Some(1800));
        let opaque = &snapshot
            .generation_intent
            .creative_overrides
            .opaque_overrides;
        assert_eq!(opaque.get("length_mode"), Some(&json!("expand")));
        assert_eq!(opaque.get("context_chars"), Some(&json!(600)));
        assert_eq!(
            opaque.get("style_prompt_content"),
            Some(&json!("冷峻克制的叙事风格"))
        );
    }

    #[test]
    fn should_keep_full_regeneration_route_defaults_digest_stable() {
        let request = build_full_chapter_regeneration_stream_request_from_route_payload(
            FullChapterRegenerationStreamRouteRequest::default(),
        );
        let first = build_full_chapter_regeneration_contract_snapshot(
            &project_model(),
            chapter_story_packet(),
            &request,
            false,
        )
        .expect("first default contract should build");
        let second = build_full_chapter_regeneration_contract_snapshot(
            &project_model(),
            chapter_story_packet(),
            &request,
            false,
        )
        .expect("second default contract should build");

        assert_eq!(first, second);
        assert_eq!(first.generation_intent.target_word_count, Some(3000));
        assert_eq!(
            first
                .generation_intent
                .regeneration_scope
                .as_ref()
                .and_then(|scope| scope.reason.as_deref()),
            Some("custom")
        );
    }

    #[test]
    fn should_reject_non_chapter_story_packet_for_regeneration() {
        let story_packet = StoryPacketV1::new(
            "project-1",
            GenerationTarget::outline("project-1", Some("outline-1".to_string())),
        );
        let error = build_full_chapter_regeneration_contract_snapshot(
            &project_model(),
            story_packet,
            &FullChapterRegenerationStreamRequest::default(),
            false,
        )
        .expect_err("outline Story Packet must not build a chapter regeneration contract");

        assert!(matches!(error, GenerationContractError::InvalidTarget(_)));
    }
}
