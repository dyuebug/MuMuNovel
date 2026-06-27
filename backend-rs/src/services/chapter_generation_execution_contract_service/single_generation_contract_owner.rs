use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    build_batch_request_runtime_state_owner_contract,
    build_generation_execution_config_owner_contract, PreparedGenerationExecutionConfig,
    BATCH_REQUEST_RUNTIME_STATE_KEY, DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT,
    MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT,
};
use crate::services::chapter_generation_prompt_service::ChapterGenerationPromptOverrides;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SingleChapterGenerationCompatOptions {
    pub(crate) style_id: Option<i32>,
    pub(crate) enable_analysis: bool,
    pub(crate) enable_mcp: bool,
    pub(crate) web_research_enabled: bool,
    pub(crate) web_research_query: Option<String>,
    pub(crate) narrative_perspective: Option<String>,
    pub(crate) creative_mode: Option<String>,
    pub(crate) story_focus: Option<String>,
    pub(crate) plot_stage: Option<String>,
    pub(crate) story_creation_brief: Option<String>,
    pub(crate) quality_preset: Option<String>,
    pub(crate) quality_notes: Option<String>,
    pub(crate) story_repair_summary: Option<String>,
    pub(crate) story_repair_targets: Vec<String>,
    pub(crate) story_preserve_strengths: Vec<String>,
}

impl SingleChapterGenerationCompatOptions {
    #[cfg(test)]
    pub(crate) fn style_id(&self) -> Option<i32> {
        self.style_id
    }

    pub(crate) fn enable_analysis(&self) -> bool {
        self.enable_analysis
    }

    #[cfg(test)]
    pub(crate) fn enable_mcp(&self) -> bool {
        self.enable_mcp
    }

    pub(crate) fn web_research_enabled(&self) -> bool {
        self.web_research_enabled
    }

    pub(crate) fn web_research_query(&self) -> Option<&str> {
        self.web_research_query.as_deref()
    }

    pub(crate) fn narrative_perspective(&self) -> &str {
        self.narrative_perspective.as_deref().unwrap_or_default()
    }

