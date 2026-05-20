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
            .unwrap_or_else(|| {
                format!("默认按接近原文长度处理，原文约 {} 字", original_word_count)
            }),
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

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use serde_json::json;

    use crate::models::chapter;

    use super::{
        build_partial_length_requirement, build_regeneration_prompt,
        calculate_partial_target_words, prepare_partial_regeneration_input,
        PreparePartialRegenerationError, PreparedPartialRegenerationInput,
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

    #[test]
    fn should_build_regeneration_prompt_with_default_fields() {
        let chapter = chapter_with_content("原始正文");
        let prompt = build_regeneration_prompt(&chapter, &json!({}));

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
        let prompt = build_regeneration_prompt(
            &chapter,
            &json!({
                "target_word_count": 1800,
                "custom_instructions": "强化冲突",
                "selected_suggestion_indices": [1, "skip", 3],
                "focus_areas": ["节奏", 7, "人物"],
                "creative_mode": "dramatic",
                "story_focus": "主线推进",
                "quality_preset": "balanced",
                "preserve_elements": {
                    "preserve_structure": true,
                    "preserve_dialogues": ["对白A", "对白B"],
                    "preserve_plot_points": ["转折A"],
                    "preserve_character_traits": false
                },
                "story_creation_brief": "总控说明",
                "quality_notes": "质量偏好",
                "story_repair_summary": "修复摘要",
                "story_repair_targets": ["目标A", "目标B"],
                "story_preserve_strengths": ["优势A"]
            }),
        );

        assert!(prompt.contains("目标字数：1800"));
        assert!(prompt.contains("用户修改要求：\n强化冲突"));
        assert!(prompt.contains("选中建议索引：1, 3"));
        assert!(prompt.contains("重点优化方向：节奏、人物"));
        assert!(prompt.contains("创作模式：dramatic"));
        assert!(prompt.contains("保留结构：true"));
        assert!(prompt.contains("保留对话：对白A、对白B"));
        assert!(prompt.contains("保留剧情点：转折A"));
        assert!(prompt.contains("保留人物特征：false"));
        assert!(prompt.contains("修复目标：目标A、目标B"));
        assert!(prompt.contains("保留优势：优势A"));
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
            Some("联网说明"),
        );
        let prepared = valid_prepared_partial_input(result);

        assert_eq!(prepared.selected_text, "替换文本");
        assert_eq!(prepared.context_before, "一二");
        assert_eq!(prepared.context_after, "六七");
        assert_eq!(prepared.original_word_count, 4);
        assert_eq!(prepared.target_words, 120);
        assert!(prepared.prompt.contains("风格说明"));
        assert!(prepared.prompt.contains("联网说明"));
    }

    #[test]
    fn should_prepare_partial_regeneration_input_with_content_fallback_and_edge_context() {
        let chapter = chapter_with_content("一二三四五六七八九十");

        let result =
            prepare_partial_regeneration_input(&chapter, "  ", 0, 2, 3, "", None, None, None, None);
        let prepared = valid_prepared_partial_input(result);

        assert_eq!(prepared.selected_text, "一二");
        assert_eq!(prepared.context_before, "");
        assert_eq!(prepared.context_after, "三四五");
        assert!(prepared.prompt.contains("（无前文上下文）"));
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

        let result =
            prepare_partial_regeneration_input(&chapter, "", 2, 2, 1, "", None, None, None, None);
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

        let result =
            prepare_partial_regeneration_input(&chapter, "", 0, 1, 1, "", None, None, None, None);
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
