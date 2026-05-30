use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Sse},
    Json,
};
use chrono::Utc;
use futures::StreamExt;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::services::auth::Claims;
use crate::services::background_task_payload_adapter_service::{
    build_connected_task_event, build_missing_task_payload, build_task_list_response,
    compatible_task_payload, enrich_task_payload,
};
use crate::services::background_task_request_service::normalize_task_statuses_query;
use crate::services::generation_task_request_adapter_service::{
    adapt_character_generation_task_request, adapt_organization_generation_task_request,
};
use crate::services::outline_expansion_request_service::{
    build_outline_batch_expand_execution_request, build_outline_expand_execution_request,
    execute_outline_batch_expand_request, execute_outline_expand_request,
};
use crate::services::wizard_request_service::{
    execute_career_system_request, execute_characters_request, execute_outline_request,
    execute_regenerate_world_building_request, execute_world_building_request, CareerSystemRequest,
    CharactersRequest, OutlineRequest, RegenerateWorldBuildingRequest, WorldBuildingRequest,
};
use crate::tasks::checkpoint::touch_checkpoint;
use crate::tasks::registry::TaskRegistry;
use crate::tasks::stream::TaskStreamHub;
use crate::tasks::types::{
    TaskCreateRequest, TaskEvent, TaskListQuery, TaskRecord, TaskStatus, TaskWorkflowUpdate,
};
use crate::utils::sse::{SseChannel, SseTaskCapture};

/// POST /api/background-tasks
pub async fn create_task(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Json(req): Json<TaskCreateRequest>,
) -> impl IntoResponse {
    if req.task_type != "wizard_world_building" && req.project_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "project_id is required for this task type"})),
        )
            .into_response();
    }

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
                Json({
                    let mut payload = compatible_task_payload(&existing);
                    if let Some(map) = payload.as_object_mut() {
                        map.insert("message".to_string(), json!("相同任务已在执行中"));
                    }
                    payload
                }),
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
    let payload = enrich_task_payload(&record, req.payload.unwrap_or_else(|| json!({})));
    spawn_task_execution(
        db,
        registry.clone(),
        stream_hub.clone(),
        record.clone(),
        payload,
    );

    (StatusCode::CREATED, Json(compatible_task_payload(&record))).into_response()
}

fn spawn_task_execution(
    db: DatabaseConnection,
    registry: TaskRegistry,
    stream_hub: TaskStreamHub,
    record: TaskRecord,
    payload: serde_json::Value,
) {
    tokio::spawn(async move {
        mark_task_running(&registry, &stream_hub, &record.task_id, "任务已开始执行").await;
        let result = execute_task(&db, &registry, &stream_hub, &record, payload).await;

        if let Err(error) = result {
            fail_task(&registry, &stream_hub, &record.task_id, &error).await;
        }
    });
}

async fn mark_task_running(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    message: &str,
) {
    let updated = registry
        .update(task_id, |record| {
            record.status = TaskStatus::Running;
            record.started_at.get_or_insert_with(Utc::now);
            record.updated_at = Utc::now();
            record.message = message.to_string();
            if record.progress <= 0 {
                record.progress = 1;
            }
        })
        .await;

    if let Some(record) = updated {
        stream_hub.fanout(
            task_id,
            &TaskEvent {
                event_type: "progress".into(),
                task_id: Some(task_id.to_string()),
                message: Some(record.message),
                progress: Some(record.progress),
                status: Some(record.status.to_string()),
                data: None,
                error: None,
            },
        );
    }
}

async fn complete_task(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    result: serde_json::Value,
    message: Option<String>,
) {
    let updated = registry
        .update(task_id, |record| {
            record.status = TaskStatus::Completed;
            record.progress = 100;
            record.result = Some(result.clone());
            record.error = None;
            record.completed_at = Some(Utc::now());
            record.updated_at = Utc::now();
            record.message = message.unwrap_or_else(|| "任务执行完成".to_string());
        })
        .await;

    if let Some(record) = updated {
        stream_hub.fanout(
            task_id,
            &TaskEvent {
                event_type: "result".into(),
                task_id: Some(task_id.to_string()),
                message: Some(record.message.clone()),
                progress: Some(record.progress),
                status: Some(record.status.to_string()),
                data: record.result.clone(),
                error: None,
            },
        );
        stream_hub.fanout(
            task_id,
            &TaskEvent {
                event_type: "done".into(),
                task_id: Some(task_id.to_string()),
                message: Some(record.message),
                progress: Some(100),
                status: Some("completed".into()),
                data: record.result,
                error: None,
            },
        );
    }
}

