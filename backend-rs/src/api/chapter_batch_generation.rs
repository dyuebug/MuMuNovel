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
use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::ai::AIConfig;
use crate::models::{batch_generation_snapshot, batch_generation_task, chapter, project};
use crate::services::auth::Claims;
use crate::services::chapter_generation_service::ChapterGenerationService;
use crate::services::settings_service::SettingsService;

#[derive(Deserialize)]
struct BatchGenerateRequest {
    start_chapter_number: i32,
    count: i32,
    style_id: Option<i32>,
    target_word_count: Option<i32>,
    enable_analysis: Option<bool>,
    enable_mcp: Option<bool>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
    max_retries: Option<i32>,
    model: Option<String>,
    creative_mode: Option<String>,
    story_focus: Option<String>,
    plot_stage: Option<String>,
    story_creation_brief: Option<String>,
    quality_preset: Option<String>,
    quality_notes: Option<String>,
    story_repair_summary: Option<String>,
    story_repair_targets: Option<Vec<String>>,
    story_preserve_strengths: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ActiveQuery {
    limit: Option<u64>,
}

#[derive(Deserialize)]
struct ChapterGenerateRequest {
    target_word_count: Option<i32>,
    model: Option<String>,
    #[serde(default)]
    enable_analysis: Option<bool>,
}

fn to_iso(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|v| v.and_utc().to_rfc3339())
}

async fn build_user_ai_config(
    db: &DatabaseConnection,
    user_id: &str,
    model_override: Option<&str>,
) -> Result<AIConfig, String> {
    SettingsService::build_ai_config(db, user_id, None, model_override, None).await
}

fn load_chapter_generation_target(
    body: &ChapterGenerateRequest,
) -> i32 {
    body.target_word_count.unwrap_or(3000).max(1)
}

async fn verify_project_access(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<bool, String> {
    project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map(|result| result.is_some())
        .map_err(|error| error.to_string())
}

fn task_type(task: &batch_generation_task::Model) -> &'static str {
    if task.chapter_count == 1
        && task
            .chapter_ids
            .as_array()
            .is_some_and(|items| items.len() == 1)
    {
        "chapter_single_generate"
    } else {
        "chapters_batch_generate"
    }
}

fn task_stage_code(task: &batch_generation_task::Model) -> &'static str {
    match task_type(task) {
        "chapter_single_generate" => match task.status.as_deref() {
            Some("completed") => "6.writing.completed",
            Some("failed") => "6.writing.failed",
            Some("cancelled") => "6.writing.cancelled",
            Some("running") => "6.writing.generating",
            _ => "6.writing.pending",
        },
        _ => match task.status.as_deref() {
            Some("completed") => "6.writing.completed",
            Some("failed") => "6.writing.failed",
            Some("cancelled") => "6.writing.cancelled",
            Some("running") => "6.writing.generating",
            _ => "6.writing.pending",
        },
    }
}

fn task_execution_mode(task: &batch_generation_task::Model) -> &'static str {
    match task_type(task) {
        "chapter_single_generate" => "interactive",
        _ => "interactive",
    }
}

fn task_status_payload(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<Value>,
    latest_quality_metrics: Option<Value>,
    quality_metrics_summary: Option<Value>,
    active_story_repair_payload: Option<Value>,
) -> Value {
    let stage_code = task_stage_code(task);
    let execution_mode = task_execution_mode(task);
    let failed_chapters = task.failed_chapters.clone().unwrap_or_else(|| json!([]));
    let mut checkpoint = workflow_runtime_state
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    checkpoint.insert("stage_code".to_string(), json!(stage_code));
    checkpoint.insert("execution_mode".to_string(), json!(execution_mode));
    json!({
        "batch_id": task.id,
        "task_type": task_type(task),
        "project_id": task.project_id,
        "status": task.status,
        "stage_code": stage_code,
        "execution_mode": execution_mode,
        "total": task.total_chapters.unwrap_or(task.chapter_count),
        "completed": task.completed_chapters.unwrap_or(0),
        "current_chapter_id": task.current_chapter_id,
        "current_chapter_number": task.current_chapter_number,
        "current_retry_count": task.current_retry_count,
        "max_retries": task.max_retries,
        "failed_chapters": failed_chapters,
        "created_at": to_iso(task.created_at),
        "started_at": to_iso(task.started_at),
        "completed_at": to_iso(task.completed_at),
        "error_message": task.error_message,
        "checkpoint": checkpoint,
        "latest_quality_metrics": latest_quality_metrics,
        "quality_metrics_summary": quality_metrics_summary,
        "active_story_repair_payload": active_story_repair_payload,
        "terminal_reason": match task.status.as_deref() {
            Some("completed") => Some("completed"),
            Some("cancelled") => Some("cancelled"),
            Some("failed") => Some("error"),
            _ => None,
        },
        "terminal_label": match task.status.as_deref() {
            Some("completed") => Some("已完成"),
            Some("cancelled") => Some("已取消"),
            Some("failed") => Some("执行失败"),
            _ => None,
        },
        "review_required": false,
        "can_resume": matches!(task.status.as_deref(), Some("failed" | "cancelled")),
    })
}

