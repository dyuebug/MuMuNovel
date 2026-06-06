use std::collections::HashMap;

use sea_orm::{DatabaseConnection, EntityTrait};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tracing::warn;

use crate::ai::service::AIService;
use crate::models::{chapter, outline, project};
use crate::services::chapter_service::ChapterService;
use crate::services::outline_continue_context_service::{
    build_outline_continue_prompt_context, outline_continue_stage_instruction,
    OUTLINE_CONTINUE_RECENT_LIMIT,
};
use crate::services::outline_quality_summary_snapshot_service::build_outline_quality_guidance_bundle;
use crate::services::outline_requirement_service::{
    build_continue_outline_requirements, build_project_long_term_goal,
};
use crate::services::outline_runtime_system_prompt_service::{
    build_outline_runtime_system_prompt, OutlineRuntimeStage,
};
use crate::services::outline_service::OutlineService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
use crate::services::wizard_request_service::{
    execute_outline_request, outline_generate_request_to_wizard_request,
};
use crate::services::wizard_service::{
    build_outline_content, clean_json_response, normalize_outline_items,
};
use crate::utils::sse::SseChannel;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct OutlineGenerateRouteRequest {
    pub project_id: String,
    #[serde(default = "default_outline_count")]
    pub chapter_count: usize,
    pub narrative_perspective: Option<String>,
    #[serde(default = "default_target_words")]
    pub target_words: i32,
    pub requirements: Option<String>,
    pub creative_mode: Option<String>,
    pub story_focus: Option<String>,
    pub plot_stage: Option<String>,
    pub story_creation_brief: Option<String>,
    pub quality_preset: Option<String>,
    pub quality_notes: Option<String>,
    pub compact_mode: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub theme: Option<String>,
    pub genre: Option<String>,
    pub mode: Option<String>,
    pub story_direction: Option<String>,
    pub keep_existing: Option<bool>,
    pub world_context: Option<Value>,
    pub characters_context: Option<Vec<Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutlineGenerateMode {
    New,
    Continue,
}

#[derive(Debug, Clone, PartialEq)]
struct ContinueOutlineExecutionRequest {
    project_id: String,
    chapter_count: usize,
    narrative_perspective: Option<String>,
    requirements: Option<String>,
    creative_mode: Option<String>,
    story_focus: Option<String>,
    plot_stage: Option<String>,
    story_creation_brief: Option<String>,
    quality_preset: Option<String>,
    quality_notes: Option<String>,
    compact_mode: Option<bool>,
    provider: Option<String>,
    model: Option<String>,
    story_direction: Option<String>,
}

fn default_outline_count() -> usize {
    3
}

fn default_target_words() -> i32 {
    100000
}

pub(crate) fn resolve_outline_generate_mode(
    requested_mode: Option<&str>,
    has_existing_outlines: bool,
) -> Result<OutlineGenerateMode, String> {
    let normalized = requested_mode
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "auto".to_string());

    match normalized.as_str() {
        "auto" => {
            if has_existing_outlines {
                Ok(OutlineGenerateMode::Continue)
            } else {
                Ok(OutlineGenerateMode::New)
            }
        }
        "new" => Ok(OutlineGenerateMode::New),
        "continue" => {
            if has_existing_outlines {
                Ok(OutlineGenerateMode::Continue)
            } else {
                Err("没有可用的现有大纲，无法继续生成".to_string())
            }
        }
        _ => Err(format!("不支持的模式: {}", normalized)),
    }
}

