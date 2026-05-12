use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive},
        Json, Sse,
    },
    routing::{get, post},
    Router,
};
use chrono::{Duration, NaiveDateTime, Utc};
use std::cmp::{max, min};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::Duration as TokioDuration;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::models::{
    analysis_task, chapter, chapter_draft_attempt, character, foreshadow, generation_history,
    plot_analysis, project, regeneration_task, story_memory,
};
use crate::ai::service::AIService;
use crate::services::auth::Claims;
use crate::services::chapter_service::ChapterService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
use crate::services::wizard_service::clean_json_response;
use crate::services::writing_style_service::WritingStyleService;

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    title: String,
    chapter_number: i32,
    content: Option<String>,
    summary: Option<String>,
    outline_id: Option<String>,
    sub_index: Option<i32>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    title: Option<String>,
    content: Option<String>,
    summary: Option<String>,
    status: Option<String>,
    chapter_number: Option<i32>,
    expansion_plan: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

#[derive(Deserialize)]
struct ExpansionPlanRequest {
    plan: String,
}

#[derive(Deserialize)]
struct BatchAnalysisStatusRequest {
    chapter_ids: Vec<String>,
}

#[derive(Deserialize)]
struct ApplyPartialRegenerateRequest {
    new_text: Option<String>,
    start_position: Option<usize>,
    end_position: Option<usize>,
}

#[derive(Deserialize)]
struct PartialRegenerateRequest {
    selected_text: String,
    start_position: usize,
    end_position: usize,
    user_instructions: String,
    context_chars: Option<usize>,
    style_id: Option<i32>,
    length_mode: Option<String>,
    target_word_count: Option<usize>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
}

#[derive(Deserialize)]
struct RegenerationTasksQuery {
    limit: Option<u64>,
}

fn datetime_to_string(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn classify_analysis_error_code(error_message: Option<&str>) -> Option<&'static str> {
    let message = error_message?;
    if message.contains("正在重试(") {
        Some("retrying")
    } else if message.contains("JSON解析失败") || message.contains("AI返回格式异常") {
        Some("json_parse_failed")
    } else if message.contains("AI响应为空或过短") {
        Some("ai_empty")
    } else if message.contains("流式响应中断") || message.contains("流式生成出错") {
        Some("stream_interrupted")
    } else if message.contains("任务超时") || message.contains("启动超时") {
        Some("timeout")
    } else if message.contains("章节不存在或内容为空") {
        Some("chapter_empty")
    } else if message.contains("项目不存在") {
        Some("project_missing")
    } else {
        Some("unknown")
    }
}

fn compatible_chapter_payload(chapter: chapter::Model) -> Value {
    let chapter_value = serde_json::to_value(&chapter).unwrap_or_else(|_| json!({}));
    match chapter_value {
        Value::Object(mut map) => {
            map.insert("success".to_string(), json!(true));
            map.insert("data".to_string(), json!(chapter));
            Value::Object(map)
        }
        _ => json!({
            "success": true,
            "data": chapter
        }),
    }
}

fn parse_reviser_result_from_history(generated_content: Option<&str>) -> Option<Value> {
    let generated_content = generated_content?;
    let payload: Value = serde_json::from_str(generated_content).ok()?;
    if payload.get("log_type").and_then(Value::as_str) != Some("chapter_text_reviser_v1") {
        return None;
    }
    let reviser_result = payload.get("reviser_result")?;
    reviser_result.is_object().then(|| reviser_result.clone())
}

fn is_draft_stale(
    chapter_updated_at: Option<NaiveDateTime>,
    draft_created_at: Option<NaiveDateTime>,
) -> bool {
    matches!(
        (chapter_updated_at, draft_created_at),
        (Some(chapter_updated_at), Some(draft_created_at)) if chapter_updated_at > draft_created_at
    )
}

fn normalize_candidate_items(value: Option<&Value>, limit: usize) -> Vec<String> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let push_item = |items: &mut Vec<String>,
                     seen: &mut std::collections::HashSet<String>,
                     raw: &str,
                     limit: usize| {
        let trimmed = raw.trim();
        if trimmed.is_empty() || seen.contains(trimmed) || items.len() >= limit {
            return;
        }
        seen.insert(trimmed.to_string());
        items.push(trimmed.to_string());
    };

    match value {
        Some(Value::String(text)) => {
            push_item(&mut items, &mut seen, text, limit);
        }
        Some(Value::Array(values)) => {
            for item in values {
                match item {
                    Value::String(text) => push_item(&mut items, &mut seen, text, limit),
                    Value::Object(map) => {
                        if let Some(text) = map
                            .get("label")
                            .and_then(Value::as_str)
                            .or_else(|| map.get("name").and_then(Value::as_str))
                            .or_else(|| map.get("value").and_then(Value::as_str))
                            .or_else(|| map.get("item").and_then(Value::as_str))
                        {
                            push_item(&mut items, &mut seen, text, limit);
                        }
                    }
                    _ => {}
                }
                if items.len() >= limit {
                    break;
                }
            }
        }
        Some(Value::Object(map)) => {
            for key in ["label", "name", "value", "item", "summary"] {
                if let Some(text) = map.get(key).and_then(Value::as_str) {
                    push_item(&mut items, &mut seen, text, limit);
                    break;
                }
            }
        }
        _ => {}
    }

    items
}