fn active_task_payload(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<Value>,
    latest_quality_metrics: Option<Value>,
    quality_metrics_summary: Option<Value>,
    active_story_repair_payload: Option<Value>,
) -> Value {
    let stage_code = task_stage_code(task);
    let execution_mode = task_execution_mode(task);
    let mut checkpoint = workflow_runtime_state
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    checkpoint.insert("stage_code".to_string(), json!(stage_code));
    checkpoint.insert("execution_mode".to_string(), json!(execution_mode));
    json!({
        "batch_id": task.id,
        "task_type": task_type(task),
        "project_id": task.project_id,
        "status": task.status,
        "stage_code": stage_code,
        "execution_mode": execution_mode,
        "total": task.total_chapters.unwrap_or(task.chapter_count),
        "completed": task.completed_chapters.unwrap_or(0),
        "current_chapter_id": task.current_chapter_id,
        "current_chapter_number": task.current_chapter_number,
        "checkpoint": checkpoint,
        "latest_quality_metrics": latest_quality_metrics,
        "quality_metrics_summary": quality_metrics_summary,
        "active_story_repair_payload": active_story_repair_payload,
        "created_at": to_iso(task.created_at),
        "started_at": to_iso(task.started_at),
        "completed_at": to_iso(task.completed_at),
        "error_message": task.error_message,
    })
}

async fn load_batch_generation_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<Option<batch_generation_snapshot::Model>, String> {
    batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.eq(task_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
}

async fn upsert_batch_generation_runtime_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let existing = load_batch_generation_snapshot(db, task_id).await?;

    if let Some(snapshot) = existing {
        let mut active: batch_generation_snapshot::ActiveModel = snapshot.into();
        let merged_runtime_state = match (active.workflow_runtime_state.clone().take(), workflow_runtime_state) {
            (Some(Some(Value::Object(mut current))), Value::Object(incoming)) => {
                for (key, value) in incoming {
                    current.insert(key, value);
                }
                Value::Object(current)
            }
            (_, incoming) => incoming,
        };
        active.workflow_runtime_state = Set(Some(merged_runtime_state));
        active.updated_at = Set(Some(now));
        active.update(db).await.map_err(|error| error.to_string())?;
        return Ok(());
    }

    let active = batch_generation_snapshot::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        batch_task_id: Set(task_id.to_string()),
        latest_quality_metrics: Set(None),
        quality_metrics_history: Set(None),
        quality_metrics_summary: Set(None),
        workflow_runtime_state: Set(Some(workflow_runtime_state)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
    };
    active.insert(db).await.map_err(|error| error.to_string())?;
    Ok(())
}

fn build_runtime_checkpoint(
    phase: &str,
    progress: i32,
    status: &str,
    last_event: &str,
    last_message: &str,
    chapter_id: Option<&str>,
    current_chapter_number: Option<i32>,
) -> Value {
    json!({
        "phase": phase,
        "progress": progress.clamp(0, 100),
        "status": status,
        "last_event": last_event,
        "last_message": last_message,
        "chapter_id": chapter_id,
        "current_chapter_id": chapter_id,
        "current_chapter_number": current_chapter_number,
        "updated_at": Utc::now().to_rfc3339(),
    })
}

fn build_single_generation_runtime_checkpoint(
    phase: &str,
    progress: i32,
    status: &str,
    last_event: &str,
    last_message: &str,
    chapter_id: &str,
    current_chapter_number: Option<i32>,
    word_count: Option<i32>,
) -> Value {
    let mut checkpoint = build_runtime_checkpoint(
        phase,
        progress,
        status,
        last_event,
        last_message,
        Some(chapter_id),
        current_chapter_number,
    );
    if let Some(object) = checkpoint.as_object_mut() {
        if let Some(value) = word_count {
            object.insert("word_count".to_string(), json!(value.max(0)));
        }
    }
    checkpoint
}

fn build_batch_generation_runtime_checkpoint(
    phase: &str,
    progress: i32,
    status: &str,
    last_event: &str,
    last_message: &str,
    chapter_id: Option<&str>,
    current_chapter_number: Option<i32>,
    completed_chapters: i32,
    total_chapters: i32,
) -> Value {
    let mut checkpoint = build_runtime_checkpoint(
        phase,
        progress,
        status,
        last_event,
        last_message,
        chapter_id,
        current_chapter_number,
    );
    if let Some(object) = checkpoint.as_object_mut() {
        object.insert("completed".to_string(), json!(completed_chapters.max(0)));
        object.insert("total".to_string(), json!(total_chapters.max(0)));
    }
    checkpoint
}

fn active_story_repair_payload_from_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("active_story_repair_payload"))
        .filter(|payload| payload.is_object())
        .cloned()
}

