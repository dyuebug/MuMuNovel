use std::collections::HashMap;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use crate::ai::service::AIService;
use crate::models::{career, character, project};
use crate::services::career_service::CareerService;
use crate::services::chapter_service::ChapterService;
use crate::services::character_service::CharacterService;
use crate::services::outline_service::OutlineService;
use crate::services::project_service::{CreateProjectParams, ProjectService};
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
use crate::utils::sse::SseChannel;

const MAX_WORLD_RETRIES: u32 = 3;

pub fn clean_json_response(text: &str) -> String {
    let text = text.trim();
    if let Some(start) = text.find("```json") {
        let rest = &text[start + 7..];
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find("```") {
        let rest = &text[start + 3..];
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return text[start..=end].to_string();
        }
    }
    text.to_string()
}

fn default_world_data() -> Value {
    serde_json::json!({
        "time_period": "AI多次返回为空，请稍后重试",
        "location": "AI多次返回为空，请稍后重试",
        "atmosphere": "AI多次返回为空，请稍后重试",
        "rules": "AI多次返回为空，请稍后重试"
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_world_building(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    title: &str,
    description: &str,
    theme: &str,
    genre: &str,
    narrative_perspective: Option<&str>,
    target_words: Option<i32>,
    chapter_count: Option<i32>,
    character_count: Option<i32>,
    outline_mode: Option<&str>,
    default_creative_mode: Option<&str>,
    default_story_focus: Option<&str>,
    default_plot_stage: Option<&str>,
    default_story_creation_brief: Option<&str>,
    default_quality_preset: Option<&str>,
    default_quality_notes: Option<&str>,
    provider_override: Option<&str>,
    model_override: Option<&str>,
) {
    let progress = Mutex::new(0u32);

    macro_rules! send_progress {
        ($channel:expr, $progress:expr, $msg:expr, $pct:expr, $status:expr) => {
            $channel.progress($msg, $pct, $status).await;
            *$progress.lock().await = $pct;
        };
    }

    // --- Validate required fields ---
    if title.is_empty() || description.is_empty() || theme.is_empty() || genre.is_empty() {
        channel
            .error("缺少必填参数：title、description、theme、genre", 400)
            .await;
        return;
    }

    // --- Build AI config ---
    let ai_config = match SettingsService::build_ai_config(
        db, user_id, provider_override, model_override, None,
    )
    .await
    {
        Ok(cfg) => cfg,
        Err(e) => {
            channel.error(&format!("AI配置失败: {}", e), 500).await;
            return;
        }
    };

    // --- Look up WORLD_BUILDING template ---
    send_progress!(
        channel,
        progress,
        &format!("准备AI提示词..."),
        15,
        "processing"
    );

    let template = match PromptTemplateService::system_template_info("WORLD_BUILDING") {
        Some(t) => t,
        None => {
            channel.error("WORLD_BUILDING模板未找到", 500).await;
            return;
        }
    };

    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("title".into(), title.to_string());
    params.insert("theme".into(), theme.to_string());
    params.insert("genre".into(), genre.to_string());
    params.insert("description".into(), description.to_string());

    let prompt = match PromptTemplateService::format_prompt(&template.content, &params) {
        Ok(p) => p,
        Err(e) => {
            channel.error(&format!("提示词格式化失败: {}", e), 500).await;
            return;
        }
    };

    let system_prompt: Option<String> = ai_config.system_prompt.clone();

    let ai_service = AIService::new(ai_config);
    let mut world_generation_success = false;
    let mut world_retry_count = 0u32;
    let mut world_data: Value = serde_json::json!({});

    // --- Retry loop ---
    while world_retry_count < MAX_WORLD_RETRIES && !world_generation_success {
        if world_retry_count > 0 {
            channel
                .progress(
                    &format!(
                        "⚠ 重试中... ({}/{})",
                        world_retry_count, MAX_WORLD_RETRIES
                    ),
                    *progress.lock().await,
                    "processing",
                )
                .await;
        }

        let mut accumulated_text = String::new();
        let estimated_total = 3000usize;
        let mut chunk_count = 0u64;

        send_progress!(
            channel,
            progress,
            &format!("生成世界观中..."),
            20,
            "processing"
        );

        // Stream AI response
        let mut rx = ai_service.generate_text_stream(
            prompt.clone(),
            system_prompt.clone(),
            None,
        );

        while let Some(chunk_result) = rx.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if let Some(text) = chunk.content {
                        accumulated_text.push_str(&text);
                        channel.chunk(&text).await;
                        chunk_count += 1;

                        if chunk_count % 10 == 0 {
                            let current_len = accumulated_text.len();
                            let char_bonus =
                                (current_len as f64 / estimated_total as f64 * 60.0) as u32;
                            let pct = (20 + char_bonus).clamp(20, 80);
                            channel
                                .progress(
                                    &format!(
                                        "生成世界观中... ({}字符)",
                                        current_len
                                    ),
                                    pct,
                                    "processing",
                                )
                                .await;
                            *progress.lock().await = pct;
                        }
                    }
                    if chunk.done {
                        break;
                    }
                }
                Err(e) => {
                    channel
                        .progress(
                            &format!("⚠ 生成警告: {}", e),
                            *progress.lock().await,
                            "processing",
                        )
                        .await;
                }
            }
        }

        // Check for empty response
        if accumulated_text.trim().is_empty() {
            world_retry_count += 1;
            if world_retry_count < MAX_WORLD_RETRIES {
                channel
                    .progress(
                        &format!("⚠ AI返回为空，重试 ({}/{})", world_retry_count, MAX_WORLD_RETRIES),
                        *progress.lock().await,
                        "processing",
                    )
                    .await;
                continue;
            } else {
                world_data = default_world_data();
                break;
            }
        }

        // Parse JSON
        channel
            .progress("解析世界观数据...", 85, "processing")
            .await;
        *progress.lock().await = 85;

        let cleaned = clean_json_response(&accumulated_text);
        match serde_json::from_str::<Value>(&cleaned) {
            Ok(data) => {
                world_data = data;
                world_generation_success = true;
            }
            Err(_e) => {
                world_retry_count += 1;
                if world_retry_count < MAX_WORLD_RETRIES {
                    channel
                        .progress(
                            &format!(
                                "⚠ JSON解析失败，重试 ({}/{})",
                                world_retry_count, MAX_WORLD_RETRIES
                            ),
                            *progress.lock().await,
                            "processing",
                        )
                        .await;
                    continue;
                } else {
                    world_data = default_world_data();
                    world_generation_success = true;
                }
            }
        }
    }

    // --- Save to DB ---
    channel
        .progress("保存世界观到数据库...", 90, "processing")
        .await;

    let world_time_period = world_data
        .get("time_period")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let world_location = world_data
        .get("location")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let world_atmosphere = world_data
        .get("atmosphere")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let world_rules = world_data
        .get("rules")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let create_params = CreateProjectParams {
        user_id: user_id.to_string(),
        title: title.to_string(),
        description: Some(description.to_string()),
        theme: Some(theme.to_string()),
        genre: Some(genre.to_string()),
        world_time_period,
        world_location,
        world_atmosphere,
        world_rules,
        narrative_perspective: narrative_perspective.map(|s| s.to_string()),
        target_words: target_words.unwrap_or(0),
        chapter_count,
        character_count: character_count.unwrap_or(5),
        outline_mode: outline_mode.unwrap_or("one-to-many").to_string(),
        default_creative_mode: default_creative_mode.map(|s| s.to_string()),
        default_story_focus: default_story_focus.map(|s| s.to_string()),
        default_plot_stage: default_plot_stage.map(|s| s.to_string()),
        default_story_creation_brief: default_story_creation_brief.map(|s| s.to_string()),
        default_quality_preset: default_quality_preset.map(|s| s.to_string()),
        default_quality_notes: default_quality_notes.map(|s| s.to_string()),
    };

    let project = match ProjectService::create_full(db, create_params).await {
        Ok(p) => {
            // Auto-assign default writing style (best-effort)
            if let Err(e) = ProjectService::assign_default_style(db, &p.id).await {
                channel
                    .progress(&format!("⚠ 默认风格设置失败: {}", e), 95, "processing")
                    .await;
            }
            p
        }
        Err(e) => {
            channel.error(&format!("项目创建失败: {}", e), 500).await;
            return;
        }
    };

    // --- Complete ---
    channel.progress("世界观生成完成!", 100, "success").await;
    channel
        .result(&serde_json::json!({
            "project_id": project.id,
            "time_period": world_data.get("time_period"),
            "location": world_data.get("location"),
            "atmosphere": world_data.get("atmosphere"),
            "rules": world_data.get("rules"),
        }))
        .await;
    channel.done().await;
}

