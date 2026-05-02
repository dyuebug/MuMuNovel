use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Sse},
    Json,
};
use chrono::Utc;
use futures::StreamExt;
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::services::auth::Claims;
use crate::tasks::checkpoint::touch_checkpoint;
use crate::tasks::registry::TaskRegistry;
use crate::tasks::stream::TaskStreamHub;
use crate::tasks::types::{
    TaskCreateRequest, TaskEvent, TaskListQuery, TaskRecord, TaskStatus, TaskWorkflowUpdate,
};

/// POST /api/background-tasks
pub async fn create_task(
    Extension(claims): Extension<Claims>,
    Extension(registry): Extension<TaskRegistry>,
    Json(req): Json<TaskCreateRequest>,
) -> impl IntoResponse {
    let task_id = Uuid::new_v4().to_string();

    let fingerprint = req.payload.as_ref().map(|p| {
        let json_str = serde_json::to_string(p).unwrap_or_default();
        format!("{:x}", md5::compute(json_str.as_bytes()))
    });

    // Deduplication: check for existing active task with same fingerprint
    if let Some(ref fp) = fingerprint {
        if let Some(existing) = registry
            .find_active(&claims.sub, &req.task_type, &req.project_id, Some(fp))
            .await
        {
            return (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "相同任务已在执行中",
                    "task_id": existing.task_id,
                    "data": existing,
                })),
            )
                .into_response();
        }
    }

    let mut record = TaskRecord::new(
        task_id.clone(),
        req.task_type,
        claims.sub.clone(),
        req.project_id,
        req.execution_mode,
    );

    record.stage_code = req.stage_code;
    record.workflow_scope = req.workflow_scope;
    record.payload_fingerprint = fingerprint;
    if let Some(cp) = req.checkpoint {
        record.checkpoint = Some(cp);
    }

    registry.insert(record.clone()).await;

    (
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "data": record,
        })),
    )
        .into_response()
}

/// GET /api/background-tasks
pub async fn list_tasks(
    Extension(claims): Extension<Claims>,
    Extension(registry): Extension<TaskRegistry>,
    Query(query): Query<TaskListQuery>,
) -> impl IntoResponse {
    let statuses: Option<Vec<TaskStatus>> = query.statuses.as_ref().map(|s| {
        s.split(',')
            .filter_map(|part| match part.trim() {
                "pending" => Some(TaskStatus::Pending),
                "running" => Some(TaskStatus::Running),
                "completed" => Some(TaskStatus::Completed),
                "failed" => Some(TaskStatus::Failed),
                "cancelled" => Some(TaskStatus::Cancelled),
                _ => None,
            })
            .collect()
    });

    let tasks = registry
        .list_for_user(
            &claims.sub,
            query.project_id.as_deref(),
            statuses.as_deref(),
            query.active_only.unwrap_or(false),
            query.limit,
        )
        .await;

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": tasks,
            "total": tasks.len(),
        })),
    )
        .into_response()
}

/// GET /api/background-tasks/:task_id
pub async fn get_task(
    Extension(claims): Extension<Claims>,
    Extension(registry): Extension<TaskRegistry>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    match registry.get(&task_id).await {
        Some(record) => {
            if record.user_id != claims.sub {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"success": false, "message": "无权限访问此任务"})),
                )
                    .into_response();
            }
            (
                StatusCode::OK,
                Json(json!({"success": true, "data": record})),
            )
                .into_response()
        }
        None => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "任务不存在",
                "data": serde_json::json!({
                    "task_id": task_id,
                    "status": "cancelled",
                }),
            })),
        )
            .into_response()
    }
}

/// POST /api/background-tasks/:task_id/cancel
pub async fn cancel_task(
    Extension(claims): Extension<Claims>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    match registry.get(&task_id).await {
        Some(record) if record.user_id == claims.sub && record.status.is_active() => {
            let new_cp = touch_checkpoint(
                record.checkpoint.as_ref(),
                "cancelled",
                Some(record.progress),
                Some("任务已取消"),
                Some(&json!({"error": "用户取消"})),
            );

            registry
                .update(&task_id, |r| {
                    r.status = TaskStatus::Cancelled;
                    r.message = "任务已取消".into();
                    r.completed_at = Some(Utc::now());
                    r.checkpoint = Some(new_cp);
                })
                .await;

            let event = TaskEvent {
                event_type: "cancelled".into(),
                task_id: Some(task_id.clone()),
                message: Some("任务已取消".into()),
                progress: None,
                status: Some("cancelled".into()),
                data: None,
                error: None,
            };
            stream_hub.fanout(&task_id, &event);

            (
                StatusCode::OK,
                Json(json!({"success": true, "message": "任务已取消"})),
            )
                .into_response()
        }
        Some(_) => (
            StatusCode::OK,
            Json(json!({"success": false, "message": "任务不存在或已完成"})),
        )
            .into_response(),
        None => (
            StatusCode::OK,
            Json(json!({"success": false, "message": "任务不存在"})),
        )
            .into_response(),
    }
}