fn extract_candidate_draft_full_content(
    draft_attempt: &chapter_draft_attempt::Model,
) -> (String, bool) {
    let repair_payload = draft_attempt
        .repair_payload
        .as_ref()
        .and_then(Value::as_object);
    if let Some(full_content) = repair_payload
        .and_then(|payload| payload.get("candidate_full_content"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return (full_content.to_string(), true);
    }

    let preview_content = draft_attempt
        .content_preview
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if preview_content.is_empty() {
        return (String::new(), false);
    }

    if repair_payload
        .and_then(|payload| payload.get("content_complete"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return (preview_content, true);
    }

    let word_count = draft_attempt.word_count.max(0) as usize;
    if word_count > 0 && preview_content.chars().count() == word_count {
        return (preview_content, true);
    }

    (String::new(), false)
}

fn build_auto_revision_draft_payload(
    reviser_result: &Value,
    history_id: Option<&str>,
    created_at: Option<NaiveDateTime>,
    chapter_updated_at: Option<NaiveDateTime>,
    include_full_text: bool,
) -> Value {
    let revised_text = reviser_result
        .get("revised_text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut revised_text_preview = reviser_result
        .get("revised_text_preview")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if revised_text_preview.is_empty() && !revised_text.is_empty() {
        revised_text_preview = revised_text.chars().take(500).collect();
    }

    let critical_count = reviser_result
        .get("critical_count")
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;
    let major_count = reviser_result
        .get("major_count")
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;
    let priority_issue_count = reviser_result
        .get("priority_issue_count")
        .and_then(Value::as_i64)
        .unwrap_or((critical_count + major_count) as i64) as i32;
    let applied_issue_count = reviser_result
        .get("applied_issue_count")
        .and_then(Value::as_i64)
        .or_else(|| {
            reviser_result
                .get("applied_critical_count")
                .and_then(Value::as_i64)
        })
        .unwrap_or(0) as i32;
    let applied_critical_count = reviser_result
        .get("applied_critical_count")
        .and_then(Value::as_i64)
        .unwrap_or(applied_issue_count as i64) as i32;
    let revised_word_count = reviser_result
        .get("revised_word_count")
        .and_then(Value::as_i64)
        .unwrap_or(revised_text.chars().count() as i64) as i32;

    let mut payload = serde_json::Map::new();
    payload.insert("history_id".to_string(), json!(history_id));
    payload.insert("critical_count".to_string(), json!(critical_count));
    payload.insert("major_count".to_string(), json!(major_count));
    payload.insert("priority_issue_count".to_string(), json!(priority_issue_count));
    payload.insert(
        "applied_critical_count".to_string(),
        json!(applied_critical_count),
    );
    payload.insert("applied_issue_count".to_string(), json!(applied_issue_count));
    payload.insert(
        "change_summary".to_string(),
        reviser_result
            .get("change_summary")
            .cloned()
            .unwrap_or(Value::Null),
    );
    payload.insert("revised_word_count".to_string(), json!(revised_word_count));
    payload.insert(
        "unresolved_issues".to_string(),
        reviser_result
            .get("unresolved_issues")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    payload.insert(
        "revised_text_preview".to_string(),
        json!(revised_text_preview),
    );
    payload.insert("has_full_text".to_string(), json!(!revised_text.is_empty()));
    payload.insert(
        "is_stale".to_string(),
        json!(is_draft_stale(chapter_updated_at, created_at)),
    );
    payload.insert("created_at".to_string(), json!(datetime_to_string(created_at)));
    if include_full_text {
        payload.insert("revised_text".to_string(), json!(revised_text));
    }
    Value::Object(payload)
}

fn build_candidate_draft_payload(
    draft_attempt: &chapter_draft_attempt::Model,
    chapter_updated_at: Option<NaiveDateTime>,
    include_full_text: bool,
) -> Value {
    let quality_metrics = draft_attempt
        .quality_metrics
        .clone()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let repair_payload = draft_attempt
        .repair_payload
        .clone()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let quality_gate = quality_metrics
        .get("quality_gate")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let repair_guidance = quality_metrics
        .get("repair_guidance")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let candidate_selection = quality_metrics
        .get("candidate_selection")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or(Value::Null);
    let quality_highlights = quality_metrics
        .get("quality_highlights")
        .cloned()
        .or_else(|| repair_payload.get("quality_highlights").cloned())
        .filter(Value::is_object)
        .unwrap_or(Value::Null);
    let apply_risk = quality_metrics
        .get("apply_risk")
        .cloned()
        .or_else(|| repair_payload.get("apply_risk").cloned())
        .filter(Value::is_object)
        .unwrap_or(Value::Null);

    let (full_content, has_full_content) = extract_candidate_draft_full_content(draft_attempt);
    let mut preview_text = draft_attempt
        .content_preview
        .clone()
        .or_else(|| draft_attempt.summary_preview.clone())
        .unwrap_or_default()
        .trim()
        .to_string();
    if preview_text.is_empty() && !full_content.is_empty() {
        preview_text = full_content.chars().take(500).collect();
    }

    let failed_metrics = quality_gate
        .get("failed_metrics")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter_map(|item| {
                    let object = item.as_object()?;
                    Some(json!({
                        "key": object.get("key").and_then(Value::as_str).unwrap_or_default(),
                        "label": object.get("label").and_then(Value::as_str).or_else(|| object.get("key").and_then(Value::as_str)).unwrap_or_default(),
                        "value": object.get("value").and_then(Value::as_f64),
                        "threshold": object.get("threshold").and_then(Value::as_f64),
                        "gap": object.get("gap").and_then(Value::as_f64),
                        "focus_area": object.get("focus_area").and_then(Value::as_str),
                        "repair_target": object.get("repair_target").and_then(Value::as_str),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let repair_targets = {
        let values = repair_payload
            .get("repair_targets")
            .or_else(|| repair_guidance.get("repair_targets"));
        normalize_candidate_items(values, 4)
    };
    let preserve_strengths = {
        let values = repair_payload
            .get("preserve_strengths")
            .or_else(|| repair_guidance.get("preserve_strengths"));
        normalize_candidate_items(values, 4)
    };
    let focus_areas = {
        let values = repair_guidance
            .get("focus_areas")
            .or_else(|| quality_gate.get("focus_areas"));
        normalize_candidate_items(values, 4)
    };

    let mut payload = serde_json::Map::new();
    payload.insert("attempt_id".to_string(), json!(draft_attempt.id));
    payload.insert("source".to_string(), json!(draft_attempt.source));
    payload.insert("attempt_state".to_string(), json!(draft_attempt.attempt_state));
    payload.insert(
        "quality_gate_action".to_string(),
        json!(draft_attempt.quality_gate_action),
    );
    payload.insert(
        "quality_gate_decision".to_string(),
        json!(draft_attempt.quality_gate_decision),
    );
    payload.insert("word_count".to_string(), json!(draft_attempt.word_count));
    payload.insert(
        "summary_preview".to_string(),
        json!(draft_attempt.summary_preview.clone().unwrap_or_default()),
    );
    payload.insert("content_preview".to_string(), json!(preview_text));
    payload.insert("has_full_content".to_string(), json!(has_full_content));
    payload.insert("content_complete".to_string(), json!(has_full_content));
    payload.insert("can_apply".to_string(), json!(has_full_content));
    payload.insert(
        "is_stale".to_string(),
        json!(is_draft_stale(chapter_updated_at, draft_attempt.created_at)),
    );
    payload.insert(
        "created_at".to_string(),
        json!(datetime_to_string(draft_attempt.created_at)),
    );
    payload.insert(
        "repair_summary".to_string(),
        json!(
            repair_payload
                .get("summary")
                .and_then(Value::as_str)
                .or_else(|| repair_guidance.get("summary").and_then(Value::as_str))
        ),
    );
    payload.insert("repair_targets".to_string(), json!(repair_targets));
    payload.insert("preserve_strengths".to_string(), json!(preserve_strengths));
    payload.insert("focus_areas".to_string(), json!(focus_areas));
    payload.insert("failed_metrics".to_string(), json!(failed_metrics));
    payload.insert("candidate_selection".to_string(), candidate_selection);
    payload.insert("quality_highlights".to_string(), quality_highlights);
    payload.insert("apply_risk".to_string(), apply_risk);
    if include_full_text && has_full_content {
        payload.insert("content".to_string(), json!(full_content));
    }
    Value::Object(payload)
}

async fn load_latest_reviser_history(
    db: &DatabaseConnection,
    chapter_id: &str,
    history_id: Option<&str>,
) -> Result<Option<(generation_history::Model, Value)>, sea_orm::DbErr> {
    if let Some(history_id) = history_id.filter(|value| !value.trim().is_empty()) {
        let history = generation_history::Entity::find_by_id(history_id)
            .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.to_string())))
            .one(db)
            .await?;
        return Ok(history.and_then(|model| {
            parse_reviser_result_from_history(model.generated_content.as_deref())
                .map(|reviser_result| (model, reviser_result))
        }));
    }

    let histories = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.to_string())))
        .order_by_desc(generation_history::Column::CreatedAt)
        .limit(60)
        .all(db)
        .await?;

    Ok(histories.into_iter().find_map(|history| {
        parse_reviser_result_from_history(history.generated_content.as_deref())
            .map(|reviser_result| (history, reviser_result))
    }))
}

async fn load_candidate_draft_attempt(
    db: &DatabaseConnection,
    chapter_id: &str,
    attempt_id: Option<&str>,
) -> Result<Option<chapter_draft_attempt::Model>, sea_orm::DbErr> {
    let mut query =
        chapter_draft_attempt::Entity::find().filter(chapter_draft_attempt::Column::ChapterId.eq(
            Some(chapter_id.to_string()),
        ));

    if let Some(attempt_id) = attempt_id.filter(|value| !value.trim().is_empty()) {
        query = query.filter(chapter_draft_attempt::Column::Id.eq(attempt_id.to_string()));
    } else {
        query = query
            .order_by_desc(chapter_draft_attempt::Column::CreatedAt)
            .limit(1);
    }

    query.one(db).await
}

async fn apply_chapter_draft_content_with_history(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    content: &str,
    prompt: String,
    generated_content: String,
    model_name: String,
) -> Result<(chapter::Model, i32, i32), String> {
    let now = Utc::now().naive_utc();
    let old_word_count = chapter_model.word_count.max(0);
    let new_word_count = content.chars().count() as i32;
    let txn = db.begin().await.map_err(|error| error.to_string())?;

    let mut chapter_active: chapter::ActiveModel = chapter_model.clone().into();
    chapter_active.content = Set(Some(content.to_string()));
    chapter_active.word_count = Set(new_word_count);
    chapter_active.updated_at = Set(Some(now));
    let updated_chapter = chapter_active
        .update(&txn)
        .await
        .map_err(|error| error.to_string())?;

    let history = generation_history::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(chapter_model.project_id.clone()),
        chapter_id: Set(Some(chapter_model.id.clone())),
        prompt: Set(Some(prompt)),
        generated_content: Set(Some(generated_content)),
        model: Set(Some(model_name)),
        tokens_used: Set(None),
        generation_time: Set(None),
        created_at: Set(Some(now)),
    };
    history
        .insert(&txn)
        .await
        .map_err(|error| error.to_string())?;

    txn.commit().await.map_err(|error| error.to_string())?;
    Ok((updated_chapter, old_word_count, new_word_count))
}

async fn latest_analysis_task(
    db: &DatabaseConnection,
    chapter_id: &str,
) -> Result<Option<analysis_task::Model>, sea_orm::DbErr> {
    analysis_task::Entity::find()
        .filter(analysis_task::Column::ChapterId.eq(chapter_id))
        .order_by_desc(analysis_task::Column::CreatedAt)
        .limit(1)
        .one(db)
        .await
}

async fn recover_analysis_task_if_needed(
    db: &DatabaseConnection,
    task: &analysis_task::Model,
) -> Result<(analysis_task::Model, bool), sea_orm::DbErr> {
    let now = Utc::now().naive_utc();
    let mut recovered = false;
    let mut error_message = task.error_message.clone();
    let mut completed_at = task.completed_at;
    let mut progress = task.progress;
    let timeout_minutes = if task
        .error_message
        .as_deref()
        .map(|message| message.contains("重试"))
        .unwrap_or(false)
    {
        15
    } else {
        10
    };

    if task.status == "running" {
        if let Some(started_at) = task.started_at {
            if now - started_at > Duration::minutes(timeout_minutes) {
                error_message = Some(format!(
                    "任务超时（超过{}分钟未完成，已自动恢复）",
                    timeout_minutes
                ));
                completed_at = Some(now);
                progress = 0;
                recovered = true;
            }
        }
    } else if task.status == "pending" {
        if let Some(created_at) = task.created_at {
            if now - created_at > Duration::minutes(3) {
                error_message = Some("任务启动超时（超过3分钟未启动，已自动恢复）".to_string());
                completed_at = Some(now);
                progress = 0;
                recovered = true;
            }
        }
    }

    if !recovered {
        return Ok((task.clone(), false));
    }

    let mut active: analysis_task::ActiveModel = task.clone().into();
    active.status = Set("failed".to_string());
    active.error_message = Set(error_message);
    active.completed_at = Set(completed_at);
    active.progress = Set(progress);
    active.update(db).await.map(|updated| (updated, true))
}

async fn analysis_task_status_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    task: Option<analysis_task::Model>,
) -> Result<Value, sea_orm::DbErr> {
    let Some(task) = task else {
        return Ok(json!({
            "has_task": false,
            "chapter_id": chapter_id,
            "status": "none",
            "progress": 0,
            "error_message": null,
            "auto_recovered": false,
            "task_id": null,
            "created_at": null,
            "started_at": null,
            "completed_at": null,
        }));
    };

    let (task, auto_recovered) = recover_analysis_task_if_needed(db, &task).await?;
    Ok(json!({
        "has_task": true,
        "task_id": task.id,
        "chapter_id": task.chapter_id,
        "status": task.status,
        "progress": task.progress,
        "error_message": task.error_message,
        "error_code": classify_analysis_error_code(task.error_message.as_deref()),
        "auto_recovered": auto_recovered,
        "created_at": datetime_to_string(task.created_at),
        "started_at": datetime_to_string(task.started_at),
        "completed_at": datetime_to_string(task.completed_at),
    }))
}

fn value_or_null(value: Option<serde_json::Value>) -> Value {
    value.unwrap_or(Value::Null)
}

fn json_i32(value: Option<i64>) -> i32 {
    value.unwrap_or_default().clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn json_f64(value: Option<f64>) -> Option<f64> {
    value.filter(|number| number.is_finite())
}

fn normalize_analysis_status(status: &str) -> String {
    match status {
        "pending" | "running" | "completed" | "failed" => status.to_string(),
        _ => "failed".to_string(),
    }
}

fn build_chapter_analysis_report(payload: &Value) -> Option<String> {
    let mut sections = Vec::new();

    if let Some(plot_stage) = payload.get("plot_stage").and_then(Value::as_str) {
        if !plot_stage.trim().is_empty() {
            sections.push(format!("剧情阶段：{}", plot_stage.trim()));
        }
    }

    if let Some(conflict) = payload.get("conflict") {
        let description = conflict
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !description.is_empty() {
            sections.push(format!("冲突分析：{}", description));
        }
    }

    if let Some(scores) = payload.get("scores") {
        let justification = scores
            .get("score_justification")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !justification.is_empty() {
            sections.push(format!("评分说明：{}", justification));
        }
    }

    if let Some(suggestions) = payload.get("suggestions").and_then(Value::as_array) {
        let joined = suggestions
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("；");
        if !joined.is_empty() {
            sections.push(format!("改进建议：{}", joined));
        }
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n"))
    }
}

async fn build_chapter_analysis_prompt(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    project_model: &project::Model,
) -> Result<String, String> {
    let template = PromptTemplateService::system_template_info("PLOT_ANALYSIS")
        .ok_or_else(|| "找不到章节分析模板 PLOT_ANALYSIS".to_string())?;

    let unresolved_foreshadows = foreshadow::Entity::find()
        .filter(foreshadow::Column::ProjectId.eq(&project_model.id))
        .filter(foreshadow::Column::Status.ne("resolved"))
        .order_by_desc(foreshadow::Column::CreatedAt)
        .limit(50)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let existing_foreshadows = if unresolved_foreshadows.is_empty() {
        "[]".to_string()
    } else {
        unresolved_foreshadows
            .iter()
            .map(|item| {
                format!(
                    "- [ID: {}] 标题：{}；埋入章节：{}；内容：{}",
                    item.id,
                    item.title,
                    item.plant_chapter_number
                        .map(|number| number.to_string())
                        .unwrap_or_else(|| "未知".to_string()),
                    item.content.replace('\n', " ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&project_model.id))
        .order_by_asc(character::Column::Name)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let characters_info = if characters.is_empty() {
        "[]".to_string()
    } else {
        characters
            .iter()
            .map(|item| {
                format!(
                    "- {}（身份：{}；状态：{}）",
                    item.name,
                    item.role_type.clone().unwrap_or_else(|| "未设定".to_string()),
                    item.status
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut params = std::collections::HashMap::new();
    params.insert(
        "chapter_number".to_string(),
        chapter_model.chapter_number.to_string(),
    );
    params.insert("title".to_string(), chapter_model.title.clone());
    params.insert(
        "word_count".to_string(),
        chapter_model.word_count.max(0).to_string(),
    );
    params.insert(
        "content".to_string(),
        chapter_model.content.clone().unwrap_or_default(),
    );
    params.insert("existing_foreshadows".to_string(), existing_foreshadows);
    params.insert("characters_info".to_string(), characters_info);

    PromptTemplateService::format_prompt(&template.content, &params)
}

async fn mark_analysis_task_running(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<(), sea_orm::DbErr> {
    if let Some(existing) = analysis_task::Entity::find_by_id(task_id).one(db).await? {
        let mut active: analysis_task::ActiveModel = existing.into();
        active.status = Set("running".to_string());
        active.progress = Set(10);
        active.started_at = Set(Some(Utc::now().naive_utc()));
        active.error_message = Set(None);
        let _ = active.update(db).await?;
    }
    Ok(())
}

async fn mark_analysis_task_failed(
    db: &DatabaseConnection,
    task_id: &str,
    error_message: String,
) -> Result<(), sea_orm::DbErr> {
    if let Some(existing) = analysis_task::Entity::find_by_id(task_id).one(db).await? {
        let mut active: analysis_task::ActiveModel = existing.into();
        active.status = Set("failed".to_string());
        active.progress = Set(0);
        active.error_message = Set(Some(error_message));
        active.completed_at = Set(Some(Utc::now().naive_utc()));
        let _ = active.update(db).await?;
    }
    Ok(())
}

async fn persist_chapter_analysis_result(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    task_id: &str,
    payload: &Value,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let scores = payload.get("scores").cloned().unwrap_or(Value::Null);
    let conflict = payload.get("conflict").cloned().unwrap_or(Value::Null);
    let emotional_arc = payload.get("emotional_arc").cloned().unwrap_or(Value::Null);

    let analysis = plot_analysis::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(chapter_model.project_id.clone()),
        chapter_id: Set(chapter_model.id.clone()),
        plot_stage: Set(
            payload
                .get("plot_stage")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        conflict_level: Set(Some(json_i32(
            conflict.get("level").and_then(Value::as_i64),
        ))),
        conflict_types: Set(conflict.get("types").cloned()),
        emotional_tone: Set(
            emotional_arc
                .get("primary_emotion")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        emotional_intensity: Set(json_f64(
            emotional_arc.get("intensity").and_then(Value::as_f64),
        )),
        emotional_curve: Set(
            emotional_arc
                .get("curve")
                .cloned()
                .or_else(|| emotional_arc.get("secondary_emotions").cloned()),
        ),
        hooks: Set(payload.get("hooks").cloned()),
        hooks_count: Set(
            payload
                .get("hooks")
                .and_then(Value::as_array)
                .map(|items| items.len() as i32)
                .unwrap_or(0),
        ),
        hooks_avg_strength: Set(payload.get("hooks").and_then(Value::as_array).and_then(
            |items| {
                let strengths = items
                    .iter()
                    .filter_map(|item| item.get("strength").and_then(Value::as_f64))
                    .collect::<Vec<_>>();
                if strengths.is_empty() {
                    None
                } else {
                    Some(strengths.iter().sum::<f64>() / strengths.len() as f64)
                }
            },
        )),
        foreshadows: Set(payload.get("foreshadows").cloned()),
        foreshadows_planted: Set(
            payload
                .get("foreshadows")
                .and_then(Value::as_array)
                .map(|items| {
                    items.iter()
                        .filter(|item| item.get("type").and_then(Value::as_str) == Some("planted"))
                        .count() as i32
                })
                .unwrap_or(0),
        ),
        foreshadows_resolved: Set(
            payload
                .get("foreshadows")
                .and_then(Value::as_array)
                .map(|items| {
                    items.iter()
                        .filter(|item| item.get("type").and_then(Value::as_str) == Some("resolved"))
                        .count() as i32
                })
                .unwrap_or(0),
        ),
        plot_points: Set(payload.get("plot_points").cloned()),
        plot_points_count: Set(
            payload
                .get("plot_points")
                .and_then(Value::as_array)
                .map(|items| items.len() as i32)
                .unwrap_or(0),
        ),
        character_states: Set(payload.get("character_states").cloned()),
        scenes: Set(
            payload
                .get("scenes")
                .cloned()
                .or_else(|| payload.get("serial_rhythm").cloned()),
        ),
        pacing: Set(
            payload
                .get("pacing")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        overall_quality_score: Set(json_f64(
            scores.get("overall").and_then(Value::as_f64),
        )),
        pacing_score: Set(json_f64(
            scores.get("pacing").and_then(Value::as_f64),
        )),
        engagement_score: Set(json_f64(
            scores.get("engagement").and_then(Value::as_f64),
        )),
        coherence_score: Set(json_f64(
            scores.get("coherence").and_then(Value::as_f64),
        )),
        analysis_report: Set(
            build_chapter_analysis_report(payload).or_else(|| Some(payload.to_string())),
        ),
        suggestions: Set(payload.get("suggestions").cloned()),
        word_count: Set(Some(chapter_model.word_count)),
        dialogue_ratio: Set(json_f64(
            payload.get("dialogue_ratio").and_then(Value::as_f64),
        )),
        description_ratio: Set(json_f64(
            payload.get("description_ratio").and_then(Value::as_f64),
        )),
        created_at: Set(Some(now)),
    };

    analysis.insert(db).await.map_err(|error| error.to_string())?;

    if let Some(existing) = analysis_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: analysis_task::ActiveModel = existing.into();
        active.status = Set(normalize_analysis_status("completed"));
        active.progress = Set(100);
        active.completed_at = Set(Some(now));
        active.error_message = Set(None);
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    Ok(())
}

async fn execute_chapter_analysis_background(
    db: DatabaseConnection,
    user_id: String,
    chapter_id: String,
    task_id: String,
) {
    let run = async {
        mark_analysis_task_running(&db, &task_id)
            .await
            .map_err(|error| error.to_string())?;

        let chapter_model = ChapterService::get(&db, &chapter_id, &user_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "章节不存在或内容为空".to_string())?;

        let chapter_content = chapter_model.content.clone().unwrap_or_default();
        if chapter_content.trim().is_empty() {
            return Err("章节不存在或内容为空".to_string());
        }

        let project_model = project::Entity::find_by_id(&chapter_model.project_id)
            .one(&db)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "项目不存在".to_string())?;

        if project_model.user_id != user_id {
            return Err("项目不存在".to_string());
        }

        let prompt = build_chapter_analysis_prompt(&db, &chapter_model, &project_model).await?;
        let ai_config = SettingsService::build_ai_config(&db, &user_id, None, None, None).await?;
        let ai_service = AIService::new(ai_config);
        let response = ai_service
            .generate_text(&prompt, None, None)
            .await
            .map_err(|error| error.to_string())?;

        let cleaned = clean_json_response(&response.content);
        let parsed: Value =
            serde_json::from_str(&cleaned).map_err(|error| format!("JSON解析失败: {}", error))?;

        persist_chapter_analysis_result(&db, &chapter_model, &task_id, &parsed).await
    }
    .await;

    if let Err(error_message) = run {
        let _ = mark_analysis_task_failed(&db, &task_id, error_message).await;
    }
}

fn bool_from_int(value: i32) -> bool {
    value != 0
}

fn is_likely_chapter_meta_line(line: &str) -> bool {
    let stripped = line.trim();
    if stripped.is_empty() {
        return false;
    }
    if stripped.starts_with("```") {
        return true;
    }

    let lowered = stripped.to_lowercase();
    let meta_prefixes = ["以下是章节正文：", "以下是正文：", "章节正文：", "正文："];
    if meta_prefixes.iter().any(|prefix| stripped == *prefix) {
        return true;
    }

    let prefix_checks = ["步骤", "step", "执行"];
    if prefix_checks
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
    {
        return true;
    }

    let contains_checks = [
        "调用 agent",
        "流程说明",
        "步骤说明",
        "流程日志",
        "步骤日志",
        "流程总结",
        "步骤总结",
        "流程复盘",
        "步骤复盘",
        "流程评审",
        "步骤评审",
        "方案对比",
        "方案评审",
        "复盘结论",
        "执行计划",
    ];
    if contains_checks
        .iter()
        .any(|needle| lowered.contains(needle))
    {
        return true;
    }

    (lowered.starts_with("作为ai")
        || lowered.starts_with("作为 ai")
        || lowered.starts_with("身为ai")
        || lowered.starts_with("身为 ai")
        || lowered.starts_with("作为助手")
        || lowered.starts_with("身为助手")
        || lowered.starts_with("作为模型")
        || lowered.starts_with("身为模型"))
        && [':', '：', '?', '？', ',', '，']
            .iter()
            .any(|c| stripped.contains(*c))
}

fn lightly_polish_template_phrases(text: &str) -> String {
    let mut result = String::new();
    let mut next_second_seen = 0;
    let mut that_moment_seen = 0;
    for line in text.lines() {
        let mut current = line.to_string();
        if current.contains("下一秒") {
            next_second_seen += current.matches("下一秒").count();
            if next_second_seen > 1 {
                current = current.replacen("下一秒，", "", 1);
                current = current.replacen("下一秒、", "", 1);
                current = current.replacen("下一秒", "", 1);
            }
        }
        if current.contains("那一瞬") {
            that_moment_seen += current.matches("那一瞬").count();
            if that_moment_seen > 1 {
                current = current.replacen("那一瞬，", "", 1);
                current = current.replacen("那一瞬、", "", 1);
                current = current.replacen("那一瞬", "", 1);
            }
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(current.trim_end());
    }

    result = result.replace("像是有什么", "像有");
    result = result.replace("像有什么", "像有");
    result
}

fn normalize_partial_regeneration_output(text: &str) -> String {
    let mut cleaned = text.replace("\r\n", "\n").trim().to_string();
    let prefixes = [
        "重写后：",
        "重写后:",
        "改写后：",
        "改写后:",
        "以下是重写后的内容：",
        "以下是重写后的内容:",
        "重写内容：",
        "重写内容:",
    ];
    for prefix in prefixes {
        if cleaned.starts_with(prefix) {
            cleaned = cleaned[prefix.len()..].trim().to_string();
            break;
        }
    }

    if (cleaned.starts_with('"') && cleaned.ends_with('"'))
        || (cleaned.starts_with('\'') && cleaned.ends_with('\''))
    {
        let mut chars = cleaned.chars();
        let _ = chars.next();
        let _ = chars.next_back();
        cleaned = chars.collect::<String>().trim().to_string();
    }
    if (cleaned.starts_with('「') && cleaned.ends_with('」'))
        || (cleaned.starts_with('『') && cleaned.ends_with('』'))
    {
        let mut chars = cleaned.chars();
        let _ = chars.next();
        let _ = chars.next_back();
        cleaned = chars.collect::<String>().trim().to_string();
    }

    cleaned.trim().to_string()
}

fn build_partial_length_requirement(
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
            format!("尽量保持与原文接近，原文约 {} 字，目标 {}-{} 字", original_word_count, min_words, max_words)
        }
    }
}

fn calculate_partial_target_words(
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    original_word_count: usize,
) -> usize {
    match length_mode.unwrap_or("similar") {
        "expand" => (original_word_count as f64 * 2.0) as usize,
        "custom" => target_word_count.unwrap_or_else(|| (original_word_count as f64 * 1.5) as usize),
        _ => (original_word_count as f64 * 1.5) as usize,
    }
}

fn build_partial_regeneration_prompt(
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
        if context_before.is_empty() { "（无前文上下文）" } else { context_before },
        if context_after.is_empty() { "（无后文上下文）" } else { context_after },
        if user_instructions.is_empty() { "（无额外要求）" } else { user_instructions },
        length_requirement,
        style_content,
        web_research_note,
    )
}

async fn load_partial_style_content(
    db: &DatabaseConnection,
    claims: &Claims,
    style_id: Option<i32>,
) -> Result<Option<String>, (StatusCode, Json<Value>)> {
    let Some(style_id) = style_id else {
        return Ok(None);
    };

    let value = WritingStyleService::get_style(db, &claims.sub, style_id)
        .await
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    Ok(value
        .get("prompt_content")
        .and_then(Value::as_str)
        .map(str::to_string))
}

fn sanitize_generated_narrative_text(text: &str) -> (String, usize) {
    let original = text.replace("\r\n", "\n").trim().to_string();
    if original.is_empty() {
        return (String::new(), 0);
    }

    let mut removed_line_count = 0usize;
    let mut kept_lines = Vec::new();
    for raw_line in original.lines() {
        let stripped = raw_line.trim();
        if stripped.is_empty() {
            kept_lines.push(String::new());
            continue;
        }
        if is_likely_chapter_meta_line(stripped) {
            removed_line_count += 1;
            continue;
        }
        kept_lines.push(raw_line.to_string());
    }

    let mut cleaned = kept_lines.join("\n");
    while cleaned.contains("\n\n\n") {
        cleaned = cleaned.replace("\n\n\n", "\n\n");
    }
    cleaned = lightly_polish_template_phrases(cleaned.trim());
    while cleaned.contains("\n\n\n") {
        cleaned = cleaned.replace("\n\n\n", "\n\n");
    }
    (cleaned.trim().to_string(), removed_line_count)
}

fn contains_chapter_workflow_meta_text(text: &str) -> bool {
    text.lines().any(is_likely_chapter_meta_line)
}

async fn build_regeneration_ai_service(
    db: &DatabaseConnection,
    user_id: &str,
    max_tokens_override: Option<u32>,
) -> Result<AIService, (StatusCode, Json<Value>)> {
    let mut ai_config = SettingsService::build_ai_config(db, user_id, None, None, None)
        .await
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": error})),
            )
        })?;
    if let Some(max_tokens) = max_tokens_override {
        ai_config.max_tokens = max_tokens;
    }
    Ok(AIService::new(ai_config))
}

fn build_regeneration_prompt(chapter: &chapter::Model, body: &serde_json::Value) -> String {
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

async fn load_accessible_chapter_or_404(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, (StatusCode, Json<Value>)> {
    match ChapterService::get(db, chapter_id, user_id).await {
        Ok(Some(chapter)) => Ok(chapter),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found or access denied"})),
        )),
        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        )),
    }
}

async fn get_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = match ChapterService::get(&db, &chapter_id, &claims.sub).await {
        Ok(Some(chapter)) => chapter,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "Chapter not found or access denied"})),
            ));
        }
        Err(error) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            ));
        }
    };

    let analysis = match plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ChapterId.eq(&chapter_id))
        .order_by_desc(plot_analysis::Column::CreatedAt)
        .one(&db)
        .await
    {
        Ok(Some(model)) => model,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "Chapter analysis not found"})),
            ));
        }
        Err(error) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            ));
        }
    };

    let memories = match story_memory::Entity::find()
        .filter(story_memory::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(story_memory::Column::ImportanceScore)
        .all(&db)
        .await
    {
        Ok(items) => items,
        Err(error) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            ));
        }
    };

    let histories: Vec<generation_history::Model> = match generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(generation_history::Column::CreatedAt)
        .limit(30)
        .all(&db)
        .await
    {
        Ok(items) => items,
        Err(error) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            ));
        }
    };

    let candidate_attempt = match chapter_draft_attempt::Entity::find()
        .filter(chapter_draft_attempt::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(chapter_draft_attempt::Column::CreatedAt)
        .one(&db)
        .await
    {
        Ok(item) => item,
        Err(error) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            ));
        }
    };

    let latest_checker_result = histories.iter().find_map(|history| {
        history.generated_content.as_ref().and_then(|content| {
            serde_json::from_str::<Value>(content)
                .ok()
                .and_then(|payload| {
                    if payload.get("log_type").and_then(Value::as_str)
                        == Some("chapter_text_checker_v1")
                    {
                        payload.get("checker_result").cloned()
                    } else {
                        None
                    }
                })
        })
    });

    let checker_created_at = histories.iter().find_map(|history| {
        history.generated_content.as_ref().and_then(|content| {
            serde_json::from_str::<Value>(content)
                .ok()
                .and_then(|payload| {
                    if payload.get("log_type").and_then(Value::as_str)
                        == Some("chapter_text_checker_v1")
                    {
                        datetime_to_string(history.created_at)
                    } else {
                        None
                    }
                })
        })
    });

    let auto_revision_draft = histories.iter().find_map(|history| {
        let reviser_result =
            parse_reviser_result_from_history(history.generated_content.as_deref())?;
        Some(build_auto_revision_draft_payload(
            &reviser_result,
            Some(&history.id),
            history.created_at,
            chapter.updated_at,
            false,
        ))
    });

    let candidate_draft = candidate_attempt
        .as_ref()
        .map(|attempt| build_candidate_draft_payload(attempt, chapter.updated_at, false));

    let quality_metrics = candidate_attempt
        .as_ref()
        .and_then(|attempt| attempt.quality_metrics.clone())
        .or_else(|| {
            histories.iter().find_map(|history| {
                history.generated_content.as_ref().and_then(|content| {
                    serde_json::from_str::<Value>(content)
                        .ok()
                        .and_then(|payload| payload.get("quality_metrics").cloned())
                })
            })
        });

    let quality_metrics_summary = quality_metrics.as_ref().map(|metrics| {
        json!({
            "repair_guidance": metrics.get("repair_guidance").cloned(),
            "quality_gate": metrics.get("quality_gate").cloned(),
            "raw": metrics,
        })
    });

    Ok(Json(json!({
        "chapter_id": chapter.id,
        "analysis": {
            "id": analysis.id,
            "project_id": analysis.project_id,
            "chapter_id": analysis.chapter_id,
            "plot_stage": analysis.plot_stage,
            "conflict_level": analysis.conflict_level,
            "conflict_types": value_or_null(analysis.conflict_types),
            "emotional_tone": analysis.emotional_tone,
            "emotional_intensity": analysis.emotional_intensity,
            "emotional_curve": value_or_null(analysis.emotional_curve),
            "hooks": value_or_null(analysis.hooks),
            "hooks_count": analysis.hooks_count,
            "hooks_avg_strength": analysis.hooks_avg_strength,
            "foreshadows": value_or_null(analysis.foreshadows),
            "foreshadows_planted": analysis.foreshadows_planted,
            "foreshadows_resolved": analysis.foreshadows_resolved,
            "plot_points": value_or_null(analysis.plot_points),
            "plot_points_count": analysis.plot_points_count,
            "character_states": value_or_null(analysis.character_states),
            "scenes": value_or_null(analysis.scenes),
            "pacing": analysis.pacing,
            "overall_quality_score": analysis.overall_quality_score,
            "pacing_score": analysis.pacing_score,
            "engagement_score": analysis.engagement_score,
            "coherence_score": analysis.coherence_score,
            "analysis_report": analysis.analysis_report,
            "suggestions": value_or_null(analysis.suggestions),
            "word_count": analysis.word_count,
            "dialogue_ratio": analysis.dialogue_ratio,
            "description_ratio": analysis.description_ratio,
            "created_at": datetime_to_string(analysis.created_at),
        },
        "memories": memories.into_iter().map(|memory| json!({
            "id": memory.id,
            "type": memory.memory_type,
            "title": memory.title,
            "content": memory.content,
            "importance": memory.importance_score,
            "tags": value_or_null(memory.tags),
            "is_foreshadow": bool_from_int(memory.is_foreshadow),
            "position": memory.chapter_position,
            "related_characters": value_or_null(memory.related_characters),
        })).collect::<Vec<_>>(),
        "checker_result": latest_checker_result,
        "checker_created_at": checker_created_at,
        "auto_revision_draft": auto_revision_draft,
        "candidate_draft": candidate_draft,
        "quality_metrics": quality_metrics,
        "quality_metrics_summary": quality_metrics_summary,
        "created_at": datetime_to_string(analysis.created_at)
            .or_else(|| chapter.updated_at.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string()))
            .unwrap_or_default(),
    })))
}