pub async fn regenerate_world_building(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    project_id: &str,
    provider_override: Option<&str>,
    model_override: Option<&str>,
) {
    let progress = Mutex::new(0u32);

    // --- Load project ---
    channel.progress("加载项目信息...", 5, "processing").await;
    *progress.lock().await = 5;

    let project = match project::Entity::find_by_id(project_id).one(db).await {
        Ok(Some(p)) => p,
        Ok(None) => { channel.error("项目不存在", 404).await; return; }
        Err(e) => { channel.error(&format!("加载项目失败: {}", e), 500).await; return; }
    };
    if project.user_id != user_id {
        channel.error("无权访问该项目", 403).await; return;
    }

    // --- Build AI config ---
    let ai_config = match SettingsService::build_ai_config(
        db, user_id, provider_override, model_override, None,
    ).await {
        Ok(cfg) => cfg,
        Err(e) => { channel.error(&format!("AI配置失败: {}", e), 500).await; return; }
    };

    // --- Load WORLD_BUILDING template ---
    channel.progress("准备AI提示词...", 15, "processing").await;
    *progress.lock().await = 15;

    let template = match PromptTemplateService::system_template_info("WORLD_BUILDING") {
        Some(t) => t,
        None => { channel.error("WORLD_BUILDING模板未找到", 500).await; return; }
    };

    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("title".into(), project.title.clone());
    params.insert("theme".into(), project.theme.unwrap_or_else(|| "未设定".into()));
    params.insert("genre".into(), project.genre.unwrap_or_else(|| "通用".into()));
    params.insert("description".into(), project.description.unwrap_or_default());

    let prompt = match PromptTemplateService::format_prompt(&template.content, &params) {
        Ok(p) => p,
        Err(e) => { channel.error(&format!("提示词格式化失败: {}", e), 500).await; return; }
    };

    let system_prompt: Option<String> = ai_config.system_prompt.clone();
    let ai_service = AIService::new(ai_config);
    let mut world_generation_success = false;
    let mut world_retry_count = 0u32;
    let mut world_data: Value = serde_json::json!({});

    // --- Retry loop ---
    while world_retry_count < MAX_WORLD_RETRIES && !world_generation_success {
        if world_retry_count > 0 {
            channel.progress(
                &format!("⚠ 重试中... ({}/{})", world_retry_count, MAX_WORLD_RETRIES),
                *progress.lock().await, "processing",
            ).await;
        }

        let mut accumulated_text = String::new();
        let estimated_total = 3000usize;
        let mut chunk_count = 0u64;

        channel.progress("重新生成世界观...", 20, "processing").await;
        *progress.lock().await = 20;

        let mut rx = ai_service.generate_text_stream(
            prompt.clone(),
            system_prompt.clone(),
            None,
        );

        while let Some(chunk_result) = rx.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if let Some(text) = chunk.content {
                        accumulated_text.push_str(&text);
                        channel.chunk(&text).await;
                        chunk_count += 1;

                        if chunk_count % 10 == 0 {
                            let current_len = accumulated_text.len();
                            let char_bonus = (current_len as f64 / estimated_total as f64 * 60.0) as u32;
                            let pct = (20 + char_bonus).clamp(20, 80);
                            channel.progress(
                                &format!("重新生成世界观... ({}字符)", current_len),
                                pct, "processing",
                            ).await;
                            *progress.lock().await = pct;
                        }
                    }
                    if chunk.done { break; }
                }
                Err(e) => {
                    channel.progress(
                        &format!("⚠ 生成警告: {}", e),
                        *progress.lock().await, "processing",
                    ).await;
                }
            }
        }

        // Check for empty response
        if accumulated_text.trim().is_empty() {
            world_retry_count += 1;
            if world_retry_count < MAX_WORLD_RETRIES {
                channel.progress(
                    &format!("⚠ AI返回为空，重试 ({}/{})", world_retry_count, MAX_WORLD_RETRIES),
                    *progress.lock().await, "processing",
                ).await;
                continue;
            } else {
                world_data = default_world_data();
                break;
            }
        }

        // Parse JSON
        channel.progress("解析世界观数据...", 85, "processing").await;
        *progress.lock().await = 85;

        let cleaned = clean_json_response(&accumulated_text);
        match serde_json::from_str::<Value>(&cleaned) {
            Ok(data) => {
                world_data = data;
                world_generation_success = true;
            }
            Err(_e) => {
                world_retry_count += 1;
                if world_retry_count < MAX_WORLD_RETRIES {
                    channel.progress(
                        &format!("⚠ JSON解析失败，重试 ({}/{})", world_retry_count, MAX_WORLD_RETRIES),
                        *progress.lock().await, "processing",
                    ).await;
                } else {
                    world_data = default_world_data();
                    break;
                }
            }
        }
    }

    // --- Return result without saving to DB (preview only) ---
    channel.progress("生成完成，等待用户确认...", 95, "processing").await;

    channel.progress("世界观重新生成完成!", 100, "success").await;
    channel.result(&serde_json::json!({
        "time_period": world_data.get("time_period"),
        "location": world_data.get("location"),
        "atmosphere": world_data.get("atmosphere"),
        "rules": world_data.get("rules"),
    })).await;
    channel.done().await;
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_career_system(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    project_id: &str,
    provider_override: Option<&str>,
    model_override: Option<&str>,
) {
    let progress = Mutex::new(0u32);

    // Load project
    channel.progress("加载项目信息...", 5, "processing").await;

    let project = match project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .one(db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            channel.error("项目不存在", 404).await;
            return;
        }
        Err(e) => {
            channel.error(&format!("加载项目失败: {}", e), 500).await;
            return;
        }
    };

    let world_data = serde_json::json!({
        "time_period": project.world_time_period.as_deref().unwrap_or("未设定"),
        "location": project.world_location.as_deref().unwrap_or("未设定"),
        "atmosphere": project.world_atmosphere.as_deref().unwrap_or("未设定"),
        "rules": project.world_rules.as_deref().unwrap_or("未设定"),
    });

    // Build AI config
    let ai_config = match SettingsService::build_ai_config(db, user_id, provider_override, model_override, None).await {
        Ok(cfg) => cfg,
        Err(e) => {
            channel.error(&format!("AI配置失败: {}", e), 500).await;
            return;
        }
    };

    // Look up template
    channel.progress("准备AI提示词...", 15, "processing").await;
    *progress.lock().await = 15;

    let template = match PromptTemplateService::system_template_info("CAREER_SYSTEM_GENERATION") {
        Some(t) => t,
        None => {
            channel.error("CAREER_SYSTEM_GENERATION模板未找到", 500).await;
            return;
        }
    };

    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("title".into(), project.title.clone());
    params.insert("genre".into(), project.genre.unwrap_or_default());
    params.insert("theme".into(), project.theme.unwrap_or_default());
    params.insert("description".into(), project.description.unwrap_or_default());
    params.insert("time_period".into(), world_data["time_period"].as_str().unwrap_or("未设定").into());
    params.insert("location".into(), world_data["location"].as_str().unwrap_or("未设定").into());
    params.insert("atmosphere".into(), world_data["atmosphere"].as_str().unwrap_or("未设定").into());
    params.insert("rules".into(), world_data["rules"].as_str().unwrap_or("未设定").into());

    let prompt = match PromptTemplateService::format_prompt(&template.content, &params) {
        Ok(p) => p,
        Err(e) => {
            channel.error(&format!("提示词格式化失败: {}", e), 500).await;
            return;
        }
    };

    let system_prompt: Option<String> = ai_config.system_prompt.clone();
    let ai_service = AIService::new(ai_config);

    const MAX_RETRIES: u32 = 3;
    let mut retry_count = 0u32;
    let mut success = false;
    let mut result_json: Value = serde_json::json!({});

    // Retry loop
    while retry_count < MAX_RETRIES && !success {
        if retry_count > 0 {
            channel.progress(
                &format!("⚠ 重试中... ({}/{})", retry_count, MAX_RETRIES),
                *progress.lock().await,
                "processing",
            ).await;
        }

        let mut accumulated = String::new();
        let mut chunk_count = 0u64;

        channel.progress("生成职业体系中...", 20, "processing").await;
        *progress.lock().await = 20;

        let mut rx = ai_service.generate_text_stream(prompt.clone(), system_prompt.clone(), None);

        while let Some(chunk_result) = rx.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if let Some(text) = chunk.content {
                        accumulated.push_str(&text);
                        channel.chunk(&text).await;
                        chunk_count += 1;

                        if chunk_count % 10 == 0 {
                            let pct = (20 + (accumulated.len() as u32 / 50).min(60)).clamp(20, 80);
                            channel.progress(
                                &format!("生成职业体系中... ({}字符)", accumulated.len()),
                                pct,
                                "processing",
                            ).await;
                            *progress.lock().await = pct;
                        }
                    }
                    if chunk.done { break; }
                }
                Err(e) => {
                    channel.progress(&format!("⚠ 生成警告: {}", e), *progress.lock().await, "processing").await;
                }
            }
        }

        if accumulated.trim().is_empty() {
            retry_count += 1;
            if retry_count < MAX_RETRIES {
                channel.progress(&format!("⚠ AI返回为空，重试 ({}/{})", retry_count, MAX_RETRIES), *progress.lock().await, "processing").await;
                continue;
            } else {
                channel.error("职业体系生成失败（AI多次返回为空）", 500).await;
                return;
            }
        }

        channel.progress("解析职业体系数据...", 82, "processing").await;
        *progress.lock().await = 82;

        let cleaned = clean_json_response(&accumulated);
        match serde_json::from_str::<Value>(&cleaned) {
            Ok(data) => {
                result_json = data;
                success = true;
            }
            Err(_e) => {
                retry_count += 1;
                if retry_count < MAX_RETRIES {
                    channel.progress(&format!("⚠ JSON解析失败，重试 ({}/{})", retry_count, MAX_RETRIES), *progress.lock().await, "processing").await;
                    continue;
                } else {
                    channel.error("职业体系解析失败（已达最大重试次数）", 500).await;
                    return;
                }
            }
        }
    }

    // Save to DB
    channel.progress("保存职业数据...", 87, "processing").await;

    let mut main_names: Vec<String> = Vec::new();
    let mut sub_names: Vec<String> = Vec::new();

    // Save main careers
    if let Some(main_careers) = result_json.get("main_careers").and_then(|v| v.as_array()) {
        for (idx, career_info) in main_careers.iter().enumerate() {
            let default_name = format!("未命名主职业{}", idx + 1);
            let name = career_info.get("name").and_then(|v| v.as_str()).unwrap_or(&default_name);
            match CareerService::create_full(
                db, project_id, name, "main",
                career_info.get("description").and_then(|v| v.as_str()),
                career_info.get("category").and_then(|v| v.as_str()),
                &career_info.get("stages").map(|v| v.to_string()).unwrap_or_else(|| "[]".into()),
                career_info.get("max_stage").and_then(|v| v.as_i64()).unwrap_or(10) as i32,
                career_info.get("requirements").and_then(|v| v.as_str()),
                career_info.get("special_abilities").and_then(|v| v.as_str()),
                career_info.get("worldview_rules").and_then(|v| v.as_str()),
                career_info.get("attribute_bonuses").map(|v| v.to_string()).as_deref(),
            ).await {
                Ok(_) => main_names.push(name.to_string()),
                Err(e) => {
                    channel.progress(&format!("⚠ 创建主职业失败: {}", e), *progress.lock().await, "processing").await;
                }
            }
        }
    }

    // Save sub careers
    if let Some(sub_careers) = result_json.get("sub_careers").and_then(|v| v.as_array()) {
        for (idx, career_info) in sub_careers.iter().enumerate() {
            let default_name = format!("未命名副职业{}", idx + 1);
            let name = career_info.get("name").and_then(|v| v.as_str()).unwrap_or(&default_name);
            match CareerService::create_full(
                db, project_id, name, "sub",
                career_info.get("description").and_then(|v| v.as_str()),
                career_info.get("category").and_then(|v| v.as_str()),
                &career_info.get("stages").map(|v| v.to_string()).unwrap_or_else(|| "[]".into()),
                career_info.get("max_stage").and_then(|v| v.as_i64()).unwrap_or(5) as i32,
                career_info.get("requirements").and_then(|v| v.as_str()),
                career_info.get("special_abilities").and_then(|v| v.as_str()),
                career_info.get("worldview_rules").and_then(|v| v.as_str()),
                career_info.get("attribute_bonuses").map(|v| v.to_string()).as_deref(),
            ).await {
                Ok(_) => sub_names.push(name.to_string()),
                Err(e) => {
                    channel.progress(&format!("⚠ 创建副职业失败: {}", e), *progress.lock().await, "processing").await;
                }
            }
        }
    }

    // Update wizard step
    if let Err(e) = ProjectService::update_wizard_step(db, project_id, 2).await {
        channel.progress(&format!("⚠ 向导步骤更新失败: {}", e), 95, "processing").await;
    }

    // Complete
    channel.progress("职业体系生成完成!", 100, "success").await;
    channel.result(&serde_json::json!({
        "project_id": project_id,
        "main_careers_count": main_names.len(),
        "sub_careers_count": sub_names.len(),
        "main_careers": main_names,
        "sub_careers": sub_names,
    })).await;
    channel.done().await;
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_characters(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    project_id: &str,
    count: usize,
    world_context: Option<Value>,
    theme: Option<&str>,
    genre: Option<&str>,
    requirements: Option<&str>,
    provider_override: Option<&str>,
    model_override: Option<&str>,
) {
    let progress = Mutex::new(0u32);
    const BATCH_SIZE: usize = 5;
    const MAX_RETRIES: u32 = 3;

    // --- Load project ---
    channel.progress("验证项目...", 5, "processing").await;

    let project = match project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .one(db).await
    {
        Ok(Some(p)) => p,
        Ok(None) => { channel.error("项目不存在", 404).await; return; }
        Err(e) => { channel.error(&format!("加载项目失败: {}", e), 500).await; return; }
    };

    // Build world context from project or param
    let wctx = world_context.unwrap_or_else(|| serde_json::json!({
        "time_period": project.world_time_period.as_deref().unwrap_or("未设定"),
        "location": project.world_location.as_deref().unwrap_or("未设定"),
        "atmosphere": project.world_atmosphere.as_deref().unwrap_or("未设定"),
        "rules": project.world_rules.as_deref().unwrap_or("未设定"),
    }));

    // --- Load careers ---
    channel.progress("加载职业体系...", 10, "processing").await;
    *progress.lock().await = 10;

    let careers = match career::Entity::find()
        .filter(career::Column::ProjectId.eq(project_id))
        .order_by_asc(career::Column::CareerType)
        .order_by_asc(career::Column::Id)
        .all(db).await
    {
        Ok(c) => c,
        Err(e) => { channel.progress(&format!("⚠ 加载职业失败: {}", e), 10, "processing").await; vec![] }
    };

    let main_careers: Vec<&career::Model> = careers.iter().filter(|c| c.career_type == "main").collect();
    let sub_careers: Vec<&career::Model> = careers.iter().filter(|c| c.career_type == "sub").collect();

    let mut careers_context = String::new();
    if !main_careers.is_empty() || !sub_careers.is_empty() {
        careers_context.push_str("\n\n【职业体系】\n");
        if !main_careers.is_empty() {
            careers_context.push_str("主职业：\n");
            for c in &main_careers {
                careers_context.push_str(&format!("- {}: {}\n", c.name, c.description.as_deref().unwrap_or("暂无描述")));
            }
        }
        if !sub_careers.is_empty() {
            careers_context.push_str("\n副职业：\n");
            for c in &sub_careers {
                careers_context.push_str(&format!("- {}: {}\n", c.name, c.description.as_deref().unwrap_or("暂无描述")));
            }
        }
        careers_context.push_str("\n请为每个角色分配职业：\n");
        careers_context.push_str("- 每个角色必须有1个主职业\n- 每个角色可以有0-2个副职业\n");
        careers_context.push_str("- 主职业初始阶段建议为1-3\n- 副职业初始阶段建议为1-2\n");
        careers_context.push_str("- 请在返回的JSON中包含 career_assignment 字段\n");
    }

    // Build career name map
    let career_name_map: HashMap<&str, &career::Model> = careers.iter().map(|c| (c.name.as_str(), c)).collect();

    // --- Build AI config ---
    let ai_config = match SettingsService::build_ai_config(db, user_id, provider_override, model_override, None).await {
        Ok(cfg) => cfg,
        Err(e) => { channel.error(&format!("AI配置失败: {}", e), 500).await; return; }
    };

    let template = match PromptTemplateService::system_template_info("CHARACTERS_BATCH_GENERATION") {
        Some(t) => t,
        None => { channel.error("CHARACTERS_BATCH_GENERATION模板未找到", 500).await; return; }
    };

    let sys_prompt: Option<String> = ai_config.system_prompt.clone();
    let ai_service = AIService::new(ai_config);

    let total_batches = (count + BATCH_SIZE - 1) / BATCH_SIZE;
    let mut all_characters: Vec<Value> = Vec::new();

    // --- Batch generation loop ---
    for batch_idx in 0..total_batches {
        let remaining = count - all_characters.len();
        if remaining == 0 { break; }
        let current_batch_size = BATCH_SIZE.min(remaining);

        let batch_progress = 15 + (batch_idx * 60 / total_batches) as u32;

        let mut retry_count = 0u32;
        let mut batch_success = false;

        while retry_count < MAX_RETRIES && !batch_success {
            if retry_count > 0 {
                channel.progress(
                    &format!("⚠ 第{}批重试 ({}/{})", batch_idx + 1, retry_count, MAX_RETRIES),
                    batch_progress, "processing",
                ).await;
            }

            // Build batch requirements
            let mut existing_context = String::new();
            if !all_characters.is_empty() {
                existing_context.push_str("\n\n【已生成的角色】:\n");
                for ch in &all_characters {
                    existing_context.push_str(&format!(
                        "- {}: {}, {}...\n",
                        ch.get("name").and_then(|v| v.as_str()).unwrap_or("?"),
                        ch.get("role_type").and_then(|v| v.as_str()).unwrap_or("未知"),
                        ch.get("personality").and_then(|v| v.as_str()).unwrap_or("暂无").chars().take(50).collect::<String>()
                    ));
                }
                existing_context.push_str("\n请确保新角色与已有角色形成合理的关系网络。\n");
            }

            let req_str = requirements.unwrap_or("");
            let batch_req = if batch_idx == 0 {
                if current_batch_size == 1 {
                    format!("{}\n请生成1个主角(protagonist)", req_str)
                } else {
                    format!("{}\n请精确生成{}个角色:1个主角(protagonist)和{}个核心配角(supporting)", req_str, current_batch_size, current_batch_size - 1)
                }
            } else if batch_idx == total_batches - 1 {
                format!("{}\n请精确生成{}个角色{}\n可以包含组织或反派(antagonist)", req_str, current_batch_size, existing_context)
            } else {
                format!("{}\n请精确生成{}个角色{}\n主要是配角(supporting)和反派(antagonist)", req_str, current_batch_size, existing_context)
            };

            let mut params: HashMap<String, String> = HashMap::new();
            params.insert("count".into(), current_batch_size.to_string());
            params.insert("time_period".into(), wctx["time_period"].as_str().unwrap_or("未设定").into());
            params.insert("location".into(), wctx["location"].as_str().unwrap_or("未设定").into());
            params.insert("atmosphere".into(), wctx["atmosphere"].as_str().unwrap_or("未设定").into());
            params.insert("rules".into(), wctx["rules"].as_str().unwrap_or("未设定").into());
            params.insert("theme".into(), theme.unwrap_or(project.theme.as_deref().unwrap_or("未设定")).into());
            params.insert("genre".into(), genre.unwrap_or(project.genre.as_deref().unwrap_or("未设定")).into());
            params.insert("requirements".into(), format!("{}{}", batch_req, careers_context));
            params.insert("external_assets".into(), String::new());
            params.insert("reference_assets".into(), String::new());

            let prompt = match PromptTemplateService::format_prompt(&template.content, &params) {
                Ok(p) => p,
                Err(e) => { channel.error(&format!("提示词格式化失败: {}", e), 500).await; return; }
            };

            channel.progress(
                &format!("生成第{}/{}批角色 ({}个)", batch_idx + 1, total_batches, current_batch_size),
                batch_progress, "processing",
            ).await;
            *progress.lock().await = batch_progress;

            // Stream AI
            let mut accumulated = String::new();
            let mut chunk_count = 0u64;
            let _estimated_total = BATCH_SIZE * 800;

            let mut rx = ai_service.generate_text_stream(prompt.clone(), sys_prompt.clone(), None);
            while let Some(chunk_result) = rx.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(text) = chunk.content {
                            accumulated.push_str(&text);
                            channel.chunk(&text).await;
                            chunk_count += 1;

                            if chunk_count % 10 == 0 {
                                let pct = (batch_progress + (accumulated.len() as u32 / 40).min(45)).clamp(batch_progress, batch_progress + 45);
                                channel.progress(
                                    &format!("生成第{}/{}批角色中... ({}字符)", batch_idx + 1, total_batches, accumulated.len()),
                                    pct, "processing",
                                ).await;
                                *progress.lock().await = pct;
                            }
                        }
                        if chunk.done { break; }
                    }
                    Err(e) => {
                        channel.progress(&format!("⚠ 生成警告: {}", e), *progress.lock().await, "processing").await;
                    }
                }
            }

            // Parse JSON
            let cleaned = clean_json_response(&accumulated);
            match serde_json::from_str::<Value>(&cleaned) {
                Ok(data) => {
                    let chars: Vec<Value> = if data.is_array() {
                        data.as_array().unwrap().clone()
                    } else {
                        vec![data]
                    };

                    if chars.len() != current_batch_size {
                        retry_count += 1;
                        if retry_count < MAX_RETRIES {
                            channel.progress(
                                &format!("⚠ 批次{}数量不正确: 期望{}个, 实际{}个，重试 ({}/{})",
                                    batch_idx + 1, current_batch_size, chars.len(), retry_count, MAX_RETRIES),
                                *progress.lock().await, "processing",
                            ).await;
                            continue;
                        } else {
                            channel.error(
                                &format!("批次{}生成数量不正确: 期望{}个, 实际{}个", batch_idx + 1, current_batch_size, chars.len()),
                                500,
                            ).await;
                            return;
                        }
                    }

                    all_characters.extend(chars);
                    batch_success = true;
                    channel.progress(
                        &format!("第{}批完成 ({}角色, 累计{}/{})", batch_idx + 1, current_batch_size, all_characters.len(), count),
                        batch_progress + 50, "processing",
                    ).await;
                    *progress.lock().await = batch_progress + 50;
                }
                Err(_e) => {
                    retry_count += 1;
                    if retry_count < MAX_RETRIES {
                        channel.progress(
                            &format!("⚠ 第{}批JSON解析失败，重试 ({}/{})", batch_idx + 1, retry_count, MAX_RETRIES),
                            *progress.lock().await, "processing",
                        ).await;
                    } else {
                        channel.error(&format!("第{}批JSON解析失败（已达最大重试次数）", batch_idx + 1), 500).await;
                        return;
                    }
                }
            }
        }

        if !batch_success {
            channel.error(&format!("第{}批在{}次重试后仍然失败", batch_idx + 1, MAX_RETRIES), 500).await;
            return;
        }
    }

    // --- Build entity name set for hallucination cleanup ---
    let valid_names: Vec<String> = all_characters.iter()
        .filter_map(|c| c.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();
    let valid_name_set: std::collections::HashSet<&str> = valid_names.iter().map(|s| s.as_str()).collect();
    let _org_names: std::collections::HashSet<&str> = all_characters.iter()
        .filter(|c| c.get("is_organization").and_then(|v| v.as_bool()).unwrap_or(false))
        .filter_map(|c| c.get("name").and_then(|v| v.as_str()))
        .collect();

    // --- Phase 1: Create Characters ---
    channel.progress("保存角色到数据库...", 78, "processing").await;
    *progress.lock().await = 78;

    let mut created: Vec<(character::Model, Value)> = Vec::new();
    let mut name_to_id: HashMap<String, String> = HashMap::new();

    for char_data in &all_characters {
        let name = char_data.get("name").and_then(|v| v.as_str()).unwrap_or("未命名角色");
        let is_org = char_data.get("is_organization").and_then(|v| v.as_bool()).unwrap_or(false);

        let traits_str = char_data.get("traits")
            .map(|v| v.to_string());

        // Build relationships text from relationships_array
        let rel_text = char_data.get("relationships_array").and_then(|v| v.as_array()).map(|rels| {
            let descs: Vec<String> = rels.iter().map(|r| {
                let target = r.get("target_character_name").and_then(|v| v.as_str()).unwrap_or("未知");
                let rt = r.get("relationship_type").and_then(|v| v.as_str()).unwrap_or("关系");
                let desc = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
                format!("{}({}): {}", target, rt, desc)
            }).collect();
            descs.join("; ")
        });

        match CharacterService::create_full(
            db, project_id, name, is_org,
            char_data.get("role_type").and_then(|v| v.as_str()),
            char_data.get("personality").and_then(|v| v.as_str()),
            char_data.get("background").and_then(|v| v.as_str()),
            char_data.get("appearance").and_then(|v| v.as_str()),
            char_data.get("age").and_then(|v| v.as_str()).or_else(|| char_data.get("age").and_then(|v| v.as_i64()).map(|_| "0")),
            char_data.get("gender").and_then(|v| v.as_str()),
            traits_str.as_deref(),
            char_data.get("organization_type").and_then(|v| v.as_str()),
            char_data.get("organization_purpose").and_then(|v| v.as_str()),
            rel_text.as_deref(),
        ).await {
            Ok(model) => {
                name_to_id.insert(name.to_string(), model.id.clone());
                created.push((model, char_data.clone()));
            }
            Err(e) => {
                channel.progress(&format!("⚠ 创建角色失败: {} - {}", name, e), *progress.lock().await, "processing").await;
            }
        }
    }

    // --- Phase 2: Assign Careers ---
    if !career_name_map.is_empty() {
        channel.progress("分配角色职业...", 83, "processing").await;
        *progress.lock().await = 83;

        for (char_model, char_data) in &created {
            if char_model.is_organization { continue; }
            if let Some(ca) = char_data.get("career_assignment") {
                let main_name = ca.get("main_career").and_then(|v| v.as_str());
                let main_stage = ca.get("main_stage").and_then(|v| v.as_i64()).unwrap_or(1) as i32;

                let main_id = main_name.and_then(|n| career_name_map.get(n)).map(|c| c.id.as_str());
                let sub_list: Vec<Value> = ca.get("sub_careers").and_then(|v| v.as_array()).map(|a| {
                    a.iter().filter_map(|s| {
                        let sn = s.get("career").and_then(|v| v.as_str())?;
                        let stage = s.get("stage").and_then(|v| v.as_i64()).unwrap_or(1);
                        career_name_map.get(sn).map(|c| serde_json::json!({"career_id": c.id, "stage": stage}))
                    }).collect()
                }).unwrap_or_default();

                let sub_json = if sub_list.is_empty() { None } else { Some(serde_json::to_string(&sub_list).unwrap_or_default()) };
                let _ = CharacterService::assign_career(
                    db, &char_model.id, main_id, Some(main_stage), sub_json.as_deref(),
                ).await;
            }
        }
    }

    // --- Phase 3: Create Organizations ---
    channel.progress("创建组织记录...", 88, "processing").await;
    *progress.lock().await = 88;

    let mut org_name_to_id: HashMap<String, String> = HashMap::new();
    for (char_model, char_data) in &created {
        if !char_model.is_organization { continue; }
        let name = &char_model.name;
        let power = char_data.get("power_level").and_then(|v| v.as_i64()).unwrap_or(50) as i32;
        match CharacterService::create_organization(
            db, &char_model.id, project_id, power,
            char_data.get("location").and_then(|v| v.as_str()),
            char_data.get("motto").and_then(|v| v.as_str()),
            char_data.get("color").and_then(|v| v.as_str()),
        ).await {
            Ok(org) => { org_name_to_id.insert(name.clone(), org.id); }
            Err(e) => { channel.progress(&format!("⚠ 创建组织失败: {} - {}", name, e), *progress.lock().await, "processing").await; }
        }
    }

    // --- Phase 4: Create Relationships ---
    channel.progress("创建角色关系...", 93, "processing").await;
    *progress.lock().await = 93;

    let mut rel_count = 0;
    for (char_model, char_data) in &created {
        if char_model.is_organization { continue; }
        if let Some(rels) = char_data.get("relationships_array").and_then(|v| v.as_array()) {
            for rel in rels {
                let target_name = rel.get("target_character_name").and_then(|v| v.as_str());
                let Some(target_name) = target_name else { continue; };
                let Some(target_id) = name_to_id.get(target_name) else { continue; };

                if valid_name_set.contains(target_name) {
                    let _ = CharacterService::create_relationship(
                        db, project_id, &char_model.id, target_id, None,
                        rel.get("relationship_type").and_then(|v| v.as_str()),
                        rel.get("intimacy_level").and_then(|v| v.as_i64()).unwrap_or(50) as i32,
                        rel.get("description").and_then(|v| v.as_str()),
                        rel.get("started_at").and_then(|v| v.as_str()),
                    ).await;
                    rel_count += 1;
                }
            }
        }
    }
    channel.progress(&format!("已创建{}条关系", rel_count), 95, "processing").await;

    // --- Update project ---
    if let Err(e) = ProjectService::update_wizard_step(db, project_id, 3).await {
        channel.progress(&format!("⚠ 向导步骤更新失败: {}", e), 96, "processing").await;
    }

    // --- Complete ---
    let result_names: Vec<&str> = created.iter().map(|(m, _)| m.name.as_str()).collect();
    channel.progress("角色生成完成!", 100, "success").await;
    channel.result(&serde_json::json!({
        "message": format!("成功生成{}个角色/组织（分{}批完成）", created.len(), total_batches),
        "count": created.len(),
        "batches": total_batches,
        "characters": result_names,
    })).await;
    channel.done().await;
}

