#[derive(Debug, Clone, Default)]
pub struct BatchGenerationRequestCompatFields {
    pub enable_mcp: Option<bool>,
    pub enable_web_research: Option<bool>,
    pub web_research_query: Option<String>,
    pub creative_mode: Option<String>,
    pub story_focus: Option<String>,
    pub plot_stage: Option<String>,
    pub story_creation_brief: Option<String>,
    pub quality_preset: Option<String>,
    pub quality_notes: Option<String>,
    pub story_repair_summary: Option<String>,
    pub story_repair_targets: Option<Vec<String>>,
    pub story_preserve_strengths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy)]
pub struct BatchGenerationRequestCompatView<'a> {
    pub enable_mcp: Option<bool>,
    pub enable_web_research: Option<bool>,
    pub web_research_query: Option<&'a str>,
    pub creative_mode: Option<&'a str>,
    pub story_focus: Option<&'a str>,
    pub plot_stage: Option<&'a str>,
    pub story_creation_brief: Option<&'a str>,
    pub quality_preset: Option<&'a str>,
    pub quality_notes: Option<&'a str>,
    pub story_repair_summary: Option<&'a str>,
    pub story_repair_targets: Option<&'a [String]>,
    pub story_preserve_strengths: Option<&'a [String]>,
}

pub fn project_batch_generation_request_compat_fields<'a>(
    fields: &'a BatchGenerationRequestCompatFields,
) -> BatchGenerationRequestCompatView<'a> {
    BatchGenerationRequestCompatView {
        enable_mcp: fields.enable_mcp,
        enable_web_research: fields.enable_web_research,
        web_research_query: fields.web_research_query.as_deref(),
        creative_mode: fields.creative_mode.as_deref(),
        story_focus: fields.story_focus.as_deref(),
        plot_stage: fields.plot_stage.as_deref(),
        story_creation_brief: fields.story_creation_brief.as_deref(),
        quality_preset: fields.quality_preset.as_deref(),
        quality_notes: fields.quality_notes.as_deref(),
        story_repair_summary: fields.story_repair_summary.as_deref(),
        story_repair_targets: fields.story_repair_targets.as_deref(),
        story_preserve_strengths: fields.story_preserve_strengths.as_deref(),
    }
}

pub fn consume_batch_generation_request_compat_fields(
    fields: &BatchGenerationRequestCompatFields,
) {
    let compat = project_batch_generation_request_compat_fields(fields);
    let _ = (
        compat.enable_mcp,
        compat.enable_web_research,
        compat.web_research_query,
        compat.creative_mode,
        compat.story_focus,
        compat.plot_stage,
        compat.story_creation_brief,
        compat.quality_preset,
        compat.quality_notes,
        compat.story_repair_summary,
        compat.story_repair_targets,
        compat.story_preserve_strengths,
    );
}