async fn load_owned_task(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Option<batch_generation_task::Model>, String> {
    let task = batch_generation_task::Entity::find_by_id(batch_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?;
    Ok(task.filter(|task| task.user_id == user_id))
}

fn spawn_single_chapter_generation(
    db: DatabaseConnection,
    task_id: String,
    user_id: String,
    chapter_id: String,
    target_word_count: i32,
    ai_config: AIConfig,
) {
    tokio::spawn(async move {
        let now = Utc::now().naive_utc();
        if let Ok(Some(task_model)) = batch_generation_task::Entity::find_by_id(&task_id).one(&db).await {
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            active.status = Set(Some("running".to_string()));
            active.started_at = Set(Some(now));
            active.completed_at = Set(None);
            active.error_message = Set(None);
            active.current_retry_count = Set(Some(0));
            let _ = active.update(&db).await;
        }
        let _ = upsert_batch_generation_runtime_snapshot(
            &db,
            &task_id,
            build_single_generation_runtime_checkpoint(
                "generating",
                15,
                "running",
                "chapter_start",
                "正在准备章节生成...",
                &chapter_id,
                None,
                None,
            ),
        )
        .await;
        let _ = upsert_batch_generation_runtime_snapshot(
            &db,
            &task_id,
            build_single_generation_runtime_checkpoint(
                "generating",
                65,
                "running",
                "progress",
                "正在生成正文...",
                &chapter_id,
                None,
                None,
            ),
        )
        .await;

        let ai_service = AIService::new(ai_config);
        let generation_result = ChapterGenerationService::generate_and_persist_chapter_content(
            &db,
            &ai_service,
            &user_id,
            &chapter_id,
            target_word_count,
        )
        .await;

        match generation_result {
            Ok(payload) => {
                let chapter_number = payload
                    .get("chapter_number")
                    .and_then(Value::as_i64)
                    .map(|value| value as i32);
                let word_count = payload
                    .get("word_count")
                    .and_then(Value::as_i64)
                    .map(|value| value as i32);
                let _ = upsert_batch_generation_runtime_snapshot(
                    &db,
                    &task_id,
                    build_single_generation_runtime_checkpoint(
                        "finalizing",
                        95,
                        "running",
                        "progress",
                        "正在整理生成结果...",
                        &chapter_id,
                        chapter_number,
                        word_count,
                    ),
                )
                .await;
                if let Ok(Some(task_model)) =
                    batch_generation_task::Entity::find_by_id(&task_id).one(&db).await
                {
                    let mut active: batch_generation_task::ActiveModel = task_model.into();
                    active.status = Set(Some("completed".to_string()));
                    active.completed_chapters = Set(Some(1));
                    active.completed_at = Set(Some(Utc::now().naive_utc()));
                    active.error_message = Set(None);
                    active.current_chapter_id =
                        Set(payload.get("chapter_id").and_then(Value::as_str).map(str::to_string));
                    active.current_chapter_number = Set(
                        payload
                            .get("chapter_number")
                            .and_then(Value::as_i64)
                            .map(|value| value as i32),
                    );
                    let _ = active.update(&db).await;
                }
                let _ = upsert_batch_generation_runtime_snapshot(
                    &db,
                    &task_id,
                    build_single_generation_runtime_checkpoint(
                        "completed",
                        100,
                        "completed",
                        "done",
                        "章节生成完成",
                        &chapter_id,
                        chapter_number,
                        word_count,
                    ),
                )
                .await;
            }
            Err(error) => {
                if let Ok(Some(task_model)) =
                    batch_generation_task::Entity::find_by_id(&task_id).one(&db).await
                {
                    let mut active: batch_generation_task::ActiveModel = task_model.into();
                    active.status = Set(Some("failed".to_string()));
                    active.completed_at = Set(Some(Utc::now().naive_utc()));
                    active.error_message = Set(Some(error));
                    let _ = active.update(&db).await;
                }
                let _ = upsert_batch_generation_runtime_snapshot(
                    &db,
                    &task_id,
                    build_single_generation_runtime_checkpoint(
                        "failed",
                        100,
                        "failed",
                        "error",
                        "章节生成失败",
                        &chapter_id,
                        None,
                        None,
                    ),
                )
                .await;
            }
        }
    });
}

async fn load_accessible_chapter_for_generation(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, (StatusCode, Json<Value>)> {
    let chapter_model = chapter::Entity::find_by_id(chapter_id)
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
                Json(json!({"detail": "Chapter not found"})),
            )
        })?;

    if !verify_project_access(db, &chapter_model.project_id, user_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
    {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found or access denied"})),
        ));
    }

    Ok(chapter_model)
}

fn build_single_task_chapter_payload(chapter_model: &chapter::Model) -> Value {
    json!([{
        "id": chapter_model.id,
        "chapter_number": chapter_model.chapter_number,
        "title": chapter_model.title,
    }])
}

fn parse_batch_task_chapter_ids(task: &batch_generation_task::Model) -> Vec<String> {
    task.chapter_ids
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.as_str()
                .map(str::to_string)
                .or_else(|| item.get("id").and_then(Value::as_str).map(str::to_string))
        })
        .collect()
}