async fn fail_task(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    error: &str,
) {
    let updated = registry
        .update(task_id, |record| {
            record.status = TaskStatus::Failed;
            record.error = Some(error.to_string());
            record.completed_at = Some(Utc::now());
            record.updated_at = Utc::now();
            record.message = error.to_string();
        })
        .await;

    if let Some(record) = updated {
        stream_hub.fanout(
            task_id,
            &TaskEvent {
                event_type: "error".into(),
                task_id: Some(task_id.to_string()),
                message: Some(record.message.clone()),
                progress: Some(record.progress),
                status: Some(record.status.to_string()),
                data: None,
                error: record.error,
            },
        );
    }
}

async fn sync_channel_state_to_task(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    state_capture: Arc<Mutex<SseTaskCapture>>,
    result_capture: Arc<Mutex<Option<serde_json::Value>>>,
) -> Result<serde_json::Value, String> {
    let state = state_capture.lock().await.clone();
    let result = result_capture.lock().await.clone().or(state.result.clone());

    if let Some(error) = state.error {
        return Err(error);
    }

    let updated = registry
        .update(task_id, |record| {
            if let Some(message) = &state.message {
                record.message = message.clone();
            }
            if let Some(progress) = state.progress {
                record.progress = progress as i32;
            }
            if let Some(status) = &state.status {
                if status == "success" {
                    record.status = TaskStatus::Completed;
                }
            }
            record.updated_at = Utc::now();
        })
        .await;

    if let Some(record) = updated {
        stream_hub.fanout(
            task_id,
            &TaskEvent {
                event_type: "progress".into(),
                task_id: Some(task_id.to_string()),
                message: Some(record.message),
                progress: Some(record.progress),
                status: Some(record.status.to_string()),
                data: None,
                error: None,
            },
        );
    }

    result.ok_or_else(|| "后台任务未返回结果".to_string())
}

async fn execute_task(
    db: &DatabaseConnection,
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<(), String> {
    match record.task_type.as_str() {
        "wizard_world_building" => {
            let result =
                run_wizard_world_building(db, registry, stream_hub, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("世界观生成完成".to_string()),
            )
            .await;
        }
        "wizard_career_system" | "careers_generate_system" => {
            let result =
                run_wizard_career_system(db, registry, stream_hub, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("职业体系生成完成".to_string()),
            )
            .await;
        }
        "wizard_characters" => {
            let result = run_wizard_characters(db, registry, stream_hub, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("角色生成完成".to_string()),
            )
            .await;
        }
        "wizard_outline" | "outline_generate" => {
            let result = run_wizard_outline(db, registry, stream_hub, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("大纲生成完成".to_string()),
            )
            .await;
        }
        "world_regenerate" => {
            let result = run_world_regenerate(db, registry, stream_hub, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("世界观重生成完成".to_string()),
            )
            .await;
        }
        "outline_expand" => {
            let result = run_outline_expand(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("大纲展开完成".to_string()),
            )
            .await;
        }
        "outline_batch_expand" => {
            let result = run_outline_batch_expand(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("批量展开完成".to_string()),
            )
            .await;
        }
        "character_generate" => {
            let result = run_character_generate(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("角色生成完成".to_string()),
            )
            .await;
        }
        "organization_generate" => {
            let result = run_organization_generate(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("组织生成完成".to_string()),
            )
            .await;
        }
        other => {
            return Err(format!("unsupported task type: {}", other));
        }
    }

    Ok(())
}

async fn run_wizard_world_building(
    db: &DatabaseConnection,
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<WorldBuildingRequest>(payload)
        .map_err(|error| format!("无效的世界观任务参数: {}", error))?;
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());

    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_world_building_request(db, &channel, &record.user_id, body).await;

    drop(channel);
    let _ = drain_handle.await;
    sync_channel_state_to_task(
        registry,
        stream_hub,
        &record.task_id,
        state_capture,
        result_capture,
    )
    .await
}

