use std::collections::HashMap;

use crate::models::{chapter, project};
use crate::services::chapter_generation_prompt_service::{
    build_creative_mode_block, build_external_assets_block, build_narrative_blueprint_block_owner,
    build_novel_quality_prompt_blocks, build_optional_instruction_block,
    build_quality_contract_block, build_quality_generation_protocol_block,
    build_quality_json_protocol_block, build_quality_preference_block,
    build_quality_profile_payload, build_repair_diagnostic_block, build_repair_target_block,
    build_story_acceptance_card_block_owner, build_story_action_rendering_card_block_owner,
    build_story_character_arc_card_block_owner, build_story_cliffhanger_card_block_owner,
    build_story_dialogue_advancement_card_block_owner,
    build_story_emotion_landing_card_block_owner, build_story_execution_checklist_block_owner,
    build_story_focus_block, build_story_information_release_card_block_owner,
    build_story_objective_card_block_owner, build_story_opening_hook_card_block_owner,
    build_story_payoff_chain_card_block_owner, build_story_repetition_control_card_block_owner,
    build_story_repetition_risk_block_owner, build_story_result_card_block_owner,
    build_story_rule_grounding_card_block_owner, build_story_scene_anchor_card_block_owner,
    build_story_scene_density_card_block_owner, build_story_summary_tone_control_card_block_owner,
    build_story_viewpoint_discipline_card_block_owner, build_web_research_block,
    normalize_prompt_list, prompt_block_text, resolve_prompt_preference,
    ChapterGenerationPromptOverrides, PromptContextProviderPayload,
};

