use std::collections::HashMap;

use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::ai::service::AIService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;

pub struct InspirationService;

const TEMPERATURES: &[(&str, f64)] = &[
    ("title", 0.9),
    ("description", 0.78),
    ("theme", 0.72),
    ("genre", 0.62),
];

const MAX_RETRIES: u32 = 3;

const STYLE_GUARD_BASE: &str = r#"【风格与可读性要求（必须遵守）】
1. 使用中文网文读者习惯的自然表达，避免公文腔、论文腔和模板腔。
2. 句子长短要有变化，避免整段都是同长度短句或流水账长句。
3. 可以少量借用网络语感，但必须克制，不能堆砌流行词。
4. 优先写人物目标、阻碍、代价和情绪，不要只堆设定名词。
5. 如果出现术语或特殊设定，要顺手补一句白话解释。
6. 避免高频模板开头，例如"这是一个关于……"或"故事围绕……"这类空泛引入。
7. 只输出当前任务结果，不输出流程说明、调度术语或自我评价。
8. 信息不足时，优先保住"目标 → 阻力 → 选择 → 后果"的最小冲突链。
9. 六个选项必须有明显区分，至少覆盖不同切入角度，不能只换同义词。
10. 描述要带具体场景感或动作感，避免只给抽象大词。
11. 避免"鸡汤式收尾"和"下章预告式空话"，优先保留具体冲突钩子。
12. 优先给可传播的记忆点：反常识信息、极端选择、倒计时压力，至少命中其一。"#;