    pub(crate) fn creative_mode(&self) -> &str {
        self.creative_mode.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_focus(&self) -> &str {
        self.story_focus.as_deref().unwrap_or_default()
    }

    pub(crate) fn plot_stage(&self) -> &str {
        self.plot_stage.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_creation_brief(&self) -> &str {
        self.story_creation_brief.as_deref().unwrap_or_default()
    }

    pub(crate) fn quality_preset(&self) -> &str {
        self.quality_preset.as_deref().unwrap_or_default()
    }

    pub(crate) fn quality_notes(&self) -> &str {
        self.quality_notes.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_repair_summary(&self) -> &str {
        self.story_repair_summary.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_repair_targets(&self) -> &[String] {
        &self.story_repair_targets
    }

    pub(crate) fn story_preserve_strengths(&self) -> &[String] {
        &self.story_preserve_strengths
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SingleChapterGenerationExecutionInput {
    pub(crate) target_word_count: i32,
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) execution_config: PreparedGenerationExecutionConfig,
}

pub(crate) fn normalize_chapter_generation_target_word_count(
    target_word_count: Option<i32>,
) -> i32 {
    target_word_count
        .unwrap_or(DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT)
        .max(MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT)
}

fn option_from_non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn build_prompt_overrides_from_compat_options(
    compat_options: &SingleChapterGenerationCompatOptions,
) -> ChapterGenerationPromptOverrides {
    ChapterGenerationPromptOverrides {
        narrative_perspective: option_from_non_empty(compat_options.narrative_perspective()),
        creative_mode: option_from_non_empty(compat_options.creative_mode()),
        story_focus: option_from_non_empty(compat_options.story_focus()),
        plot_stage: option_from_non_empty(compat_options.plot_stage()),
        story_creation_brief: option_from_non_empty(compat_options.story_creation_brief()),
        quality_preset: option_from_non_empty(compat_options.quality_preset()),
        quality_notes: option_from_non_empty(compat_options.quality_notes()),
        web_research_enabled: compat_options.web_research_enabled(),
        web_research_query: compat_options.web_research_query().map(str::to_string),
        story_repair_summary: option_from_non_empty(compat_options.story_repair_summary()),
        story_repair_targets: compat_options.story_repair_targets().to_vec(),
        story_preserve_strengths: compat_options.story_preserve_strengths().to_vec(),
    }
}

pub(crate) fn build_single_generation_execution_contract_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_execution_contract_service::single_generation_contract_owner",
        "scope": "single_generation_execution_input_and_compat_options",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs",
            "backend-rs/src/services/chapter_generation_execution_contract_service/single_generation_contract_owner.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service/research_payload_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "compat_option_fields": [
                "style_id",
                "enable_analysis",
                "enable_mcp",
                "web_research_enabled",
                "web_research_query",
                "narrative_perspective",
                "creative_mode",
                "story_focus",
                "plot_stage",
                "story_creation_brief",
                "quality_preset",
                "quality_notes",
                "story_repair_summary",
                "story_repair_targets",
                "story_preserve_strengths"
            ],
            "execution_input_fields": [
                "target_word_count",
                "compat_options",
                "execution_config"
            ],
            "target_word_count_entrypoint": "normalize_chapter_generation_target_word_count",
            "default_target_word_count": DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT,
            "minimum_target_word_count": MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT,
            "request_runtime_state_key": BATCH_REQUEST_RUNTIME_STATE_KEY,
            "request_runtime_state_fields": [
                "compat_options",
                "model_override",
                "active_story_repair_payload"
            ],
            "request_runtime_state_entrypoints": [
                "BatchGenerationRequestRuntimeState::new",
                "BatchGenerationRequestRuntimeState::active_story_repair_payload_with_scope",
                "batch_generation_request_runtime_state_payload",
                "parse_batch_generation_request_runtime_state",
                "active_story_repair_payload_from_runtime_state"
            ],
            "generation_execution_config_owner_contract": build_generation_execution_config_owner_contract(),
            "request_runtime_state_owner_contract": build_batch_request_runtime_state_owner_contract(),
            "prompt_override_builder": "build_prompt_overrides_from_compat_options",
            "empty_string_prompt_overrides_skipped": true,
            "web_research_fields_preserved": true,
            "story_repair_arrays_preserved": true,
            "execution_config_owner": "PreparedGenerationExecutionConfig",
            "request_runtime_state_owner": "BatchGenerationRequestRuntimeState",
            "request_runtime_state_empty_payload_policy": "empty_summary_targets_and_strengths_do_not_emit_active_story_repair_payload",
            "request_runtime_state_parse_fallback_policy": "missing_or_malformed_batch_request_runtime_state_returns_default",
            "request_runtime_state_extraction_policy": "active_story_repair_payload_must_be_object"
        },
        "active_consumers": [
            "chapter_single_generation_prepare_service",
            "chapter_single_generation_stream_workflow_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_single_generation_runtime_state_service",
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_single_generation_prepare_service::research_payload_owner",
            "chapter_generation_runtime_service::story_repair_quality_context_owner",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter-batch-generation-active-gateway-smoke-rust"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_execution_contract_service",
            "cargo test chapter_single_generation_prepare_service",
            "cargo test chapter_single_generation_stream_workflow_service",
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 17,
            "python_fallback_probe_count": 0,
            "compat_options_owner": "SingleChapterGenerationCompatOptions",
            "execution_input_owner": "SingleChapterGenerationExecutionInput",
            "request_runtime_state_owner": "BatchGenerationRequestRuntimeState",
            "prompt_override_owner": "build_prompt_overrides_from_compat_options",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "single-generation execution contract owner is rust-only; surviving Python exit work is tracked by shared prompt and shared runtime owner contracts outside this owner",
            "status": "rust_shared_execution_contract_owner_direct_package_closed_out"
        },
        "rollback_boundary": {
            "source_map_policy": "single_generation_execution_contract_owner_is_rust_only_and_shared_prompt_story_repair_python_surfaces_are_tracked_by_external_owner_contracts",
            "runtime_knobs": [
                "SingleChapterGenerationCompatOptions",
                "ChapterCandidateRouteGatewayConfig"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_route_smoke"
        }
    })
}