fn spawn_batch_generation(
    db: DatabaseConnection,
    task_id: String,
    user_id: String,
    chapter_ids: Vec<String>,
    target_word_count: i32,
    ai_config: AIConfig,
) {
    tokio::spawn(async move {
        let now = Utc::now().naive_utc();
        if let Ok(Some(task_model)) = batch_generation_task::Entity::find_by_id(&task_id).one(&db).await {
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            active.status = Set(Some("running".to_string()));
            active.started_at = Set(Some(now));
            active.completed_at = Set(None);
            active.error_message = Set(None);
            active.current_retry_count = Set(Some(0));
            let _ = active.update(&db).await;
        }
        let _ = upsert_batch_generation_runtime_snapshot(
            &db,
            &task_id,
            build_batch_generation_runtime_checkpoint(
                "generating",
                10,
                "running",
                "progress",
                "正在准备批量生成...",
                None,
                None,
                0,
                chapter_ids.len() as i32,
            ),
        )
        .await;

        let ai_service = AIService::new(ai_config);
        let total = chapter_ids.len() as i32;
        let mut completed = 0i32;

        for chapter_id in &chapter_ids {
            let task_model = match batch_generation_task::Entity::find_by_id(&task_id).one(&db).await {
                Ok(Some(task_model)) => task_model,
                _ => return,
            };

            if matches!(task_model.status.as_deref(), Some("cancelled")) {
                let mut active: batch_generation_task::ActiveModel = task_model.into();
                active.completed_at = Set(Some(Utc::now().naive_utc()));
                let _ = active.update(&db).await;
                let _ = upsert_batch_generation_runtime_snapshot(
                    &db,
                    &task_id,
                    build_batch_generation_runtime_checkpoint(
                        "cancelled",
                        100,
                        "cancelled",
                        "cancelled",
                        "批量生成已取消",
                        None,
                        None,
                        completed,
                        total,
                    ),
                )
                .await;
                return;
            }

            let chapter_model = match chapter::Entity::find_by_id(chapter_id).one(&db).await {
                Ok(Some(chapter_model)) => chapter_model,
                Ok(None) => {
                    let mut active: batch_generation_task::ActiveModel = task_model.into();
                    active.status = Set(Some("failed".to_string()));
                    active.completed_at = Set(Some(Utc::now().naive_utc()));
                    active.error_message = Set(Some(format!("Chapter not found: {}", chapter_id)));
                    let _ = active.update(&db).await;
                    let _ = upsert_batch_generation_runtime_snapshot(
                        &db,
                        &task_id,
                        build_batch_generation_runtime_checkpoint(
                            "failed",
                            100,
                            "failed",
                            "error",
                            "批量生成失败：章节不存在",
                            Some(chapter_id),
                            None,
                            completed,
                            total,
                        ),
                    )
                    .await;
                    return;
                }
                Err(error) => {
                    let mut active: batch_generation_task::ActiveModel = task_model.into();
                    active.status = Set(Some("failed".to_string()));
                    active.completed_at = Set(Some(Utc::now().naive_utc()));
                    active.error_message = Set(Some(error.to_string()));
                    let _ = active.update(&db).await;
                    let _ = upsert_batch_generation_runtime_snapshot(
                        &db,
                        &task_id,
                        build_batch_generation_runtime_checkpoint(
                            "failed",
                            100,
                            "failed",
                            "error",
                            "批量生成失败：加载章节异常",
                            Some(chapter_id),
                            None,
                            completed,
                            total,
                        ),
                    )
                    .await;
                    return;
                }
            };

            let mut active: batch_generation_task::ActiveModel = task_model.into();
            active.status = Set(Some("running".to_string()));
            active.current_chapter_id = Set(Some(chapter_model.id.clone()));
            active.current_chapter_number = Set(Some(chapter_model.chapter_number));
            active.total_chapters = Set(Some(total));
            active.completed_chapters = Set(Some(completed));
            active.error_message = Set(None);
            let _ = active.update(&db).await;
            let base_progress = if total <= 0 {
                15
            } else {
                ((completed * 100) / total).clamp(0, 100)
            };
            let running_progress = (base_progress + 15).clamp(15, 95);
            let _ = upsert_batch_generation_runtime_snapshot(
                &db,
                &task_id,
                build_batch_generation_runtime_checkpoint(
                    "generating",
                    running_progress,
                    "running",
                    "chapter_start",
                    &format!("正在生成第 {} 章...", chapter_model.chapter_number),
                    Some(&chapter_model.id),
                    Some(chapter_model.chapter_number),
                    completed,
                    total,
                ),
            )
            .await;

            let generation_result = ChapterGenerationService::generate_and_persist_chapter_content(
                &db,
                &ai_service,
                &user_id,
                &chapter_model.id,
                target_word_count,
            )
            .await;

            match generation_result {
                Ok(_) => {
                    completed += 1;
                    let completed_progress = if total <= 0 {
                        100
                    } else {
                        ((completed * 100) / total).clamp(0, 100)
                    };
                    if let Ok(Some(task_model)) =
                        batch_generation_task::Entity::find_by_id(&task_id).one(&db).await
                    {
                        let mut active: batch_generation_task::ActiveModel = task_model.into();
                        active.status = Set(Some(if completed >= total {
                            "completed".to_string()
                        } else {
                            "running".to_string()
                        }));
                        active.completed_chapters = Set(Some(completed));
                        active.current_chapter_id = Set(Some(chapter_model.id.clone()));
                        active.current_chapter_number = Set(Some(chapter_model.chapter_number));
                        if completed >= total {
                            active.completed_at = Set(Some(Utc::now().naive_utc()));
                        }
                        let _ = active.update(&db).await;
                    }
                    let _ = upsert_batch_generation_runtime_snapshot(
                        &db,
                        &task_id,
                        build_batch_generation_runtime_checkpoint(
                            if completed >= total {
                                "completed"
                            } else {
                                "generating"
                            },
                            if completed >= total { 100 } else { completed_progress },
                            if completed >= total { "completed" } else { "running" },
                            if completed >= total { "done" } else { "progress" },
                            if completed >= total {
                                "批量生成完成"
                            } else {
                                "当前章节生成完成，继续下一章..."
                            },
                            Some(&chapter_model.id),
                            Some(chapter_model.chapter_number),
                            completed,
                            total,
                        ),
                    )
                    .await;
                }
                Err(error) => {
                    if let Ok(Some(task_model)) =
                        batch_generation_task::Entity::find_by_id(&task_id).one(&db).await
                    {
                        let mut active: batch_generation_task::ActiveModel = task_model.into();
                        active.status = Set(Some("failed".to_string()));
                        active.completed_chapters = Set(Some(completed));
                        active.current_chapter_id = Set(Some(chapter_model.id.clone()));
                        active.current_chapter_number = Set(Some(chapter_model.chapter_number));
                        active.completed_at = Set(Some(Utc::now().naive_utc()));
                        active.error_message = Set(Some(error));
                        let _ = active.update(&db).await;
                    }
                    let _ = upsert_batch_generation_runtime_snapshot(
                        &db,
                        &task_id,
                        build_batch_generation_runtime_checkpoint(
                            "failed",
                            100,
                            "failed",
                            "error",
                            "批量生成失败",
                            Some(&chapter_model.id),
                            Some(chapter_model.chapter_number),
                            completed,
                            total,
                        ),
                    )
                    .await;
                    return;
                }
            }
        }
    });
}

