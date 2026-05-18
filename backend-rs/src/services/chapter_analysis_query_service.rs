use chrono::{Duration, NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::{json, Value};

use crate::models::{
    analysis_task, chapter, chapter_draft_attempt, generation_history, plot_analysis,
    story_memory,
};
use crate::services::chapter_analysis_checker_query_service::build_chapter_analysis_checker_fragments;
use crate::services::chapter_analysis_service::LoadAnalysisTaskStatusError;
use crate::services::chapter_analysis_view_payload_adapter_service::{
    build_chapter_analysis_view_payload,
};
use crate::services::chapter_draft_query_service::{
    build_chapter_draft_analysis_view_fragments,
};
use crate::services::chapter_quality_query_service::{
    build_chapter_analysis_quality_fragments,
    load_chapter_quality_metrics_payload as load_chapter_quality_metrics_query_payload,
};
use crate::services::chapter_service::ChapterService;

fn format_datetime(value: Option<NaiveDateTime>) -> Option<String> {
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

pub async fn latest_analysis_task(
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

pub async fn analysis_task_status_payload(
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
        "created_at": format_datetime(task.created_at),
        "started_at": format_datetime(task.started_at),
        "completed_at": format_datetime(task.completed_at),
    }))
}

pub async fn load_analysis_task_status_payload(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
) -> Result<Value, LoadAnalysisTaskStatusError> {
    let chapter = ChapterService::get(db, chapter_id, user_id)
        .await
        .map_err(LoadAnalysisTaskStatusError::Internal)?;
    let Some(_chapter) = chapter else {
        return Err(LoadAnalysisTaskStatusError::ChapterNotFound);
    };

    let task = latest_analysis_task(db, chapter_id)
        .await
        .map_err(|error| LoadAnalysisTaskStatusError::Internal(error.to_string()))?;
    analysis_task_status_payload(db, chapter_id, task)
        .await
        .map_err(|error| LoadAnalysisTaskStatusError::Internal(error.to_string()))
}

pub async fn load_batch_analysis_task_status_payload(
    db: &DatabaseConnection,
    user_id: &str,
    raw_chapter_ids: Vec<String>,
) -> Result<Value, String> {
    let mut chapter_ids = Vec::new();
    for raw_id in raw_chapter_ids.into_iter().take(200) {
        let chapter_id = raw_id.trim().to_string();
        if !chapter_id.is_empty() && !chapter_ids.contains(&chapter_id) {
            chapter_ids.push(chapter_id);
        }
    }

    if chapter_ids.is_empty() {
        return Ok(json!({
            "project_id": "",
            "total": 0,
            "items": {},
        }));
    }

    let mut response_project_id = String::new();
    let mut items = serde_json::Map::new();
    for chapter_id in &chapter_ids {
        let chapter = ChapterService::get(db, chapter_id, user_id)
            .await
            .map_err(|error| error.to_string())?;

        if let Some(chapter) = chapter {
            if response_project_id.is_empty() {
                response_project_id = chapter.project_id;
            }
            let task = latest_analysis_task(db, chapter_id)
                .await
                .map_err(|error| error.to_string())?;
            let payload = analysis_task_status_payload(db, chapter_id, task)
                .await
                .map_err(|error| error.to_string())?;
            items.insert(chapter_id.clone(), payload);
        } else {
            let payload = analysis_task_status_payload(db, chapter_id, None)
                .await
                .map_err(|error| error.to_string())?;
            items.insert(chapter_id.clone(), payload);
        }
    }

    Ok(json!({
        "project_id": response_project_id,
        "total": items.len(),
        "items": items,
    }))
}

pub async fn load_chapter_analysis_view_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
) -> Result<Value, String> {
    let chapter_id = chapter.id.clone();

    let analysis = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ChapterId.eq(&chapter_id))
        .order_by_desc(plot_analysis::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Chapter analysis not found".to_string())?;

    let memories = story_memory::Entity::find()
        .filter(story_memory::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(story_memory::Column::ImportanceScore)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let histories: Vec<generation_history::Model> = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(generation_history::Column::CreatedAt)
        .limit(30)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let candidate_attempt = chapter_draft_attempt::Entity::find()
        .filter(chapter_draft_attempt::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(chapter_draft_attempt::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| error.to_string())?;

    let checker_fragments = build_chapter_analysis_checker_fragments(&histories);
    let draft_fragments = build_chapter_draft_analysis_view_fragments(
        &histories,
        candidate_attempt.as_ref(),
        chapter.updated_at,
    );
    let quality_fragments =
        build_chapter_analysis_quality_fragments(&histories, candidate_attempt.as_ref());
    let analysis_created_at = format_datetime(analysis.created_at);
    let created_at = analysis_created_at
        .clone()
        .or_else(|| format_datetime(chapter.updated_at))
        .unwrap_or_default();

    Ok(build_chapter_analysis_view_payload(
        chapter,
        analysis,
        memories,
        checker_fragments,
        draft_fragments,
        quality_fragments,
        created_at,
        analysis_created_at,
    ))
}

pub async fn load_chapter_quality_metrics_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
) -> Result<Value, String> {
    load_chapter_quality_metrics_query_payload(db, chapter).await
}
