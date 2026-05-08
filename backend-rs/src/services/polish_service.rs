use std::collections::HashMap;

use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::ai::service::AIService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;

pub struct PolishService;

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

impl PolishService {
    fn focus_instruction(mode: &str) -> &'static str {
        FOCUS_INSTRUCTIONS
            .iter()
            .find(|(m, _)| *m == mode)
            .map(|(_, i)| *i)
            .unwrap_or(FOCUS_INSTRUCTIONS[0].1)
    }

    fn build_runtime_blocks(
        style: Option<&str>,
        focus_mode: &str,
        preserve_paragraphs: bool,
        retain_hooks: bool,
    ) -> HashMap<String, String> {
        let focus = Self::focus_instruction(focus_mode).to_string();

        let structure = vec![
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
        let style_block = if !style_hint.is_empty() {
            format!("【额外风格偏好】\n- {}", style_hint)
        } else {
            "【额外风格偏好】\n- 无额外补充，按自然中文网文表达处理。".to_string()
        };

        let mut blocks = HashMap::new();
        blocks.insert("focus_instruction".to_string(), focus);
        blocks.insert("structure_instruction".to_string(), structure);
        blocks.insert("style_hint_block".to_string(), style_block);
        blocks
    }

    pub async fn polish_text(
        db: &DatabaseConnection,
        user_id: &str,
        original_text: &str,
        style: Option<&str>,
        focus_mode: &str,
        preserve_paragraphs: bool,
        retain_hooks: bool,
        provider_override: Option<&str>,
        model_override: Option<&str>,
        temperature_override: Option<f64>,
    ) -> Result<Value, String> {
        let mut params =
            Self::build_runtime_blocks(style, focus_mode, preserve_paragraphs, retain_hooks);
        params.insert("original_text".to_string(), original_text.to_string());

        let template = PromptTemplateService::system_template_info("AI_DENOISING")
            .ok_or("AI_DENOISING 模板不存在")?;
        let prompt = PromptTemplateService::format_prompt(&template.content, &params)?;

        let config = SettingsService::build_ai_config(
            db,
            user_id,
            provider_override,
            model_override,
            temperature_override,
        )
        .await?;
        let service = AIService::new(config);
        let response = service.generate_text(&prompt, None, None).await?;

        Ok(serde_json::json!({
            "original_text": original_text,
            "polished_text": response.content,
            "word_count_before": original_text.chars().count(),
            "word_count_after": response.content.chars().count(),
        }))
    }

    pub async fn polish_batch(
        db: &DatabaseConnection,
        user_id: &str,
        texts: &[String],
        style: Option<&str>,
        focus_mode: &str,
        preserve_paragraphs: bool,
        retain_hooks: bool,
        provider_override: Option<&str>,
        model_override: Option<&str>,
        temperature_override: Option<f64>,
    ) -> Result<Value, String> {
        let runtime_blocks =
            Self::build_runtime_blocks(style, focus_mode, preserve_paragraphs, retain_hooks);

        let template = PromptTemplateService::system_template_info("AI_DENOISING")
            .ok_or("AI_DENOISING 模板不存在")?;

        let config = SettingsService::build_ai_config(
            db,
            user_id,
            provider_override,
            model_override,
            temperature_override,
        )
        .await?;
        let service = AIService::new(config);

        let mut results = Vec::new();
        for (idx, text) in texts.iter().enumerate() {
            let mut params = runtime_blocks.clone();
            params.insert("original_text".to_string(), text.clone());

            let prompt = PromptTemplateService::format_prompt(&template.content, &params)?;
            let response = service.generate_text(&prompt, None, None).await?;

            results.push(serde_json::json!({
                "index": idx,
                "original": text,
                "polished": response.content,
                "word_count_before": text.chars().count(),
                "word_count_after": response.content.chars().count(),
            }));
        }

        Ok(serde_json::json!({
            "total": results.len(),
            "results": results,
        }))
    }
}