async fn get_chapter_quality_metrics(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;

    let candidate_attempt = chapter_draft_attempt::Entity::find()
        .filter(chapter_draft_attempt::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(chapter_draft_attempt::Column::CreatedAt)
        .one(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    let histories: Vec<generation_history::Model> = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(generation_history::Column::CreatedAt)
        .limit(30)
        .all(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    let latest_history_metrics = histories.iter().find_map(|history| {
        history.generated_content.as_ref().and_then(|content| {
            serde_json::from_str::<Value>(content)
                .ok()
                .and_then(|payload| payload.get("quality_metrics").cloned())
                .map(|metrics| (history, metrics))
        })
    });

    let latest_quality_metrics = candidate_attempt
        .as_ref()
        .and_then(|attempt| attempt.quality_metrics.clone())
        .or_else(|| latest_history_metrics.as_ref().map(|(_, metrics)| metrics.clone()));

    let history_id = candidate_attempt
        .as_ref()
        .map(|attempt| attempt.id.clone())
        .or_else(|| latest_history_metrics.as_ref().map(|(history, _)| history.id.clone()));

    let generated_at = candidate_attempt
        .as_ref()
        .and_then(|attempt| datetime_to_string(attempt.created_at))
        .or_else(|| {
            latest_history_metrics
                .as_ref()
                .and_then(|(history, _)| datetime_to_string(history.created_at))
        });

    let quality_metrics_summary = latest_quality_metrics.as_ref().map(|metrics| {
        json!({
            "repair_guidance": metrics.get("repair_guidance").cloned(),
            "quality_gate": metrics.get("quality_gate").cloned(),
            "quality_runtime_context": metrics.get("quality_runtime_context").cloned(),
            "raw": metrics,
        })
    });

    Ok(Json(json!({
        "chapter_id": chapter_id,
        "has_metrics": latest_quality_metrics.is_some(),
        "latest_metrics": latest_quality_metrics,
        "history_id": history_id,
        "generated_at": generated_at,
        "latest_quality_metrics": latest_quality_metrics,
        "quality_metrics_summary": quality_metrics_summary,
        "quality_profile_summary": Value::Null,
    })))
}

async fn get_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = match ChapterService::get(&db, &chapter_id, &claims.sub).await {
        Ok(Some(chapter)) => chapter,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "Chapter not found or access denied"})),
            ));
        }
        Err(error) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            ));
        }
    };
    let history_id = query.get("history_id").map(String::as_str);
    let reviser_loaded = load_latest_reviser_history(&db, &chapter_id, history_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    let (reviser_history, reviser_result) = match reviser_loaded {
        Some(item) => item,
        None if history_id.is_some() => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "指定的自动修订草稿不存在或不可用"})),
            ));
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "该章节暂无自动修订草稿"})),
            ));
        }
    };

    Ok(Json(json!({
        "chapter_id": chapter.id,
        "auto_revision_draft": build_auto_revision_draft_payload(
            &reviser_result,
            Some(&reviser_history.id),
            reviser_history.created_at,
            chapter.updated_at,
            true,
        ),
    })))
}