const STEP_EXTRA_GUARD: &[(&str, &str)] = &[
    ("title", r#"【书名专项】
    - 风格要拉开差异，避免同构词组批量改写。
    - 名称要顺口、好记，避免生造拗口长词。
    - 至少覆盖以下命名策略中的四种：身份反差、强事件、关系张力、情绪钩子、世界观异化、命运投择。"#),
    ("description", r#"【简介专项】
    - 每个选项都要体现：主角当前目标 + 关键阻碍或代价。
    - 冲突必须让读者感知得到，不能只写抽象观点。
    - 六个选项的开场方式要明显变化，比如动作切入、对话切入、结果倒叙、困境切入等。
    - 开头尽量尽快出现冲突触发、异常变化或高压任务，不要慢热铺垫。
    - 至少两个选项使用短句爆点开场，至少两个选项带明确转折连接词。"#),
    ("theme", r#"【主题专项】
    - 主题要先给人话结论，再落回角色冲突现场，避免高概念空转。
    - 保持情绪温度，不要写成教科书总结。
    - 每个主题都要包含一个价值冲突对撞点，避免全是正确废话。
    - 优先采用"命题句 → 冲突现场 → 情绪余震"的三拍结构。
    - 至少一个主题要体现"反常识但合理"的价值碰撞。"#),
    ("genre", r#"【类型专项】
    - 标签以读者常见认知为主，可以组合，但不要互相冲突。
    - 禁止生造难懂标签。
    - 至少体现"主赛道 + 冲突气质"两个维度。"#),
];

impl InspirationService {
    fn step_temperature(step: &str) -> f64 {
        TEMPERATURES
            .iter()
            .find(|(s, _)| *s == step)
            .map(|(_, t)| *t)
            .unwrap_or(0.7)
    }

    fn template_keys(step: &str) -> Option<(&'static str, &'static str)> {
        match step {
            "title" => Some(("INSPIRATION_TITLE_SYSTEM", "INSPIRATION_TITLE_USER")),
            "description" => Some(("INSPIRATION_DESCRIPTION_SYSTEM", "INSPIRATION_DESCRIPTION_USER")),
            "theme" => Some(("INSPIRATION_THEME_SYSTEM", "INSPIRATION_THEME_USER")),
            "genre" => Some(("INSPIRATION_GENRE_SYSTEM", "INSPIRATION_GENRE_USER")),
            _ => None,
        }
    }

    fn build_style_guard(step: &str) -> String {
        let extra = STEP_EXTRA_GUARD
            .iter()
            .find(|(s, _)| *s == step)
            .map(|(_, t)| *t)
            .unwrap_or("");
        format!("{}\n{}", STYLE_GUARD_BASE, extra)
    }

    fn clean_json_response(content: &str) -> String {
        let trimmed = content.trim();

        // Strip markdown fences: ```json ... ``` or ``` ... ```
        if let Some(inner) = trimmed
            .strip_prefix("```json")
            .and_then(|s| s.strip_suffix("```"))
            .or_else(|| trimmed.strip_prefix("```").and_then(|s| s.strip_suffix("```")))
        {
            return inner.trim().to_string();
        }

        // Try to extract content between first { and last }
        if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
            let extracted = &trimmed[start..=end];
            if extracted.len() > trimmed.len() / 3 {
                return extracted.to_string();
            }
        }

        trimmed.to_string()
    }

    fn validate_options(step: &str, result: &Value) -> Result<(), String> {
        let options = result
            .get("options")
            .and_then(|o| o.as_array())
            .ok_or("缺少options字段或不是数组")?;

        if options.len() < 3 {
            return Err(format!("选项数量不足，至少需要3个，当前只有{}个", options.len()));
        }
        if options.len() > 10 {
            return Err(format!("选项数量过多，最多10个，当前有{}个", options.len()));
        }

        let mut seen: Vec<String> = Vec::new();
        for (i, option) in options.iter().enumerate() {
            let s = option.as_str().unwrap_or("");
            if s.trim().is_empty() {
                return Err(format!("第{}个选项为空", i + 1));
            }
            if s.chars().count() > 500 {
                return Err(format!("第{}个选项过长（超过500字符）", i + 1));
            }
            if step == "genre" && s.chars().count() > 10 {
                return Err(format!("类型标签【{}】过长，应该在2-10字之间", s));
            }

            // Dedup check
            let normalized: String = s
                .trim()
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            if seen.contains(&normalized) {
                return Err("选项存在重复或近似重复，请提升差异度".to_string());
            }
            seen.push(normalized);
        }

        if step == "description" || step == "theme" {
            let min_len = if step == "description" { 50 } else { 35 };
            for (i, option) in options.iter().enumerate() {
                let s = option.as_str().unwrap_or("");
                if s.trim().chars().count() < min_len {
                    return Err(format!("第{}个选项过短，信息密度不足", i + 1));
                }
            }
        }

        Ok(())
    }

    fn build_format_params(context: &Value) -> HashMap<String, String> {
        let mut params = HashMap::new();
        let empty = String::new();

        let initial_idea = context
            .get("initial_idea")
            .and_then(|v| v.as_str())
            .or_else(|| context.get("description").and_then(|v| v.as_str()))
            .unwrap_or("");
        params.insert("initial_idea".to_string(), initial_idea.to_string());
        params.insert(
            "title".to_string(),
            context
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or(&empty)
                .to_string(),
        );
        params.insert(
            "description".to_string(),
            context
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or(&empty)
                .to_string(),
        );
        params.insert(
            "theme".to_string(),
            context
                .get("theme")
                .and_then(|v| v.as_str())
                .unwrap_or(&empty)
                .to_string(),
        );
        params
    }

    fn format_template(template_content: &str, params: &HashMap<String, String>) -> String {
        let mut result = template_content.to_string();
        for (key, value) in params {
            let placeholder = format!("{{{}}}", key);
            if result.contains(&placeholder) {
                result = result.replace(&placeholder, value);
            }
        }
        result
    }

    async fn call_ai_for_json(
        db: &DatabaseConnection,
        user_id: &str,
        system_prompt: &str,
        user_prompt: &str,
        temperature: f64,
    ) -> Result<String, String> {
        let config =
            SettingsService::build_ai_config(db, user_id, None, None, Some(temperature)).await?;
        let service = AIService::new(config);

        // Use streaming internally but accumulate for full response (matching Python behavior)
        let rx = service.generate_text_stream(
            user_prompt.to_string(),
            Some(system_prompt.to_string()),
            None,
        );
        use futures::StreamExt;
        let mut accumulated = String::new();
        tokio::pin!(rx);
        while let Some(chunk) = rx.next().await {
            match chunk {
                Ok(c) => {
                    if let Some(text) = c.content {
                        accumulated.push_str(&text);
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Ok(accumulated)
    }

    pub async fn generate_options(
        db: &DatabaseConnection,
        user_id: &str,
        step: &str,
        context: &Value,
    ) -> Result<Value, String> {
        let (system_key, user_key) =
            Self::template_keys(step).ok_or(format!("不支持的步骤: {}", step))?;

        let system_template = PromptTemplateService::system_template_info(system_key)
            .ok_or(format!("模板 {} 不存在", system_key))?;
        let user_template = PromptTemplateService::system_template_info(user_key)
            .ok_or(format!("模板 {} 不存在", user_key))?;

        let format_params = Self::build_format_params(context);
        let system_prompt =
            format!("{}\n\n{}", system_template.content, Self::build_style_guard(step));
        let user_prompt = Self::format_template(&user_template.content, &format_params);
        let temperature = Self::step_temperature(step);

        let mut last_error = String::new();
        for attempt in 0..MAX_RETRIES {
            let mut sp = system_prompt.clone();
            if attempt > 0 {
                sp.push_str(&format!(
                    "\n\n这是第{}次生成，请只返回合法JSON，并确保options里有6个有效选项。",
                    attempt + 1
                ));
            }

            let content = Self::call_ai_for_json(db, user_id, &sp, &user_prompt, temperature)
                .await
                .map_err(|e| format!("AI调用失败: {}", e))?;

            let cleaned = Self::clean_json_response(&content);
            let result: Value =
                serde_json::from_str(&cleaned).map_err(|e| format!("JSON解析失败: {}", e))?;

            match Self::validate_options(step, &result) {
                Ok(()) => return Ok(result),
                Err(e) => {
                    last_error = e;
                    if attempt < MAX_RETRIES - 1 {
                        continue;
                    }
                }
            }
        }

        Ok(json!({
            "prompt": format!("请为【{}】提供内容：", step),
            "options": ["让AI重新生成", "我自己输入"],
            "error": format!("AI生成格式错误（{}），已自动重试{}次，请手动重试或自己输入", last_error, MAX_RETRIES)
        }))
    }

    pub async fn refine_options(
        db: &DatabaseConnection,
        user_id: &str,
        step: &str,
        context: &Value,
        feedback: &str,
        previous_options: &[String],
    ) -> Result<Value, String> {
        let (system_key, user_key) =
            Self::template_keys(step).ok_or(format!("不支持的步骤: {}", step))?;

        let system_template = PromptTemplateService::system_template_info(system_key)
            .ok_or(format!("模板 {} 不存在", system_key))?;
        let user_template = PromptTemplateService::system_template_info(user_key)
            .ok_or(format!("模板 {} 不存在", user_key))?;

        let format_params = Self::build_format_params(context);
        let mut system_prompt =
            format!("{}\n\n{}", system_template.content, Self::build_style_guard(step));
        let user_prompt = Self::format_template(&user_template.content, &format_params);

        // Append feedback instruction
        let prev_opts: String = if previous_options.is_empty() {
            "（无）".to_string()
        } else {
            previous_options
                .iter()
                .map(|o| format!("- {}", o))
                .collect::<Vec<_>>()
                .join("\n")
        };

        system_prompt.push_str(&format!(
            "\n\n用户对上一轮选项不满意，反馈如下：\n【{}】\n上一轮选项：\n{}\n\n\
             请根据反馈调整方向，给出更贴近用户预期的新选项。要求：\n\
             1. 先理解反馈意图，再改写方向。\n\
             2. 新选项要体现用户提出的偏好变化。\n\
             3. 与现有上下文保持一致，不跑题。\n\
             4. 返回 6 个有效选项。\n\
             5. 至少 2 个选项必须明显跳出上一轮表达结构，不能只做同义改写。",
            feedback, prev_opts
        ));

        let temperature = (Self::step_temperature(step) + 0.1).min(0.9);

        let mut last_error = String::new();
        for attempt in 0..MAX_RETRIES {
            let mut sp = system_prompt.clone();
            if attempt > 0 {
                sp.push_str(&format!(
                    "\n\n这是第{}次生成，请只返回合法JSON。",
                    attempt + 1
                ));
            }

            let content = Self::call_ai_for_json(db, user_id, &sp, &user_prompt, temperature)
                .await
                .map_err(|e| format!("AI调用失败: {}", e))?;

            let cleaned = Self::clean_json_response(&content);
            let result: Value =
                serde_json::from_str(&cleaned).map_err(|e| format!("JSON解析失败: {}", e))?;

            match Self::validate_options(step, &result) {
                Ok(()) => return Ok(result),
                Err(e) => {
                    last_error = e;
                    if attempt < MAX_RETRIES - 1 {
                        continue;
                    }
                }
            }
        }

        Ok(json!({
            "prompt": format!("请为【{}】提供内容：", step),
            "options": ["让AI重新生成", "我自己输入"],
            "error": format!("AI生成格式错误（{}），已自动重试{}次", last_error, MAX_RETRIES)
        }))
    }

    pub async fn quick_generate(
        db: &DatabaseConnection,
        user_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        theme: Option<&str>,
        genre: Option<&[String]>,
        narrative_perspective: Option<&str>,
    ) -> Result<Value, String> {
        let template = PromptTemplateService::system_template_info("INSPIRATION_QUICK_COMPLETE")
            .ok_or("INSPIRATION_QUICK_COMPLETE 模板不存在")?;

        // Build existing text
        let mut existing_parts: Vec<String> = Vec::new();
        if let Some(t) = title {
            if !t.is_empty() {
                existing_parts.push(format!("- 书名：{}", t));
            }
        }
        if let Some(d) = description {
            if !d.is_empty() {
                existing_parts.push(format!("- 简介：{}", d));
            }
        }
        if let Some(t) = theme {
            if !t.is_empty() {
                existing_parts.push(format!("- 主题：{}", t));
            }
        }
        if let Some(g) = genre {
            if !g.is_empty() {
                existing_parts.push(format!("- 类型：{}", g.join(", ")));
            }
        }
        if let Some(p) = narrative_perspective {
            if !p.is_empty() {
                existing_parts.push(format!("- 叙事视角：{}", p));
            }
        }
        let existing_text = if existing_parts.is_empty() {
            "暂无信息".to_string()
        } else {
            existing_parts.join("\n")
        };

        let mut fmt_params: HashMap<String, String> = HashMap::new();
        fmt_params.insert("existing".to_string(), existing_text);
        let mut system_prompt = PromptTemplateService::format_prompt(
            &template.content,
            &fmt_params,
        )?;

        system_prompt.push_str(&format!(
            "\n\n{}\n【智能补全专项】保证四个字段像同一部小说，人物语气自然，信息前后一致；\
             仅返回JSON字段值，不输出流程说明或执行步骤；\
             信息不足时先补目标->阻力->选择->后果链；\
             如果用户没给叙事视角，请补一个最适合题材与冲突表达的视角。",
            Self::build_style_guard("description")
        ));

        let user_prompt = "请在不偏离现有信息的前提下补全缺失字段，只返回JSON。";
        let temperature = 0.78;

        let content =
            Self::call_ai_for_json(db, user_id, &system_prompt, user_prompt, temperature)
                .await
                .map_err(|e| format!("AI调用失败: {}", e))?;

        let cleaned = Self::clean_json_response(&content);
        let result: Value =
            serde_json::from_str(&cleaned).map_err(|e| format!("JSON解析失败: {}", e))?;

        // Normalize genre
        let result_genre = Self::normalize_genre_list(result.get("genre"));
        let result_perspective = result
            .get("narrative_perspective")
            .and_then(|v| v.as_str())
            .unwrap_or("第三人称");

        // Use user-provided genre if available, otherwise use AI result
        let final_genre: Vec<String> = match genre {
            Some(g) if !g.is_empty() => g.to_vec(),
            _ => result_genre,
        };

        Ok(json!({
            "title": title.unwrap_or(""),
            "description": description.unwrap_or(""),
            "theme": theme.unwrap_or(""),
            "genre": final_genre,
            "narrative_perspective": narrative_perspective.unwrap_or(result_perspective),
        }))
    }

    fn normalize_genre_list(value: Option<&Value>) -> Vec<String> {
        let value = match value {
            Some(v) => v,
            None => return vec![],
        };

        let mut items: Vec<String> = Vec::new();
        if let Some(arr) = value.as_array() {
            for item in arr {
                if let Some(s) = item.as_str() {
                    for part in s.split(|c: char| c == '，' || c == ',' || c == '、' || c == '/' || c == '|' || c == '｜')
                    {
                        let trimmed = part.trim();
                        if !trimmed.is_empty() {
                            items.push(trimmed.to_string());
                        }
                    }
                }
            }
        } else if let Some(s) = value.as_str() {
            for part in s.split(|c: char| c == '，' || c == ',' || c == '、' || c == '/' || c == '|' || c == '｜')
            {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    items.push(trimmed.to_string());
                }
            }
        }

        let mut seen = std::collections::HashSet::new();
        items.retain(|item| seen.insert(item.clone()));
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_json_markdown_fence() {
        let input = "```json\n{\"options\": [\"a\", \"b\", \"c\"]}\n```";
        let result = InspirationService::clean_json_response(input);
        assert_eq!(result, "{\"options\": [\"a\", \"b\", \"c\"]}");
    }

    #[test]
    fn test_clean_json_raw() {
        let input = "  {\"options\": [\"a\", \"b\", \"c\"]}  ";
        let result = InspirationService::clean_json_response(input);
        assert!(result.contains("\"options\""));
    }

    #[test]
    fn test_validate_options_ok() {
        let result = json!({"options": ["选项一", "选项二", "选项三", "选项四", "选项五", "选项六"]});
        assert!(InspirationService::validate_options("title", &result).is_ok());
    }

    #[test]
    fn test_validate_options_too_few() {
        let result = json!({"options": ["a", "b"]});
        assert!(InspirationService::validate_options("title", &result).is_err());
    }
}
