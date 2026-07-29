use std::{collections::HashMap, fmt};

use crate::{
    ai::{service::AIService, AIConfig},
    services::{
        chapter_content_digest_service::chapter_content_digest,
        controlled_generation_guidance_service::append_controlled_generation_guidance,
        cooperative_cancellation_service::CooperativeCancellationToken,
        prompt_template_service::PromptTemplateService,
    },
};

const FOCUS_INSTRUCTIONS: &[(&str, &str)] = &[
    (
        "balanced",
        "- 平衡处理叙事、对话、情绪和节奏，整体降低模板腔。",
    ),
    (
        "dialogue",
        "- 优先处理人物对白，让说话方式更像真人，保住角色区分度。",
    ),
    (
        "pacing",
        "- 优先处理叙事节奏，减少拖沓解释，强化场面推进和段落落点。",
    ),
    (
        "emotion",
        "- 优先处理情绪表达，让反应更具体，少空泛感慨和统一抒情。",
    ),
    ("hook", "- 优先处理开场与结尾牵引，保住追读钩子和信息差。"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BookPolishCandidate {
    pub(crate) content: String,
    pub(crate) word_count_before: i32,
    pub(crate) word_count_after: i32,
    pub(crate) content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BookPolishGenerationError {
    Cancelled,
    InvalidInput(&'static str),
    Template(String),
    Generation(String),
    InvalidResult(&'static str),
}

impl BookPolishGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::InvalidInput(_) => "invalid_input",
            Self::Template(_) => "template_error",
            Self::Generation(_) => "generation_error",
            Self::InvalidResult(_) => "invalid_result",
        }
    }
}

impl fmt::Display for BookPolishGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("book polish generation was cancelled"),
            Self::InvalidInput(field) => write!(formatter, "invalid book polish input: {field}"),
            Self::Template(_) => formatter.write_str("failed to build book polish prompt"),
            Self::Generation(_) => formatter.write_str("book polish generation failed"),
            Self::InvalidResult(field) => write!(formatter, "invalid book polish result: {field}"),
        }
    }
}

impl std::error::Error for BookPolishGenerationError {}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn generate_book_polish_candidate(
    original_text: &str,
    style: Option<&str>,
    focus_mode: &str,
    preserve_paragraphs: bool,
    retain_hooks: bool,
    ai_config: AIConfig,
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<BookPolishCandidate, BookPolishGenerationError> {
    generate_book_polish_candidate_with_guidance(
        original_text,
        style,
        focus_mode,
        preserve_paragraphs,
        retain_hooks,
        ai_config,
        None,
        cancellation_token,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn generate_book_polish_candidate_with_guidance(
    original_text: &str,
    style: Option<&str>,
    focus_mode: &str,
    preserve_paragraphs: bool,
    retain_hooks: bool,
    ai_config: AIConfig,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<BookPolishCandidate, BookPolishGenerationError> {
    ensure_not_cancelled(cancellation_token)?;
    if original_text.trim().is_empty() {
        return Err(BookPolishGenerationError::InvalidInput("original_text"));
    }

    let mut params = build_runtime_blocks(style, focus_mode, preserve_paragraphs, retain_hooks);
    params.insert("original_text".to_string(), original_text.to_string());
    let template = PromptTemplateService::system_template_info("AI_DENOISING")
        .ok_or_else(|| BookPolishGenerationError::Template("template_not_found".to_string()))?;
    let prompt = PromptTemplateService::format_prompt(&template.content, &params)
        .map_err(BookPolishGenerationError::Template)?;
    let prompt = finalize_book_polish_prompt(prompt, additional_guidance);

    ensure_not_cancelled(cancellation_token)?;
    let response = AIService::new(ai_config)
        .generate_text(&prompt, None, None)
        .await
        .map_err(BookPolishGenerationError::Generation)?;
    ensure_not_cancelled(cancellation_token)?;

    let content = response.content;
    if content.trim().is_empty() {
        return Err(BookPolishGenerationError::InvalidResult("content"));
    }

    Ok(BookPolishCandidate {
        word_count_before: saturated_char_count(original_text),
        word_count_after: saturated_char_count(&content),
        content_digest: chapter_content_digest(&content),
        content,
    })
}

fn finalize_book_polish_prompt(prompt: String, additional_guidance: Option<&str>) -> String {
    append_controlled_generation_guidance(prompt, additional_guidance)
}

fn focus_instruction(mode: &str) -> &'static str {
    FOCUS_INSTRUCTIONS
        .iter()
        .find(|(candidate, _)| *candidate == mode)
        .map(|(_, instruction)| *instruction)
        .unwrap_or(FOCUS_INSTRUCTIONS[0].1)
}

fn build_runtime_blocks(
    style: Option<&str>,
    focus_mode: &str,
    preserve_paragraphs: bool,
    retain_hooks: bool,
) -> HashMap<String, String> {
    let structure = [
        "- 尽量保留原文的情节顺序和信息密度，不要重写成另一种故事。".to_string(),
        if preserve_paragraphs {
            "- 保留原段落边界和段间呼吸感，除非原文断段明显影响阅读。".to_string()
        } else {
            "- 允许按节奏重新切分段落，但不要打散原有事件顺序。".to_string()
        },
        if retain_hooks {
            "- 保留段尾和章尾的悬念、动作牵引或情绪悬置，不要抹平成总结句。".to_string()
        } else {
            "- 可以适度重写尾句，但仍要保住阅读牵引力。".to_string()
        },
    ]
    .join("\n");

    let style_hint = style.unwrap_or("").trim();
    let style_block = if style_hint.is_empty() {
        "【额外风格偏好】\n- 无额外补充，按自然中文网文表达处理。".to_string()
    } else {
        format!("【额外风格偏好】\n- {style_hint}")
    };

    HashMap::from([
        (
            "focus_instruction".to_string(),
            focus_instruction(focus_mode).to_string(),
        ),
        ("structure_instruction".to_string(), structure),
        ("style_hint_block".to_string(), style_block),
    ])
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), BookPolishGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        Err(BookPolishGenerationError::Cancelled)
    } else {
        Ok(())
    }
}

fn saturated_char_count(value: &str) -> i32 {
    i32::try_from(value.chars().count()).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{build_runtime_blocks, finalize_book_polish_prompt, saturated_char_count};

    #[test]
    fn polish_guidance_is_only_added_for_guided_generation() {
        let base_prompt = "Polish the chapter text.".to_string();

        assert_eq!(
            finalize_book_polish_prompt(base_prompt.clone(), None),
            base_prompt
        );

        let guided = finalize_book_polish_prompt(
            base_prompt.clone(),
            Some("保留章尾悬念，不要添加新的世界观设定"),
        );
        assert!(guided.starts_with(&base_prompt));
        assert!(guided.contains("<autopilot_additional_guidance>"));
        assert!(guided.contains("保留章尾悬念，不要添加新的世界观设定"));
    }

    #[test]
    fn polish_runtime_blocks_keep_safe_defaults() {
        let blocks = build_runtime_blocks(None, "unknown", true, true);
        assert!(blocks["focus_instruction"].contains("平衡处理"));
        assert!(blocks["structure_instruction"].contains("保留原段落边界"));
        assert!(blocks["structure_instruction"].contains("保留段尾和章尾"));
        assert!(blocks["style_hint_block"].contains("无额外补充"));
    }

    #[test]
    fn polish_word_count_is_saturated_i32() {
        assert_eq!(saturated_char_count("你好 world"), 8);
    }
}