async fn apply_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let history_id = body
        .get("history_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let allow_stale = body
        .get("allow_stale")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let reviser_loaded = load_latest_reviser_history(&db, &chapter_id, history_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;
    let (reviser_history, reviser_result) = match reviser_loaded {
        Some(item) => item,
        None if history_id.is_some() => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "指定的自动修订草稿不存在或不可用"})),
            ));
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "该章节暂无可应用的自动修订草稿"})),
            ));
        }
    };

    let revised_text_raw = reviser_result
        .get("revised_text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (revised_text, _) = sanitize_generated_narrative_text(revised_text_raw);
    if revised_text.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "自动修订草稿内容为空，无法应用"})),
        ));
    }
    if contains_chapter_workflow_meta_text(&revised_text) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "自动修订草稿包含流程化元文本，无法应用"})),
        ));
    }

    let stale = is_draft_stale(chapter.updated_at, reviser_history.created_at);
    if stale && !allow_stale {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"detail": "自动修订草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true"})),
        ));
    }

    let critical_count = reviser_result
        .get("critical_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let major_count = reviser_result
        .get("major_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let priority_issue_count = reviser_result
        .get("priority_issue_count")
        .and_then(Value::as_i64)
        .unwrap_or(critical_count + major_count);
    let applied_critical_count = reviser_result
        .get("applied_critical_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let applied_issue_count = reviser_result
        .get("applied_issue_count")
        .and_then(Value::as_i64)
        .or(Some(applied_critical_count))
        .unwrap_or(0);
    let history_payload = json!({
        "log_type": "chapter_text_reviser_apply_v1",
        "source_history_id": reviser_history.id,
        "source_created_at": datetime_to_string(reviser_history.created_at),
        "critical_count": critical_count,
        "major_count": major_count,
        "priority_issue_count": priority_issue_count,
        "applied_critical_count": applied_critical_count,
        "applied_issue_count": applied_issue_count,
        "old_word_count": chapter.word_count,
        "new_word_count": revised_text.chars().count(),
        "stale_applied": stale,
        "allow_stale": allow_stale,
        "applied_at": datetime_to_string(Some(Utc::now().naive_utc())),
    });

    let (_updated, old_word_count, new_word_count) = apply_chapter_draft_content_with_history(
        &db,
        &chapter,
        &revised_text,
        format!("自动修订应用: 第{}章 {}", chapter.chapter_number, chapter.title),
        history_payload.to_string(),
        "chapter_text_reviser_apply_v1".to_string(),
    )
    .await
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "chapter_id": chapter.id,
        "word_count": new_word_count,
        "old_word_count": old_word_count,
        "draft_history_id": reviser_history.id,
        "draft_created_at": datetime_to_string(reviser_history.created_at),
        "stale_applied": stale,
        "message": "自动修订草稿已应用到章节正文",
    })))
}