pub(crate) async fn execute_outline_generate_route_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    request: &OutlineGenerateRouteRequest,
) {
    let project_model = match project::Entity::find_by_id(&request.project_id)
        .one(db)
        .await
    {
        Ok(Some(model)) => model,
        Ok(None) => {
            channel.error("项目不存在或无权限", 404).await;
            return;
        }
        Err(error) => {
            channel
                .error(&format!("加载项目信息失败: {}", error), 500)
                .await;
            return;
        }
    };

    if project_model.user_id != user_id {
        channel.error("项目不存在或无权限", 404).await;
        return;
    }

    let existing_outlines = match OutlineService::list(db, &request.project_id, user_id).await {
        Ok(Some(items)) => items,
        Ok(None) => {
            channel.error("项目不存在或无权限", 404).await;
            return;
        }
        Err(error) => {
            channel
                .error(&format!("加载大纲失败: {}", error), 500)
                .await;
            return;
        }
    };

    let mode =
        match resolve_outline_generate_mode(request.mode.as_deref(), !existing_outlines.is_empty())
        {
            Ok(mode) => mode,
            Err(error) => {
                channel.error(&error, 400).await;
                return;
            }
        };

    match mode {
        OutlineGenerateMode::New => {
            let wizard_request = outline_generate_request_to_wizard_request(
                request.project_id.clone(),
                request.chapter_count,
                request.narrative_perspective.clone(),
                request.target_words,
                request.requirements.clone(),
                request.creative_mode.clone(),
                request.story_focus.clone(),
                request.plot_stage.clone(),
                request.story_creation_brief.clone(),
                request.quality_preset.clone(),
                request.quality_notes.clone(),
                request.compact_mode,
                request.provider.clone(),
                request.model.clone(),
            );
            execute_outline_request(db, channel, user_id, wizard_request).await;
        }
        OutlineGenerateMode::Continue => {
            let continue_request = ContinueOutlineExecutionRequest {
                project_id: request.project_id.clone(),
                chapter_count: request.chapter_count,
                narrative_perspective: request.narrative_perspective.clone(),
                requirements: request.requirements.clone(),
                creative_mode: request.creative_mode.clone(),
                story_focus: request.story_focus.clone(),
                plot_stage: request.plot_stage.clone(),
                story_creation_brief: request.story_creation_brief.clone(),
                quality_preset: request.quality_preset.clone(),
                quality_notes: request.quality_notes.clone(),
                compact_mode: request.compact_mode,
                provider: request.provider.clone(),
                model: request.model.clone(),
                story_direction: request.story_direction.clone(),
            };

            execute_continue_outline_request(
                db,
                channel,
                user_id,
                &project_model,
                &existing_outlines,
                &continue_request,
            )
            .await;
        }
    }
}

fn trimmed_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn build_outline_continue_system_prompt(
    project_model: &project::Model,
    chapter_count: usize,
) -> String {
    build_outline_runtime_system_prompt(
        project_model,
        chapter_count,
        OutlineRuntimeStage::Continuation,
    )
}

fn outline_model_to_result(outline_model: &outline::Model) -> Value {
    json!({
        "id": outline_model.id,
        "project_id": outline_model.project_id,
        "title": outline_model.title,
        "content": outline_model.content,
        "order_index": outline_model.order_index,
        "structure": outline_model.structure,
        "created_at": outline_model.created_at.and_utc().to_rfc3339(),
        "updated_at": outline_model.updated_at.map(|value| value.and_utc().to_rfc3339()),
    })
}

fn chapter_model_to_result(chapter_model: &chapter::Model) -> Value {
    json!({
        "id": chapter_model.id,
        "project_id": chapter_model.project_id,
        "title": chapter_model.title,
        "chapter_number": chapter_model.chapter_number,
        "summary": chapter_model.summary,
        "status": chapter_model.status,
        "outline_id": chapter_model.outline_id,
        "sub_index": chapter_model.sub_index,
    })
}