async fn create_batch_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<BatchGenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _ = (
        &body.enable_mcp,
        &body.enable_web_research,
        &body.web_research_query,
        &body.model,
        &body.creative_mode,
        &body.story_focus,
        &body.plot_stage,
        &body.story_creation_brief,
        &body.quality_preset,
        &body.quality_notes,
        &body.story_repair_summary,
        &body.story_repair_targets,
        &body.story_preserve_strengths,
    );

    if !verify_project_access(&db, &project_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
    {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found or access denied"})),
        ));
    }

    if body.count <= 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "count must be greater than 0"})),
        ));
    }

    let end_chapter_number = body.start_chapter_number + body.count - 1;
    let chapters_to_generate = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&project_id))
        .filter(chapter::Column::ChapterNumber.gte(body.start_chapter_number))
        .filter(chapter::Column::ChapterNumber.lte(end_chapter_number))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;
    if chapters_to_generate.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "未找到指定范围内的章节"})),
        ));
    }

    let chapter_id_values: Vec<Value> = chapters_to_generate
        .iter()
        .map(|chapter_model| json!(chapter_model.id))
        .collect();
    let target_word_count = body.target_word_count.unwrap_or(3000).max(1);
    let ai_config = build_user_ai_config(&db, &claims.sub, body.model.as_deref())
        .await
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": error})),
            )
        })?;

    let now = Utc::now().naive_utc();
    let task = batch_generation_task::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(project_id.clone()),
        user_id: Set(claims.sub.clone()),
        start_chapter_number: Set(body.start_chapter_number),
        chapter_count: Set(chapters_to_generate.len() as i32),
        chapter_ids: Set(Value::Array(chapter_id_values)),
        style_id: Set(body.style_id),
        target_word_count: Set(Some(target_word_count)),
        enable_analysis: Set(body.enable_analysis),
        status: Set(Some("pending".to_string())),
        total_chapters: Set(Some(chapters_to_generate.len() as i32)),
        completed_chapters: Set(Some(0)),
        failed_chapters: Set(Some(json!([]))),
        current_chapter_id: Set(None),
        current_chapter_number: Set(None),
        current_retry_count: Set(Some(0)),
        max_retries: Set(body.max_retries),
        created_at: Set(Some(now)),
        started_at: Set(None),
        completed_at: Set(None),
        error_message: Set(None),
    };
    let saved = task.insert(&db).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error.to_string()})),
        )
    })?;
    let _ = upsert_batch_generation_runtime_snapshot(
        &db,
        &saved.id,
        build_batch_generation_runtime_checkpoint(
            "pending",
            0,
            "pending",
            "queued",
            "批量生成任务已创建，等待开始...",
            None,
            None,
            0,
            chapters_to_generate.len() as i32,
        ),
    )
    .await;

    spawn_batch_generation(
        db.clone(),
        saved.id.clone(),
        claims.sub.clone(),
        chapters_to_generate.iter().map(|chapter_model| chapter_model.id.clone()).collect(),
        target_word_count,
        ai_config,
    );

    Ok(Json(json!({
        "batch_id": saved.id,
        "message": "Batch generation task created",
        "chapters_to_generate": chapters_to_generate.iter().map(|chapter_model| json!({
            "id": chapter_model.id,
            "chapter_number": chapter_model.chapter_number,
            "title": chapter_model.title,
        })).collect::<Vec<_>>(),
        "estimated_time_minutes": (chapters_to_generate.len() as i32).max(1) * 2,
    })))
}