async fn get_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let attempt_id = query.get("attempt_id").map(String::as_str);
    let draft_attempt = load_candidate_draft_attempt(&db, &chapter_id, attempt_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    let draft_attempt = match draft_attempt {
        Some(item) => item,
        None if attempt_id.is_some() => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "指定的候选草稿不存在或不可用"})),
            ));
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "该章节暂无候选草稿"})),
            ));
        }
    };

    Ok(Json(json!({
        "chapter_id": chapter.id,
        "candidate_draft": build_candidate_draft_payload(&draft_attempt, chapter.updated_at, true),
    })))
}

async fn apply_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let attempt_id = body
        .get("attempt_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let allow_stale = body
        .get("allow_stale")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let draft_attempt = load_candidate_draft_attempt(&db, &chapter_id, attempt_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    let draft_attempt = match draft_attempt {
        Some(item) => item,
        None if attempt_id.is_some() => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "指定的候选草稿不存在或不可用"})),
            ));
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "该章节暂无可应用的候选草稿"})),
            ));
        }
    };

    let (candidate_content_raw, has_full_content) = extract_candidate_draft_full_content(&draft_attempt);
    if !has_full_content || candidate_content_raw.trim().is_empty() {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"detail": "该候选草稿仅保留了预览，无法直接恢复正文"})),
        ));
    }

    let (candidate_content, _) = sanitize_generated_narrative_text(&candidate_content_raw);
    if candidate_content.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "候选草稿内容为空，无法应用"})),
        ));
    }
    if contains_chapter_workflow_meta_text(&candidate_content) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "候选草稿包含流程化元文本，无法应用"})),
        ));
    }

    let stale = is_draft_stale(chapter.updated_at, draft_attempt.created_at);
    if stale && !allow_stale {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"detail": "候选草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true"})),
        ));
    }

    let generated_content = json!({
        "content": candidate_content,
        "quality_metrics": draft_attempt.quality_metrics.clone().unwrap_or(Value::Null),
        "content_applied": true,
        "attempt_state": "applied_from_candidate",
    });

    let (_updated, old_word_count, new_word_count) = apply_chapter_draft_content_with_history(
        &db,
        &chapter,
        &candidate_content,
        format!("apply candidate draft: chapter {} {}", chapter.chapter_number, chapter.title),
        generated_content.to_string(),
        "chapter_candidate_apply_v1".to_string(),
    )
    .await
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "chapter_id": chapter.id,
        "word_count": new_word_count,
        "old_word_count": old_word_count,
        "draft_attempt_id": draft_attempt.id,
        "draft_created_at": datetime_to_string(draft_attempt.created_at),
        "stale_applied": stale,
        "message": "候选草稿已恢复到章节正文",
    })))
}

