use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::chapter_generation_execution_contract_service::{
    deserialize_optional_non_null, BatchGenerationRequestRuntimeState,
    SingleChapterGenerationCompatOptions,
};

use super::{
    BatchGenerationCreateTaskSpec, BatchGenerationCreateWorkflowRequest,
    PrepareBatchGenerationCreateRequestError, BATCH_GENERATION_CREATE_CREATIVE_MODE_VALUES,
    BATCH_GENERATION_CREATE_PLOT_STAGE_VALUES, BATCH_GENERATION_CREATE_QUALITY_PRESET_VALUES,
    BATCH_GENERATION_CREATE_STORY_FOCUS_VALUES, MAX_BATCH_GENERATION_CREATE_COUNT,
    MAX_BATCH_GENERATION_CREATE_QUALITY_NOTES_LENGTH, MAX_BATCH_GENERATION_CREATE_RETRIES,
    MAX_BATCH_GENERATION_CREATE_STORY_CREATION_BRIEF_LENGTH,
    MAX_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT, MIN_BATCH_GENERATION_CREATE_RETRIES,
    MIN_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT,
};

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct BatchGenerationCreateRouteRequest {
    pub(crate) start_chapter_number: i32,
    pub(crate) count: i32,
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) enable_analysis: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) enable_mcp: Option<bool>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) max_retries: Option<i32>,
    pub(crate) model: Option<String>,
    pub(crate) creative_mode: Option<String>,
    pub(crate) story_focus: Option<String>,
    pub(crate) plot_stage: Option<String>,
    pub(crate) story_creation_brief: Option<String>,
    pub(crate) quality_preset: Option<String>,
    pub(crate) quality_notes: Option<String>,
    pub(crate) story_repair_summary: Option<String>,
    pub(crate) story_repair_targets: Option<Vec<String>>,
    pub(crate) story_preserve_strengths: Option<Vec<String>>,
}

pub(crate) fn build_batch_generation_request_contract_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_write_workflow_service::request_prepare_owner::request_contract_owner",
        "scope": "batch_generation_create_route_request_workflow_request_normalization_runtime_projection_and_bounds_validation",
        "python_source_map": [
            "backend/app/api/chapter_batch_generation_routes.py",
            "backend/app/api/chapters.py",
            "backend/app/services/batch_generation/create_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/request_prepare_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/request_prepare_owner/request_contract_owner.rs",
            "backend-rs/src/api/chapter_batch_generation.rs"
        ],
        "behavior_contract": {
            "request_entrypoints": [
                "build_batch_generation_create_workflow_request_from_route_payload",
                "BatchGenerationCreateWorkflowRequest::from_route_request",
                "BatchGenerationCreateWorkflowRequest::compat_options_with_web_research_default",
                "BatchGenerationCreateWorkflowRequest::into_request_runtime_state",
                "BatchGenerationCreateWorkflowRequest::validate_request_bounds"
            ],
            "request_contract_fields": [
                "start_chapter_number",
                "count",
                "style_id",
                "target_word_count",
                "enable_analysis",
                "enable_mcp",
                "enable_web_research",
                "web_research_query",
                "max_retries",
                "model",
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
            "validation_bounds": {
                "count_max": MAX_BATCH_GENERATION_CREATE_COUNT,
                "target_word_count_min": MIN_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT,
                "target_word_count_max": MAX_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT,
                "max_retries_min": MIN_BATCH_GENERATION_CREATE_RETRIES,
                "max_retries_max": MAX_BATCH_GENERATION_CREATE_RETRIES,
                "story_creation_brief_max_chars": MAX_BATCH_GENERATION_CREATE_STORY_CREATION_BRIEF_LENGTH,
                "quality_notes_max_chars": MAX_BATCH_GENERATION_CREATE_QUALITY_NOTES_LENGTH
            }
        },
        "validation_boundary": [
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ]
    })
}

impl BatchGenerationCreateWorkflowRequest {
    pub(crate) fn from_route_request(route_request: BatchGenerationCreateRouteRequest) -> Self {
        Self {
            start_chapter_number: route_request.start_chapter_number,
            count: route_request.count,
            style_id: route_request.style_id,
            target_word_count: route_request.target_word_count,
            enable_analysis: route_request.enable_analysis.unwrap_or(false),
            enable_mcp: route_request.enable_mcp,
            enable_web_research: route_request.enable_web_research,
            web_research_query: route_request.web_research_query,
            max_retries: route_request.max_retries.unwrap_or(3),
            model_override: route_request.model,
            creative_mode: normalize_optional_create_request_string(route_request.creative_mode),
            story_focus: normalize_optional_create_request_string(route_request.story_focus),
            plot_stage: normalize_optional_create_request_string(route_request.plot_stage),
            story_creation_brief: normalize_optional_create_request_string(
                route_request.story_creation_brief,
            ),
            quality_preset: normalize_optional_create_request_string(route_request.quality_preset),
            quality_notes: normalize_optional_create_request_string(route_request.quality_notes),
            story_repair_summary: normalize_optional_create_request_string(
                route_request.story_repair_summary,
            ),
            story_repair_targets: route_request.story_repair_targets.unwrap_or_default(),
            story_preserve_strengths: route_request.story_preserve_strengths.unwrap_or_default(),
        }
    }