// --- Outline generation helpers ---

fn normalize_outline_items(raw: &Value) -> Vec<Value> {
    if let Some(obj) = raw.as_object() {
        if let Some(chapters) = obj.get("chapters").and_then(|v| v.as_array()) {
            if !chapters.is_empty() {
                return chapters.clone();
            }
        }
        return vec![raw.clone()];
    }
    if let Some(arr) = raw.as_array() {
        return arr.clone();
    }
    vec![raw.clone()]
}

fn pick_outline_field<'a>(data: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(v) = data.get(*key) {
            if !v.is_null() {
                return Some(v);
            }
        }
    }
    None
}

fn format_outline_value(value: &Value) -> String {
    if value.is_null() {
        return String::new();
    }
    if let Some(arr) = value.as_array() {
        let items: Vec<String> = arr.iter()
            .take(3)
            .filter_map(|v| {
                let s = v.as_str().unwrap_or("").trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            })
            .collect();
        return items.join("；");
    }
    if value.is_object() {
        let parts: Vec<String> = value.as_object().unwrap().iter()
            .take(3)
            .map(|(k, v)| format!("{}: {}", k, v.as_str().unwrap_or("")))
            .collect();
        return parts.join("；");
    }
    value.as_str().unwrap_or("").to_string()
}