async fn create_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match ChapterService::create(
        &db,
        &body.project_id,
        &claims.sub,
        &body.title,
        body.chapter_number,
        body.content.as_deref(),
        body.summary.as_deref(),
        body.outline_id.as_deref(),
        body.sub_index,
    )
    .await
    {
        Ok(Some(chapter)) => Ok((StatusCode::CREATED, Json(compatible_chapter_payload(chapter)))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn list_chapters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::list_by_project(&db, &query.project_id, &claims.sub).await {
        Ok(Some(chapters)) => Ok(Json(json!({
            "success": true,
            "data": chapters,
            "items": chapters,
            "total": chapters.len()
        }))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn list_chapters_by_project_path(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::list_by_project(&db, &project_id, &claims.sub).await {
        Ok(Some(chapters)) => Ok(Json(json!({"items": chapters, "total": chapters.len()}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn get_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::get(&db, &chapter_id, &claims.sub).await {
        Ok(Some(chapter)) => Ok(Json(compatible_chapter_payload(chapter))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Chapter not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::update(
        &db,
        &chapter_id,
        &claims.sub,
        body.title.as_deref(),
        body.content.as_deref(),
        body.summary.as_deref(),
        body.status.as_deref(),
        body.chapter_number,
        body.expansion_plan.as_deref(),
    )
    .await
    {
        Ok(Some(chapter)) => Ok(Json(compatible_chapter_payload(chapter))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Chapter not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::delete(&db, &chapter_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(
            json!({"success": true, "message": "Chapter deleted successfully"}),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Chapter not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn get_navigation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::navigation(&db, &chapter_id, &claims.sub).await {
        Ok(Some((previous, current, next))) => Ok(Json(json!({
            "previous": previous,
            "current": current,
            "next": next,
        }))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn update_expansion_plan(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ExpansionPlanRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::update_expansion_plan(&db, &chapter_id, &claims.sub, &body.plan).await {
        Ok(Some(chapter)) => Ok(Json(compatible_chapter_payload(chapter))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Chapter not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn get_annotations(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::get_annotations(&db, &chapter_id, &claims.sub).await {
        Ok(Some(annotations)) => Ok(Json(annotations)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn get_quality_trend(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::quality_trend(&db, &project_id, &claims.sub).await {
        Ok(Some(trend)) => Ok(Json(trend.into())),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn get_can_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::can_generate(&db, &chapter_id, &claims.sub).await {
        Ok(Some(can_generate)) => Ok(Json(json!({"can_generate": can_generate}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found or access denied"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn get_analysis_task_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::get(&db, &chapter_id, &claims.sub).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "Chapter not found or access denied"})),
            ));
        }
        Err(error) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            ));
        }
    }

    let task = latest_analysis_task(&db, &chapter_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;
    analysis_task_status_payload(&db, &chapter_id, task)
        .await
        .map(Json)
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })
}

async fn get_batch_analysis_task_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<BatchAnalysisStatusRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut chapter_ids = Vec::new();
    for raw_id in body.chapter_ids.into_iter().take(200) {
        let chapter_id = raw_id.trim().to_string();
        if !chapter_id.is_empty() && !chapter_ids.contains(&chapter_id) {
            chapter_ids.push(chapter_id);
        }
    }

    if chapter_ids.is_empty() {
        return Ok(Json(json!({
            "project_id": "",
            "total": 0,
            "items": {},
        })));
    }

    let mut response_project_id = String::new();
    let mut items = serde_json::Map::new();
    for chapter_id in &chapter_ids {
        let chapter = ChapterService::get(&db, chapter_id, &claims.sub)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": error})),
                )
            })?;

        if let Some(chapter) = chapter {
            if response_project_id.is_empty() {
                response_project_id = chapter.project_id;
            }
            let task = latest_analysis_task(&db, chapter_id)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"detail": error.to_string()})),
                    )
                })?;
            let payload = analysis_task_status_payload(&db, chapter_id, task)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"detail": error.to_string()})),
                    )
                })?;
            items.insert(chapter_id.clone(), payload);
        } else {
            items.insert(
                chapter_id.clone(),
                json!({
                    "has_task": false,
                    "chapter_id": chapter_id,
                    "status": "none",
                    "progress": 0,
                    "error_message": null,
                    "auto_recovered": false,
                    "task_id": null,
                    "created_at": null,
                    "started_at": null,
                    "completed_at": null,
                }),
            );
        }
    }

    Ok(Json(json!({
        "project_id": response_project_id,
        "total": items.len(),
        "items": items,
    })))
}

async fn trigger_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match enqueue_chapter_analysis_task(&db, &claims.sub, &chapter_id).await {
        Ok(payload) => Ok(Json(payload)),
        Err(error) => Err(error),
    }
}

pub(crate) async fn enqueue_chapter_analysis_task(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
) -> Result<Value, (StatusCode, Json<Value>)> {
    let chapter_model = load_accessible_chapter_or_404(db, chapter_id, user_id).await?;
    let chapter_content = chapter_model.content.clone().unwrap_or_default();
    if chapter_content.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "章节不存在或内容为空"})),
        ));
    }

    let project_model = project::Entity::find_by_id(&chapter_model.project_id)
        .one(db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "项目不存在"})),
            )
        })?;

    if project_model.user_id != user_id {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "项目不存在"})),
        ));
    }

    let now = Utc::now().naive_utc();
    let task_id = Uuid::new_v4().to_string();
    let task = analysis_task::ActiveModel {
        id: Set(task_id.clone()),
        chapter_id: Set(chapter_id.to_string()),
        user_id: Set(user_id.to_string()),
        project_id: Set(project_model.id.clone()),
        status: Set("pending".to_string()),
        progress: Set(0),
        error_message: Set(None),
        created_at: Set(Some(now)),
        started_at: Set(None),
        completed_at: Set(None),
    };

    task.insert(db).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error.to_string()})),
        )
    })?;

    let db_for_task = db.clone();
    let user_id = user_id.to_string();
    let chapter_id_for_task = chapter_id.to_string();
    let task_id_for_task = task_id.clone();
    tokio::spawn(async move {
        execute_chapter_analysis_background(
            db_for_task,
            user_id,
            chapter_id_for_task,
            task_id_for_task,
        )
        .await;
    });

    Ok(json!({
        "task_id": task_id,
        "chapter_id": chapter_id,
        "status": "pending",
        "message": "章节分析任务已创建",
    }))
}