fn continuation_point(previous_chapter: Option<&chapter::Model>) -> String {
    previous_chapter
        .and_then(|item| item.content.clone())
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
        .chars()
        .rev()
        .take(500)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn previous_chapter_content(previous_chapter: Option<&chapter::Model>) -> String {
    previous_chapter
        .and_then(|item| item.content.clone())
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
        .chars()
        .rev()
        .take(500)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PreviousChapterPromptContext {
    pub(crate) continuation_point: String,
    pub(crate) previous_chapter_content: String,
}

pub(crate) fn build_previous_chapter_prompt_context(
    previous_chapter: Option<&chapter::Model>,
) -> PreviousChapterPromptContext {
    PreviousChapterPromptContext {
        continuation_point: continuation_point(previous_chapter),
        previous_chapter_content: previous_chapter_content(previous_chapter),
    }
}

pub(crate) fn build_prompt_params_with_provider_payload(
    chapter_model: &chapter::Model,
    project_model: &project::Model,
    previous_chapter_prompt_context: PreviousChapterPromptContext,
    _has_previous_chapter: bool,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let narrative_perspective = resolve_prompt_preference(
        overrides.narrative_perspective.as_deref(),
        project_model.narrative_perspective.as_deref(),
    );
    let creative_mode = resolve_prompt_preference(
        overrides.creative_mode.as_deref(),
        project_model.default_creative_mode.as_deref(),
    );
    let story_focus = resolve_prompt_preference(
        overrides.story_focus.as_deref(),
        project_model.default_story_focus.as_deref(),
    );
    let plot_stage = resolve_prompt_preference(
        overrides.plot_stage.as_deref(),
        project_model.default_plot_stage.as_deref(),
    );
    let story_creation_brief = resolve_prompt_preference(
        overrides.story_creation_brief.as_deref(),
        project_model.default_story_creation_brief.as_deref(),
    );
    let quality_preset = resolve_prompt_preference(
        overrides.quality_preset.as_deref(),
        project_model.default_quality_preset.as_deref(),
    );
    let quality_notes = resolve_prompt_preference(
        overrides.quality_notes.as_deref(),
        project_model.default_quality_notes.as_deref(),
    );
    let web_research_query = overrides
        .web_research_query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(str::to_string);
    let story_repair_summary = overrides
        .story_repair_summary
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string();
    let story_repair_targets = normalize_prompt_list(&overrides.story_repair_targets);
    let story_preserve_strengths = normalize_prompt_list(&overrides.story_preserve_strengths);
    let mcp_references = provider_payload.mcp_references.trim().to_string();
    let quality_profile_payload =
        build_quality_profile_payload(project_model, &quality_preset, &provider_payload);
    let quality_prompt_blocks = build_novel_quality_prompt_blocks(Some(&quality_profile_payload));
    let external_assets_block = build_external_assets_block(
        &provider_payload.external_assets,
        &provider_payload.reference_assets,
        &provider_payload.mcp_references,
    );
    params.insert("project_title".to_string(), project_model.title.clone());
    params.insert(
        "genre".to_string(),
        project_model.genre.clone().unwrap_or_default(),
    );
    params.insert(
        "chapter_number".to_string(),
        chapter_model.chapter_number.to_string(),
    );
    params.insert("chapter_title".to_string(), chapter_model.title.clone());
    params.insert(
        "target_word_count".to_string(),
        target_word_count.to_string(),
    );
    params.insert(
        "narrative_perspective".to_string(),
        if narrative_perspective.is_empty() {
            "第三人称".to_string()
        } else {
            narrative_perspective
        },
    );
    params.insert(
        "chapter_outline".to_string(),
        chapter_model
            .expansion_plan
            .clone()
            .unwrap_or_else(|| "暂无大纲".to_string()),
    );
    params.insert(
        "world_time_period".to_string(),
        project_model.world_time_period.clone().unwrap_or_default(),
    );
    params.insert(
        "world_location".to_string(),
        project_model.world_location.clone().unwrap_or_default(),
    );
    params.insert(
        "world_atmosphere".to_string(),
        project_model.world_atmosphere.clone().unwrap_or_default(),
    );
    params.insert(
        "world_rules".to_string(),
        project_model.world_rules.clone().unwrap_or_default(),
    );
    params.insert("creative_mode".to_string(), creative_mode.clone());
    params.insert(
        "creative_mode_block".to_string(),
        build_creative_mode_block(&creative_mode),
    );
    params.insert("story_focus".to_string(), story_focus.clone());
    params.insert(
        "story_focus_block".to_string(),
        build_story_focus_block(&story_focus),
    );
    params.insert("plot_stage".to_string(), plot_stage.clone());
    params.insert(
        "narrative_blueprint_block".to_string(),
        build_narrative_blueprint_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_creation_brief".to_string(),
        story_creation_brief.clone(),
    );
    let web_research_block = build_web_research_block(
        overrides.web_research_enabled,
        web_research_query.as_deref(),
    );
    let story_creation_brief_block = format!(
        "{}{}",
        build_optional_instruction_block("创作总控摘要", &story_creation_brief),
        web_research_block
    );
    params.insert(
        "story_creation_brief_block".to_string(),
        story_creation_brief_block,
    );
    params.insert(
        "web_research_query".to_string(),
        web_research_query.clone().unwrap_or_default(),
    );
    params.insert("web_research_block".to_string(), web_research_block);
    params.insert("quality_preset".to_string(), quality_preset);
    params.insert("quality_notes".to_string(), quality_notes);
    let quality_preset = params.get("quality_preset").cloned().unwrap_or_default();
    let quality_notes = params.get("quality_notes").cloned().unwrap_or_default();
    params.insert(
        "quality_generation_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "generation"),
    );
    params.insert(
        "quality_analysis_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "checker"),
    );
    params.insert(
        "quality_checker_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "checker"),
    );
    params.insert(
        "quality_reviser_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "reviser"),
    );
    params.insert(
        "quality_regeneration_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "generation"),
    );
    params.insert(
        "quality_generation_protocol_block".to_string(),
        build_quality_generation_protocol_block(),
    );
    params.insert(
        "quality_json_protocol_block".to_string(),
        build_quality_json_protocol_block(),
    );
    params.insert(
        "quality_mcp_guard_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "mcp_guard"),
    );
    params.insert(
        "mcp_guard".to_string(),
        prompt_block_text(&quality_prompt_blocks, "mcp_guard"),
    );
    params.insert(
        "quality_preference_block".to_string(),
        build_quality_preference_block(&quality_preset, &quality_notes),
    );
    params.insert(
        "story_objective_card_block".to_string(),
        build_story_objective_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_result_card_block".to_string(),
        build_story_result_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_payoff_chain_card_block".to_string(),
        build_story_payoff_chain_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_rule_grounding_card_block".to_string(),
        build_story_rule_grounding_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_information_release_card_block".to_string(),
        build_story_information_release_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_emotion_landing_card_block".to_string(),
        build_story_emotion_landing_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_action_rendering_card_block".to_string(),
        build_story_action_rendering_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_summary_tone_control_card_block".to_string(),
        build_story_summary_tone_control_card_block_owner(
            &creative_mode,
            &story_focus,
            &plot_stage,
        ),
    );
    params.insert(
        "story_repetition_control_card_block".to_string(),
        build_story_repetition_control_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_viewpoint_discipline_card_block".to_string(),
        build_story_viewpoint_discipline_card_block_owner(
            &creative_mode,
            &story_focus,
            &plot_stage,
        ),
    );
    params.insert(
        "story_dialogue_advancement_card_block".to_string(),
        build_story_dialogue_advancement_card_block_owner(
            &creative_mode,
            &story_focus,
            &plot_stage,
        ),
    );
    params.insert(
        "story_opening_hook_card_block".to_string(),
        build_story_opening_hook_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_repair_summary".to_string(),
        story_repair_summary.clone(),
    );
    params.insert(
        "story_repair_targets".to_string(),
        story_repair_targets.join("；"),
    );
    params.insert(
        "story_preserve_strengths".to_string(),
        story_preserve_strengths.join("；"),
    );
    params.insert(
        "story_repair_target_block".to_string(),
        build_repair_target_block(&story_repair_targets, &story_preserve_strengths),
    );
    params.insert(
        "story_repair_diagnostic_block".to_string(),
        build_repair_diagnostic_block(
            &story_repair_summary,
            &story_repair_targets,
            &story_preserve_strengths,
        ),
    );
    params.insert(
        "story_execution_checklist_block".to_string(),
        build_story_execution_checklist_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_scene_anchor_card_block".to_string(),
        build_story_scene_anchor_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_scene_density_card_block".to_string(),
        build_story_scene_density_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_repetition_risk_block".to_string(),
        build_story_repetition_risk_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_acceptance_card_block".to_string(),
        build_story_acceptance_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_cliffhanger_card_block".to_string(),
        build_story_cliffhanger_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_character_arc_card_block".to_string(),
        build_story_character_arc_card_block_owner(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "quality_external_assets_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "external_assets"),
    );
    params.insert(
        "quality_raw_external_assets_block".to_string(),
        external_assets_block,
    );
    params.insert(
        "quality_mcp_references_block".to_string(),
        mcp_references.clone(),
    );
    params.insert(
        "quality_contract_block".to_string(),
        build_quality_contract_block(&params),
    );
    params.extend(provider_payload.into_prompt_params());
    params.insert(
        "previous_chapter_content".to_string(),
        previous_chapter_prompt_context.previous_chapter_content,
    );
    params.insert(
        "continuation_point".to_string(),
        previous_chapter_prompt_context.continuation_point,
    );
    params
}