fn build_outline_content(item: &Value) -> String {
    let summary = item.get("summary")
        .or_else(|| item.get("content"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if summary.len() >= 80 {
        return summary;
    }

    let mut segments: Vec<String> = Vec::new();
    if !summary.is_empty() {
        segments.push(summary.clone());
    }

    let field_groups: &[(&str, &[&str])] = &[
        ("叙事目标", &["goal", "narrative_goal"]),
        ("冲突主线", &["conflict", "conflict_line", "conflict_type"]),
        ("角色抉择", &["decision", "dilemma"]),
        ("代价/风险", &["cost", "stakes"]),
        ("规则影响", &["rule_impact", "world_rule_trigger"]),
        ("人物转折", &["character_turns", "character_arc", "twist"]),
        ("对话钩子", &["dialogue_hook"]),
        ("关键事件", &["key_events", "key_points"]),
    ];

    for (label, keys) in field_groups {
        if let Some(value) = pick_outline_field(item, keys) {
            let text = format_outline_value(value);
            if !text.is_empty() {
                segments.push(format!("{}：{}", label, text));
            }
        }
    }

    let combined = segments.join("\n").trim().to_string();
    if !combined.is_empty() {
        if combined.len() > 1800 {
            combined[..1800].to_string()
        } else {
            combined
        }
    } else {
        summary
    }
}

pub async fn generate_outline(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    project_id: &str,
    chapter_count: usize,
    narrative_perspective: Option<&str>,
    target_words: i32,
    requirements: Option<&str>,
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
    story_creation_brief: Option<&str>,
    quality_preset: Option<&str>,
    quality_notes: Option<&str>,
    provider: Option<&str>,
    model: Option<&str>,
) {
    let chapter_count = chapter_count.clamp(1, 10);

    // --- Load project ---
    channel.progress("加载项目信息...", 1, "processing").await;
    let project = match project::Entity::find_by_id(project_id).one(db).await {
        Ok(Some(p)) => p,
        Ok(None) => { channel.error("项目不存在", 404).await; return; }
        Err(e) => { channel.error(&format!("加载项目失败: {}", e), 500).await; return; }
    };
    if project.user_id != user_id {
        channel.error("无权访问该项目", 403).await; return;
    }

    // --- Load characters ---
    channel.progress("加载角色信息...", 3, "processing").await;
    let characters = match character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .all(db).await
    {
        Ok(c) => c,
        Err(e) => { channel.error(&format!("加载角色失败: {}", e), 500).await; return; }
    };
    let characters_info = if characters.is_empty() {
        "暂无角色信息".to_string()
    } else {
        characters.iter().map(|c| {
            format!("- {}（{}，{}）: {}",
                c.name,
                if c.is_organization { "组织" } else { "角色" },
                c.role_type.as_deref().unwrap_or("未知"),
                c.personality.as_deref().unwrap_or("暂无描述").chars().take(100).collect::<String>()
            )
        }).collect::<Vec<_>>().join("\n")
    };

    // --- Build AI config ---
    let ai_config = match SettingsService::build_ai_config(db, user_id, provider, model, None).await {
        Ok(cfg) => cfg,
        Err(e) => { channel.error(&format!("AI配置失败: {}", e), 500).await; return; }
    };
    let ai_service = AIService::new(ai_config);

    // --- Load template ---
    channel.progress(&format!("准备生成{}个大纲节点...", chapter_count), 5, "processing").await;
    let template = match PromptTemplateService::system_template_info("OUTLINE_CREATE") {
        Some(t) => t,
        None => { channel.error("加载大纲模板失败", 500).await; return; }
    };

    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("title".into(), project.title.clone());
    params.insert("theme".into(), project.theme.unwrap_or_else(|| "未设定".into()));
    params.insert("genre".into(), project.genre.unwrap_or_else(|| "通用".into()));
    params.insert("chapter_count".into(), chapter_count.to_string());
    params.insert("narrative_perspective".into(), narrative_perspective.unwrap_or("").to_string());
    params.insert("target_words".into(), (target_words / 10).to_string());
    params.insert("time_period".into(), project.world_time_period.unwrap_or_else(|| "未设定".into()));
    params.insert("location".into(), project.world_location.unwrap_or_else(|| "未设定".into()));
    params.insert("atmosphere".into(), project.world_atmosphere.unwrap_or_else(|| "未设定".into()));
    params.insert("rules".into(), project.world_rules.unwrap_or_else(|| "未设定".into()));
    params.insert("characters_info".into(), characters_info);
    params.insert("mcp_references".into(), String::new());
    params.insert("requirements".into(), requirements.unwrap_or("").to_string());
    params.insert("external_assets".into(), String::new());
    params.insert("reference_assets".into(), String::new());
    params.insert("creative_mode".into(), creative_mode.unwrap_or("").to_string());
    params.insert("story_focus".into(), story_focus.unwrap_or("").to_string());
    params.insert("plot_stage".into(), plot_stage.unwrap_or("").to_string());
    params.insert("story_creation_brief".into(), story_creation_brief.unwrap_or("").to_string());
    params.insert("quality_preset".into(), quality_preset.unwrap_or("").to_string());
    params.insert("quality_notes".into(), quality_notes.unwrap_or("").to_string());

    let prompt = match PromptTemplateService::format_prompt(&template.content, &params) {
        Ok(p) => p,
        Err(e) => { channel.error(&format!("提示词格式化失败: {}", e), 500).await; return; }
    };

    let sys_prompt = format!(
        "你是一位专业的小说大纲设计师。你需要为小说《{}》生成{}个大纲节点。请严格输出JSON格式。",
        project.title, chapter_count
    );

    // --- Stream AI generation ---
    channel.progress("AI正在生成大纲...", 10, "processing").await;
    let progress = Mutex::new(10u32);
    let mut accumulated = String::new();
    let mut chunk_count = 0u64;

    {
        let mut rx = ai_service.generate_text_stream(prompt.clone(), Some(sys_prompt.clone()), None);
        while let Some(chunk_result) = rx.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if let Some(text) = chunk.content {
                        accumulated.push_str(&text);
                        channel.chunk(&text).await;
                        chunk_count += 1;

                        if chunk_count % 10 == 0 {
                            let pct = (*progress.lock().await + 1).min(55);
                            channel.progress(
                                &format!("生成大纲中... ({}字符)", accumulated.len()),
                                pct, "processing",
                            ).await;
                            *progress.lock().await = pct;
                        }
                    }
                    if chunk.done { break; }
                }
                Err(e) => {
                    channel.progress(&format!("⚠ 生成警告: {}", e), *progress.lock().await, "processing").await;
                }
            }
        }
    }

    // --- Parse JSON (with auto-retry) ---
    channel.progress("解析大纲数据...", 55, "processing").await;
    let cleaned = clean_json_response(&accumulated);
    let outline_data = match serde_json::from_str::<Value>(&cleaned) {
        Ok(data) => normalize_outline_items(&data),
        Err(_parse_error) => {
            channel.progress("JSON解析失败，自动重试...", 56, "processing").await;
            let mut retry_acc = String::new();
            let mut retry_rx = ai_service.generate_text_stream(prompt, Some(sys_prompt), None);
            while let Some(chunk_result) = retry_rx.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(text) = chunk.content {
                            retry_acc.push_str(&text);
                            channel.chunk(&text).await;
                        }
                        if chunk.done { break; }
                    }
                    Err(_) => {}
                }
            }
            let retry_cleaned = clean_json_response(&retry_acc);
            match serde_json::from_str::<Value>(&retry_cleaned) {
                Ok(data) => {
                    channel.progress("已自动修复返回格式，继续保存...", 58, "processing").await;
                    normalize_outline_items(&data)
                }
                Err(e) => {
                    channel.error(&format!("大纲JSON解析失败（已重试）: {}", e), 500).await;
                    return;
                }
            }
        }
    };

    if outline_data.is_empty() {
        channel.error("大纲生成失败，AI返回为空", 500).await;
        return;
    }

    // --- Save outlines to DB ---
    channel.progress("保存大纲到数据库...", 60, "processing").await;

    let existing_order_max = match project::Entity::find_by_id(project_id).one(db).await {
        Ok(Some(_)) => {
            // Use outline entity to query max order
            crate::models::outline::Entity::find()
                .filter(crate::models::outline::Column::ProjectId.eq(project_id))
                .order_by_desc(crate::models::outline::Column::OrderIndex)
                .one(db).await
                .map(|o| o.and_then(|m| m.order_index).unwrap_or(0))
                .unwrap_or(0)
        }
        _ => 0,
    };
    let order_start = existing_order_max + 1;

    let limit = chapter_count.min(outline_data.len());
    let mut created_outlines: Vec<(crate::models::outline::Model, Value)> = Vec::new();

    for (i, item) in outline_data.iter().take(limit).enumerate() {
        let title = item.get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("第{}节", order_start + i as i32));
        let content = build_outline_content(item);
        let structure = serde_json::to_string(item).unwrap_or_default();

        match OutlineService::create(
            db, project_id, user_id,
            &title,
            Some(&content),
            Some(order_start + i as i32),
            Some(&structure),
        ).await {
            Ok(Some(m)) => created_outlines.push((m, item.clone())),
            Ok(None) => { channel.error("无权创建大纲", 403).await; return; }
            Err(e) => { channel.error(&format!("保存大纲失败: {}", e), 500).await; return; }
        }
    }

    channel.progress(&format!("已创建{}个大纲节点", created_outlines.len()), 70, "processing").await;

    // --- Auto-create chapters for one-to-one mode ---
    let mut created_chapters: Vec<crate::models::chapter::Model> = Vec::new();
    let outline_mode = project.outline_mode.clone();

    if outline_mode == "one-to-one" {
        channel.progress("一对一模式：自动创建章节...", 75, "processing").await;

        let existing_max_chapter = crate::models::chapter::Entity::find()
            .filter(crate::models::chapter::Column::ProjectId.eq(project_id))
            .order_by_desc(crate::models::chapter::Column::ChapterNumber)
            .one(db).await
            .map(|c| c.map(|ch| ch.chapter_number).unwrap_or(0))
            .unwrap_or(0);

        for (i, (outline, _)) in created_outlines.iter().enumerate() {
            let chapter_number = existing_max_chapter + 1 + i as i32;
            match ChapterService::create_pending(db, project_id, &outline.title, chapter_number).await {
                Ok(ch) => created_chapters.push(ch),
                Err(e) => {
                    channel.progress(&format!("⚠ 创建章节失败: {}", e), 80, "processing").await;
                }
            }
        }
        channel.progress(&format!("已自动创建{}个章节", created_chapters.len()), 85, "processing").await;
    } else {
        channel.progress("细化模式：跳过自动创建章节", 85, "processing").await;
    }

    // --- Finalize project ---
    channel.progress("完成项目设置...", 90, "processing").await;

    let chapter_total = crate::models::chapter::Entity::find()
        .filter(crate::models::chapter::Column::ProjectId.eq(project_id))
        .all(db).await
        .map(|v| v.len() as i32)
        .unwrap_or(0);

    if let Err(e) = ProjectService::complete_wizard(
        db, project_id,
        chapter_total,
        narrative_perspective,
        target_words,
    ).await {
        channel.progress(&format!("⚠ 项目更新失败: {}", e), 92, "processing").await;
    }

    // --- Result ---
    let note = if outline_mode == "one-to-one" { "已自动创建章节" } else { "可在大纲页面手动展开" };
    let outline_items: Vec<Value> = created_outlines.iter().map(|(m, _)| {
        serde_json::json!({
            "id": m.id,
            "order_index": m.order_index,
            "title": m.title,
            "content": m.content.as_deref().unwrap_or("").chars().take(100).collect::<String>(),
            "note": note,
        })
    }).collect();

    let chapter_items: Vec<Value> = created_chapters.iter().map(|c| {
        serde_json::json!({
            "id": c.id,
            "chapter_number": c.chapter_number,
            "title": c.title,
            "status": c.status,
        })
    }).collect();

    let result_message = if outline_mode == "one-to-one" {
        format!("成功生成{}个大纲节点并自动创建{}个章节（传统模式）", created_outlines.len(), created_chapters.len())
    } else {
        format!("成功生成{}个大纲节点（细化模式，可在大纲页面手动展开）", created_outlines.len())
    };

    channel.progress("大纲生成完成!", 100, "success").await;
    channel.result(&serde_json::json!({
        "message": result_message,
        "outline_count": created_outlines.len(),
        "chapter_count": created_chapters.len(),
        "outline_mode": outline_mode,
        "outlines": outline_items,
        "chapters": chapter_items,
    })).await;
    channel.done().await;
}
