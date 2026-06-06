use std::collections::HashMap;

use crate::models::{chapter, project};
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::prompt_template_service::PromptTemplateService;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChapterGenerationPromptOverrides {
    pub narrative_perspective: Option<String>,
    pub creative_mode: Option<String>,
    pub story_focus: Option<String>,
    pub plot_stage: Option<String>,
    pub story_creation_brief: Option<String>,
    pub quality_preset: Option<String>,
    pub quality_notes: Option<String>,
    pub web_research_enabled: bool,
    pub web_research_query: Option<String>,
    pub story_repair_summary: Option<String>,
    pub story_repair_targets: Vec<String>,
    pub story_preserve_strengths: Vec<String>,
}

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

fn resolve_prompt_preference(
    override_value: Option<&str>,
    project_default: Option<&str>,
) -> String {
    override_value
        .filter(|value| !value.trim().is_empty())
        .or(project_default.filter(|value| !value.trim().is_empty()))
        .unwrap_or_default()
        .to_string()
}

fn build_optional_instruction_block(label: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        String::new()
    } else {
        format!("【{}】\n{}\n", label, value)
    }
}

fn normalize_prompt_list(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn build_repair_target_block(targets: &[String], strengths: &[String]) -> String {
    let targets = normalize_prompt_list(targets);
    let strengths = normalize_prompt_list(strengths);

    if targets.is_empty() && strengths.is_empty() {
        return String::new();
    }

    let mut lines = vec!["【修复目标】".to_string()];
    if !targets.is_empty() {
        lines.push(format!("需要修复：{}", targets.join("；")));
    }
    if !strengths.is_empty() {
        lines.push(format!("必须保留：{}", strengths.join("；")));
    }

    format!("{}\n", lines.join("\n"))
}

fn build_repair_diagnostic_block(
    summary: &str,
    targets: &[String],
    strengths: &[String],
) -> String {
    let summary = summary.trim();
    let targets = normalize_prompt_list(targets);
    let strengths = normalize_prompt_list(strengths);

    if summary.is_empty() && targets.is_empty() && strengths.is_empty() {
        return String::new();
    }

    let mut lines = vec!["【修复诊断】".to_string()];
    if !summary.is_empty() {
        lines.push(summary.to_string());
    }
    if !targets.is_empty() {
        lines.push(format!("本章修复项：{}", targets.join("；")));
    }
    if !strengths.is_empty() {
        lines.push(format!("保留优势：{}", strengths.join("；")));
    }

    format!("{}\n", lines.join("\n"))
}

fn build_web_research_block(enabled: bool, query: Option<&str>) -> String {
    if !enabled {
        return String::new();
    }

    let note = query
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(|query| {
            format!(
                "已请求联网检索，优先吸收与以下问题直接相关的资料：{}",
                query
            )
        })
        .unwrap_or_else(|| {
            "已请求联网检索，可适度补充与本章设定、背景、职业、场景相关的外部事实参考。".to_string()
        });

    format!("【联网检索说明】\n{}\n", note)
}

fn build_external_assets_block(
    external_assets: &str,
    reference_assets: &str,
    mcp_references: &str,
) -> String {
    let external_assets = external_assets.trim();
    let reference_assets = reference_assets.trim();
    let mcp_references = mcp_references.trim();

    if (external_assets.is_empty() || external_assets == "[]")
        && (reference_assets.is_empty() || reference_assets == "[]")
        && mcp_references.is_empty()
    {
        return String::new();
    }

    let mut lines = vec!["【外部参考资产】".to_string()];
    if !external_assets.is_empty() && external_assets != "[]" {
        lines.push(format!("external_assets: {}", external_assets));
    }
    if !reference_assets.is_empty() && reference_assets != "[]" {
        lines.push(format!("reference_assets: {}", reference_assets));
    }
    if !mcp_references.is_empty() {
        lines.push(format!("mcp_references: {}", mcp_references));
    }

    format!("{}\n", lines.join("\n"))
}

pub fn chapter_template_key(outline_mode: &str, has_previous: bool) -> &'static str {
    match (outline_mode, has_previous) {
        ("one-to-many", false) => "CHAPTER_GENERATION_ONE_TO_MANY",
        ("one-to-many", true) => "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
        ("one-to-one", false) | (_, false) => "CHAPTER_GENERATION_ONE_TO_ONE",
        _ => "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
    }
}

