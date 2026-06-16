use std::collections::HashMap;

use axum::{extract::Extension, http::StatusCode, response::Json, routing::post, Router};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ai::service::AIService;
use crate::services::auth::Claims;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;

const INSPIRATION_GENERATE_OPTIONS_ROUTE: &str = "/inspiration/generate-options";
const INSPIRATION_REFINE_OPTIONS_ROUTE: &str = "/inspiration/refine-options";
const INSPIRATION_QUICK_GENERATE_ROUTE: &str = "/inspiration/quick-generate";

struct InspirationService;

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
    (
        "title",
        r#"【书名专项】
    - 风格要拉开差异，避免同构词组批量改写。
    - 名称要顺口、好记，避免生造拗口长词。
    - 至少覆盖以下命名策略中的四种：身份反差、强事件、关系张力、情绪钩子、世界观异化、命运投择。"#,
    ),
    (
        "description",
        r#"【简介专项】
    - 每个选项都要体现：主角当前目标 + 关键阻碍或代价。
    - 冲突必须让读者感知得到，不能只写抽象观点。
    - 六个选项的开场方式要明显变化，比如动作切入、对话切入、结果倒叙、困境切入等。
    - 开头尽量尽快出现冲突触发、异常变化或高压任务，不要慢热铺垫。
    - 至少两个选项使用短句爆点开场，至少两个选项带明确转折连接词。"#,
    ),
    (
        "theme",
        r#"【主题专项】
    - 主题要先给人话结论，再落回角色冲突现场，避免高概念空转。
    - 保持情绪温度，不要写成教科书总结。
    - 每个主题都要包含一个价值冲突对撞点，避免全是正确废话。
    - 优先采用"命题句 → 冲突现场 → 情绪余震"的三拍结构。
    - 至少一个主题要体现"反常识但合理"的价值碰撞。"#,
    ),
    (
        "genre",
        r#"【类型专项】
    - 标签以读者常见认知为主，可以组合，但不要互相冲突。
    - 禁止生造难懂标签。
    - 至少体现"主赛道 + 冲突气质"两个维度。"#,
    ),
];

