use serde::Deserialize;
use serde_json::Value;

use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions;

use super::{
    MAX_PARTIAL_REGENERATION_CONTEXT_CHARS, MAX_PARTIAL_REGENERATION_TARGET_WORD_COUNT,
    MAX_PARTIAL_REGENERATION_USER_INSTRUCTIONS_LENGTH,
    MAX_PARTIAL_REGENERATION_WEB_RESEARCH_QUERY_LENGTH, MAX_REGENERATION_QUALITY_NOTES_LENGTH,
    MAX_REGENERATION_STORY_CREATION_BRIEF_LENGTH, MAX_REGENERATION_TARGET_WORD_COUNT,
    MAX_REGENERATION_WEB_RESEARCH_QUERY_LENGTH, MIN_PARTIAL_REGENERATION_CONTEXT_CHARS,
    MIN_PARTIAL_REGENERATION_TARGET_WORD_COUNT, MIN_REGENERATION_TARGET_WORD_COUNT,
    REGENERATION_CREATIVE_MODE_VALUES, REGENERATION_PLOT_STAGE_VALUES,
    REGENERATION_QUALITY_PRESET_VALUES, REGENERATION_STORY_FOCUS_VALUES,
};

fn normalize_optional_regeneration_request_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_valid_optional_regeneration_choice(value: Option<&str>, allowed_values: &[&str]) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| allowed_values.contains(&value))
        .unwrap_or(true)
}

