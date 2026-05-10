use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use chrono::{Duration, NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::{
    analysis_task, chapter_draft_attempt, generation_history, plot_analysis, story_memory,
};
use crate::services::auth::Claims;
use crate::services::chapter_service::ChapterService;

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

fn bool_from_int(value: i32) -> bool {
    value != 0
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

    let latest_reviser: Option<(&generation_history::Model, Value)> =
        histories.iter().find_map(|history| {
            history.generated_content.as_ref().and_then(|content| {
                serde_json::from_str::<Value>(content)
                    .ok()
                    .and_then(|payload| {
                        if payload.get("log_type").and_then(Value::as_str)
                            == Some("chapter_text_reviser_v1")
                        {
                            Some((history, payload))
                        } else {
                            None
                        }
                    })
            })
        });

    let auto_revision_draft = latest_reviser.map(|(history, payload)| {
        let reviser_result = payload
            .get("reviser_result")
            .cloned()
            .unwrap_or(Value::Null);
        let revised_text = reviser_result
            .get("revised_text")
            .cloned()
            .unwrap_or(Value::Null);
        let revised_text_preview = reviser_result
            .get("revised_text_preview")
            .cloned()
            .unwrap_or(Value::Null);
        let content_preview = revised_text_preview
            .as_str()
            .filter(|text: &&str| !text.trim().is_empty())
            .map(|text: &str| Value::String(text.to_string()))
            .unwrap_or_else(|| {
                revised_text
                    .as_str()
                    .map(|text: &str| Value::String(text.chars().take(500).collect()))
                    .unwrap_or(Value::Null)
            });

        json!({
            "history_id": history.id,
            "source": "history",
            "revised_text": revised_text,
            "revised_text_preview": revised_text_preview,
            "content_preview": content_preview,
            "created_at": datetime_to_string(history.created_at),
            "can_apply": true,
            "has_full_content": revised_text
                .as_str()
                .map(|text: &str| !text.trim().is_empty())
                .unwrap_or(false),
            "content_complete": revised_text
                .as_str()
                .map(|text: &str| !text.trim().is_empty())
                .unwrap_or(false),
        })
    });

    let candidate_draft = candidate_attempt.as_ref().map(|attempt| {
        let content_complete = attempt
            .content_preview
            .as_ref()
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false);
        let is_stale = match (chapter.updated_at, attempt.created_at) {
            (Some(chapter_updated_at), Some(attempt_created_at)) => {
                chapter_updated_at > attempt_created_at
            }
            _ => false,
        };

        json!({
            "attempt_id": attempt.id,
            "source": attempt.source,
            "attempt_state": attempt.attempt_state,
            "quality_gate_action": attempt.quality_gate_action,
            "quality_gate_decision": attempt.quality_gate_decision,
            "word_count": attempt.word_count,
            "summary_preview": attempt.summary_preview,
            "content_preview": attempt.content_preview,
            "quality_metrics": attempt.quality_metrics.clone().unwrap_or(Value::Null),
            "repair_payload": attempt.repair_payload.clone().unwrap_or(Value::Null),
            "created_at": datetime_to_string(attempt.created_at),
            "has_full_content": content_complete,
            "content_complete": content_complete,
            "can_apply": content_complete,
            "is_stale": is_stale,
        })
    });

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

async fn get_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _history_id = query.get("history_id").cloned();
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

    let auto_revision_draft = histories.iter().find_map(|history| {
        history.generated_content.as_ref().and_then(|content| {
            serde_json::from_str::<Value>(content)
                .ok()
                .and_then(|payload| {
                    if payload.get("log_type").and_then(Value::as_str)
                        == Some("chapter_text_reviser_v1")
                    {
                        let reviser_result = payload
                            .get("reviser_result")
                            .cloned()
                            .unwrap_or(Value::Null);
                        let revised_text = reviser_result
                            .get("revised_text")
                            .cloned()
                            .unwrap_or(Value::Null);
                        let revised_text_preview = reviser_result
                            .get("revised_text_preview")
                            .cloned()
                            .unwrap_or(Value::Null);
                        let content_preview = revised_text_preview
                            .as_str()
                            .filter(|text: &&str| !text.trim().is_empty())
                            .map(|text: &str| Value::String(text.to_string()))
                            .unwrap_or_else(|| {
                                revised_text
                                    .as_str()
                                    .map(|text: &str| {
                                        Value::String(text.chars().take(500).collect())
                                    })
                                    .unwrap_or(Value::Null)
                            });

                        Some(json!({
                            "history_id": history.id,
                            "source": "history",
                            "revised_text": revised_text,
                            "revised_text_preview": revised_text_preview,
                            "content_preview": content_preview,
                            "created_at": datetime_to_string(history.created_at),
                            "can_apply": true,
                            "has_full_content": revised_text
                                .as_str()
                                .map(|text: &str| !text.trim().is_empty())
                                .unwrap_or(false),
                            "content_complete": revised_text
                                .as_str()
                                .map(|text: &str| !text.trim().is_empty())
                                .unwrap_or(false),
                        }))
                    } else {
                        None
                    }
                })
        })
    });

    Ok(Json(json!({
        "chapter_id": chapter.id,
        "auto_revision_draft": auto_revision_draft,
        "analysis_created_at": datetime_to_string(analysis.created_at),
    })))
}

async fn apply_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::get(&db, &chapter_id, &claims.sub).await {
        Ok(Some(chapter)) => Ok(Json(json!({
            "success": true,
            "chapter_id": chapter.id,
            "word_count": chapter.word_count,
            "old_word_count": chapter.word_count,
            "draft_history_id": null,
            "draft_created_at": null,
            "stale_applied": false,
            "message": "Auto revision draft application is not implemented yet",
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

async fn get_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(_query): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::get(&db, &chapter_id, &claims.sub).await {
        Ok(Some(chapter)) => Ok(Json(json!({
            "chapter_id": chapter.id,
            "candidate_draft": null,
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

async fn apply_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::get(&db, &chapter_id, &claims.sub).await {
        Ok(Some(chapter)) => Ok(Json(json!({
            "success": true,
            "chapter_id": chapter.id,
            "word_count": chapter.word_count,
            "old_word_count": chapter.word_count,
            "draft_attempt_id": null,
            "draft_created_at": null,
            "stale_applied": false,
            "message": "Candidate draft application is not implemented yet",
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
        Ok(Some(chapter)) => Ok((
            StatusCode::CREATED,
            Json(json!({"success": true, "data": chapter})),
        )),
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
        Ok(Some(chapter)) => Ok(Json(json!({"success": true, "data": chapter}))),
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
        Ok(Some(chapter)) => Ok(Json(json!({"success": true, "data": chapter}))),
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
        Ok(Some(chapter)) => Ok(Json(json!({"success": true, "data": chapter}))),
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
        .route("/chapters/{chapter_id}/analysis", get(get_chapter_analysis))
        .route(
            "/chapters/{chapter_id}/analysis/status",
            get(get_analysis_task_status),
        )
        .route(
            "/chapters/analysis/status/batch",
            axum::routing::post(get_batch_analysis_task_status),
        )
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
            "/chapters",
            axum::routing::get(list_chapters).post(create_chapter),
        )
        .route(
            "/chapters/{chapter_id}",
            get(get_chapter).put(update_chapter).delete(delete_chapter),
        )
}
