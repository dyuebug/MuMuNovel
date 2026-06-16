use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::models::chapter;
use crate::services::chapter_access_service::{
    load_accessible_chapter_for_generation, LoadAccessibleChapterForGenerationError,
};
use crate::services::chapter_generation_execution_contract_service::{
    deserialize_optional_non_null, prepare_generation_execution_config_with_provider_payload,
    BatchGenerationRequestRuntimeState, PreparedGenerationExecutionConfig,
};

use super::research_payload_owner::build_single_chapter_research_provider_payload;
use super::{
    check_chapter_generation_prerequisites, is_valid_optional_choice,
    is_valid_optional_text_length, normalize_optional_single_generation_request_string,
    normalize_single_generation_web_research_enabled, SingleChapterGenerationCompatOptions,
    MAX_SINGLE_GENERATION_QUALITY_NOTES_LENGTH, MAX_SINGLE_GENERATION_STORY_CREATION_BRIEF_LENGTH,
    MAX_SINGLE_GENERATION_TARGET_WORD_COUNT, MIN_SINGLE_GENERATION_TARGET_WORD_COUNT,
    SINGLE_GENERATION_CREATIVE_MODE_VALUES, SINGLE_GENERATION_PLOT_STAGE_VALUES,
    SINGLE_GENERATION_QUALITY_PRESET_VALUES, SINGLE_GENERATION_STORY_FOCUS_VALUES,
};

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SingleChapterGenerationRouteRequest {
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: Option<i32>,
    pub(crate) model: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) enable_analysis: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) enable_mcp: Option<bool>,
    #[serde(default)]
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    pub(crate) narrative_perspective: Option<String>,
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

