use std::cmp::{max, min};

use serde_json::Value;

use crate::ai::service::AIService;
use crate::models::chapter;
use crate::services::settings_service::SettingsService;
use crate::services::writing_style_service::WritingStyleService;

pub enum BuildRegenerationAiServiceError {
    InvalidConfig(String),
}

pub enum LoadPartialStyleContentError {
    InvalidStyle(String),
}

pub enum PreparePartialRegenerationError {
    InvalidRange,
    EmptySelectedText,
}

pub enum PreparePartialRegenerationStreamError {
    InvalidRange,
    EmptySelectedText,
    InvalidStyle(String),
    InvalidConfig(String),
}

pub enum PrepareChapterRegenerationStreamError {
    InvalidConfig(String),
}

pub struct PreparedPartialRegenerationInput {
    pub selected_text: String,
    pub context_before: String,
    pub context_after: String,
    pub original_word_count: usize,
    pub target_words: usize,
    pub max_tokens: u32,
    pub prompt: String,
}

pub struct PreparedPartialRegenerationStream {
    pub prepared: PreparedPartialRegenerationInput,
    pub ai_service: AIService,
}

pub struct PreparedChapterRegenerationStream {
    pub prompt: String,
    pub ai_service: AIService,
}

pub fn build_regeneration_prompt(chapter: &chapter::Model, body: &Value) -> String {
    let selected_suggestions = body
        .get("selected_suggestion_indices")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_i64)
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let custom_instructions = body
        .get("custom_instructions")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let focus_areas = body
        .get("focus_areas")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    let story_creation_brief = body
        .get("story_creation_brief")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let quality_notes = body
        .get("quality_notes")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let story_repair_summary = body
        .get("story_repair_summary")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let creative_mode = body
        .get("creative_mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let story_focus = body
        .get("story_focus")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let quality_preset = body
        .get("quality_preset")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let preserve_elements = body.get("preserve_elements");
    let preserve_structure = preserve_elements
        .and_then(|value| value.get("preserve_structure"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let preserve_dialogues = preserve_elements
        .and_then(|value| value.get("preserve_dialogues"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    let preserve_plot_points = preserve_elements
        .and_then(|value| value.get("preserve_plot_points"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    let preserve_character_traits = preserve_elements
        .and_then(|value| value.get("preserve_character_traits"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let story_repair_targets = body
        .get("story_repair_targets")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    let story_preserve_strengths = body
        .get("story_preserve_strengths")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();

    format!(
        "你是小说正文重写助手。请基于以下章节内容和要求输出重写后的正文，不要输出解释。\n\n章节标题：{}\n章节编号：{}\n目标字数：{}\n\n原章节内容：\n{}\n\n用户修改要求：\n{}\n\n选中建议索引：{}\n重点优化方向：{}\n创作模式：{}\n故事关注点：{}\n质量预设：{}\n保留结构：{}\n保留对话：{}\n保留剧情点：{}\n保留人物特征：{}\n创作总控：{}\n质量补充偏好：{}\n剧情质量修复摘要：{}\n修复目标：{}\n保留优势：{}\n\n要求：\n- 只输出可直接替换的正文内容\n- 不要输出标题、编号、前言、后记或流程说明\n- 如果有角色/世界观信息，保持一致\n- 尽量保留原有剧情骨架",
        chapter.title,
        chapter.chapter_number,
        body.get("target_word_count")
            .and_then(Value::as_i64)
            .unwrap_or(3000),
        chapter.content.clone().unwrap_or_default(),
        custom_instructions,
        selected_suggestions,
        focus_areas,
        creative_mode,
        story_focus,
        quality_preset,
        preserve_structure,
        preserve_dialogues,
        preserve_plot_points,
        preserve_character_traits,
        story_creation_brief,
        quality_notes,
        story_repair_summary,
        story_repair_targets,
        story_preserve_strengths,
    )
}

pub fn build_partial_length_requirement(
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    original_word_count: usize,
) -> String {
    match length_mode.unwrap_or("similar") {
        "expand" => {
            let min_words = (original_word_count as f64 * 1.2) as usize;
            let max_words = (original_word_count as f64 * 2.0) as usize;
            format!("建议扩写至 {}-{} 字", min_words, max_words)
        }
        "condense" => {
            let min_words = (original_word_count as f64 * 0.5) as usize;
            let max_words = (original_word_count as f64 * 0.8) as usize;
            format!("建议压缩至 {}-{} 字", min_words, max_words)
        }
        "custom" => target_word_count
            .map(|count| format!("目标长度约 {} 字，允许上下浮动 20%", count))
            .unwrap_or_else(|| format!("默认按接近原文长度处理，原文约 {} 字", original_word_count)),
        _ => {
            let min_words = (original_word_count as f64 * 0.8) as usize;
            let max_words = (original_word_count as f64 * 1.2) as usize;
            format!(
                "尽量保持与原文接近，原文约 {} 字，目标 {}-{} 字",
                original_word_count, min_words, max_words
            )
        }
    }
}

pub fn calculate_partial_target_words(
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    original_word_count: usize,
) -> usize {
    match length_mode.unwrap_or("similar") {
        "expand" => (original_word_count as f64 * 2.0) as usize,
        "custom" => {
            target_word_count.unwrap_or_else(|| (original_word_count as f64 * 1.5) as usize)
        }
        _ => (original_word_count as f64 * 1.5) as usize,
    }
}

pub fn build_partial_regeneration_prompt(
    chapter: &chapter::Model,
    selected_text: &str,
    context_before: &str,
    context_after: &str,
    user_instructions: &str,
    length_requirement: &str,
    style_content: Option<&str>,
    web_research_note: Option<&str>,
) -> String {
    let style_content = style_content.unwrap_or("（未提供风格约束）");
    let web_research_note = web_research_note.unwrap_or("（未启用）");

    format!(
        "你是小说正文局部重写助手。请基于以下内容重写选中片段，只输出可直接替换的正文内容，不要输出解释。\n\n章节标题：{}\n章节编号：{}\n原文选中片段：\n{}\n\n前文上下文：\n{}\n\n后文上下文：\n{}\n\n用户修改要求：\n{}\n\n长度要求：{}\n\n风格约束：\n{}\n\n联网检索说明：{}\n\n要求：\n- 只输出重写后的正文\n- 不要输出标题、编号、前言、后记或流程说明\n- 保持人物、设定与上下文一致\n- 尽量贴合原文节奏与叙事视角",
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
        web_research_note,
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
    web_research_note: Option<&str>,
) -> Result<PreparedPartialRegenerationInput, PreparePartialRegenerationError> {
    let current_content = chapter.content.clone().unwrap_or_default();
    let content_chars: Vec<char> = current_content.chars().collect();
    let content_length = content_chars.len();
    if start_position >= end_position || end_position > content_length {
        return Err(PreparePartialRegenerationError::InvalidRange);
    }

    let selected_text_from_content: String = content_chars[start_position..end_position]
        .iter()
        .collect();
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
    let context_after_end = end_position.saturating_add(context_chars).min(content_length);
    let context_after: String = content_chars[end_position..context_after_end].iter().collect();

    let original_word_count = selected_text.chars().count();
    let length_requirement = build_partial_length_requirement(
        length_mode,
        target_word_count,
        original_word_count,
    );
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
        web_research_note,
    );

    Ok(PreparedPartialRegenerationInput {
        selected_text,
        context_before,
        context_after,
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
) -> Result<Option<String>, LoadPartialStyleContentError> {
    let Some(style_id) = style_id else {
        return Ok(None);
    };

    let value = WritingStyleService::get_style(db, user_id, style_id)
        .await
        .map_err(|error| LoadPartialStyleContentError::InvalidStyle(error.to_string()))?;

    Ok(value
        .get("prompt_content")
        .and_then(Value::as_str)
        .map(str::to_string))
}

pub async fn prepare_chapter_regeneration_stream(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter: &chapter::Model,
    body: &Value,
) -> Result<PreparedChapterRegenerationStream, PrepareChapterRegenerationStreamError> {
    let prompt = build_regeneration_prompt(chapter, body);
    let ai_service = build_regeneration_ai_service(db, user_id, None)
        .await
        .map_err(|error| match error {
            BuildRegenerationAiServiceError::InvalidConfig(detail) => {
                PrepareChapterRegenerationStreamError::InvalidConfig(detail)
            }
        })?;

    Ok(PreparedChapterRegenerationStream { prompt, ai_service })
}

pub async fn prepare_partial_regeneration_stream(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter: &chapter::Model,
    selected_text_override: &str,
    start_position: usize,
    end_position: usize,
    context_chars: usize,
    user_instructions: &str,
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    style_id: Option<i32>,
    enable_web_research: bool,
    web_research_query: Option<&str>,
) -> Result<PreparedPartialRegenerationStream, PreparePartialRegenerationStreamError> {
    let style_content = load_partial_style_content(db, user_id, style_id)
        .await
        .map_err(|error| match error {
            LoadPartialStyleContentError::InvalidStyle(detail) => {
                PreparePartialRegenerationStreamError::InvalidStyle(detail)
            }
        })?;

    let web_research_note = if enable_web_research {
        web_research_query.map(|query| format!("已请求联网检索，检索问题：{}", query))
    } else {
        None
    };

    let prepared = prepare_partial_regeneration_input(
        chapter,
        selected_text_override,
        start_position,
        end_position,
        context_chars,
        user_instructions,
        length_mode,
        target_word_count,
        style_content.as_deref(),
        web_research_note.as_deref(),
    )
    .map_err(|error| match error {
        PreparePartialRegenerationError::InvalidRange => {
            PreparePartialRegenerationStreamError::InvalidRange
        }
        PreparePartialRegenerationError::EmptySelectedText => {
            PreparePartialRegenerationStreamError::EmptySelectedText
        }
    })?;

    let ai_service = build_regeneration_ai_service(db, user_id, Some(prepared.max_tokens))
        .await
        .map_err(|error| match error {
            BuildRegenerationAiServiceError::InvalidConfig(detail) => {
                PreparePartialRegenerationStreamError::InvalidConfig(detail)
            }
        })?;

    Ok(PreparedPartialRegenerationStream {
        prepared,
        ai_service,
    })
}