    pub(crate) fn compat_options_with_web_research_default(
        &self,
        web_research_default: bool,
    ) -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: self.style_id,
            enable_analysis: self.enable_analysis,
            enable_mcp: self.enable_mcp.unwrap_or(true),
            web_research_enabled: self.enable_web_research.unwrap_or(web_research_default),
            web_research_query: self.web_research_query.clone(),
            narrative_perspective: None,
            creative_mode: self.creative_mode.clone(),
            story_focus: self.story_focus.clone(),
            plot_stage: self.plot_stage.clone(),
            story_creation_brief: self.story_creation_brief.clone(),
            quality_preset: self.quality_preset.clone(),
            quality_notes: self.quality_notes.clone(),
            story_repair_summary: self.story_repair_summary.clone(),
            story_repair_targets: self.story_repair_targets.clone(),
            story_preserve_strengths: self.story_preserve_strengths.clone(),
        }
    }

    pub(crate) fn into_request_runtime_state(
        &self,
        web_research_default: bool,
    ) -> BatchGenerationRequestRuntimeState {
        BatchGenerationRequestRuntimeState::new(
            self.compat_options_with_web_research_default(web_research_default),
            self.model_override.clone(),
        )
    }

    pub(crate) fn task_spec(&self) -> BatchGenerationCreateTaskSpec {
        BatchGenerationCreateTaskSpec {
            start_chapter_number: self.start_chapter_number,
            style_id: self.style_id,
            enable_analysis: self.enable_analysis,
            max_retries: self.max_retries,
        }
    }

    pub(crate) fn validate_request_bounds(
        &self,
    ) -> Result<(), PrepareBatchGenerationCreateRequestError> {
        if self.count <= 0 {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidCount);
        }
        if self.count > MAX_BATCH_GENERATION_CREATE_COUNT {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidCountTooLarge);
        }
        if let Some(target_word_count) = self.target_word_count {
            if target_word_count < MIN_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT {
                return Err(
                    PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooSmall,
                );
            }
            if target_word_count > MAX_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT {
                return Err(
                    PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooLarge,
                );
            }
        }
        if self.max_retries < MIN_BATCH_GENERATION_CREATE_RETRIES
            || self.max_retries > MAX_BATCH_GENERATION_CREATE_RETRIES
        {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidMaxRetries);
        }
        if !is_valid_optional_choice(
            self.creative_mode.as_deref(),
            BATCH_GENERATION_CREATE_CREATIVE_MODE_VALUES,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidCreativeMode);
        }
        if !is_valid_optional_choice(
            self.story_focus.as_deref(),
            BATCH_GENERATION_CREATE_STORY_FOCUS_VALUES,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidStoryFocus);
        }
        if !is_valid_optional_choice(
            self.plot_stage.as_deref(),
            BATCH_GENERATION_CREATE_PLOT_STAGE_VALUES,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidPlotStage);
        }
        if !is_valid_optional_choice(
            self.quality_preset.as_deref(),
            BATCH_GENERATION_CREATE_QUALITY_PRESET_VALUES,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidQualityPreset);
        }
        if !is_valid_optional_text_length(
            self.story_creation_brief.as_deref(),
            MAX_BATCH_GENERATION_CREATE_STORY_CREATION_BRIEF_LENGTH,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::StoryCreationBriefTooLong);
        }
        if !is_valid_optional_text_length(
            self.quality_notes.as_deref(),
            MAX_BATCH_GENERATION_CREATE_QUALITY_NOTES_LENGTH,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::QualityNotesTooLong);
        }

        Ok(())
    }
}

pub(crate) fn normalize_optional_create_request_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn is_valid_optional_choice(value: Option<&str>, allowed_values: &[&str]) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| allowed_values.contains(&value))
        .unwrap_or(true)
}

pub(crate) fn is_valid_optional_text_length(value: Option<&str>, max_chars: usize) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().count() <= max_chars)
        .unwrap_or(true)
}

pub(crate) fn build_batch_generation_create_workflow_request_from_route_payload(
    route_request: BatchGenerationCreateRouteRequest,
) -> BatchGenerationCreateWorkflowRequest {
    BatchGenerationCreateWorkflowRequest::from_route_request(route_request)
}