impl SingleChapterGenerationRouteRequest {
    pub(crate) fn into_generation_request(self) -> SingleChapterGenerationRequest {
        SingleChapterGenerationRequest {
            style_id: self.style_id,
            target_word_count: self.target_word_count,
            model: self.model,
            enable_analysis: self.enable_analysis,
            enable_mcp: self.enable_mcp,
            enable_web_research: self.enable_web_research,
            web_research_query: self.web_research_query,
            narrative_perspective: self.narrative_perspective,
            creative_mode: normalize_optional_single_generation_request_string(self.creative_mode),
            story_focus: normalize_optional_single_generation_request_string(self.story_focus),
            plot_stage: normalize_optional_single_generation_request_string(self.plot_stage),
            story_creation_brief: normalize_optional_single_generation_request_string(
                self.story_creation_brief,
            ),
            quality_preset: normalize_optional_single_generation_request_string(
                self.quality_preset,
            ),
            quality_notes: normalize_optional_single_generation_request_string(self.quality_notes),
            story_repair_summary: normalize_optional_single_generation_request_string(
                self.story_repair_summary,
            ),
            story_repair_targets: self.story_repair_targets,
            story_preserve_strengths: self.story_preserve_strengths,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SingleChapterGenerationRequest {
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: Option<i32>,
    pub(crate) model: Option<String>,
    pub(crate) enable_analysis: Option<bool>,
    pub(crate) enable_mcp: Option<bool>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    pub(crate) narrative_perspective: Option<String>,
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

impl SingleChapterGenerationRequest {
    pub(crate) fn validate_request_bounds(
        &self,
    ) -> Result<(), PrepareSingleChapterGenerationRequestError> {
        if let Some(target_word_count) = self.target_word_count {
            if target_word_count < MIN_SINGLE_GENERATION_TARGET_WORD_COUNT {
                return Err(
                    PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooSmall,
                );
            }
            if target_word_count > MAX_SINGLE_GENERATION_TARGET_WORD_COUNT {
                return Err(
                    PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooLarge,
                );
            }
        }
        if !is_valid_optional_choice(
            self.creative_mode.as_deref(),
            SINGLE_GENERATION_CREATIVE_MODE_VALUES,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::InvalidCreativeMode);
        }
        if !is_valid_optional_choice(
            self.story_focus.as_deref(),
            SINGLE_GENERATION_STORY_FOCUS_VALUES,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::InvalidStoryFocus);
        }
        if !is_valid_optional_choice(
            self.plot_stage.as_deref(),
            SINGLE_GENERATION_PLOT_STAGE_VALUES,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::InvalidPlotStage);
        }
        if !is_valid_optional_choice(
            self.quality_preset.as_deref(),
            SINGLE_GENERATION_QUALITY_PRESET_VALUES,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::InvalidQualityPreset);
        }
        if !is_valid_optional_text_length(
            self.story_creation_brief.as_deref(),
            MAX_SINGLE_GENERATION_STORY_CREATION_BRIEF_LENGTH,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::StoryCreationBriefTooLong);
        }
        if !is_valid_optional_text_length(
            self.quality_notes.as_deref(),
            MAX_SINGLE_GENERATION_QUALITY_NOTES_LENGTH,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::QualityNotesTooLong);
        }

        Ok(())
    }

    pub(crate) fn compat_options_with_web_research_default(
        &self,
        web_research_default: bool,
    ) -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: self.style_id,
            enable_analysis: self.enable_analysis.unwrap_or(true),
            enable_mcp: self.enable_mcp.unwrap_or(true),
            web_research_enabled: normalize_single_generation_web_research_enabled(
                self.enable_web_research,
                web_research_default,
            ),
            web_research_query: self.web_research_query.clone(),
            narrative_perspective: self.narrative_perspective.clone(),
            creative_mode: self.creative_mode.clone(),
            story_focus: self.story_focus.clone(),
            plot_stage: self.plot_stage.clone(),
            story_creation_brief: self.story_creation_brief.clone(),
            quality_preset: self.quality_preset.clone(),
            quality_notes: self.quality_notes.clone(),
            story_repair_summary: self.story_repair_summary.clone(),
            story_repair_targets: self.story_repair_targets.clone().unwrap_or_default(),
            story_preserve_strengths: self.story_preserve_strengths.clone().unwrap_or_default(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PrepareSingleChapterGenerationRequestError {
    Chapter(LoadAccessibleChapterForGenerationError),
    Config(String),
    PrerequisitesBlocked(String),
    InvalidTargetWordCountTooSmall,
    InvalidTargetWordCountTooLarge,
    InvalidCreativeMode,
    InvalidStoryFocus,
    InvalidPlotStage,
    InvalidQualityPreset,
    StoryCreationBriefTooLong,
    QualityNotesTooLong,
    Internal(String),
}

impl PrepareSingleChapterGenerationRequestError {
    pub(crate) fn detail_message(&self) -> String {
        match self {
            Self::Chapter(LoadAccessibleChapterForGenerationError::ChapterNotFound) => {
                "Chapter not found".to_string()
            }
            Self::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
            ) => "Chapter not found or access denied".to_string(),
            Self::Chapter(LoadAccessibleChapterForGenerationError::Internal(detail))
            | Self::Config(detail)
            | Self::PrerequisitesBlocked(detail)
            | Self::Internal(detail) => detail.clone(),
            Self::InvalidTargetWordCountTooSmall => {
                "target_word_count must be greater than or equal to 500".to_string()
            }
            Self::InvalidTargetWordCountTooLarge => {
                "target_word_count must be less than or equal to 10000".to_string()
            }
            Self::InvalidCreativeMode => "creative_mode is invalid".to_string(),
            Self::InvalidStoryFocus => "story_focus is invalid".to_string(),
            Self::InvalidPlotStage => "plot_stage is invalid".to_string(),
            Self::InvalidQualityPreset => "quality_preset is invalid".to_string(),
            Self::StoryCreationBriefTooLong => {
                "story_creation_brief must be at most 1200 characters".to_string()
            }
            Self::QualityNotesTooLong => "quality_notes must be at most 600 characters".to_string(),
        }
    }

    pub(crate) fn request_validation_detail_message(&self) -> Option<String> {
        match self {
            Self::InvalidTargetWordCountTooSmall
            | Self::InvalidTargetWordCountTooLarge
            | Self::InvalidCreativeMode
            | Self::InvalidStoryFocus
            | Self::InvalidPlotStage
            | Self::InvalidQualityPreset
            | Self::StoryCreationBriefTooLong
            | Self::QualityNotesTooLong => Some(self.detail_message()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleChapterGenerationTarget {
    pub(crate) project_id: String,
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) title: String,
}

impl SingleChapterGenerationTarget {
    pub(crate) fn from_model(chapter_model: &chapter::Model) -> Self {
        Self {
            project_id: chapter_model.project_id.clone(),
            chapter_id: chapter_model.id.clone(),
            chapter_number: chapter_model.chapter_number,
            title: chapter_model.title.clone(),
        }
    }
}

pub(crate) async fn load_single_chapter_generation_target(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<SingleChapterGenerationTarget, PrepareSingleChapterGenerationRequestError> {
    let chapter_model = load_accessible_chapter_for_generation(db, chapter_id, user_id)
        .await
        .map_err(PrepareSingleChapterGenerationRequestError::Chapter)?;
    let prerequisite = check_chapter_generation_prerequisites(db, &chapter_model)
        .await
        .map_err(PrepareSingleChapterGenerationRequestError::Internal)?;
    if !prerequisite.can_generate {
        return Err(
            PrepareSingleChapterGenerationRequestError::PrerequisitesBlocked(
                prerequisite.error_message,
            ),
        );
    }

    Ok(SingleChapterGenerationTarget::from_model(&chapter_model))
}

pub(crate) async fn prepare_single_chapter_generation_execution_config_from_runtime_state(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Result<PreparedGenerationExecutionConfig, PrepareSingleChapterGenerationRequestError> {
    let provider_payload = build_single_chapter_research_provider_payload(
        db,
        user_id,
        chapter_target,
        &request_runtime_state.compat_options,
    )
    .await
    .map_err(PrepareSingleChapterGenerationRequestError::Config)?;

    prepare_generation_execution_config_with_provider_payload(
        db,
        user_id,
        request_runtime_state.model_override.as_deref(),
        provider_payload,
    )
    .await
    .map_err(PrepareSingleChapterGenerationRequestError::Config)
}