async fn generate_chapter_content_background(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ChapterGenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _ = body.enable_analysis;

    let chapter_model = load_accessible_chapter_for_generation(&db, &chapter_id, &claims.sub).await?;
    let ai_config = build_user_ai_config(&db, &claims.sub, body.model.as_deref())
        .await
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": error})),
            )
        })?;

    let now = Utc::now().naive_utc();
    let task_id = Uuid::new_v4().to_string();
    let task = batch_generation_task::ActiveModel {
        id: Set(task_id.clone()),
        project_id: Set(chapter_model.project_id.clone()),
        user_id: Set(claims.sub.clone()),
        start_chapter_number: Set(chapter_model.chapter_number),
        chapter_count: Set(1),
        chapter_ids: Set(build_single_task_chapter_payload(&chapter_model)),
        style_id: Set(None),
        target_word_count: Set(body.target_word_count.or(Some(3000))),
        enable_analysis: Set(Some(false)),
        status: Set(Some("pending".to_string())),
        total_chapters: Set(Some(1)),
        completed_chapters: Set(Some(0)),
        failed_chapters: Set(Some(json!([]))),
        current_chapter_id: Set(Some(chapter_model.id.clone())),
        current_chapter_number: Set(Some(chapter_model.chapter_number)),
        current_retry_count: Set(Some(0)),
        max_retries: Set(Some(0)),
        created_at: Set(Some(now)),
        started_at: Set(None),
        completed_at: Set(None),
        error_message: Set(None),
    };
    task.insert(&db).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error.to_string()})),
        )
    })?;
    let _ = upsert_batch_generation_runtime_snapshot(
        &db,
        &task_id,
        build_single_generation_runtime_checkpoint(
            "pending",
            0,
            "pending",
            "queued",
            "单章生成任务已创建，等待开始...",
            &chapter_model.id,
            Some(chapter_model.chapter_number),
            None,
        ),
    )
    .await;

    spawn_single_chapter_generation(
        db.clone(),
        task_id.clone(),
        claims.sub.clone(),
        chapter_model.id.clone(),
        load_chapter_generation_target(&body),
        ai_config,
    );

    Ok(Json(json!({
        "task_id": task_id,
        "chapter_id": chapter_model.id,
        "status": "pending",
        "message": "单章后台生成任务已创建",
        "estimated_time_minutes": 2,
        "active_story_repair_payload": null,
    })))
}

async fn generate_chapter_content_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ChapterGenerateRequest>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let chapter_model = load_accessible_chapter_for_generation(&db, &chapter_id, &claims.sub).await?;
    let ai_config = build_user_ai_config(&db, &claims.sub, body.model.as_deref())
        .await
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": error})),
            )
        })?;
    let target_word_count = load_chapter_generation_target(&body);
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(32);
    let db_clone = db.clone();
    let user_id = claims.sub.clone();
    let chapter_id_clone = chapter_model.id.clone();

    tokio::spawn(async move {
        let mut tracker = crate::utils::sse::SseProgress::new("Chapter Generation");
        let _ = tx.send(Ok(tracker.start())).await;
        let _ = tx
            .send(Ok(tracker.preparing(Some("Preparing chapter generation..."))))
            .await;
        let _ = tx
            .send(Ok(tracker.generating(
                Some("Generating chapter content..."),
                (15, 95),
                target_word_count as usize,
                None,
            )))
            .await;

        let ai_service = AIService::new(ai_config);
        match ChapterGenerationService::generate_and_persist_chapter_content(
            &db_clone,
            &ai_service,
            &user_id,
            &chapter_id_clone,
            target_word_count,
        )
        .await
        {
            Ok(payload) => {
                let _ = tx.send(Ok(tracker.complete(Some("Generation complete")))).await;
                let _ = tx.send(Ok(crate::utils::sse::sse_result(&payload))).await;
                let _ = tx.send(Ok(crate::utils::sse::sse_done())).await;
            }
            Err(error) => {
                let _ = tx.send(Ok(crate::utils::sse::sse_error(&error, 500))).await;
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::new().interval(Duration::from_secs(10))))
}

async fn get_batch_generation_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let task = load_owned_task(&db, &batch_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?;
    let Some(task) = task else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Batch generation task not found"})),
        ));
    };
    let snapshot = load_batch_generation_snapshot(&db, &task.id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?;
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.clone());
    let active_story_repair_payload =
        active_story_repair_payload_from_runtime_state(workflow_runtime_state.as_ref());
    let latest_quality_metrics = snapshot
        .as_ref()
        .and_then(|item| item.latest_quality_metrics.clone());
    let quality_metrics_summary = snapshot
        .as_ref()
        .and_then(|item| item.quality_metrics_summary.clone());

    Ok(Json(task_status_payload(
        &task,
        workflow_runtime_state,
        latest_quality_metrics,
        quality_metrics_summary,
        active_story_repair_payload,
    )))
}