async fn apply_partial_regenerate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ApplyPartialRegenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;

    let new_text_raw = body.new_text.unwrap_or_default();
    let start_position = body.start_position.unwrap_or(0);
    let end_position = body.end_position.unwrap_or(0);

    let (new_text, _) = sanitize_generated_narrative_text(&new_text_raw);
    if new_text.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "改写内容为空"})),
        ));
    }
    if contains_chapter_workflow_meta_text(&new_text) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "改写内容仍包含工作流提示文本"})),
        ));
    }

    let current_content = chapter.content.unwrap_or_default();
    let content_chars: Vec<char> = current_content.chars().collect();
    let content_length = content_chars.len();
    if start_position >= end_position || end_position > content_length {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "改写位置非法"})),
        ));
    }

    let prefix: String = content_chars[..start_position].iter().collect();
    let suffix: String = content_chars[end_position..].iter().collect();
    let new_content = format!("{prefix}{new_text}{suffix}");
    let old_word_count = chapter.word_count;

    match ChapterService::update(
        &db,
        &chapter_id,
        &claims.sub,
        None,
        Some(&new_content),
        None,
        None,
        None,
        None,
    )
    .await
    {
        Ok(Some(updated)) => Ok(Json(json!({
            "success": true,
            "chapter_id": chapter_id,
            "word_count": updated.word_count,
            "old_word_count": old_word_count,
            "message": "局部改写已应用",
        }))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found or access denied"})),
        )),
        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        )),
    }
}

async fn regenerate_chapter_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let prompt = build_regeneration_prompt(&chapter, &body);
    let ai_service = build_regeneration_ai_service(&db, &claims.sub, None).await?;

    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(64);
    tokio::spawn(async move {
        let mut tracker = crate::utils::sse::SseProgress::new("Chapter Rewrite");
        let _ = tx.send(Ok(tracker.start())).await;
        let _ = tx.send(Ok(tracker.preparing(Some("Building rewrite prompt...")))).await;
        let _ = tx.send(Ok(tracker.generating(Some("Rewriting chapter..."), (20, 95), chapter.word_count as usize, None))).await;

        let mut full_content = String::new();
        let mut rx_stream = ai_service.generate_text_stream(prompt, None, None);
        while let Some(chunk) = rx_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let chunk_content = chunk.content.unwrap_or_default();
                    full_content.push_str(&chunk_content);
                    let _ = tx.send(Ok(crate::utils::sse::sse_chunk(&chunk_content))).await;
                }
                Err(error) => {
                    let _ = tx.send(Ok(crate::utils::sse::sse_error(&error, 500))).await;
                    return;
                }
            }
        }

        let (cleaned, _) = sanitize_generated_narrative_text(&full_content);
        if cleaned.trim().is_empty() {
            let _ = tx
                .send(Ok(crate::utils::sse::sse_error("Rewrite result is empty after sanitization", 500)))
                .await;
            return;
        }
        if contains_chapter_workflow_meta_text(&cleaned) {
            let _ = tx
                .send(Ok(crate::utils::sse::sse_error("Rewrite result still contains workflow meta text", 500)))
                .await;
            return;
        }

        let result = json!({
            "content": cleaned,
            "word_count": cleaned.chars().count(),
            "generation_task_id": chapter_id,
            "analysis_task_id": Value::Null,
        });
        let _ = tx.send(Ok(tracker.complete(Some("Rewrite complete")))).await;
        let _ = tx.send(Ok(crate::utils::sse::sse_result(&result))).await;
        let _ = tx.send(Ok(crate::utils::sse::sse_done())).await;
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::new().interval(TokioDuration::from_secs(10))))
}

