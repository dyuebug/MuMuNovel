use std::cmp::{max, min};

use serde::Deserialize;
use serde_json::Value;

use crate::ai::service::AIService;
use crate::models::chapter;
use crate::services::chapter_generation_context_compaction_service::compact_generation_context;
use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions;
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::chapter_generation_prompt_service::PreviousChapterPromptContext;
use crate::services::chapter_generation_research_payload_service::build_single_chapter_research_provider_payload;
use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationTarget;
use crate::services::settings_service::SettingsService;
use crate::services::writing_style_service::WritingStyleService;

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

#[derive(Debug, PartialEq, Eq)]
pub enum BuildRegenerationAiServiceError {
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
pub enum PreparePartialRegenerationError {
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

pub enum PreparePartialRegenerationStreamError {
    Input(PreparePartialRegenerationError),
    Style(String),
    Config(BuildRegenerationAiServiceError),
}

pub struct FullChapterRegenerationStreamInput {
    pub chapter_id: String,
    pub chapter_word_count: usize,
    pub prompt: String,
    pub ai_service: AIService,
}

pub struct PartialChapterRegenerationStreamInput {
    pub target_words: usize,
    pub original_word_count: usize,
    pub start_position: usize,
    pub end_position: usize,
    pub prompt: String,
    pub ai_service: AIService,
}

pub struct PreparedPartialRegenerationInput {
    pub original_word_count: usize,
    pub target_words: usize,
    pub max_tokens: u32,
    pub prompt: String,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct FullChapterRegenerationStreamRouteRequest {
    pub target_word_count: Option<i64>,
    pub custom_instructions: Option<String>,
    #[serde(default)]
    pub selected_suggestion_indices: Vec<Value>,
    #[serde(default)]
    pub focus_areas: Vec<Value>,
    pub story_creation_brief: Option<String>,
    pub quality_notes: Option<String>,
    pub story_repair_summary: Option<String>,
    pub creative_mode: Option<String>,
    pub story_focus: Option<String>,
    pub plot_stage: Option<String>,
    pub quality_preset: Option<String>,
    pub enable_web_research: Option<bool>,
    pub web_research_query: Option<String>,
    pub preserve_elements: Option<Value>,
    #[serde(default)]
    pub story_repair_targets: Vec<Value>,
    #[serde(default)]
    pub story_preserve_strengths: Vec<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FullChapterRegenerationStreamRequest {
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
    ) -> Self {
        Self {
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
        )
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

    fn validate_request_bounds(&self) -> Result<(), BuildRegenerationAiServiceError> {
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

    pub fn enable_web_research(&self) -> Option<bool> {
        self.enable_web_research
    }

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

    pub fn compat_options_with_web_research_default(
        &self,
        web_research_default: bool,
    ) -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: None,
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

pub fn build_full_chapter_regeneration_stream_request_from_route_payload(
    route_request: FullChapterRegenerationStreamRouteRequest,
) -> FullChapterRegenerationStreamRequest {
    FullChapterRegenerationStreamRequest::from_route_request(route_request)
}

pub(crate) fn validate_full_chapter_regeneration_stream_request_bounds(
    request: &FullChapterRegenerationStreamRequest,
) -> Result<(), BuildRegenerationAiServiceError> {
    request.validate_request_bounds()
}

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

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PartialRegenerationStreamRouteRequest {
    pub selected_text: String,
    pub start_position: usize,
    pub end_position: usize,
    pub user_instructions: String,
    pub context_chars: Option<usize>,
    pub style_id: Option<i32>,
    pub length_mode: Option<String>,
    pub target_word_count: Option<usize>,
    pub enable_web_research: Option<bool>,
    pub web_research_query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialRegenerationStreamWorkflowRequest {
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

    fn validate_request_bounds(&self) -> Result<(), PreparePartialRegenerationError> {
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

pub fn build_partial_regeneration_stream_workflow_request_from_route_payload(
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
enum PartialRegenerationLengthMode {
    Similar,
    Expand,
    Condense,
    Custom,
}

impl PartialRegenerationLengthMode {
    fn normalize(length_mode: Option<&str>) -> Self {
        match length_mode.unwrap_or("similar") {
            "expand" => PartialRegenerationLengthMode::Expand,
            "condense" => PartialRegenerationLengthMode::Condense,
            "custom" => PartialRegenerationLengthMode::Custom,
            _ => PartialRegenerationLengthMode::Similar,
        }
    }

    fn resolve_plan(
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
struct PartialRegenerationLengthPlan {
    requirement: String,
    target_words: usize,
}

impl PartialRegenerationLengthPlan {
    fn requirement(self) -> String {
        self.requirement
    }

    fn target_words(self) -> usize {
        self.target_words
    }
}

fn join_regeneration_prompt_items(items: &[String], separator: &str) -> String {
    items.join(separator)
}

fn build_regeneration_external_assets_block(
    external_assets: Option<&str>,
    reference_assets: Option<&str>,
) -> String {
    let external_assets = external_assets.unwrap_or_default().trim();
    let reference_assets = reference_assets.unwrap_or_default().trim();
    if (external_assets.is_empty() || external_assets == "[]")
        && (reference_assets.is_empty() || reference_assets == "[]")
    {
        return "（未提供）".to_string();
    }

    let mut lines = Vec::new();
    if !external_assets.is_empty() && external_assets != "[]" {
        lines.push(format!("external_assets: {}", external_assets));
    }
    if !reference_assets.is_empty() && reference_assets != "[]" {
        lines.push(format!("reference_assets: {}", reference_assets));
    }

    if lines.is_empty() {
        "（未提供）".to_string()
    } else {
        lines.join("\n")
    }
}

pub fn build_regeneration_prompt(
    chapter: &chapter::Model,
    request: &FullChapterRegenerationStreamRequest,
    provider_payload: &PromptContextProviderPayload,
    web_research_note: Option<&str>,
    external_assets: Option<&str>,
    reference_assets: Option<&str>,
) -> String {
    let web_research_note = web_research_note.unwrap_or("（未启用）");
    let external_assets_block =
        build_regeneration_external_assets_block(external_assets, reference_assets);
    format!(
        "你是小说正文重写助手。请基于以下章节内容和要求输出重写后的正文，不要输出解释。\n\n章节标题：{}\n章节编号：{}\n目标字数：{}\n\n原章节内容：\n{}\n\n用户修改要求：\n{}\n\n选中建议索引：{}\n重点优化方向：{}\n创作模式：{}\n故事关注点：{}\n质量预设：{}\n\n最近章节规划：\n{}\n\n上一章已完成剧情：\n{}\n\n本章角色信息：\n{}\n\n本章职业信息：\n{}\n\n伏笔提醒：\n{}\n\n相关记忆：\n{}\n\n联网检索说明：{}\n外部参考资料：\n{}\n保留结构：{}\n保留对话：{}\n保留剧情点：{}\n保留人物特征：{}\n创作总控：{}\n质量补充偏好：{}\n剧情质量修复摘要：{}\n修复目标：{}\n保留优势：{}\n\n要求：\n- 只输出可直接替换的正文内容\n- 不要输出标题、编号、前言、后记或流程说明\n- 如果有角色/世界观信息，保持一致\n- 尽量保留原有剧情骨架",
        chapter.title,
        chapter.chapter_number,
        request.target_word_count(),
        chapter.content.clone().unwrap_or_default(),
        request.custom_instructions(),
        join_regeneration_prompt_items(request.selected_suggestion_indices(), ", "),
        join_regeneration_prompt_items(request.focus_areas(), "、"),
        request.creative_mode(),
        request.story_focus(),
        request.quality_preset(),
        if provider_payload.recent_chapters_context.trim().is_empty() {
            "（未提供）"
        } else {
            provider_payload.recent_chapters_context.as_str()
        },
        if provider_payload.previous_chapter_summary.trim().is_empty() {
            "（未提供）"
        } else {
            provider_payload.previous_chapter_summary.as_str()
        },
        if provider_payload.characters_info.trim().is_empty()
            || provider_payload.characters_info == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.characters_info.as_str()
        },
        if provider_payload.chapter_careers.trim().is_empty()
            || provider_payload.chapter_careers == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.chapter_careers.as_str()
        },
        if provider_payload.foreshadow_reminders.trim().is_empty()
            || provider_payload.foreshadow_reminders == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.foreshadow_reminders.as_str()
        },
        if provider_payload.relevant_memories.trim().is_empty()
            || provider_payload.relevant_memories == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.relevant_memories.as_str()
        },
        web_research_note,
        external_assets_block,
        request.preserve_structure(),
        join_regeneration_prompt_items(request.preserve_dialogues(), "、"),
        join_regeneration_prompt_items(request.preserve_plot_points(), "、"),
        request.preserve_character_traits(),
        request.story_creation_brief(),
        request.quality_notes(),
        request.story_repair_summary(),
        join_regeneration_prompt_items(request.story_repair_targets(), "、"),
        join_regeneration_prompt_items(request.story_preserve_strengths(), "、"),
    )
}

pub fn build_partial_length_requirement(
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    original_word_count: usize,
) -> String {
    PartialRegenerationLengthMode::normalize(length_mode)
        .resolve_plan(target_word_count, original_word_count)
        .requirement()
}

pub fn calculate_partial_target_words(
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    original_word_count: usize,
) -> usize {
    PartialRegenerationLengthMode::normalize(length_mode)
        .resolve_plan(target_word_count, original_word_count)
        .target_words()
}

pub fn build_partial_regeneration_prompt(
    chapter: &chapter::Model,
    selected_text: &str,
    context_before: &str,
    context_after: &str,
    user_instructions: &str,
    length_requirement: &str,
    style_content: Option<&str>,
    provider_payload: &PromptContextProviderPayload,
    web_research_note: Option<&str>,
    external_assets: Option<&str>,
    reference_assets: Option<&str>,
) -> String {
    let style_content = style_content.unwrap_or("（未提供风格约束）");
    let web_research_note = web_research_note.unwrap_or("（未启用）");
    let external_assets_block =
        build_regeneration_external_assets_block(external_assets, reference_assets);

    format!(
        "你是小说正文局部重写助手。请基于以下内容重写选中片段，只输出可直接替换的正文内容，不要输出解释。\n\n章节标题：{}\n章节编号：{}\n原文选中片段：\n{}\n\n前文上下文：\n{}\n\n后文上下文：\n{}\n\n用户修改要求：\n{}\n\n长度要求：{}\n\n风格约束：\n{}\n\n上一章已完成剧情：\n{}\n\n本章角色信息：\n{}\n\n本章职业信息：\n{}\n\n伏笔提醒：\n{}\n\n相关记忆：\n{}\n\n联网检索说明：{}\n\n外部参考资料：\n{}\n\n要求：\n- 只输出重写后的正文\n- 不要输出标题、编号、前言、后记或流程说明\n- 保持人物、设定与上下文一致\n- 尽量贴合原文节奏与叙事视角",
        chapter.title,
        chapter.chapter_number,
        selected_text,
        if context_before.is_empty() {
            "（无前文上下文）"
        } else {
            context_before
        },
        if context_after.is_empty() {
            "（无后文上下文）"
        } else {
            context_after
        },
        if user_instructions.is_empty() {
            "（无额外要求）"
        } else {
            user_instructions
        },
        length_requirement,
        style_content,
        if provider_payload.previous_chapter_summary.trim().is_empty() {
            "（未提供）"
        } else {
            provider_payload.previous_chapter_summary.as_str()
        },
        if provider_payload.characters_info.trim().is_empty()
            || provider_payload.characters_info == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.characters_info.as_str()
        },
        if provider_payload.chapter_careers.trim().is_empty()
            || provider_payload.chapter_careers == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.chapter_careers.as_str()
        },
        if provider_payload.foreshadow_reminders.trim().is_empty()
            || provider_payload.foreshadow_reminders == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.foreshadow_reminders.as_str()
        },
        if provider_payload.relevant_memories.trim().is_empty()
            || provider_payload.relevant_memories == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.relevant_memories.as_str()
        },
        web_research_note,
        external_assets_block,
    )
}

pub fn prepare_partial_regeneration_input(
    chapter: &chapter::Model,
    selected_text_override: &str,
    start_position: usize,
    end_position: usize,
    context_chars: usize,
    user_instructions: &str,
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    style_content: Option<&str>,
    provider_payload: &PromptContextProviderPayload,
    web_research_note: Option<&str>,
    external_assets: Option<&str>,
    reference_assets: Option<&str>,
) -> Result<PreparedPartialRegenerationInput, PreparePartialRegenerationError> {
    let current_content = chapter.content.clone().unwrap_or_default();
    let content_chars: Vec<char> = current_content.chars().collect();
    let content_length = content_chars.len();
    if start_position >= end_position || end_position > content_length {
        return Err(PreparePartialRegenerationError::InvalidRange);
    }

    let selected_text_from_content: String =
        content_chars[start_position..end_position].iter().collect();
    let selected_text = {
        let provided = selected_text_override.trim();
        if provided.is_empty() {
            selected_text_from_content
        } else {
            provided.to_string()
        }
    };
    if selected_text.trim().is_empty() {
        return Err(PreparePartialRegenerationError::EmptySelectedText);
    }

    let context_before_start = start_position.saturating_sub(context_chars);
    let context_before: String = content_chars[context_before_start..start_position]
        .iter()
        .collect();
    let context_after_end = end_position
        .saturating_add(context_chars)
        .min(content_length);
    let context_after: String = content_chars[end_position..context_after_end]
        .iter()
        .collect();

    let original_word_count = selected_text.chars().count();
    let length_requirement =
        build_partial_length_requirement(length_mode, target_word_count, original_word_count);
    let target_words =
        calculate_partial_target_words(length_mode, target_word_count, original_word_count);
    let max_tokens = max(500, min(target_words.saturating_mul(3), 8000)) as u32;
    let prompt = build_partial_regeneration_prompt(
        chapter,
        &selected_text,
        &context_before,
        &context_after,
        user_instructions,
        &length_requirement,
        style_content,
        provider_payload,
        web_research_note,
        external_assets,
        reference_assets,
    );

    Ok(PreparedPartialRegenerationInput {
        original_word_count,
        target_words,
        max_tokens,
        prompt,
    })
}

pub async fn build_regeneration_ai_service(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    max_tokens_override: Option<u32>,
) -> Result<AIService, BuildRegenerationAiServiceError> {
    let mut ai_config = SettingsService::build_ai_config(db, user_id, None, None, None)
        .await
        .map_err(BuildRegenerationAiServiceError::InvalidConfig)?;
    if let Some(max_tokens) = max_tokens_override {
        ai_config.max_tokens = max_tokens;
    }
    Ok(AIService::new(ai_config))
}

pub async fn load_partial_style_content(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    style_id: Option<i32>,
) -> Result<Option<String>, String> {
    let Some(style_id) = style_id else {
        return Ok(None);
    };

    let value = WritingStyleService::get_style(db, user_id, style_id)
        .await
        .map_err(|error| error.to_string())?;

    Ok(value
        .get("prompt_content")
        .and_then(Value::as_str)
        .map(str::to_string))
}

pub async fn prepare_chapter_regeneration_stream(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter: &chapter::Model,
    request: &FullChapterRegenerationStreamRequest,
) -> Result<FullChapterRegenerationStreamInput, BuildRegenerationAiServiceError> {
    request.validate_request_bounds()?;

    let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
        .await
        .map_err(|error| BuildRegenerationAiServiceError::InvalidConfig(error.to_string()))?;
    let compat_options = request.compat_options_with_web_research_default(web_research_default);
    let provider_payload = build_single_chapter_research_provider_payload(
        db,
        user_id,
        &SingleChapterGenerationTarget {
            project_id: chapter.project_id.clone(),
            chapter_id: chapter.id.clone(),
            chapter_number: chapter.chapter_number,
            title: chapter.title.clone(),
        },
        &compat_options,
    )
    .await
    .map_err(BuildRegenerationAiServiceError::InvalidConfig)?;
    let (provider_payload, _) = compact_generation_context(
        "one-to-many",
        request.target_word_count() as i32,
        provider_payload,
        PreviousChapterPromptContext::default(),
    );
    let web_research_note = if compat_options.web_research_enabled() {
        compat_options
            .web_research_query()
            .map(|query| format!("已请求联网检索，检索问题：{}", query))
            .or_else(|| Some("已请求联网检索，请优先吸收外部资料中的事实与细节。".to_string()))
    } else {
        None
    };
    let prompt = build_regeneration_prompt(
        chapter,
        request,
        &provider_payload,
        web_research_note.as_deref(),
        Some(&provider_payload.external_assets),
        Some(&provider_payload.reference_assets),
    );
    let ai_service = build_regeneration_ai_service(db, user_id, None).await?;

    Ok(FullChapterRegenerationStreamInput {
        chapter_id: chapter.id.clone(),
        chapter_word_count: chapter.word_count as usize,
        prompt,
        ai_service,
    })
}

pub async fn prepare_partial_regeneration_stream(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter: &chapter::Model,
    request: &PartialRegenerationStreamWorkflowRequest,
) -> Result<PartialChapterRegenerationStreamInput, PreparePartialRegenerationStreamError> {
    request
        .validate_request_bounds()
        .map_err(PreparePartialRegenerationStreamError::Input)?;

    let style_content = load_partial_style_content(db, user_id, request.style_id())
        .await
        .map_err(PreparePartialRegenerationStreamError::Style)?;

    let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
        .await
        .map_err(|error| {
            PreparePartialRegenerationStreamError::Config(
                BuildRegenerationAiServiceError::InvalidConfig(error.to_string()),
            )
        })?;
    let compat_options = request.compat_options_with_web_research_default(web_research_default);
    let provider_payload = build_single_chapter_research_provider_payload(
        db,
        user_id,
        &SingleChapterGenerationTarget {
            project_id: chapter.project_id.clone(),
            chapter_id: chapter.id.clone(),
            chapter_number: chapter.chapter_number,
            title: chapter.title.clone(),
        },
        &compat_options,
    )
    .await
    .map_err(|error| {
        PreparePartialRegenerationStreamError::Config(
            BuildRegenerationAiServiceError::InvalidConfig(error),
        )
    })?;
    let (provider_payload, _) = compact_generation_context(
        "one-to-one",
        request
            .target_word_count()
            .unwrap_or(chapter.word_count as usize) as i32,
        provider_payload,
        PreviousChapterPromptContext::default(),
    );

    let web_research_note = if compat_options.web_research_enabled() {
        compat_options
            .web_research_query()
            .map(|query| format!("已请求联网检索，检索问题：{}", query))
            .or_else(|| Some("已请求联网检索，请优先吸收外部资料中的事实与细节。".to_string()))
    } else {
        None
    };

    let prepared = prepare_partial_regeneration_input(
        chapter,
        request.selected_text(),
        request.start_position(),
        request.end_position(),
        request.context_chars(),
        request.user_instructions(),
        request.length_mode(),
        request.target_word_count(),
        style_content.as_deref(),
        &provider_payload,
        web_research_note.as_deref(),
        Some(&provider_payload.external_assets),
        Some(&provider_payload.reference_assets),
    )
    .map_err(PreparePartialRegenerationStreamError::Input)?;

    let ai_service = build_regeneration_ai_service(db, user_id, Some(prepared.max_tokens))
        .await
        .map_err(PreparePartialRegenerationStreamError::Config)?;

    Ok(PartialChapterRegenerationStreamInput {
        target_words: prepared.target_words,
        original_word_count: prepared.original_word_count,
        start_position: request.start_position(),
        end_position: request.end_position(),
        prompt: prepared.prompt,
        ai_service,
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use serde_json::Value;

    use crate::models::chapter;
    use crate::services::chapter_generation_prompt_context_provider_service::{
        build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
    };

    use super::{
        build_full_chapter_regeneration_stream_request_from_route_payload,
        build_partial_length_requirement,
        build_partial_regeneration_stream_workflow_request_from_route_payload,
        build_regeneration_prompt, calculate_partial_target_words,
        prepare_partial_regeneration_input, BuildRegenerationAiServiceError,
        FullChapterRegenerationStreamRequest, FullChapterRegenerationStreamRouteRequest,
        PartialRegenerationLengthMode, PartialRegenerationStreamRouteRequest,
        PartialRegenerationStreamWorkflowRequest, PreparePartialRegenerationError,
        PreparedPartialRegenerationInput,
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
    fn should_build_regeneration_prompt_with_explicit_fields() {
        let chapter = chapter_with_content("原始正文");
        let route_request = FullChapterRegenerationStreamRouteRequest {
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
}