async fn execute_continue_outline_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    project_model: &project::Model,
    existing_outlines: &[outline::Model],
    request: &ContinueOutlineExecutionRequest,
) {
    if request.chapter_count == 0 {
        channel.error("chapter_count 必须大于 0", 400).await;
        return;
    }

    let ai_config = match SettingsService::build_ai_config(
        db,
        user_id,
        request.provider.as_deref(),
        request.model.as_deref(),
        None,
    )
    .await
    {
        Ok(config) => config,
        Err(error) => {
            channel.error(&format!("AI配置失败: {}", error), 500).await;
            return;
        }
    };
    let ai_service = AIService::new(ai_config);

    channel
        .progress("准备续写大纲提示词...", 5, "processing")
        .await;

    let template = match PromptTemplateService::system_template_info("OUTLINE_CONTINUE") {
        Some(template) => template,
        None => {
            channel.error("加载续写大纲模板失败", 500).await;
            return;
        }
    };

    let guidance_limit = request.chapter_count.max(OUTLINE_CONTINUE_RECENT_LIMIT);
    let quality_guidance_bundle = match build_outline_quality_guidance_bundle(
        db,
        &request.project_id,
        guidance_limit,
    )
    .await
    {
        Ok(bundle) => bundle,
        Err(error) => {
            warn!("Build outline-continue quality guidance failed: {}", error);
            Default::default()
        }
    };

    let last_chapter_number = existing_outlines
        .last()
        .and_then(|item| item.order_index)
        .unwrap_or(existing_outlines.len() as i32);
    let start_chapter = last_chapter_number + 1;
    let end_chapter = start_chapter + request.chapter_count as i32 - 1;
    let effective_plot_stage = trimmed_non_empty(request.plot_stage.as_deref())
        .or(trimmed_non_empty(
            project_model.default_plot_stage.as_deref(),
        ))
        .unwrap_or("development");
    let stage_instruction = outline_continue_stage_instruction(effective_plot_stage);
    let narrative_perspective = trimmed_non_empty(request.narrative_perspective.as_deref())
        .or(trimmed_non_empty(
            project_model.narrative_perspective.as_deref(),
        ))
        .unwrap_or("第三人称");
    let story_direction =
        trimmed_non_empty(request.story_direction.as_deref()).unwrap_or("自然延续");
    let prompt_context = match build_outline_continue_prompt_context(
        db,
        &request.project_id,
        existing_outlines,
        start_chapter,
        Some(story_direction),
        request.requirements.as_deref(),
    )
    .await
    {
        Ok(context) => context,
        Err(error) => {
            channel.error(&error, 500).await;
            return;
        }
    };

    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("title".into(), project_model.title.clone());
    params.insert(
        "theme".into(),
        project_model
            .theme
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "genre".into(),
        project_model.genre.clone().unwrap_or_else(|| "通用".into()),
    );
    params.insert(
        "narrative_perspective".into(),
        narrative_perspective.to_string(),
    );
    params.insert(
        "time_period".into(),
        project_model
            .world_time_period
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "location".into(),
        project_model
            .world_location
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "atmosphere".into(),
        project_model
            .world_atmosphere
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "rules".into(),
        project_model
            .world_rules
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert("recent_outlines".into(), prompt_context.recent_outlines);
    params.insert("characters_info".into(), prompt_context.characters_info);
    params.insert("chapter_count".into(), request.chapter_count.to_string());
    params.insert("start_chapter".into(), start_chapter.to_string());
    params.insert("end_chapter".into(), end_chapter.to_string());
    params.insert(
        "current_chapter_count".into(),
        existing_outlines.len().to_string(),
    );
    params.insert(
        "plot_stage_instruction".into(),
        stage_instruction.to_string(),
    );
    params.insert("story_direction".into(), story_direction.to_string());
    let project_long_term_goal = build_project_long_term_goal(
        project_model.theme.as_deref(),
        project_model.description.as_deref(),
        request
            .story_creation_brief
            .as_deref()
            .or(project_model.default_story_creation_brief.as_deref()),
        project_model
            .chapter_count
            .and_then(|value| usize::try_from(value).ok()),
        project_model
            .target_words
            .try_into()
            .ok()
            .filter(|value: &usize| *value > 0),
    );
    params.insert(
        "requirements".into(),
        build_continue_outline_requirements(
            request.requirements.as_deref(),
            request.chapter_count,
            request
                .creative_mode
                .as_deref()
                .or(project_model.default_creative_mode.as_deref()),
            request
                .story_focus
                .as_deref()
                .or(project_model.default_story_focus.as_deref()),
            Some(effective_plot_stage),
            request
                .story_creation_brief
                .as_deref()
                .or(project_model.default_story_creation_brief.as_deref()),
            request
                .quality_preset
                .as_deref()
                .or(project_model.default_quality_preset.as_deref()),
            request
                .quality_notes
                .as_deref()
                .or(project_model.default_quality_notes.as_deref()),
            project_long_term_goal.as_deref(),
            Some(prompt_context.focus_names.as_slice()),
            Some(prompt_context.foreshadow_payoff_plan.as_slice()),
            Some(prompt_context.foreshadow_state_ledger.as_slice()),
            Some(prompt_context.character_state_ledger.as_slice()),
            Some(prompt_context.relationship_state_ledger.as_slice()),
            Some(prompt_context.organization_state_ledger.as_slice()),
            Some(prompt_context.career_state_ledger.as_slice()),
            Some(prompt_context.memory_guidance.as_str()),
            Some(quality_guidance_bundle.quality_repair_guidance.as_str()),
            Some(quality_guidance_bundle.quality_trend_guidance.as_str()),
            request.compact_mode.unwrap_or(true),
        ),
    );
    params.insert("mcp_references".into(), String::new());

    let prompt = match PromptTemplateService::format_prompt(&template.content, &params) {
        Ok(prompt) => prompt,
        Err(error) => {
            channel
                .error(&format!("提示词格式化失败: {}", error), 500)
                .await;
            return;
        }
    };
    let sys_prompt = build_outline_continue_system_prompt(project_model, request.chapter_count);

    channel
        .progress("AI正在续写大纲...", 10, "processing")
        .await;
    let progress = Mutex::new(10u32);
    let mut accumulated = String::new();
    let mut chunk_count = 0u64;

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
                        channel
                            .progress(
                                &format!("续写大纲中... ({}字符)", accumulated.len()),
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
            Err(error) => {
                channel
                    .progress(
                        &format!("⚠ 续写警告: {}", error),
                        *progress.lock().await,
                        "processing",
                    )
                    .await;
            }
        }
    }

    channel
        .progress("解析续写大纲数据...", 55, "processing")
        .await;
    let cleaned = clean_json_response(&accumulated);
    let outline_data = match serde_json::from_str::<Value>(&cleaned) {
        Ok(data) => normalize_outline_items(&data),
        Err(_error) => {
            channel
                .progress("JSON解析失败，自动重试...", 56, "processing")
                .await;
            let mut retry_acc = String::new();
            let mut retry_rx = ai_service.generate_text_stream(prompt, Some(sys_prompt), None);
            while let Some(chunk_result) = retry_rx.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(text) = chunk.content {
                            retry_acc.push_str(&text);
                            channel.chunk(&text).await;
                        }
                        if chunk.done {
                            break;
                        }
                    }
                    Err(_) => {}
                }
            }
            let retry_cleaned = clean_json_response(&retry_acc);
            match serde_json::from_str::<Value>(&retry_cleaned) {
                Ok(data) => {
                    channel
                        .progress("已自动修复返回格式，继续保存...", 58, "processing")
                        .await;
                    normalize_outline_items(&data)
                }
                Err(error) => {
                    channel
                        .error(&format!("续写大纲JSON解析失败（已重试）: {}", error), 500)
                        .await;
                    return;
                }
            }
        }
    };

    if outline_data.is_empty() {
        channel.error("续写大纲生成失败，AI返回为空", 500).await;
        return;
    }

    channel
        .progress("保存续写大纲到数据库...", 60, "processing")
        .await;

    let mut created_outlines = Vec::new();
    let mut created_chapters = Vec::new();
    for (index, item) in outline_data
        .iter()
        .take(request.chapter_count.min(outline_data.len()))
        .enumerate()
    {
        let fallback_number = start_chapter + index as i32;
        let chapter_number = item
            .get("chapter_number")
            .and_then(Value::as_i64)
            .map(|value| value as i32)
            .unwrap_or(fallback_number);
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("第{}章", chapter_number));
        let content = build_outline_content(item);
        let structure = serde_json::to_string(item).unwrap_or_default();

        let created_outline = match OutlineService::create(
            db,
            &request.project_id,
            user_id,
            &title,
            Some(&content),
            Some(chapter_number),
            Some(&structure),
        )
        .await
        {
            Ok(Some(model)) => model,
            Ok(None) => {
                channel.error("无权创建续写大纲", 403).await;
                return;
            }
            Err(error) => {
                channel
                    .error(&format!("保存续写大纲失败: {}", error), 500)
                    .await;
                return;
            }
        };

        if project_model.outline_mode == "one-to-one" {
            match ChapterService::create(
                db,
                &request.project_id,
                user_id,
                &title,
                chapter_number,
                None,
                Some(&content),
                Some("pending"),
                None,
                Some(1),
                None,
            )
            .await
            {
                Ok(Some(chapter_model)) => created_chapters.push(chapter_model),
                Ok(None) => {
                    channel.error("无权创建续写章节", 403).await;
                    return;
                }
                Err(error) => {
                    channel
                        .progress(&format!("⚠ 创建章节失败: {}", error), 80, "processing")
                        .await;
                }
            }
        }

        created_outlines.push(created_outline);
    }

    channel
        .progress(
            &format!("已续写{}个大纲节点", created_outlines.len()),
            78,
            "processing",
        )
        .await;

    if project_model.outline_mode == "one-to-one" {
        channel
            .progress(
                &format!("已自动创建{}个续写章节", created_chapters.len()),
                85,
                "processing",
            )
            .await;
    }

    let all_outlines = match OutlineService::list(db, &request.project_id, user_id).await {
        Ok(Some(items)) => items,
        Ok(None) => {
            channel.error("项目不存在或无权限", 404).await;
            return;
        }
        Err(error) => {
            channel
                .error(&format!("加载续写结果失败: {}", error), 500)
                .await;
            return;
        }
    };

    channel.progress("续写完成", 100, "success").await;
    channel
        .result(&json!({
            "message": format!(
                "续写完成！新增{}章，总计{}章",
                created_outlines.len(),
                all_outlines.len()
            ),
            "new_chapters": created_outlines.len(),
            "total_chapters": all_outlines.len(),
            "outline_count": all_outlines.len(),
            "chapter_count": created_chapters.len(),
            "outlines": all_outlines.iter().map(outline_model_to_result).collect::<Vec<_>>(),
            "chapters": created_chapters.iter().map(chapter_model_to_result).collect::<Vec<_>>(),
        }))
        .await;
    channel.done().await;
}