#[cfg(test)]
fn build_inspiration_route_owner_contract() -> Value {
    json!({
        "owner": "inspiration",
        "rust_owner": "backend-rs/src/api/inspiration.rs",
        "routes": {
            "generate_options": INSPIRATION_GENERATE_OPTIONS_ROUTE,
            "refine_options": INSPIRATION_REFINE_OPTIONS_ROUTE,
            "quick_generate": INSPIRATION_QUICK_GENERATE_ROUTE
        },
        "methods": {
            "generate_options": ["POST"],
            "refine_options": ["POST"],
            "quick_generate": ["POST"]
        },
        "service_owners": [
            "backend-rs/src/api/inspiration.rs",
            "backend-rs/src/services/prompt_template_service.rs",
            "backend-rs/src/services/settings_service.rs",
            "backend-rs/src/ai/service.rs"
        ],
        "readiness_probes": [
            "inspiration-generate-options-auth-guard-rust",
            "inspiration-quick-generate-auth-guard-rust",
            "inspiration-configure-mock-openai-business-rust",
            "inspiration-generate-options-business-rust",
            "inspiration-refine-options-business-rust",
            "inspiration-quick-generate-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-inspiration-business-owner",
            "business_probes": [
                "inspiration-configure-mock-openai-business-rust",
                "inspiration-generate-options-business-rust",
                "inspiration-refine-options-business-rust",
                "inspiration-quick-generate-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-inspiration-business-owner",
            "readiness_probe_count": 6,
            "business_probe_count": 4,
            "auth_guard_probe_count": 2,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "source_map_files": [
            "backend/app/api/inspiration.py",
            "backend/app/services/prompt_service.py",
            "backend/app/services/chapter_web_research_service.py"
        ],
        "next_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Inspiration route business smoke is covered by phase5-inspiration-business-owner; final completion now requires explicit source-map freeze/delete/repoint approval with same-round rollback policy.",
        "rollback_boundary": {
            "source_map_policy": "keep_python_inspiration_route_prompt_service_web_research_files_as_source_map_until_explicit_freeze_delete_round",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit source-map freeze/delete/repoint approval"
            ],
            "freeze_reason": "phase5-inspiration-business-owner covers mock OpenAI configuration, generate-options, refine-options, and quick-generate probes with zero Python fallback probes."
        }
    })
}

impl InspirationService {
    fn normalize_research_text(value: impl AsRef<str>, limit: usize) -> String {
        let text = value
            .as_ref()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if text.chars().count() <= limit {
            return text;
        }
        text.chars()
            .take(limit.saturating_sub(3))
            .collect::<String>()
            + "..."
    }

    fn compose_research_query(
        step: &str,
        context: &Value,
        feedback: Option<&str>,
        custom_query: Option<&str>,
    ) -> String {
        let custom_query = custom_query.unwrap_or("").trim();
        if !custom_query.is_empty() {
            return Self::normalize_research_text(custom_query, 320);
        }

        let step_label = match step {
            "title" => "小说书名创意",
            "description" => "小说简介与冲突设计",
            "theme" => "小说主题与价值冲突",
            "genre" => "小说类型定位与读者偏好",
            _ => step,
        };

        let mut parts = vec![step_label.to_string()];
        for key in ["initial_idea", "title", "description", "theme"] {
            if let Some(value) = context.get(key).and_then(|value| value.as_str()) {
                let text = Self::normalize_research_text(value, 140);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }
        if let Some(feedback) = feedback {
            let text = Self::normalize_research_text(feedback, 100);
            if !text.is_empty() {
                parts.push(text);
            }
        }
        Self::normalize_research_text(parts.join(" | "), 320)
    }

    fn build_research_payload(query: &str, enabled: bool) -> Value {
        if !enabled || query.trim().is_empty() {
            return json!({});
        }
        json!({
            "research_query": query,
            "research_assets": []
        })
    }

    fn attach_research_payload(mut result: Value, query: &str, enabled: bool) -> Value {
        let research_payload = Self::build_research_payload(query, enabled);
        let Some(map) = result.as_object_mut() else {
            return result;
        };
        if let Some(research_query) = research_payload.get("research_query") {
            map.insert("research_query".to_string(), research_query.clone());
        }
        if let Some(research_assets) = research_payload.get("research_assets") {
            map.insert("research_assets".to_string(), research_assets.clone());
        }
        result
    }

    fn step_temperature(step: &str) -> f64 {
        TEMPERATURES
            .iter()
            .find(|(candidate, _)| *candidate == step)
            .map(|(_, temperature)| *temperature)
            .unwrap_or(0.7)
    }

    fn template_keys(step: &str) -> Option<(&'static str, &'static str)> {
        match step {
            "title" => Some(("INSPIRATION_TITLE_SYSTEM", "INSPIRATION_TITLE_USER")),
            "description" => Some((
                "INSPIRATION_DESCRIPTION_SYSTEM",
                "INSPIRATION_DESCRIPTION_USER",
            )),
            "theme" => Some(("INSPIRATION_THEME_SYSTEM", "INSPIRATION_THEME_USER")),
            "genre" => Some(("INSPIRATION_GENRE_SYSTEM", "INSPIRATION_GENRE_USER")),
            _ => None,
        }
    }

    fn build_style_guard(step: &str) -> String {
        let extra = STEP_EXTRA_GUARD
            .iter()
            .find(|(candidate, _)| *candidate == step)
            .map(|(_, text)| *text)
            .unwrap_or("");
        format!("{}\n{}", STYLE_GUARD_BASE, extra)
    }

    fn clean_json_response(content: &str) -> String {
        let trimmed = content.trim();

        if let Some(inner) = trimmed
            .strip_prefix("```json")
            .and_then(|value| value.strip_suffix("```"))
            .or_else(|| {
                trimmed
                    .strip_prefix("```")
                    .and_then(|value| value.strip_suffix("```"))
            })
        {
            return inner.trim().to_string();
        }

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
            .and_then(|options| options.as_array())
            .ok_or("缺少options字段或不是数组")?;

        if options.len() < 3 {
            return Err(format!(
                "选项数量不足，至少需要3个，当前只有{}个",
                options.len()
            ));
        }
        if options.len() > 10 {
            return Err(format!("选项数量过多，最多10个，当前有{}个", options.len()));
        }

        let mut seen: Vec<String> = Vec::new();
        for (index, option) in options.iter().enumerate() {
            let text = option.as_str().unwrap_or("");
            if text.trim().is_empty() {
                return Err(format!("第{}个选项为空", index + 1));
            }
            if text.chars().count() > 500 {
                return Err(format!("第{}个选项过长（超过500字符）", index + 1));
            }
            if step == "genre" && text.chars().count() > 10 {
                return Err(format!("类型标签【{}】过长，应该在2-10字之间", text));
            }

            let normalized: String = text
                .trim()
                .to_lowercase()
                .chars()
                .filter(|character| character.is_alphanumeric())
                .collect();
            if seen.contains(&normalized) {
                return Err("选项存在重复或近似重复，请提升差异度".to_string());
            }
            seen.push(normalized);
        }

        if step == "description" || step == "theme" {
            let min_len = if step == "description" { 50 } else { 35 };
            for (index, option) in options.iter().enumerate() {
                let text = option.as_str().unwrap_or("");
                if text.trim().chars().count() < min_len {
                    return Err(format!("第{}个选项过短，信息密度不足", index + 1));
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
            .and_then(|value| value.as_str())
            .or_else(|| context.get("description").and_then(|value| value.as_str()))
            .unwrap_or("");
        params.insert("initial_idea".to_string(), initial_idea.to_string());
        params.insert(
            "title".to_string(),
            context
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or(&empty)
                .to_string(),
        );
        params.insert(
            "description".to_string(),
            context
                .get("description")
                .and_then(|value| value.as_str())
                .unwrap_or(&empty)
                .to_string(),
        );
        params.insert(
            "theme".to_string(),
            context
                .get("theme")
                .and_then(|value| value.as_str())
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

        use futures::StreamExt;

        let rx = service.generate_text_stream(
            user_prompt.to_string(),
            Some(system_prompt.to_string()),
            None,
        );
        let mut accumulated = String::new();
        tokio::pin!(rx);
        while let Some(chunk) = rx.next().await {
            match chunk {
                Ok(chunk) => {
                    if let Some(text) = chunk.content {
                        accumulated.push_str(&text);
                    }
                }
                Err(error) => return Err(error),
            }
        }

        Ok(accumulated)
    }

    async fn generate_options(
        db: &DatabaseConnection,
        user_id: &str,
        step: &str,
        context: &Value,
        enable_web_research: bool,
        web_research_query: Option<&str>,
    ) -> Result<Value, String> {
        let (system_key, user_key) =
            Self::template_keys(step).ok_or(format!("不支持的步骤: {}", step))?;

        let system_template = PromptTemplateService::system_template_info(system_key)
            .ok_or(format!("模板 {} 不存在", system_key))?;
        let user_template = PromptTemplateService::system_template_info(user_key)
            .ok_or(format!("模板 {} 不存在", user_key))?;

        let format_params = Self::build_format_params(context);
        let research_query = Self::compose_research_query(step, context, None, web_research_query);
        let system_prompt = format!(
            "{}\n\n{}",
            system_template.content,
            Self::build_style_guard(step)
        );
        let user_prompt = Self::format_template(&user_template.content, &format_params);
        let temperature = Self::step_temperature(step);

        let mut last_error = String::new();
        for attempt in 0..MAX_RETRIES {
            let mut prompt = system_prompt.clone();
            if attempt > 0 {
                prompt.push_str(&format!(
                    "\n\n这是第{}次生成，请只返回合法JSON，并确保options里有6个有效选项。",
                    attempt + 1
                ));
            }

            let content = Self::call_ai_for_json(db, user_id, &prompt, &user_prompt, temperature)
                .await
                .map_err(|error| format!("AI调用失败: {}", error))?;

            let cleaned = Self::clean_json_response(&content);
            let result: Value = serde_json::from_str(&cleaned)
                .map_err(|error| format!("JSON解析失败: {}", error))?;

            match Self::validate_options(step, &result) {
                Ok(()) => {
                    return Ok(Self::attach_research_payload(
                        result,
                        &research_query,
                        enable_web_research,
                    ));
                }
                Err(error) => {
                    last_error = error;
                    if attempt < MAX_RETRIES - 1 {
                        continue;
                    }
                }
            }
        }

        Ok(json!({
            "prompt": format!("请为【{}】提供内容：", step),
            "options": ["让AI重新生成", "我自己输入"],
            "error": format!("AI生成格式错误（{}），已自动重试{}次，请手动重试或自己输入", last_error, MAX_RETRIES),
            "research_query": if enable_web_research { research_query } else { String::new() },
            "research_assets": []
        }))
    }

    async fn refine_options(
        db: &DatabaseConnection,
        user_id: &str,
        step: &str,
        context: &Value,
        feedback: &str,
        previous_options: &[String],
        enable_web_research: bool,
        web_research_query: Option<&str>,
    ) -> Result<Value, String> {
        let (system_key, user_key) =
            Self::template_keys(step).ok_or(format!("不支持的步骤: {}", step))?;

        let system_template = PromptTemplateService::system_template_info(system_key)
            .ok_or(format!("模板 {} 不存在", system_key))?;
        let user_template = PromptTemplateService::system_template_info(user_key)
            .ok_or(format!("模板 {} 不存在", user_key))?;

        let format_params = Self::build_format_params(context);
        let research_query =
            Self::compose_research_query(step, context, Some(feedback), web_research_query);
        let mut system_prompt = format!(
            "{}\n\n{}",
            system_template.content,
            Self::build_style_guard(step)
        );
        let user_prompt = Self::format_template(&user_template.content, &format_params);

        let previous = if previous_options.is_empty() {
            "（无）".to_string()
        } else {
            previous_options
                .iter()
                .map(|option| format!("- {}", option))
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
            feedback, previous
        ));

        let temperature = (Self::step_temperature(step) + 0.1).min(0.9);

        let mut last_error = String::new();
        for attempt in 0..MAX_RETRIES {
            let mut prompt = system_prompt.clone();
            if attempt > 0 {
                prompt.push_str(&format!(
                    "\n\n这是第{}次生成，请只返回合法JSON。",
                    attempt + 1
                ));
            }

            let content = Self::call_ai_for_json(db, user_id, &prompt, &user_prompt, temperature)
                .await
                .map_err(|error| format!("AI调用失败: {}", error))?;

            let cleaned = Self::clean_json_response(&content);
            let result: Value = serde_json::from_str(&cleaned)
                .map_err(|error| format!("JSON解析失败: {}", error))?;

            match Self::validate_options(step, &result) {
                Ok(()) => {
                    return Ok(Self::attach_research_payload(
                        result,
                        &research_query,
                        enable_web_research,
                    ));
                }
                Err(error) => {
                    last_error = error;
                    if attempt < MAX_RETRIES - 1 {
                        continue;
                    }
                }
            }
        }

        Ok(json!({
            "prompt": format!("请为【{}】提供内容：", step),
            "options": ["让AI重新生成", "我自己输入"],
            "error": format!("AI生成格式错误（{}），已自动重试{}次", last_error, MAX_RETRIES),
            "research_query": if enable_web_research { research_query } else { String::new() },
            "research_assets": []
        }))
    }

    async fn quick_generate(
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

        let mut existing_parts: Vec<String> = Vec::new();
        if let Some(title) = title {
            if !title.is_empty() {
                existing_parts.push(format!("- 书名：{}", title));
            }
        }
        if let Some(description) = description {
            if !description.is_empty() {
                existing_parts.push(format!("- 简介：{}", description));
            }
        }
        if let Some(theme) = theme {
            if !theme.is_empty() {
                existing_parts.push(format!("- 主题：{}", theme));
            }
        }
        if let Some(genre) = genre {
            if !genre.is_empty() {
                existing_parts.push(format!("- 类型：{}", genre.join(", ")));
            }
        }
        if let Some(perspective) = narrative_perspective {
            if !perspective.is_empty() {
                existing_parts.push(format!("- 叙事视角：{}", perspective));
            }
        }
        let existing_text = if existing_parts.is_empty() {
            "暂无信息".to_string()
        } else {
            existing_parts.join("\n")
        };

        let mut format_params: HashMap<String, String> = HashMap::new();
        format_params.insert("existing".to_string(), existing_text);
        let mut system_prompt =
            PromptTemplateService::format_prompt(&template.content, &format_params)?;

        system_prompt.push_str(&format!(
            "\n\n{}\n【智能补全专项】保证四个字段像同一部小说，人物语气自然，信息前后一致；\
             仅返回JSON字段值，不输出流程说明或执行步骤；\
             信息不足时先补目标->阻力->选择->后果链；\
             如果用户没给叙事视角，请补一个最适合题材与冲突表达的视角。",
            Self::build_style_guard("description")
        ));

        let user_prompt = "请在不偏离现有信息的前提下补全缺失字段，只返回JSON。";
        let temperature = 0.78;

        let content = Self::call_ai_for_json(db, user_id, &system_prompt, user_prompt, temperature)
            .await
            .map_err(|error| format!("AI调用失败: {}", error))?;

        let cleaned = Self::clean_json_response(&content);
        let result: Value =
            serde_json::from_str(&cleaned).map_err(|error| format!("JSON解析失败: {}", error))?;

        let result_genre = Self::normalize_genre_list(result.get("genre"));
        let result_perspective = result
            .get("narrative_perspective")
            .and_then(|value| value.as_str())
            .unwrap_or("第三人称");

        let final_genre: Vec<String> = match genre {
            Some(genre) if !genre.is_empty() => genre.to_vec(),
            _ => result_genre,
        };

        Ok(json!({
            "title": title.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| result.get("title").and_then(|value| value.as_str()).unwrap_or("")),
            "description": description.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| result.get("description").and_then(|value| value.as_str()).unwrap_or("")),
            "theme": theme.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| result.get("theme").and_then(|value| value.as_str()).unwrap_or("")),
            "genre": final_genre,
            "narrative_perspective": narrative_perspective
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(result_perspective),
        }))
    }

    fn normalize_genre_list(value: Option<&Value>) -> Vec<String> {
        let Some(value) = value else {
            return vec![];
        };

        let mut items: Vec<String> = Vec::new();
        if let Some(array) = value.as_array() {
            for item in array {
                if let Some(text) = item.as_str() {
                    for part in text.split(|character: char| {
                        character == '，'
                            || character == ','
                            || character == '、'
                            || character == '/'
                            || character == '|'
                            || character == '｜'
                    }) {
                        let trimmed = part.trim();
                        if !trimmed.is_empty() {
                            items.push(trimmed.to_string());
                        }
                    }
                }
            }
        } else if let Some(text) = value.as_str() {
            for part in text.split(|character: char| {
                character == '，'
                    || character == ','
                    || character == '、'
                    || character == '/'
                    || character == '|'
                    || character == '｜'
            }) {
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

fn inspiration_error_status(detail: &str) -> StatusCode {
    let lower = detail.to_lowercase();
    let is_bad_request = lower.contains("api key")
        || lower.contains("base url")
        || lower.contains("invalid token")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
        || detail.contains("用户设置不存在")
        || detail.contains("请先在设置")
        || detail.contains("缺少有效")
        || detail.contains("配置")
        || detail.contains("密钥");

    if is_bad_request {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum GenreInput {
    Single(String),
    Multiple(Vec<String>),
}

impl GenreInput {
    fn as_vec(&self) -> Vec<String> {
        match self {
            Self::Single(value) => vec![value.clone()],
            Self::Multiple(values) => values.clone(),
        }
    }
}

#[derive(Deserialize)]
struct GenerateOptionsRequest {
    step: String,
    context: Value,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
}

#[derive(Deserialize)]
struct RefineOptionsRequest {
    step: String,
    context: Value,
    feedback: String,
    #[serde(default)]
    previous_options: Vec<String>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
}

#[derive(Deserialize)]
struct QuickGenerateRequest {
    title: Option<String>,
    description: Option<String>,
    theme: Option<String>,
    genre: Option<GenreInput>,
    narrative_perspective: Option<String>,
}

async fn generate_options(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<GenerateOptionsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match InspirationService::generate_options(
        &db,
        &claims.sub,
        &body.step,
        &body.context,
        body.enable_web_research.unwrap_or(false),
        body.web_research_query.as_deref(),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let detail = format!("生成选项失败: {}", e);
            Err((
                inspiration_error_status(&detail),
                Json(json!({ "detail": detail })),
            ))
        }
    }
}

async fn refine_options(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RefineOptionsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match InspirationService::refine_options(
        &db,
        &claims.sub,
        &body.step,
        &body.context,
        &body.feedback,
        &body.previous_options,
        body.enable_web_research.unwrap_or(false),
        body.web_research_query.as_deref(),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let detail = format!("生成选项失败: {}", e);
            Err((
                inspiration_error_status(&detail),
                Json(json!({ "detail": detail })),
            ))
        }
    }
}

async fn quick_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<QuickGenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let normalized_genre = body.genre.as_ref().map(GenreInput::as_vec);
    let genre_ref: Option<&[String]> = normalized_genre.as_deref();
    match InspirationService::quick_generate(
        &db,
        &claims.sub,
        body.title.as_deref(),
        body.description.as_deref(),
        body.theme.as_deref(),
        genre_ref,
        body.narrative_perspective.as_deref(),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let detail = format!("智能补全失败: {}", e);
            Err((
                inspiration_error_status(&detail),
                Json(json!({ "detail": detail })),
            ))
        }
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(INSPIRATION_GENERATE_OPTIONS_ROUTE, post(generate_options))
        .route(INSPIRATION_REFINE_OPTIONS_ROUTE, post(refine_options))
        .route(INSPIRATION_QUICK_GENERATE_ROUTE, post(quick_generate))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use super::{
        build_inspiration_route_owner_contract, inspiration_error_status, InspirationService,
        INSPIRATION_GENERATE_OPTIONS_ROUTE, INSPIRATION_QUICK_GENERATE_ROUTE,
        INSPIRATION_REFINE_OPTIONS_ROUTE,
    };

    #[test]
    fn should_publish_inspiration_route_owner_contract() {
        let contract = build_inspiration_route_owner_contract();

        assert_eq!(contract["owner"], "inspiration");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/inspiration.rs");
        assert_eq!(
            contract["routes"]["generate_options"],
            INSPIRATION_GENERATE_OPTIONS_ROUTE
        );
        assert_eq!(
            contract["routes"]["refine_options"],
            INSPIRATION_REFINE_OPTIONS_ROUTE
        );
        assert_eq!(
            contract["routes"]["quick_generate"],
            INSPIRATION_QUICK_GENERATE_ROUTE
        );
        let readiness_probes = contract["readiness_probes"].as_array().unwrap();
        assert_eq!(readiness_probes.len(), 6);
        assert_eq!(
            readiness_probes.last().unwrap(),
            "inspiration-quick-generate-business-rust"
        );
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 3);
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-inspiration-business-owner"
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .unwrap();
        assert_eq!(business_probes.len(), 4);
        assert!(business_probes
            .iter()
            .any(|probe| probe == "inspiration-refine-options-business-rust"));
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(4)
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            json!(2)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("phase5-inspiration-business-owner"));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
    }

    #[test]
    fn should_keep_inspiration_route_group_paths_stable() {
        assert_eq!(
            INSPIRATION_GENERATE_OPTIONS_ROUTE,
            "/inspiration/generate-options"
        );
        assert_eq!(
            INSPIRATION_REFINE_OPTIONS_ROUTE,
            "/inspiration/refine-options"
        );
        assert_eq!(
            INSPIRATION_QUICK_GENERATE_ROUTE,
            "/inspiration/quick-generate"
        );
    }

    #[test]
    fn should_map_configuration_errors_to_bad_request() {
        assert_eq!(
            inspiration_error_status("生成选项失败: api key missing"),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            inspiration_error_status("智能补全失败: 请先在设置中配置模型"),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn should_clean_json_markdown_fence() {
        let input = "```json\n{\"options\": [\"a\", \"b\", \"c\"]}\n```";
        let result = InspirationService::clean_json_response(input);
        assert_eq!(result, "{\"options\": [\"a\", \"b\", \"c\"]}");
    }

    #[test]
    fn should_keep_raw_json_response_when_already_clean() {
        let input = "  {\"options\": [\"a\", \"b\", \"c\"]}  ";
        let result = InspirationService::clean_json_response(input);
        assert!(result.contains("\"options\""));
    }

    #[test]
    fn should_validate_title_options_payload() {
        let result = json!({
            "options": ["选项一", "选项二", "选项三", "选项四", "选项五", "选项六"]
        });
        assert!(InspirationService::validate_options("title", &result).is_ok());
    }

    #[test]
    fn should_reject_too_few_options() {
        let result = json!({
            "options": ["a", "b"]
        });
        assert!(InspirationService::validate_options("title", &result).is_err());
    }
}