/// PATCH /api/background-tasks/:task_id/workflow-state
pub async fn update_workflow_state(
    Extension(claims): Extension<Claims>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Path(task_id): Path<String>,
    Json(update): Json<TaskWorkflowUpdate>,
) -> impl IntoResponse {
    match registry.get(&task_id).await {
        Some(record) if record.user_id == claims.sub => {
            registry
                .update(&task_id, |r| {
                    if let Some(sc) = &update.stage_code {
                        r.stage_code = Some(sc.clone());
                    }
                    if let Some(em) = &update.execution_mode {
                        r.execution_mode = em.clone();
                    }
                    if let Some(ws) = &update.workflow_scope {
                        r.workflow_scope = Some(ws.clone());
                    }
                    if let Some(msg) = &update.message {
                        r.message = msg.clone();
                    }
                    if let Some(prog) = update.progress {
                        r.progress = prog.clamp(0, 100);
                    }
                    if let Some(cp) = &update.checkpoint {
                        r.checkpoint = Some(cp.clone());
                    }
                    r.updated_at = Utc::now();
                })
                .await;

            if let Some(updated) = registry.get(&task_id).await {
                let event = TaskEvent {
                    event_type: "progress".into(),
                    task_id: Some(task_id.clone()),
                    message: Some(updated.message.clone()),
                    progress: Some(updated.progress),
                    status: Some(updated.status.to_string()),
                    data: None,
                    error: None,
                };
                stream_hub.fanout(&task_id, &event);

                return (
                    StatusCode::OK,
                    Json(json!({"success": true, "data": updated})),
                )
                    .into_response();
            }

            (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "任务不存在"})),
            )
                .into_response()
        }
        Some(_) => (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "无权限修改此任务"})),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "任务不存在"})),
        )
            .into_response(),
    }
}

/// GET /api/background-tasks/:task_id/stream — SSE real-time progress stream
pub async fn stream_task(
    Extension(claims): Extension<Claims>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    // Verify task exists and belongs to user, capture current state
    let record = match registry.get(&task_id).await {
        Some(r) if r.user_id != claims.sub => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"success": false, "message": "无权限访问此任务"})),
            )
                .into_response();
        }
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "任务不存在"})),
            )
                .into_response();
        }
        Some(r) => r,
    };

    // Build initial "connected" event with full task state
    let record_json = serde_json::to_value(&record).unwrap_or_default();
    let status_event = TaskEvent {
        event_type: "connected".into(),
        task_id: Some(task_id.clone()),
        message: Some(record.message),
        progress: Some(record.progress),
        status: Some(record.status.to_string()),
        data: Some(record_json),
        error: None,
    };
    let initial_json = serde_json::to_string(&status_event).unwrap_or_default();

    let rx = stream_hub.subscribe(&task_id).await;
    let events = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(data) => Some(Ok::<_, std::convert::Infallible>(
                axum::response::sse::Event::default().data(data),
            )),
            Err(_) => None,
        }
    });
    let init = tokio_stream::once(Ok::<_, std::convert::Infallible>(
        axum::response::sse::Event::default().data(initial_json),
    ));

    Sse::new(init.chain(events))
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(std::time::Duration::from_secs(10))
                .text("ping"),
        )
        .into_response()
}

pub fn routes() -> axum::Router {
    use axum::routing::{get, patch, post};

    axum::Router::new()
        .route("/background-tasks", post(create_task).get(list_tasks))
        .route("/background-tasks/{task_id}", get(get_task))
        .route("/background-tasks/{task_id}/stream", get(stream_task))
        .route("/background-tasks/{task_id}/cancel", post(cancel_task))
        .route(
            "/background-tasks/{task_id}/workflow-state",
            patch(update_workflow_state),
        )
}