async fn partial_regenerate_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<PartialRegenerateRequest>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let current_content = chapter.content.clone().unwrap_or_default();
    let content_chars: Vec<char> = current_content.chars().collect();
    let content_length = content_chars.len();
    if body.start_position >= body.end_position || body.end_position > content_length {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "改写位置非法"})),
        ));
    }

    let selected_text_from_content: String = content_chars[body.start_position..body.end_position]
        .iter()
        .collect();
    let selected_text = {
        let provided = body.selected_text.trim();
        if provided.is_empty() {
            selected_text_from_content.clone()
        } else {
            provided.to_string()
        }
    };
    if selected_text.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "选中内容为空"})),
        ));
    }

    let context_chars = body.context_chars.unwrap_or(500);
    let context_before_start = body.start_position.saturating_sub(context_chars);
    let context_before: String = content_chars[context_before_start..body.start_position]
        .iter()
        .collect();
    let context_after_end = body
        .end_position
        .saturating_add(context_chars)
        .min(content_length);
    let context_after: String = content_chars[body.end_position..context_after_end]
        .iter()
        .collect();

    let style_content = load_partial_style_content(&db, &claims, body.style_id).await?;
    let original_word_count = selected_text.chars().count();
    let length_requirement = build_partial_length_requirement(
        body.length_mode.as_deref(),
        body.target_word_count,
        original_word_count,
    );
    let target_words = calculate_partial_target_words(
        body.length_mode.as_deref(),
        body.target_word_count,
        original_word_count,
    );
    let max_tokens = max(500, min(target_words.saturating_mul(3), 8000)) as u32;
    let web_research_note = if body.enable_web_research.unwrap_or(false) {
        body.web_research_query
            .as_deref()
            .map(|query| format!("已请求联网检索，检索问题：{}", query))
    } else {
        None
    };
    let prompt = build_partial_regeneration_prompt(
        &chapter,
        &selected_text,
        &context_before,
        &context_after,
        &body.user_instructions,
        &length_requirement,
        style_content.as_deref(),
        web_research_note.as_deref(),
    );
    let ai_service = build_regeneration_ai_service(&db, &claims.sub, Some(max_tokens as u32)).await?;
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(64);
    tokio::spawn(async move {
        let mut tracker = crate::utils::sse::SseProgress::new("Partial Rewrite");
        let _ = tx.send(Ok(tracker.start())).await;
        let _ = tx.send(Ok(tracker.preparing(Some("Preparing rewrite context...")))).await;
        let _ = tx.send(Ok(tracker.preparing(Some("Starting generation...")))).await;

        let mut full_content = String::new();
        let mut chunk_count = 0u32;

        let mut rx_stream = ai_service.generate_text_stream(prompt, None, None);
        while let Some(chunk) = rx_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let chunk_content = chunk.content.unwrap_or_default();
                    full_content.push_str(&chunk_content);
                    chunk_count += 1;
                    let _ = tx
                        .send(Ok(crate::utils::sse::sse_chunk(&chunk_content)))
                        .await;
                    if chunk_count % 5 == 0 {
                        let _ = tx
                            .send(Ok(tracker.generating(
                                Some(&format!(
                                    "Generating rewrite... {}/{} chars",
                                    full_content.len(),
                                    target_words
                                )),
                                (35, 95),
                                full_content.len(),
                                None,
                            )))
                            .await;
                    }
                }
                Err(error) => {
                    let _ = tx.send(Ok(crate::utils::sse::sse_error(&error, 500))).await;
                    return;
                }
            }
        }

        let normalized = normalize_partial_regeneration_output(&full_content);
        let (cleaned, _) = sanitize_generated_narrative_text(&normalized);
        if cleaned.trim().is_empty() {
            let _ = tx
                .send(Ok(crate::utils::sse::sse_error("Rewrite result is empty after sanitization", 500)))
                .await;
            return;
        }
        if contains_chapter_workflow_meta_text(&cleaned) {
            let _ = tx
                .send(Ok(crate::utils::sse::sse_error("Rewrite result still contains workflow meta text", 500)))
                .await;
            return;
        }

        let result = json!({
            "new_text": cleaned,
            "word_count": cleaned.chars().count(),
            "original_word_count": original_word_count,
            "start_position": body.start_position,
            "end_position": body.end_position,
        });
        let _ = tx.send(Ok(tracker.complete(Some("Rewrite complete")))).await;
        let _ = tx.send(Ok(crate::utils::sse::sse_result(&result))).await;
        let _ = tx.send(Ok(crate::utils::sse::sse_done())).await;
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::new().interval(TokioDuration::from_secs(10))))
}

async fn get_regeneration_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<RegenerationTasksQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    let tasks = regeneration_task::Entity::find()
        .filter(regeneration_task::Column::ChapterId.eq(chapter_id.clone()))
        .order_by_desc(regeneration_task::Column::CreatedAt)
        .limit(limit)
        .all(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    let task_items: Vec<Value> = tasks
        .iter()
        .map(|task| {
            json!({
                "task_id": task.id,
                "status": task.status,
                "version_number": task.version_number,
                "version_note": task.version_note,
                "original_word_count": task.original_word_count,
                "regenerated_word_count": task.regenerated_word_count,
                "created_at": datetime_to_string(task.created_at),
                "completed_at": datetime_to_string(task.completed_at),
            })
        })
        .collect();

    Ok(Json(json!({
        "chapter_id": chapter_id,
        "total": task_items.len(),
        "tasks": task_items,
    })))
}

pub fn routes() -> Router {
    Router::new()
        .route(
            "/chapters/project/{project_id}",
            get(list_chapters_by_project_path),
        )
        .route(
            "/chapters/project/{project_id}/quality-trend",
            get(get_quality_trend),
        )
        .route("/chapters/{chapter_id}/navigation", get(get_navigation))
        .route(
            "/chapters/{chapter_id}/expansion-plan",
            axum::routing::put(update_expansion_plan),
        )
        .route("/chapters/{chapter_id}/annotations", get(get_annotations))
        .route("/chapters/{chapter_id}/can-generate", get(get_can_generate))
        .route(
            "/chapters/{chapter_id}/quality-metrics",
            get(get_chapter_quality_metrics),
        )
        .route("/chapters/{chapter_id}/analysis", get(get_chapter_analysis))
        .route(
            "/chapters/{chapter_id}/analysis/status",
            get(get_analysis_task_status),
        )
        .route(
            "/chapters/analysis/status/batch",
            axum::routing::post(get_batch_analysis_task_status),
        )
        .route("/chapters/{chapter_id}/analyze", post(trigger_chapter_analysis))
        .route(
            "/chapters/{chapter_id}/analysis/auto-revision-draft",
            get(get_auto_revision_draft),
        )
        .route(
            "/chapters/{chapter_id}/analysis/auto-revision-draft/apply",
            axum::routing::post(apply_auto_revision_draft),
        )
        .route(
            "/chapters/{chapter_id}/analysis/candidate-draft",
            get(get_candidate_draft),
        )
        .route(
            "/chapters/{chapter_id}/analysis/candidate-draft/apply",
            axum::routing::post(apply_candidate_draft),
        )
        .route(
            "/chapters/{chapter_id}/regenerate-stream",
            post(regenerate_chapter_stream),
        )
        .route(
            "/chapters/{chapter_id}/partial-regenerate-stream",
            post(partial_regenerate_stream),
        )
        .route(
            "/chapters/{chapter_id}/apply-partial-regenerate",
            post(apply_partial_regenerate),
        )
        .route(
            "/chapters/{chapter_id}/regeneration/tasks",
            get(get_regeneration_tasks),
        )
        .route(
            "/chapters",
            axum::routing::get(list_chapters).post(create_chapter),
        )
        .route(
            "/chapters/{chapter_id}",
            get(get_chapter).put(update_chapter).delete(delete_chapter),
        )
}