fn build_prompt_params_with_provider_payload(
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
        build_optional_instruction_block("创作模式", &creative_mode),
    );
    params.insert("story_focus".to_string(), story_focus.clone());
    params.insert(
        "story_focus_block".to_string(),
        build_optional_instruction_block("故事侧重点", &story_focus),
    );
    params.insert("plot_stage".to_string(), plot_stage);
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
        "quality_external_assets_block".to_string(),
        external_assets_block,
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

pub fn build_prompt_with_provider_payload(
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

    PromptTemplateService::format_prompt(&template.content, &params)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{
        build_previous_chapter_prompt_context, build_prompt_params_with_provider_payload,
        build_prompt_with_provider_payload, chapter_template_key, ChapterGenerationPromptOverrides,
    };
    use crate::models::{chapter, project};
    use crate::services::chapter_generation_prompt_context_provider_service::{
        build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
    };

    fn build_project(outline_mode: &str) -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "项目标题".to_string(),
            genre: Some("奇幻".to_string()),
            description: None,
            theme: None,
            target_words: 120000,
            current_words: 0,
            status: "active".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 0,
            outline_mode: outline_mode.to_string(),
            narrative_perspective: None,
            world_time_period: Some("近未来".to_string()),
            world_location: Some("浮空城".to_string()),
            world_atmosphere: Some("压抑".to_string()),
            world_rules: Some("魔力守恒".to_string()),
            chapter_count: Some(3),
            character_count: 0,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn build_chapter(
        chapter_number: i32,
        title: &str,
        expansion_plan: Option<&str>,
        content: Option<&str>,
        summary: Option<&str>,
    ) -> chapter::Model {
        chapter::Model {
            id: format!("chapter-{chapter_number}"),
            project_id: "project-1".to_string(),
            title: title.to_string(),
            chapter_number,
            content: content.map(str::to_string),
            summary: summary.map(str::to_string),
            expansion_plan: expansion_plan.map(str::to_string),
            status: "pending".to_string(),
            word_count: 0,
            outline_id: None,
            sub_index: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_select_template_keys_for_outline_mode_and_previous_chapter_state() {
        assert_eq!(
            chapter_template_key("one-to-many", false),
            "CHAPTER_GENERATION_ONE_TO_MANY"
        );
        assert_eq!(
            chapter_template_key("one-to-many", true),
            "CHAPTER_GENERATION_ONE_TO_MANY_NEXT"
        );
        assert_eq!(
            chapter_template_key("one-to-one", false),
            "CHAPTER_GENERATION_ONE_TO_ONE"
        );
        assert_eq!(
            chapter_template_key("custom-mode", true),
            "CHAPTER_GENERATION_ONE_TO_ONE_NEXT"
        );
    }

    #[test]
    fn should_inject_defaults_when_optional_prompt_fields_are_missing() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(3, "第三章", None, None, None);

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3200,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        assert!(prompt.contains("项目标题"));
        assert!(prompt.contains("第三章"));
        assert!(prompt.contains("3200"));
        assert!(prompt.contains("第三人称"));
        assert!(prompt.contains("暂无大纲"));
    }

    #[test]
    fn should_include_previous_chapter_context_and_continuation_excerpt() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(4, "第四章", Some("推进主线"), None, None);
        let previous_content = format!("{}{}", "甲".repeat(120), "乙".repeat(500));
        let previous_summary = "上一章总结";
        let previous_chapter = build_chapter(
            3,
            "第三章",
            Some("旧大纲"),
            Some(previous_content.as_str()),
            Some(previous_summary),
        );

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(Some(&previous_chapter)),
            true,
            3600,
            PromptContextProviderPayload {
                previous_chapter_summary: previous_summary.to_string(),
                ..build_placeholder_prompt_context_provider_payload()
            },
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        assert!(prompt.contains(previous_summary));
        assert!(prompt.contains(&"乙".repeat(500)));
        assert!(!prompt.contains(&"甲".repeat(120)));
    }

    #[test]
    fn should_build_prompt_with_injected_provider_payload() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(2, "第二章", Some("推进冲突"), None, None);
        let provider_payload = PromptContextProviderPayload {
            recent_chapters_context: String::new(),
            previous_chapter_summary: "上一章总结".to_string(),
            chapter_careers: "[]".to_string(),
            characters_info: "[角色甲]".to_string(),
            foreshadow_reminders: "[伏笔甲]".to_string(),
            relevant_memories: "[记忆甲]".to_string(),
            research_query: String::new(),
            research_assets: "[]".to_string(),
            external_assets: "[]".to_string(),
            reference_assets: "[]".to_string(),
            mcp_references: String::new(),
        };

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            provider_payload,
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        assert!(prompt.contains("[角色甲]"));
        assert!(prompt.contains("[伏笔甲]"));
        assert!(prompt.contains("[记忆甲]"));
    }

    #[test]
    fn should_build_prompt_params_with_defaults_and_provider_context() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(2, "第二章", None, None, None);
        let provider_payload = PromptContextProviderPayload {
            recent_chapters_context: String::new(),
            previous_chapter_summary: "上一章总结".to_string(),
            chapter_careers: "[]".to_string(),
            characters_info: "[角色甲]".to_string(),
            foreshadow_reminders: "[伏笔甲]".to_string(),
            relevant_memories: "[记忆甲]".to_string(),
            research_query: String::new(),
            research_assets: "[]".to_string(),
            external_assets: "[]".to_string(),
            reference_assets: "[]".to_string(),
            mcp_references: String::new(),
        };

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            provider_payload,
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("project_title").map(String::as_str),
            Some("项目标题")
        );
        assert_eq!(
            params.get("chapter_title").map(String::as_str),
            Some("第二章")
        );
        assert_eq!(
            params.get("target_word_count").map(String::as_str),
            Some("2800")
        );
        assert_eq!(
            params.get("narrative_perspective").map(String::as_str),
            Some("第三人称")
        );
        assert_eq!(
            params.get("chapter_outline").map(String::as_str),
            Some("暂无大纲")
        );
        assert_eq!(
            params.get("characters_info").map(String::as_str),
            Some("[角色甲]")
        );
        assert_eq!(
            params.get("previous_chapter_summary").map(String::as_str),
            Some("上一章总结")
        );
        assert_eq!(
            params.get("external_assets").map(String::as_str),
            Some("[]")
        );
    }

    #[test]
    fn should_apply_prompt_overrides_before_project_defaults() {
        let mut project_model = build_project("one-to-one");
        project_model.narrative_perspective = Some("第三人称".to_string());
        project_model.default_creative_mode = Some("balanced".to_string());
        project_model.default_story_focus = Some("advance_plot".to_string());
        project_model.default_plot_stage = Some("development".to_string());
        project_model.default_story_creation_brief = Some("项目默认总控".to_string());
        project_model.default_quality_preset = Some("balanced".to_string());
        project_model.default_quality_notes = Some("项目默认质量要求".to_string());
        let chapter_model = build_chapter(5, "第五章", Some("推进高潮"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3200,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                narrative_perspective: Some("第一人称".to_string()),
                creative_mode: Some("suspense".to_string()),
                story_focus: Some("reveal_mystery".to_string()),
                plot_stage: Some("climax".to_string()),
                story_creation_brief: Some("本章主打谜团揭晓前夜".to_string()),
                quality_preset: Some("immersive".to_string()),
                quality_notes: Some("压缩解释，强化临场感".to_string()),
                web_research_enabled: false,
                web_research_query: None,
                story_repair_summary: None,
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
            },
        );

        assert_eq!(
            params.get("narrative_perspective").map(String::as_str),
            Some("第一人称")
        );
        assert_eq!(
            params.get("creative_mode").map(String::as_str),
            Some("suspense")
        );
        assert_eq!(
            params.get("story_focus").map(String::as_str),
            Some("reveal_mystery")
        );
        assert_eq!(params.get("plot_stage").map(String::as_str), Some("climax"));
        assert_eq!(
            params.get("story_creation_brief").map(String::as_str),
            Some("本章主打谜团揭晓前夜")
        );
        assert_eq!(
            params.get("quality_preset").map(String::as_str),
            Some("immersive")
        );
        assert_eq!(
            params.get("quality_notes").map(String::as_str),
            Some("压缩解释，强化临场感")
        );
        assert!(params["creative_mode_block"].contains("创作模式"));
        assert!(params["story_creation_brief_block"].contains("本章主打谜团揭晓前夜"));
    }

    #[test]
    fn should_fallback_to_project_prompt_defaults_when_overrides_are_missing() {
        let mut project_model = build_project("one-to-many");
        project_model.narrative_perspective = Some("全知视角".to_string());
        project_model.default_creative_mode = Some("hook".to_string());
        project_model.default_story_focus = Some("escalate_conflict".to_string());
        project_model.default_story_creation_brief = Some("项目默认总控".to_string());
        project_model.default_quality_preset = Some("plot_drive".to_string());
        project_model.default_quality_notes = Some("强调推进".to_string());
        let chapter_model = build_chapter(6, "第六章", Some("冲突加压"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("narrative_perspective").map(String::as_str),
            Some("全知视角")
        );
        assert_eq!(
            params.get("creative_mode").map(String::as_str),
            Some("hook")
        );
        assert_eq!(
            params.get("story_focus").map(String::as_str),
            Some("escalate_conflict")
        );
        assert_eq!(
            params.get("story_creation_brief").map(String::as_str),
            Some("项目默认总控")
        );
        assert_eq!(
            params.get("quality_preset").map(String::as_str),
            Some("plot_drive")
        );
        assert_eq!(
            params.get("quality_notes").map(String::as_str),
            Some("强调推进")
        );
    }

    #[test]
    fn should_keep_repair_blocks_empty_when_repair_inputs_are_missing() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(7, "第七章", Some("修复节奏"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2600,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("story_repair_summary").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_repair_targets").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_preserve_strengths").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_repair_target_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_repair_diagnostic_block")
                .map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn should_build_repair_blocks_from_prompt_overrides() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(8, "第八章", Some("修复支线"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3000,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                narrative_perspective: None,
                creative_mode: None,
                story_focus: None,
                plot_stage: None,
                story_creation_brief: None,
                quality_preset: None,
                quality_notes: None,
                web_research_enabled: false,
                web_research_query: None,
                story_repair_summary: Some("上一章中段节奏拖慢，需要重新压缩".to_string()),
                story_repair_targets: vec!["缩短铺垫".to_string(), "提前冲突触发".to_string()],
                story_preserve_strengths: vec!["角色声音".to_string(), "悬念尾钩".to_string()],
            },
        );

        assert_eq!(
            params.get("story_repair_summary").map(String::as_str),
            Some("上一章中段节奏拖慢，需要重新压缩")
        );
        assert_eq!(
            params.get("story_repair_targets").map(String::as_str),
            Some("缩短铺垫；提前冲突触发")
        );
        assert_eq!(
            params.get("story_preserve_strengths").map(String::as_str),
            Some("角色声音；悬念尾钩")
        );
        assert!(params["story_repair_target_block"].contains("需要修复：缩短铺垫；提前冲突触发"));
        assert!(params["story_repair_target_block"].contains("必须保留：角色声音；悬念尾钩"));
        assert!(
            params["story_repair_diagnostic_block"].contains("上一章中段节奏拖慢，需要重新压缩")
        );
        assert!(
            params["story_repair_diagnostic_block"].contains("本章修复项：缩短铺垫；提前冲突触发")
        );
        assert!(params["story_repair_diagnostic_block"].contains("保留优势：角色声音；悬念尾钩"));
    }

    #[test]
    fn should_keep_web_research_block_empty_when_not_enabled() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(9, "第九章", Some("推进调查"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2600,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("web_research_query").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("web_research_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_creation_brief_block").map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn should_build_web_research_block_when_enabled() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(10, "第十章", Some("收束线索"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                narrative_perspective: None,
                creative_mode: None,
                story_focus: None,
                plot_stage: None,
                story_creation_brief: None,
                quality_preset: None,
                quality_notes: None,
                web_research_enabled: true,
                web_research_query: Some("晚清漕运与江南水路行会".to_string()),
                story_repair_summary: None,
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
            },
        );

        assert_eq!(
            params.get("web_research_query").map(String::as_str),
            Some("晚清漕运与江南水路行会")
        );
        assert!(params["web_research_block"].contains("已请求联网检索"));
        assert!(params["web_research_block"].contains("晚清漕运与江南水路行会"));
        assert!(params["story_creation_brief_block"].contains("晚清漕运与江南水路行会"));
    }

    #[test]
    fn should_surface_external_research_assets_from_provider_payload() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(11, "第十一章", Some("追查账册"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            PromptContextProviderPayload {
                recent_chapters_context: String::new(),
                previous_chapter_summary: String::new(),
                chapter_careers: "[]".to_string(),
                characters_info: "[]".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: "[]".to_string(),
                research_query: "晚清漕运夜航避税路线".to_string(),
                research_assets: "[]".to_string(),
                external_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"晚清漕运夜航避税路线\"}]"
                        .to_string(),
                reference_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"晚清漕运夜航避税路线\"}]"
                        .to_string(),
                mcp_references: "[]".to_string(),
            },
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("research_query").map(String::as_str),
            Some("晚清漕运夜航避税路线")
        );
        assert!(params["quality_external_assets_block"].contains("晚清漕运夜航避税路线"));
        assert!(params["reference_assets"].contains("web_research_query"));
    }
}