fn is_valid_optional_regeneration_text_length(value: Option<&str>, max_chars: usize) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().count() <= max_chars)
        .unwrap_or(true)
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BuildRegenerationAiServiceError {
    InvalidConfig(String),
    InvalidTargetWordCountTooSmall,
    InvalidTargetWordCountTooLarge,
    InvalidCreativeMode,
    InvalidStoryFocus,
    InvalidPlotStage,
    InvalidQualityPreset,
    StoryCreationBriefTooLong,
    QualityNotesTooLong,
    WebResearchQueryTooLong,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PreparePartialRegenerationError {
    InvalidRange,
    EmptySelectedText,
    EmptyUserInstructions,
    UserInstructionsTooLong,
    ContextCharsTooSmall,
    ContextCharsTooLarge,
    TargetWordCountTooSmall,
    TargetWordCountTooLarge,
    WebResearchQueryTooLong,
}

pub(crate) enum PreparePartialRegenerationStreamError {
    Input(PreparePartialRegenerationError),
    Style(String),
    Config(BuildRegenerationAiServiceError),
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub(crate) struct FullChapterRegenerationStreamRouteRequest {
    pub(crate) modification_source: Option<String>,
    pub(crate) target_word_count: Option<i64>,
    pub(crate) custom_instructions: Option<String>,
    #[serde(default)]
    pub(crate) selected_suggestion_indices: Vec<Value>,
    #[serde(default)]
    pub(crate) focus_areas: Vec<Value>,
    pub(crate) story_creation_brief: Option<String>,
    pub(crate) quality_notes: Option<String>,
    pub(crate) story_repair_summary: Option<String>,
    pub(crate) creative_mode: Option<String>,
    pub(crate) story_focus: Option<String>,
    pub(crate) plot_stage: Option<String>,
    pub(crate) quality_preset: Option<String>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    pub(crate) preserve_elements: Option<Value>,
    #[serde(default)]
    pub(crate) story_repair_targets: Vec<Value>,
    #[serde(default)]
    pub(crate) story_preserve_strengths: Vec<Value>,
    pub(crate) style_id: Option<i32>,
    pub(crate) version_note: Option<String>,
    pub(crate) auto_apply: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FullChapterRegenerationStreamRequest {
    pub(crate) modification_source: Option<String>,
    pub(crate) target_word_count: Option<i64>,
    pub(crate) custom_instructions: Option<String>,
    pub(crate) selected_suggestion_indices: Vec<String>,
    pub(crate) focus_areas: Vec<String>,
    pub(crate) story_creation_brief: Option<String>,
    pub(crate) quality_notes: Option<String>,
    pub(crate) story_repair_summary: Option<String>,
    pub(crate) creative_mode: Option<String>,
    pub(crate) story_focus: Option<String>,
    pub(crate) plot_stage: Option<String>,
    pub(crate) quality_preset: Option<String>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    pub(crate) preserve_structure: bool,
    pub(crate) preserve_dialogues: Vec<String>,
    pub(crate) preserve_plot_points: Vec<String>,
    pub(crate) preserve_character_traits: bool,
    pub(crate) story_repair_targets: Vec<String>,
    pub(crate) story_preserve_strengths: Vec<String>,
    pub(crate) style_id: Option<i32>,
    pub(crate) version_note: Option<String>,
    pub(crate) auto_apply: bool,
}

impl Default for FullChapterRegenerationStreamRequest {
    fn default() -> Self {
        Self::from_route_request(FullChapterRegenerationStreamRouteRequest::default())
    }
}

impl FullChapterRegenerationStreamRequest {
    fn read_string_list(value: Option<&Value>) -> Vec<String> {
        value
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn read_preserve_elements_string_list(preserve_elements: &Value, key: &str) -> Vec<String> {
        Self::read_string_list(preserve_elements.get(key))
    }

    fn read_preserve_elements_bool(preserve_elements: &Value, key: &str, default: bool) -> bool {
        preserve_elements
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(default)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target_word_count: Option<i64>,
        custom_instructions: Option<String>,
        selected_suggestion_indices: Vec<String>,
        focus_areas: Vec<String>,
        story_creation_brief: Option<String>,
        quality_notes: Option<String>,
        story_repair_summary: Option<String>,
        creative_mode: Option<String>,
        story_focus: Option<String>,
        plot_stage: Option<String>,
        quality_preset: Option<String>,
        enable_web_research: Option<bool>,
        web_research_query: Option<String>,
        preserve_structure: bool,
        preserve_dialogues: Vec<String>,
        preserve_plot_points: Vec<String>,
        preserve_character_traits: bool,
        story_repair_targets: Vec<String>,
        story_preserve_strengths: Vec<String>,
        modification_source: Option<String>,
        style_id: Option<i32>,
        version_note: Option<String>,
        auto_apply: bool,
    ) -> Self {
        Self {
            modification_source,
            target_word_count,
            custom_instructions,
            selected_suggestion_indices,
            focus_areas,
            story_creation_brief,
            quality_notes,
            story_repair_summary,
            creative_mode,
            story_focus,
            plot_stage,
            quality_preset,
            enable_web_research,
            web_research_query,
            preserve_structure,
            preserve_dialogues,
            preserve_plot_points,
            preserve_character_traits,
            story_repair_targets,
            story_preserve_strengths,
            style_id,
            version_note,
            auto_apply,
        }
    }

    fn from_route_request(route_request: FullChapterRegenerationStreamRouteRequest) -> Self {
        let preserve_elements = route_request.preserve_elements.unwrap_or_default();

        Self::new(
            route_request.target_word_count,
            normalize_optional_regeneration_request_string(route_request.custom_instructions),
            route_request
                .selected_suggestion_indices
                .into_iter()
                .filter_map(|value| value.as_i64().map(|value| value.to_string()))
                .collect(),
            route_request
                .focus_areas
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
            normalize_optional_regeneration_request_string(route_request.story_creation_brief),
            normalize_optional_regeneration_request_string(route_request.quality_notes),
            normalize_optional_regeneration_request_string(route_request.story_repair_summary),
            normalize_optional_regeneration_request_string(route_request.creative_mode),
            normalize_optional_regeneration_request_string(route_request.story_focus),
            normalize_optional_regeneration_request_string(route_request.plot_stage),
            normalize_optional_regeneration_request_string(route_request.quality_preset),
            route_request.enable_web_research,
            normalize_optional_regeneration_request_string(route_request.web_research_query),
            Self::read_preserve_elements_bool(&preserve_elements, "preserve_structure", false),
            Self::read_preserve_elements_string_list(&preserve_elements, "preserve_dialogues"),
            Self::read_preserve_elements_string_list(&preserve_elements, "preserve_plot_points"),
            Self::read_preserve_elements_bool(
                &preserve_elements,
                "preserve_character_traits",
                true,
            ),
            route_request
                .story_repair_targets
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
            route_request
                .story_preserve_strengths
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
            normalize_optional_regeneration_request_string(route_request.modification_source),
            route_request.style_id,
            normalize_optional_regeneration_request_string(route_request.version_note),
            route_request.auto_apply.unwrap_or(false),
        )
    }

    pub fn modification_source(&self) -> &str {
        self.modification_source
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("custom")
    }

    pub fn target_word_count(&self) -> i64 {
        self.target_word_count.unwrap_or(3000)
    }

    pub fn custom_instructions(&self) -> &str {
        self.custom_instructions.as_deref().unwrap_or_default()
    }

    pub fn selected_suggestion_indices(&self) -> &[String] {
        &self.selected_suggestion_indices
    }

    pub fn focus_areas(&self) -> &[String] {
        &self.focus_areas
    }

    pub fn story_creation_brief(&self) -> &str {
        self.story_creation_brief.as_deref().unwrap_or_default()
    }

    pub fn quality_notes(&self) -> &str {
        self.quality_notes.as_deref().unwrap_or_default()
    }

    pub fn story_repair_summary(&self) -> &str {
        self.story_repair_summary.as_deref().unwrap_or_default()
    }

    pub fn creative_mode(&self) -> &str {
        self.creative_mode.as_deref().unwrap_or_default()
    }

    pub fn story_focus(&self) -> &str {
        self.story_focus.as_deref().unwrap_or_default()
    }

    #[cfg(test)]
    pub fn plot_stage(&self) -> &str {
        self.plot_stage.as_deref().unwrap_or_default()
    }

    pub fn quality_preset(&self) -> &str {
        self.quality_preset.as_deref().unwrap_or_default()
    }

    pub(crate) fn validate_request_bounds(&self) -> Result<(), BuildRegenerationAiServiceError> {
        if self.target_word_count() < MIN_REGENERATION_TARGET_WORD_COUNT {
            return Err(BuildRegenerationAiServiceError::InvalidTargetWordCountTooSmall);
        }
        if self.target_word_count() > MAX_REGENERATION_TARGET_WORD_COUNT {
            return Err(BuildRegenerationAiServiceError::InvalidTargetWordCountTooLarge);
        }
        if !is_valid_optional_regeneration_choice(
            self.creative_mode.as_deref(),
            REGENERATION_CREATIVE_MODE_VALUES,
        ) {
            return Err(BuildRegenerationAiServiceError::InvalidCreativeMode);
        }
        if !is_valid_optional_regeneration_choice(
            self.story_focus.as_deref(),
            REGENERATION_STORY_FOCUS_VALUES,
        ) {
            return Err(BuildRegenerationAiServiceError::InvalidStoryFocus);
        }
        if !is_valid_optional_regeneration_choice(
            self.plot_stage.as_deref(),
            REGENERATION_PLOT_STAGE_VALUES,
        ) {
            return Err(BuildRegenerationAiServiceError::InvalidPlotStage);
        }
        if !is_valid_optional_regeneration_choice(
            self.quality_preset.as_deref(),
            REGENERATION_QUALITY_PRESET_VALUES,
        ) {
            return Err(BuildRegenerationAiServiceError::InvalidQualityPreset);
        }
        if !is_valid_optional_regeneration_text_length(
            self.story_creation_brief.as_deref(),
            MAX_REGENERATION_STORY_CREATION_BRIEF_LENGTH,
        ) {
            return Err(BuildRegenerationAiServiceError::StoryCreationBriefTooLong);
        }
        if !is_valid_optional_regeneration_text_length(
            self.quality_notes.as_deref(),
            MAX_REGENERATION_QUALITY_NOTES_LENGTH,
        ) {
            return Err(BuildRegenerationAiServiceError::QualityNotesTooLong);
        }
        if !is_valid_optional_regeneration_text_length(
            self.web_research_query.as_deref(),
            MAX_REGENERATION_WEB_RESEARCH_QUERY_LENGTH,
        ) {
            return Err(BuildRegenerationAiServiceError::WebResearchQueryTooLong);
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn enable_web_research(&self) -> Option<bool> {
        self.enable_web_research
    }

    #[cfg(test)]
    pub fn web_research_query(&self) -> Option<&str> {
        self.web_research_query.as_deref()
    }

    pub fn preserve_structure(&self) -> bool {
        self.preserve_structure
    }

    pub fn preserve_dialogues(&self) -> &[String] {
        &self.preserve_dialogues
    }

    pub fn preserve_plot_points(&self) -> &[String] {
        &self.preserve_plot_points
    }

    pub fn preserve_character_traits(&self) -> bool {
        self.preserve_character_traits
    }

    pub fn story_repair_targets(&self) -> &[String] {
        &self.story_repair_targets
    }

    pub fn story_preserve_strengths(&self) -> &[String] {
        &self.story_preserve_strengths
    }

    pub fn style_id(&self) -> Option<i32> {
        self.style_id
    }

    pub fn version_note(&self) -> Option<&str> {
        self.version_note.as_deref()
    }

    pub fn auto_apply(&self) -> bool {
        self.auto_apply
    }

    pub fn compat_options_with_web_research_default(
        &self,
        web_research_default: bool,
    ) -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: self.style_id,
            enable_analysis: true,
            enable_mcp: true,
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
}

pub(crate) fn build_full_chapter_regeneration_stream_request_from_route_payload(
    route_request: FullChapterRegenerationStreamRouteRequest,
) -> FullChapterRegenerationStreamRequest {
    FullChapterRegenerationStreamRequest::from_route_request(route_request)
}

pub(crate) fn validate_full_chapter_regeneration_stream_request_bounds(
    request: &FullChapterRegenerationStreamRequest,
) -> Result<(), BuildRegenerationAiServiceError> {
    request.validate_request_bounds()
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct PartialRegenerationStreamRouteRequest {
    pub(crate) selected_text: String,
    pub(crate) start_position: usize,
    pub(crate) end_position: usize,
    pub(crate) user_instructions: String,
    pub(crate) context_chars: Option<usize>,
    pub(crate) style_id: Option<i32>,
    pub(crate) length_mode: Option<String>,
    pub(crate) target_word_count: Option<usize>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PartialRegenerationStreamWorkflowRequest {
    pub(crate) selected_text: String,
    pub(crate) start_position: usize,
    pub(crate) end_position: usize,
    pub(crate) context_chars: Option<usize>,
    pub(crate) user_instructions: String,
    pub(crate) length_mode: Option<String>,
    pub(crate) target_word_count: Option<usize>,
    pub(crate) style_id: Option<i32>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
}

impl PartialRegenerationStreamWorkflowRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        selected_text: String,
        start_position: usize,
        end_position: usize,
        context_chars: Option<usize>,
        user_instructions: String,
        length_mode: Option<String>,
        target_word_count: Option<usize>,
        style_id: Option<i32>,
        enable_web_research: Option<bool>,
        web_research_query: Option<String>,
    ) -> Self {
        Self {
            selected_text,
            start_position,
            end_position,
            context_chars,
            user_instructions,
            length_mode,
            target_word_count,
            style_id,
            enable_web_research,
            web_research_query,
        }
    }

    fn from_route_request(route_request: PartialRegenerationStreamRouteRequest) -> Self {
        Self::new(
            route_request.selected_text,
            route_request.start_position,
            route_request.end_position,
            route_request.context_chars,
            normalize_optional_regeneration_request_string(Some(route_request.user_instructions))
                .unwrap_or_default(),
            normalize_optional_regeneration_request_string(route_request.length_mode),
            route_request.target_word_count,
            route_request.style_id,
            route_request.enable_web_research,
            normalize_optional_regeneration_request_string(route_request.web_research_query),
        )
    }

    pub fn selected_text(&self) -> &str {
        &self.selected_text
    }

    pub fn start_position(&self) -> usize {
        self.start_position
    }

    pub fn end_position(&self) -> usize {
        self.end_position
    }

    pub fn context_chars(&self) -> usize {
        normalize_partial_regeneration_context_chars(self.context_chars)
    }

    pub fn user_instructions(&self) -> &str {
        &self.user_instructions
    }

    pub fn length_mode(&self) -> Option<&str> {
        self.length_mode.as_deref()
    }

    pub fn target_word_count(&self) -> Option<usize> {
        self.target_word_count
    }

    pub fn style_id(&self) -> Option<i32> {
        self.style_id
    }

    #[cfg(test)]
    pub fn web_research_enabled(&self) -> bool {
        normalize_partial_regeneration_web_research_enabled(self.enable_web_research)
    }

    #[cfg(test)]
    pub fn web_research_query(&self) -> Option<&str> {
        self.web_research_query.as_deref()
    }

    pub(crate) fn validate_request_bounds(&self) -> Result<(), PreparePartialRegenerationError> {
        if self.start_position >= self.end_position {
            return Err(PreparePartialRegenerationError::InvalidRange);
        }
        if self.user_instructions.is_empty() {
            return Err(PreparePartialRegenerationError::EmptyUserInstructions);
        }
        if self.user_instructions.chars().count()
            > MAX_PARTIAL_REGENERATION_USER_INSTRUCTIONS_LENGTH
        {
            return Err(PreparePartialRegenerationError::UserInstructionsTooLong);
        }
        if let Some(context_chars) = self.context_chars {
            if context_chars < MIN_PARTIAL_REGENERATION_CONTEXT_CHARS {
                return Err(PreparePartialRegenerationError::ContextCharsTooSmall);
            }
            if context_chars > MAX_PARTIAL_REGENERATION_CONTEXT_CHARS {
                return Err(PreparePartialRegenerationError::ContextCharsTooLarge);
            }
        }
        if let Some(target_word_count) = self.target_word_count {
            if target_word_count < MIN_PARTIAL_REGENERATION_TARGET_WORD_COUNT {
                return Err(PreparePartialRegenerationError::TargetWordCountTooSmall);
            }
            if target_word_count > MAX_PARTIAL_REGENERATION_TARGET_WORD_COUNT {
                return Err(PreparePartialRegenerationError::TargetWordCountTooLarge);
            }
        }
        if !is_valid_optional_regeneration_text_length(
            self.web_research_query.as_deref(),
            MAX_PARTIAL_REGENERATION_WEB_RESEARCH_QUERY_LENGTH,
        ) {
            return Err(PreparePartialRegenerationError::WebResearchQueryTooLong);
        }

        Ok(())
    }

    pub fn compat_options_with_web_research_default(
        &self,
        web_research_default: bool,
    ) -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: self.style_id,
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: self.enable_web_research.unwrap_or(web_research_default),
            web_research_query: self.web_research_query.clone(),
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
}

pub(crate) fn build_partial_regeneration_stream_workflow_request_from_route_payload(
    route_request: PartialRegenerationStreamRouteRequest,
) -> PartialRegenerationStreamWorkflowRequest {
    PartialRegenerationStreamWorkflowRequest::from_route_request(route_request)
}

pub(crate) fn validate_partial_regeneration_stream_request_bounds(
    request: &PartialRegenerationStreamWorkflowRequest,
) -> Result<(), PreparePartialRegenerationError> {
    request.validate_request_bounds()
}

fn normalize_partial_regeneration_context_chars(context_chars: Option<usize>) -> usize {
    context_chars.unwrap_or(500)
}

#[cfg(test)]
fn normalize_partial_regeneration_web_research_enabled(enabled: Option<bool>) -> bool {
    enabled.unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PartialRegenerationLengthMode {
    Similar,
    Expand,
    Condense,
    Custom,
}

impl PartialRegenerationLengthMode {
    pub(crate) fn normalize(length_mode: Option<&str>) -> Self {
        match length_mode.unwrap_or("similar") {
            "expand" => PartialRegenerationLengthMode::Expand,
            "condense" => PartialRegenerationLengthMode::Condense,
            "custom" => PartialRegenerationLengthMode::Custom,
            _ => PartialRegenerationLengthMode::Similar,
        }
    }

    pub(crate) fn resolve_plan(
        self,
        target_word_count: Option<usize>,
        original_word_count: usize,
    ) -> PartialRegenerationLengthPlan {
        match self {
            PartialRegenerationLengthMode::Expand => {
                let min_words = (original_word_count as f64 * 1.2) as usize;
                let max_words = (original_word_count as f64 * 2.0) as usize;
                PartialRegenerationLengthPlan {
                    requirement: format!("建议扩写至 {}-{} 字", min_words, max_words),
                    target_words: max_words,
                }
            }
            PartialRegenerationLengthMode::Condense => {
                let min_words = (original_word_count as f64 * 0.5) as usize;
                let max_words = (original_word_count as f64 * 0.8) as usize;
                PartialRegenerationLengthPlan {
                    requirement: format!("建议压缩至 {}-{} 字", min_words, max_words),
                    target_words: (original_word_count as f64 * 1.5) as usize,
                }
            }
            PartialRegenerationLengthMode::Custom => PartialRegenerationLengthPlan {
                requirement: target_word_count
                    .map(|count| format!("目标长度约 {} 字，允许上下浮动 20%", count))
                    .unwrap_or_else(|| {
                        format!("默认按接近原文长度处理，原文约 {} 字", original_word_count)
                    }),
                target_words: target_word_count
                    .unwrap_or_else(|| (original_word_count as f64 * 1.5) as usize),
            },
            PartialRegenerationLengthMode::Similar => {
                let min_words = (original_word_count as f64 * 0.8) as usize;
                let max_words = (original_word_count as f64 * 1.2) as usize;
                PartialRegenerationLengthPlan {
                    requirement: format!(
                        "尽量保持与原文接近，原文约 {} 字，目标 {}-{} 字",
                        original_word_count, min_words, max_words
                    ),
                    target_words: (original_word_count as f64 * 1.5) as usize,
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PartialRegenerationLengthPlan {
    pub(crate) requirement: String,
    pub(crate) target_words: usize,
}

impl PartialRegenerationLengthPlan {
    fn requirement(self) -> String {
        self.requirement
    }

    fn target_words(self) -> usize {
        self.target_words
    }
}

pub(crate) fn build_partial_length_requirement(
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    original_word_count: usize,
) -> String {
    PartialRegenerationLengthMode::normalize(length_mode)
        .resolve_plan(target_word_count, original_word_count)
        .requirement()
}

pub(crate) fn calculate_partial_target_words(
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    original_word_count: usize,
) -> usize {
    PartialRegenerationLengthMode::normalize(length_mode)
        .resolve_plan(target_word_count, original_word_count)
        .target_words()
}