async fn stream_batch_generation_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    if load_owned_task(&db, &batch_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
        .is_none()
    {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Batch generation task not found"})),
        ));
    }

    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(32);
    let db_clone = db.clone();
    let batch_id_clone = batch_id.clone();
    let user_id = claims.sub.clone();

    tokio::spawn(async move {
        let mut last_status = String::new();
        let mut last_completed = -1;
        let mut last_progress = -1;
        let mut last_message = String::new();

        for _ in 0..300 {
            let task = match load_owned_task(&db_clone, &batch_id_clone, &user_id).await {
                Ok(Some(task)) => task,
                _ => {
                    let _ = tx
                        .send(Ok(Event::default().data(
                            json!({"type":"error","error":"Batch generation task not found","code":404}).to_string(),
                        )))
                        .await;
                    return;
                }
            };

            let status = task.status.clone().unwrap_or_else(|| "pending".to_string());
            let completed = task.completed_chapters.unwrap_or(0);
            let snapshot = load_batch_generation_snapshot(&db_clone, &task.id).await.ok().flatten();
            let checkpoint = snapshot
                .as_ref()
                .and_then(|item| item.workflow_runtime_state.as_ref());
            let progress = checkpoint
                .and_then(|item| item.get("progress"))
                .and_then(Value::as_i64)
                .map(|value| value.clamp(0, 100) as i32)
                .unwrap_or_else(|| match status.as_str() {
                    "pending" => 10,
                    "running" => 65,
                    "completed" => 100,
                    "failed" => 100,
                    "cancelled" => 100,
                    _ => 15,
                });
            let message = checkpoint
                .and_then(|item| item.get("last_message"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(match status.as_str() {
                    "pending" => "等待开始生成...",
                    "running" => "正在生成正文...",
                    "completed" => "生成完成",
                    "failed" => "生成失败",
                    "cancelled" => "生成已取消",
                    _ => "任务处理中",
                });

            if status != last_status
                || completed != last_completed
                || progress != last_progress
                || message != last_message
            {
                let progress_event = json!({
                    "type": "progress",
                    "message": message,
                    "progress": progress,
                    "status": if status == "failed" { "error" } else if status == "completed" { "success" } else { "processing" },
                });
                let _ = tx
                    .send(Ok(Event::default().data(progress_event.to_string())))
                    .await;

                if status == "completed" {
                    let result_event = json!({
                        "type": "result",
                        "data": {
                            "generation_task_id": task.id,
                            "chapter_id": task.current_chapter_id,
                            "content_source": "chapter",
                        }
                    });
                    let _ = tx
                        .send(Ok(Event::default().data(result_event.to_string())))
                        .await;
                    let _ = tx
                        .send(Ok(Event::default().data(json!({"type":"done"}).to_string())))
                        .await;
                    return;
                }

                if status == "failed" {
                    let _ = tx
                        .send(Ok(Event::default().data(
                            json!({
                                "type":"error",
                                "error": task.error_message.unwrap_or_else(|| "Generation task failed.".to_string()),
                                "code": 500
                            })
                            .to_string(),
                        )))
                        .await;
                    return;
                }

                if status == "cancelled" {
                    let _ = tx
                        .send(Ok(Event::default().data(
                            json!({
                                "type":"error",
                                "error":"Generation task was cancelled.",
                                "code": 499
                            })
                            .to_string(),
                        )))
                        .await;
                    return;
                }

                last_status = status;
                last_completed = completed;
                last_progress = progress;
                last_message = message.to_string();
            }

            sleep(Duration::from_secs(1)).await;
        }

        let _ = tx
            .send(Ok(Event::default().data(
                json!({"type":"error","error":"Generation stream timed out.","code":408}).to_string(),
            )))
            .await;
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(
        KeepAlive::new().interval(Duration::from_secs(10)).text("keep-alive"),
    ))
}

async fn get_active_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !verify_project_access(&db, &project_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
    {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found or access denied"})),
        ));
    }

    let task = batch_generation_task::Entity::find()
        .filter(batch_generation_task::Column::ProjectId.eq(&project_id))
        .filter(batch_generation_task::Column::UserId.eq(&claims.sub))
        .filter(batch_generation_task::Column::Status.is_in(["pending", "running"]))
        .order_by_desc(batch_generation_task::Column::CreatedAt)
        .one(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;
    let Some(task) = task else {
        return Ok(Json(json!({"has_active_task": false, "task": null})));
    };
    let snapshot = load_batch_generation_snapshot(&db, &task.id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?;
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.clone());
    let active_story_repair_payload =
        active_story_repair_payload_from_runtime_state(workflow_runtime_state.as_ref());
    let latest_quality_metrics = snapshot
        .as_ref()
        .and_then(|item| item.latest_quality_metrics.clone());
    let quality_metrics_summary = snapshot
        .as_ref()
        .and_then(|item| item.quality_metrics_summary.clone());

    Ok(Json(json!({
        "has_active_task": true,
        "task": active_task_payload(
            &task,
            workflow_runtime_state,
            latest_quality_metrics,
            quality_metrics_summary,
            active_story_repair_payload,
        ),
    })))
}

async fn list_active_batch_generation_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ActiveQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let tasks = batch_generation_task::Entity::find()
        .filter(batch_generation_task::Column::UserId.eq(&claims.sub))
        .filter(batch_generation_task::Column::Status.is_in(["pending", "running"]))
        .order_by_desc(batch_generation_task::Column::CreatedAt)
        .limit(limit)
        .all(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    let mut items = Vec::with_capacity(tasks.len());
    for task in &tasks {
        let snapshot = load_batch_generation_snapshot(&db, &task.id)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": error})),
                )
            })?;
        let workflow_runtime_state = snapshot
            .as_ref()
            .and_then(|item| item.workflow_runtime_state.clone());
        let active_story_repair_payload =
            active_story_repair_payload_from_runtime_state(workflow_runtime_state.as_ref());
        let latest_quality_metrics = snapshot
            .as_ref()
            .and_then(|item| item.latest_quality_metrics.clone());
        let quality_metrics_summary = snapshot
            .as_ref()
            .and_then(|item| item.quality_metrics_summary.clone());

        items.push(active_task_payload(
            task,
            workflow_runtime_state,
            latest_quality_metrics,
            quality_metrics_summary,
            active_story_repair_payload,
        ));
    }
    Ok(Json(json!({"total": items.len(), "items": items})))
}

