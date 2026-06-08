use serde::{Deserialize, Serialize};

use crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig;
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
    pub(crate) fn style_id(&self) -> Option<i32> {
        self.style_id
    }

    pub(crate) fn enable_analysis(&self) -> bool {
        self.enable_analysis
    }

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

#[cfg(test)]
mod tests {
    use super::{build_prompt_overrides_from_compat_options, SingleChapterGenerationCompatOptions};

    #[test]
    fn should_skip_empty_prompt_override_values() {
        let compat = SingleChapterGenerationCompatOptions {
            creative_mode: Some("   ".to_string()),
            story_focus: Some("advance_plot".to_string()),
            web_research_enabled: false,
            ..Default::default()
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert_eq!(prompt_overrides.creative_mode, None);
        assert_eq!(
            prompt_overrides.story_focus.as_deref(),
            Some("advance_plot")
        );
    }

    #[test]
    fn should_include_web_research_fields_in_prompt_overrides() {
        let compat = SingleChapterGenerationCompatOptions {
            style_id: Some(3),
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: true,
            web_research_query: Some("民国报馆夜班排印流程".to_string()),
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
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert!(prompt_overrides.web_research_enabled);
        assert_eq!(
            prompt_overrides.web_research_query.as_deref(),
            Some("民国报馆夜班排印流程")
        );
    }
}
