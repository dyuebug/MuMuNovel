use std::collections::HashMap;

use crate::models::{chapter, project};
use crate::services::prompt_template_service::PromptTemplateService;
use serde_json::Value;

use super::{
    build_prompt_context_provider_owner_contract, build_prompt_params_with_provider_payload,
    build_quality_profile_owner_contract, build_story_card_owner_contract,
    PreviousChapterPromptContext, PromptContextProviderPayload,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ChapterGenerationPromptOverrides {
    pub(crate) narrative_perspective: Option<String>,
    pub(crate) creative_mode: Option<String>,
    pub(crate) story_focus: Option<String>,
    pub(crate) plot_stage: Option<String>,
    pub(crate) story_creation_brief: Option<String>,
    pub(crate) quality_preset: Option<String>,
    pub(crate) quality_notes: Option<String>,
    pub(crate) web_research_enabled: bool,
    pub(crate) web_research_query: Option<String>,
    pub(crate) story_repair_summary: Option<String>,
    pub(crate) story_repair_targets: Vec<String>,
    pub(crate) story_preserve_strengths: Vec<String>,
}

fn append_prompt_block_after_tag(prompt: &str, block: &str, after_tag: &str) -> String {
    let block = block.trim();
    if block.is_empty() || prompt.contains("<quality_contract") {
        return prompt.to_string();
    }
    if let Some(index) = prompt.find(after_tag) {
        let insert_at = index + after_tag.len();
        let mut result = String::with_capacity(prompt.len() + block.len() + 2);
        result.push_str(&prompt[..insert_at]);
        result.push_str("\n\n");
        result.push_str(block);
        result.push_str(&prompt[insert_at..]);
        return result;
    }
    format!("{}\n\n{}", prompt.trim_end(), block)
}

fn inject_quality_contract(prompt: &str, params: &HashMap<String, String>) -> String {
    append_prompt_block_after_tag(
        prompt,
        params
            .get("quality_contract_block")
            .map(String::as_str)
            .unwrap_or_default(),
        "</fusion_contract>",
    )
}

pub(crate) fn prompt_block_text(prompt_blocks: &Value, key: &str) -> String {
    prompt_blocks
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn chapter_template_key(outline_mode: &str, has_previous: bool) -> &'static str {
    match (outline_mode, has_previous) {
        ("one-to-many", false) => "CHAPTER_GENERATION_ONE_TO_MANY",
        ("one-to-many", true) => "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
        ("one-to-one", false) | (_, false) => "CHAPTER_GENERATION_ONE_TO_ONE",
        _ => "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
    }
}

pub(crate) fn build_prompt_with_provider_payload(
    chapter_model: &chapter::Model,
    project_model: &project::Model,
    previous_chapter_prompt_context: PreviousChapterPromptContext,
    has_previous_chapter: bool,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
) -> Result<String, String> {
    let template_key = chapter_template_key(&project_model.outline_mode, has_previous_chapter);
    let template = PromptTemplateService::system_template_info(template_key)
        .ok_or_else(|| format!("找不到章节模板: {}", template_key))?;
    let params = build_prompt_params_with_provider_payload(
        chapter_model,
        project_model,
        previous_chapter_prompt_context,
        has_previous_chapter,
        target_word_count,
        provider_payload,
        overrides,
    );

    let rendered = PromptTemplateService::format_prompt(&template.content, &params)?;
    Ok(inject_quality_contract(&rendered, &params))
}

pub(crate) fn build_chapter_generation_prompt_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_generation_prompt_service",
        "scope": "shared_generation_prompt_template_and_runtime_block_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_prompt_service.rs",
            "backend-rs/src/services/chapter_generation_prompt_service/provider_payload_owner.rs",
            "backend-rs/src/services/chapter_generation_prompt_service/template_render_owner.rs",
            "backend-rs/src/services/chapter_generation_prompt_service/story_card_owner.rs",
            "backend-rs/src/services/chapter_generation_prompt_service/quality_profile_owner.rs",
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_previous_chapter_prompt_context",
                "chapter_template_key",
                "build_prompt_params_with_provider_payload",
                "build_prompt_with_provider_payload"
            ],
            "template_key_policy": [
                "one-to-many_without_previous -> CHAPTER_GENERATION_ONE_TO_MANY",
                "one-to-many_with_previous -> CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
                "one-to-one_or_unknown_without_previous -> CHAPTER_GENERATION_ONE_TO_ONE",
                "one-to-one_or_unknown_with_previous -> CHAPTER_GENERATION_ONE_TO_ONE_NEXT"
            ],
            "owned_prompt_inputs": [
                "chapter_model",
                "project_model",
                "previous_chapter_prompt_context",
                "target_word_count",
                "provider_payload",
                "ChapterGenerationPromptOverrides"
            ],
            "prompt_context_provider_owner_contract": build_prompt_context_provider_owner_contract(),
            "default_policy": [
                "missing_narrative_perspective -> 第三人称",
                "missing_chapter_outline -> 暂无大纲",
                "prompt_overrides_win_before_project_defaults",
                "empty_web_research_query_keeps_research_block_empty_unless_enabled"
            ],
            "runtime_blocks": [
                "creative_mode_block",
                "story_focus_block",
                "narrative_blueprint_block",
                "story_creation_brief_block",
                "web_research_block",
                "story_repair_target_block",
                "story_repair_diagnostic_block",
                "quality_contract_block"
            ],
            "quality_profile_owner": [
                "build_novel_quality_prompt_blocks",
                "resolve_quality_weight_profile",
                "resolve_adaptive_quality_gate_profile",
                "resolve_metric_threshold_adjustments"
            ],
            "story_card_owner_contract": build_story_card_owner_contract(),
            "quality_profile_owner_contract": build_quality_profile_owner_contract(),
            "quality_contract_policy": "inject_quality_contract_after_fusion_contract_without_replacing_user_template_body",
            "provider_payload_policy": "PromptContextProviderPayload::into_prompt_params_is_merged_after_runtime_blocks"
        },
        "validation_boundary": [
            "cargo test services::chapter_generation_prompt_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner",
                "phase5-chapter-regeneration-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "regeneration_manifest_probe_count": 13,
            "rust_manifest_probe_count": 30,
            "python_fallback_probe_count": 0,
            "prompt_template_owner": "chapter_generation_prompt_service",
            "quality_profile_owner": "chapter_generation_prompt_service/quality_profile_owner.rs",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "python_story_prompt_block_service_deleted": true,
            "python_prompt_template_facade_lazy_source_map_import": false,
            "python_prompt_template_facade_service_deleted": true,
            "python_prompt_service_lazy_source_map_import": false,
            "python_prompt_service_deleted": true,
            "python_story_packet_lazy_source_map_import": false,
            "python_story_packet_service_deleted": true,
            "python_story_packet_lazy_continuity_ledger_import": false,
            "python_story_packet_continuity_ledger_proxy_retired": true,
            "python_story_continuity_ledger_service_deleted": true,
            "production_promptservice_default_importers_cleared": true,
            "remaining_cutover_gate": "prompt Python production services physically closed; historical Python prompt fixtures live under backend/tests/test_support",
            "status": "rust_service_runtime_owner_with_prompt_python_production_services_deleted"
        },
        "rollback_boundary": "prompt rollback is now Rust owner plus backend/tests/test_support historical fixtures; no backend/app prompt Python service remains"
    })
}