async fn cancel_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let task = load_owned_task(&db, &batch_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?;
    let Some(task) = task else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Batch generation task not found"})),
        ));
    };

    if matches!(task.status.as_deref(), Some("completed" | "failed" | "cancelled")) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("Cannot cancel task in status {}", task.status.unwrap_or_default())})),
        ));
    }

    let mut active: batch_generation_task::ActiveModel = task.clone().into();
    active.status = Set(Some("cancelled".to_string()));
    active.completed_at = Set(Some(Utc::now().naive_utc()));
    active.update(&db).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error.to_string()})),
        )
    })?;

    Ok(Json(json!({
        "message": "Batch generation cancelled",
        "batch_id": batch_id,
        "completed_chapters": task.completed_chapters.unwrap_or(0),
        "total_chapters": task.total_chapters.unwrap_or(task.chapter_count),
    })))
}

async fn resume_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let task = load_owned_task(&db, &batch_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?;
    let Some(task) = task else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Batch generation task not found"})),
        ));
    };

    let mut active: batch_generation_task::ActiveModel = task.clone().into();
    active.status = Set(Some("pending".to_string()));
    active.error_message = Set(None);
    active.completed_at = Set(None);
    active.started_at = Set(None);
    active.completed_chapters = Set(Some(0));
    active.current_retry_count = Set(Some(0));
    let updated = active.update(&db).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error.to_string()})),
        )
    })?;

    if task_type(&task) == "chapter_single_generate" {
        if let Some(chapter_id) = task.current_chapter_id.clone() {
            let ai_config = match build_user_ai_config(&db, &claims.sub, None).await {
                Ok(config) => config,
                Err(error) => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({"detail": error})),
                    ));
                }
            };
            spawn_single_chapter_generation(
                db.clone(),
                batch_id.clone(),
                claims.sub.clone(),
                chapter_id,
                task.target_word_count.unwrap_or(3000),
                ai_config,
            );
        }
    } else {
        let chapter_ids = parse_batch_task_chapter_ids(&task);
        if chapter_ids.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "Batch generation task has no chapters to resume"})),
            ));
        }
        let ai_config = match build_user_ai_config(&db, &claims.sub, None).await {
            Ok(config) => config,
            Err(error) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": error})),
                ));
            }
        };
        spawn_batch_generation(
            db.clone(),
            batch_id.clone(),
            claims.sub.clone(),
            chapter_ids,
            task.target_word_count.unwrap_or(3000).max(1),
            ai_config,
        );
    }

    Ok(Json(json!({
        "message": "Batch generation resumed",
        "batch_id": batch_id,
        "project_id": updated.project_id,
        "task_type": task_type(&updated),
        "status": "pending",
        "stage_code": task_stage_code(&updated),
        "execution_mode": task_execution_mode(&updated),
        "current_chapter_id": updated.current_chapter_id,
        "checkpoint": {
            "stage_code": task_stage_code(&updated),
            "execution_mode": task_execution_mode(&updated),
            "chapter_id": updated.current_chapter_id,
        },
        "total_chapters": updated.total_chapters.unwrap_or(updated.chapter_count),
        "completed_chapters": 0,
        "created_at": to_iso(updated.created_at),
    })))
}

pub fn routes() -> Router {
    Router::new()
        .route(
            "/chapters/project/{project_id}/batch-generate",
            post(create_batch_generate),
        )
        .route(
            "/chapters/{chapter_id}/generate-stream",
            post(generate_chapter_content_stream),
        )
        .route(
            "/chapters/{chapter_id}/generate-background",
            post(generate_chapter_content_background),
        )
        .route(
            "/chapters/batch-generate/{batch_id}/status",
            get(get_batch_generation_status),
        )
        .route(
            "/chapters/batch-generate/{batch_id}/stream",
            get(stream_batch_generation_status),
        )
        .route(
            "/chapters/project/{project_id}/batch-generate/active",
            get(get_active_batch_generation),
        )
        .route(
            "/chapters/batch-generate/active-tasks",
            get(list_active_batch_generation_tasks),
        )
        .route(
            "/chapters/batch-generate/{batch_id}/cancel",
            post(cancel_batch_generation),
        )
        .route(
            "/chapters/batch-generate/{batch_id}/resume",
            post(resume_batch_generation),
        )
}