async fn run_wizard_career_system(
    db: &DatabaseConnection,
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<CareerSystemRequest>(payload)
        .map_err(|error| format!("无效的职业体系任务参数: {}", error))?;
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_career_system_request(db, &channel, &record.user_id, body).await;

    drop(channel);
    let _ = drain_handle.await;
    sync_channel_state_to_task(
        registry,
        stream_hub,
        &record.task_id,
        state_capture,
        result_capture,
    )
    .await
}

async fn run_wizard_characters(
    db: &DatabaseConnection,
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<CharactersRequest>(payload)
        .map_err(|error| format!("无效的角色任务参数: {}", error))?;
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_characters_request(db, &channel, &record.user_id, body).await;

    drop(channel);
    let _ = drain_handle.await;
    sync_channel_state_to_task(
        registry,
        stream_hub,
        &record.task_id,
        state_capture,
        result_capture,
    )
    .await
}

async fn run_wizard_outline(
    db: &DatabaseConnection,
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<OutlineRequest>(payload)
        .map_err(|error| format!("无效的大纲任务参数: {}", error))?;
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_outline_request(db, &channel, &record.user_id, body).await;

    drop(channel);
    let _ = drain_handle.await;
    sync_channel_state_to_task(
        registry,
        stream_hub,
        &record.task_id,
        state_capture,
        result_capture,
    )
    .await
}

async fn run_world_regenerate(
    db: &DatabaseConnection,
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body =
        serde_json::from_value::<RegenerateWorldBuildingRequest>(payload).unwrap_or_default();
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_regenerate_world_building_request(
        db,
        &channel,
        &record.user_id,
        &record.project_id,
        body,
    )
    .await;

    drop(channel);
    let _ = drain_handle.await;
    sync_channel_state_to_task(
        registry,
        stream_hub,
        &record.task_id,
        state_capture,
        result_capture,
    )
    .await
}

async fn run_outline_expand(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let outline_id = payload
        .get("outline_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "outline_id is required for outline_expand".to_string())?;
    let request = build_outline_expand_execution_request(outline_id.to_string(), &payload);
    execute_outline_expand_request(db, &record.user_id, &request).await
}

async fn run_outline_batch_expand(
    db: &DatabaseConnection,
    record: &TaskRecord,
    mut payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    if payload.get("project_id").is_none() {
        payload["project_id"] = serde_json::Value::String(record.project_id.clone());
    }
    let request = build_outline_batch_expand_execution_request(&payload);
    execute_outline_batch_expand_request(db, &record.user_id, &request).await
}

async fn run_character_generate(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = adapt_character_generation_task_request(&record.project_id, &payload)?;

    super::characters::generate_character_task(db, &record.user_id, body)
        .await
        .map_err(|error| error.to_string())
}

async fn run_organization_generate(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = adapt_organization_generation_task_request(&record.project_id, &payload)?;

    super::organizations::generate_organization_task(db, &record.user_id, body)
        .await
        .map_err(|error| error.to_string())
}

/// GET /api/background-tasks
pub async fn list_tasks(
    Extension(claims): Extension<Claims>,
    Extension(registry): Extension<TaskRegistry>,
    Query(query): Query<TaskListQuery>,
) -> impl IntoResponse {
    let statuses = normalize_task_statuses_query(&query);

    let tasks = registry
        .list_for_user(
            &claims.sub,
            query.project_id.as_deref(),
            statuses.as_deref(),
            query.active_only.unwrap_or(false),
            query.limit,
        )
        .await;

    (StatusCode::OK, Json(build_task_list_response(tasks))).into_response()
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
            (StatusCode::OK, Json(compatible_task_payload(&record))).into_response()
        }
        None => (StatusCode::OK, Json(build_missing_task_payload(&task_id))).into_response(),
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

            if let Some(updated) = registry.get(&task_id).await {
                return (
                    StatusCode::OK,
                    Json({
                        let mut payload = compatible_task_payload(&updated);
                        if let Some(map) = payload.as_object_mut() {
                            map.insert("message".to_string(), json!("任务已取消"));
                        }
                        payload
                    }),
                )
                    .into_response();
            }

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

                return (StatusCode::OK, Json(compatible_task_payload(&updated))).into_response();
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
    let status_event = build_connected_task_event(&task_id, &record);
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