#[cfg(test)]
mod tests {
    use super::{
        build_outline_continue_system_prompt, resolve_outline_generate_mode, OutlineGenerateMode,
        OutlineGenerateRouteRequest,
    };
    use crate::models::{outline, project};
    use crate::services::outline_continue_context_service::build_recent_outlines_context;
    use chrono::NaiveDateTime;
    use serde_json::json;

    fn outline_model(
        id: &str,
        order_index: i32,
        title: &str,
        structure: Option<&str>,
    ) -> outline::Model {
        outline::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            title: title.to_string(),
            content: Some("章节内容".to_string()),
            structure: structure.map(str::to_string),
            order_index: Some(order_index),
            created_at: NaiveDateTime::parse_from_str("1970-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            updated_at: None,
        }
    }

    fn project_model() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "测试小说".to_string(),
            description: None,
            theme: Some("成长".to_string()),
            genre: Some("玄幻".to_string()),
            target_words: 100000,
            current_words: 0,
            status: "active".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 4,
            outline_mode: "one-to-many".to_string(),
            world_time_period: Some("乱世末年".to_string()),
            world_location: Some("北境雪原".to_string()),
            world_atmosphere: Some("压抑肃杀".to_string()),
            world_rules: Some("灵力暴走会反噬经脉".to_string()),
            chapter_count: Some(100),
            narrative_perspective: Some("第三人称".to_string()),
            character_count: 4,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: NaiveDateTime::parse_from_str("1970-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            updated_at: None,
        }
    }

    #[test]
    fn outline_generate_mode_auto_follows_existing_outlines() {
        assert_eq!(
            resolve_outline_generate_mode(Some("auto"), false).unwrap(),
            OutlineGenerateMode::New
        );
        assert_eq!(
            resolve_outline_generate_mode(Some("auto"), true).unwrap(),
            OutlineGenerateMode::Continue
        );
    }

    #[test]
    fn outline_generate_mode_rejects_continue_without_existing_outlines() {
        let error = resolve_outline_generate_mode(Some("continue"), false).unwrap_err();
        assert!(error.contains("没有可用的现有大纲"));
    }

    #[test]
    fn recent_outlines_context_prefers_structure_summary_fields() {
        let outlines = vec![outline_model(
            "outline-1",
            3,
            "第三章",
            Some(
                r#"{
                    "summary":"主角在雨夜截住押送车队，逼出城门背后的内应名单。",
                    "key_points":["押送车队现身","截杀失败后反追踪"],
                    "characters":[{"name":"沈夜"},{"name":"顾寒舟"}],
                    "emotion":"紧绷",
                    "goal":"拿到名单"
                }"#,
            ),
        )];

        let context = build_recent_outlines_context(&outlines);
        assert!(context.contains("第3章《第三章》"));
        assert!(context.contains("概要：主角在雨夜截住押送车队"));
        assert!(context.contains("关键事件：押送车队现身"));
        assert!(context.contains("重点角色：沈夜、顾寒舟"));
        assert!(context.contains("叙事目标：拿到名单"));
    }

    #[test]
    fn continue_system_prompt_uses_shared_runtime_constraints() {
        let prompt = build_outline_continue_system_prompt(&project_model(), 4);

        assert!(prompt.contains("当前阶段：续写阶段"));
        assert!(prompt.contains("本轮目标章节数：4"));
        assert!(prompt.contains("世界规则：灵力暴走会反噬经脉"));
        assert!(prompt.contains("每章至少给一个可直接写成对白场景的冲突对话钩子"));
    }

    #[test]
    fn outline_generate_route_request_accepts_compact_mode_flag() {
        let request: OutlineGenerateRouteRequest = serde_json::from_value(json!({
            "project_id": "project-1",
            "chapter_count": 3,
            "target_words": 120000,
            "compact_mode": false
        }))
        .expect("deserialize route request");

        assert_eq!(request.project_id, "project-1");
        assert_eq!(request.chapter_count, 3);
        assert_eq!(request.compact_mode, Some(false));
    }
}
