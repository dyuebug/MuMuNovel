use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Sse},
    Json,
};
use chrono::Utc;
use futures::StreamExt;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::api::careers::execute_career_system_request;
use crate::api::careers::CareerSystemRequest;
use crate::api::characters::GenerateCharacterRequest;
use crate::api::organizations::GenerateOrganizationRequest;
use crate::api::outlines::{
    build_outline_batch_expand_execution_request, build_outline_expand_execution_request,
    execute_outline_batch_expand_request, execute_outline_expand_request, execute_outline_request,
};
use crate::api::wizard::{
    execute_characters_request, execute_regenerate_world_building_request,
    execute_world_building_request, CharactersRequest, OutlineRequest,
    RegenerateWorldBuildingRequest, WorldBuildingRequest,
};
use crate::services::auth::Claims;
use crate::tasks::checkpoint::touch_checkpoint;
use crate::tasks::registry::TaskRegistry;
use crate::tasks::stream::TaskStreamHub;
use crate::tasks::types::{
    TaskCreateRequest, TaskEvent, TaskListQuery, TaskRecord, TaskStatus, TaskWorkflowUpdate,
};
use crate::utils::sse::{SseChannel, SseTaskCapture};

const BACKGROUND_TASKS_LIST_CREATE_ROUTE: &str = "/background-tasks";
const BACKGROUND_TASKS_DETAIL_ROUTE: &str = "/background-tasks/{task_id}";
const BACKGROUND_TASKS_STREAM_ROUTE: &str = "/background-tasks/{task_id}/stream";
const BACKGROUND_TASKS_CANCEL_ROUTE: &str = "/background-tasks/{task_id}/cancel";
const BACKGROUND_TASKS_WORKFLOW_STATE_ROUTE: &str = "/background-tasks/{task_id}/workflow-state";
const TASK_LIST_LIMIT_DEFAULT: usize = 20;
const TASK_LIST_LIMIT_MIN: i64 = 1;
const TASK_LIST_LIMIT_MAX: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
enum TaskListQueryRequestError {
    InvalidStatuses(Vec<String>),
    LimitTooSmall,
    LimitTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskListRequest {
    project_id: Option<String>,
    statuses: Option<Vec<TaskStatus>>,
    active_only: bool,
    limit: usize,
}

impl TaskListRequest {
    fn from_route_query(query: TaskListQuery) -> Result<Self, TaskListQueryRequestError> {
        let statuses = normalize_task_statuses_query(&query)?;
        let limit = normalize_task_list_limit(query.limit)?;

        Ok(Self {
            project_id: query.project_id,
            statuses,
            active_only: query.active_only.unwrap_or(false),
            limit,
        })
    }

    fn project_id(&self) -> Option<&str> {
        self.project_id.as_deref()
    }

    fn statuses(&self) -> Option<&[TaskStatus]> {
        self.statuses.as_deref()
    }

    fn active_only(&self) -> bool {
        self.active_only
    }

    fn limit(&self) -> usize {
        self.limit
    }
}

fn normalize_task_statuses_query(
    query: &TaskListQuery,
) -> Result<Option<Vec<TaskStatus>>, TaskListQueryRequestError> {
    let Some(statuses) = query.statuses.as_ref() else {
        return Ok(None);
    };

    let mut parsed = Vec::new();
    let mut invalid = Vec::new();

    for part in statuses.split(',') {
        let status = part.trim().to_lowercase();
        if status.is_empty() {
            continue;
        }

        match status.as_str() {
            "pending" => parsed.push(TaskStatus::Pending),
            "running" => parsed.push(TaskStatus::Running),
            "completed" => parsed.push(TaskStatus::Completed),
            "failed" => parsed.push(TaskStatus::Failed),
            "cancelled" => parsed.push(TaskStatus::Cancelled),
            _ => invalid.push(status),
        }
    }

    if invalid.is_empty() {
        Ok(Some(parsed))
    } else {
        invalid.sort();
        invalid.dedup();
        Err(TaskListQueryRequestError::InvalidStatuses(invalid))
    }
}

fn normalize_task_list_limit(limit: Option<i64>) -> Result<usize, TaskListQueryRequestError> {
    let Some(limit) = limit else {
        return Ok(TASK_LIST_LIMIT_DEFAULT);
    };

    if limit < TASK_LIST_LIMIT_MIN {
        return Err(TaskListQueryRequestError::LimitTooSmall);
    }
    if limit > TASK_LIST_LIMIT_MAX as i64 {
        return Err(TaskListQueryRequestError::LimitTooLarge);
    }

    Ok(limit as usize)
}

fn compatible_task_payload(record: &TaskRecord) -> Value {
    let record_value = serde_json::to_value(record).unwrap_or_else(|_| json!({}));
    match record_value {
        Value::Object(mut map) => {
            map.insert("success".to_string(), json!(true));
            map.insert("data".to_string(), json!(record));
            Value::Object(map)
        }
        _ => json!({
            "success": true,
            "data": record
        }),
    }
}

fn enrich_task_payload(record: &TaskRecord, payload: Value) -> Value {
    match payload {
        Value::Object(mut map) => {
            if !record.project_id.trim().is_empty() {
                map.entry("project_id".to_string())
                    .or_insert_with(|| json!(record.project_id));
            }
            if !record.user_id.trim().is_empty() {
                map.entry("user_id".to_string())
                    .or_insert_with(|| json!(record.user_id));
            }
            Value::Object(map)
        }
        other => other,
    }
}

fn build_task_list_response(tasks: Vec<TaskRecord>) -> Value {
    json!({
        "success": true,
        "data": tasks,
        "items": tasks,
        "total": tasks.len(),
    })
}

fn build_missing_task_payload(task_id: &str) -> Value {
    json!({
        "success": true,
        "message": "任务不存在",
        "task_id": task_id,
        "project_id": "",
        "task_type": "unknown",
        "status": "cancelled",
        "progress": 100,
        "message_detail": "任务不存在",
        "data": {
            "task_id": task_id,
            "project_id": "",
            "task_type": "unknown",
            "status": "cancelled",
            "progress": 100,
            "message": "任务不存在"
        }
    })
}

fn build_connected_task_event(task_id: &str, record: &TaskRecord) -> TaskEvent {
    let record_json = serde_json::to_value(record).unwrap_or_default();
    TaskEvent {
        event_type: "connected".into(),
        task_id: Some(task_id.to_string()),
        message: Some(record.message.clone()),
        progress: Some(record.progress),
        status: Some(record.status.to_string()),
        data: Some(record_json),
        error: None,
    }
}

#[cfg(test)]
fn build_background_tasks_route_owner_contract() -> serde_json::Value {
    json!({
        "owner": "background_tasks",
        "rust_owner": "backend-rs/src/api/background_tasks.rs",
        "routes": {
            "create": BACKGROUND_TASKS_LIST_CREATE_ROUTE,
            "list": BACKGROUND_TASKS_LIST_CREATE_ROUTE,
            "detail": BACKGROUND_TASKS_DETAIL_ROUTE,
            "stream": BACKGROUND_TASKS_STREAM_ROUTE,
            "cancel": BACKGROUND_TASKS_CANCEL_ROUTE,
            "workflow_state": BACKGROUND_TASKS_WORKFLOW_STATE_ROUTE
        },
        "methods": {
            "create": ["POST"],
            "list": ["GET"],
            "detail": ["GET"],
            "stream": ["GET"],
            "cancel": ["POST"],
            "workflow_state": ["PATCH"]
        },
        "service_owners": [
            "backend-rs/src/api/background_tasks.rs",
            "backend-rs/src/api/outlines.rs",
            "backend-rs/src/api/wizard.rs",
            "backend-rs/src/api/careers.rs",
            "backend-rs/src/tasks/registry.rs",
            "backend-rs/src/tasks/stream.rs",
            "backend-rs/src/tasks/types.rs",
            "backend-rs/src/tasks/checkpoint.rs"
        ],
        "readiness_probes": [
            "background-tasks-list-auth-guard-rust",
            "background-tasks-create-auth-guard-rust",
            "background-tasks-create-business-rust",
            "background-tasks-list-business-rust",
            "background-tasks-detail-business-rust",
            "background-tasks-workflow-state-business-rust",
            "background-tasks-missing-detail-business-rust",
            "background-tasks-missing-cancel-business-rust",
            "background-tasks-missing-workflow-state-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-background-tasks-business-owner",
            "business_probes": [
                "background-tasks-create-business-rust",
                "background-tasks-list-business-rust",
                "background-tasks-detail-business-rust",
                "background-tasks-workflow-state-business-rust",
                "background-tasks-missing-detail-business-rust",
                "background-tasks-missing-cancel-business-rust",
                "background-tasks-missing-workflow-state-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "source_map_files": [],
        "rollback_boundary": {
            "source_map_policy": "background_tasks_active_route_group_no_longer_retains_python_startup_or_route_source_maps",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_bootstrap_status": "background_tasks_route_runtime_registration_deleted_no_python_route_shell_remains",
            "python_route_files_status": "background_tasks_route_and_python_startup_source_maps_deleted_active_route_group_boundary_empty",
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "rollback_files": []
        },
        "business_smoke_status": {
            "owner_profile": "phase5-background-tasks-business-owner",
            "readiness_probe_count": 9,
            "business_probe_count": 7,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "background_tasks active route group no longer retains Python startup or route source maps; any remaining Python task-manager work is now outside the direct route-group boundary",
        "migration_policy": "Background tasks route business smoke is covered by phase5-background-tasks-business-owner; the Python background_tasks route shell, its explicit bootstrap rollback registration, and the Python startup import of background_task_manager have been removed from the active production route-group boundary. Any remaining Python background task manager implementation now sits outside the direct background_tasks route ownership contract."
    })
}

fn adapt_character_generation_task_request(
    project_id: &str,
    payload: &Value,
) -> Result<GenerateCharacterRequest, String> {
    let mut body = payload.as_object().cloned().unwrap_or_default();
    body.insert("project_id".to_string(), json!(project_id));
    serde_json::from_value(Value::Object(body))
        .map_err(|error| format!("无效的角色生成任务参数: {}", error))
}

fn adapt_organization_generation_task_request(
    project_id: &str,
    payload: &Value,
) -> Result<GenerateOrganizationRequest, String> {
    let mut body = payload.as_object().cloned().unwrap_or_default();
    body.insert("project_id".to_string(), json!(project_id));
    serde_json::from_value(Value::Object(body))
        .map_err(|error| format!("无效的组织生成任务参数: {}", error))
}

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
    let request = match TaskListRequest::from_route_query(query) {
        Ok(request) => request,
        Err(error) => return map_task_list_query_request_error(error).into_response(),
    };

    let tasks = registry
        .list_for_user(
            &claims.sub,
            request.project_id(),
            request.statuses(),
            request.active_only(),
            Some(request.limit()),
        )
        .await;

    (StatusCode::OK, Json(build_task_list_response(tasks))).into_response()
}

fn map_task_list_query_request_error(
    error: TaskListQueryRequestError,
) -> (StatusCode, Json<serde_json::Value>) {
    let detail = match error {
        TaskListQueryRequestError::InvalidStatuses(invalid) => {
            format!("Invalid task statuses: {}", invalid.join(", "))
        }
        TaskListQueryRequestError::LimitTooSmall => {
            "limit must be greater than or equal to 1".to_string()
        }
        TaskListQueryRequestError::LimitTooLarge => {
            "limit must be less than or equal to 100".to_string()
        }
    };

    (StatusCode::BAD_REQUEST, Json(json!({ "detail": detail })))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use chrono::Utc;
    use serde_json::json;

    use super::{
        adapt_character_generation_task_request, adapt_organization_generation_task_request,
        build_background_tasks_route_owner_contract, build_connected_task_event,
        build_missing_task_payload, build_task_list_response, compatible_task_payload,
        enrich_task_payload, map_task_list_query_request_error, normalize_task_statuses_query,
        TaskListQueryRequestError, TaskListRequest, BACKGROUND_TASKS_CANCEL_ROUTE,
        BACKGROUND_TASKS_DETAIL_ROUTE, BACKGROUND_TASKS_LIST_CREATE_ROUTE,
        BACKGROUND_TASKS_STREAM_ROUTE, BACKGROUND_TASKS_WORKFLOW_STATE_ROUTE,
    };
    use crate::tasks::types::{TaskListQuery, TaskRecord, TaskStatus};

    fn task_record() -> TaskRecord {
        TaskRecord {
            task_id: "task-1".to_string(),
            task_type: "wizard_outline".to_string(),
            user_id: "user-1".to_string(),
            project_id: "project-1".to_string(),
            status: TaskStatus::Running,
            progress: 42,
            message: "进行中".to_string(),
            result: None,
            error: None,
            stage_code: Some("2.running".to_string()),
            execution_mode: "interactive".to_string(),
            workflow_scope: Some("outline".to_string()),
            checkpoint: None,
            payload_fingerprint: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn should_publish_background_tasks_route_owner_contract() {
        let contract = build_background_tasks_route_owner_contract();

        assert_eq!(contract["owner"], "background_tasks");
        assert_eq!(
            contract["rust_owner"],
            "backend-rs/src/api/background_tasks.rs"
        );
        assert_eq!(
            contract["routes"]["create"],
            BACKGROUND_TASKS_LIST_CREATE_ROUTE
        );
        assert_eq!(
            contract["routes"]["list"],
            BACKGROUND_TASKS_LIST_CREATE_ROUTE
        );
        assert_eq!(contract["routes"]["detail"], BACKGROUND_TASKS_DETAIL_ROUTE);
        assert_eq!(contract["routes"]["stream"], BACKGROUND_TASKS_STREAM_ROUTE);
        assert_eq!(contract["routes"]["cancel"], BACKGROUND_TASKS_CANCEL_ROUTE);
        assert_eq!(
            contract["routes"]["workflow_state"],
            BACKGROUND_TASKS_WORKFLOW_STATE_ROUTE
        );

        assert_eq!(contract["methods"]["create"], json!(["POST"]));
        assert_eq!(contract["methods"]["list"], json!(["GET"]));
        assert_eq!(contract["methods"]["workflow_state"], json!(["PATCH"]));
        assert_eq!(
            contract["service_owners"]
                .as_array()
                .expect("service owner list should be present")
                .len(),
            8
        );
        assert_eq!(
            contract["service_owners"][0],
            "backend-rs/src/api/background_tasks.rs"
        );
        assert_eq!(
            contract["readiness_probes"]
                .as_array()
                .expect("readiness probes should be present")
                .len(),
            9
        );
        assert_eq!(
            contract["readiness_probes"][8],
            "background-tasks-missing-workflow-state-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-background-tasks-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"]
                .as_array()
                .expect("business probes should be present")
                .len(),
            7
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][3],
            "background-tasks-workflow-state-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["source_map_files"]
                .as_array()
                .expect("source map files should be present")
                .len(),
            0
        );
        assert!(contract["source_map_files"].get(0).is_none());
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_bootstrap_status"],
            "background_tasks_route_runtime_registration_deleted_no_python_route_shell_remains"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_route_files_status"],
            "background_tasks_route_and_python_startup_source_maps_deleted_active_route_group_boundary_empty"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"],
            json!([])
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(9)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(7)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "background_tasks active route group no longer retains Python startup or route source maps; any remaining Python task-manager work is now outside the direct route-group boundary"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("background tasks migration policy should be present")
            .contains("phase5-background-tasks-business-owner"));
        assert!(contract["migration_policy"]
            .as_str()
            .expect("background tasks migration policy should be present")
            .contains("Python startup import of background_task_manager"));
    }

    #[test]
    fn should_keep_background_tasks_route_group_paths_stable() {
        assert_eq!(BACKGROUND_TASKS_LIST_CREATE_ROUTE, "/background-tasks");
        assert_eq!(BACKGROUND_TASKS_DETAIL_ROUTE, "/background-tasks/{task_id}");
        assert_eq!(
            BACKGROUND_TASKS_STREAM_ROUTE,
            "/background-tasks/{task_id}/stream"
        );
        assert_eq!(
            BACKGROUND_TASKS_CANCEL_ROUTE,
            "/background-tasks/{task_id}/cancel"
        );
        assert_eq!(
            BACKGROUND_TASKS_WORKFLOW_STATE_ROUTE,
            "/background-tasks/{task_id}/workflow-state"
        );
    }

    #[test]
    fn compatible_task_payload_keeps_success_and_data_contract() {
        let payload = compatible_task_payload(&task_record());

        assert_eq!(payload["success"], true);
        assert_eq!(payload["data"]["task_id"], "task-1");
        assert_eq!(payload["task_id"], "task-1");
        assert_eq!(payload["status"], "running");
    }

    #[test]
    fn enrich_task_payload_adds_project_and_user_when_missing() {
        let payload = enrich_task_payload(&task_record(), json!({"hello": "world"}));

        assert_eq!(payload["hello"], "world");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["user_id"], "user-1");
    }

    #[test]
    fn build_task_list_response_keeps_items_and_total_in_sync() {
        let payload = build_task_list_response(vec![task_record()]);

        assert_eq!(payload["success"], true);
        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["task_id"], "task-1");
        assert_eq!(payload["data"][0]["task_id"], "task-1");
    }

    #[test]
    fn build_missing_task_payload_keeps_existing_cancelled_shape() {
        let payload = build_missing_task_payload("task-missing");

        assert_eq!(payload["success"], true);
        assert_eq!(payload["task_id"], "task-missing");
        assert_eq!(payload["status"], "cancelled");
        assert_eq!(payload["data"]["message"], "任务不存在");
    }

    #[test]
    fn build_connected_task_event_keeps_existing_stream_contract() {
        let event = build_connected_task_event("task-1", &task_record());

        assert_eq!(event.event_type, "connected");
        assert_eq!(event.task_id.as_deref(), Some("task-1"));
        assert_eq!(event.progress, Some(42));
        assert_eq!(event.status.as_deref(), Some("running"));
        assert_eq!(
            event.data.as_ref().and_then(|value| value.get("task_id")),
            Some(&json!("task-1"))
        );
    }

    #[test]
    fn character_generation_task_adapter_keeps_existing_payload_contract() {
        adapt_character_generation_task_request(
            "project-1",
            &json!({
                "name": "阿青",
                "role_type": "supporting",
                "background": "来自边城",
                "requirements": "要有反差感",
                "provider": "openai",
                "model": "gpt-4o-mini"
            }),
        )
        .expect("character request should adapt");
    }

    #[test]
    fn organization_generation_task_adapter_keeps_existing_payload_contract() {
        adapt_organization_generation_task_request(
            "project-2",
            &json!({
                "name": "玄霜盟",
                "organization_type": "门派",
                "background": "北境旧盟",
                "requirements": "要有资源约束",
                "provider": "openai",
                "model": "gpt-4.1"
            }),
        )
        .expect("organization request should adapt");
    }

    #[test]
    fn task_list_query_errors_match_python_route_boundary() {
        let cases = [
            (
                TaskListQueryRequestError::InvalidStatuses(vec![
                    "archived".to_string(),
                    "unknown".to_string(),
                ]),
                "Invalid task statuses: archived, unknown",
            ),
            (
                TaskListQueryRequestError::LimitTooSmall,
                "limit must be greater than or equal to 1",
            ),
            (
                TaskListQueryRequestError::LimitTooLarge,
                "limit must be less than or equal to 100",
            ),
        ];

        for (error, expected_detail) in cases {
            let (status, body) = map_task_list_query_request_error(error);

            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(body.0["detail"], expected_detail);
        }
    }

    #[test]
    fn normalize_task_statuses_query_accepts_known_status_filtering() {
        let query = TaskListQuery {
            project_id: None,
            statuses: Some("pending, running,failed".to_string()),
            active_only: Some(false),
            limit: Some(10),
        };

        let statuses = normalize_task_statuses_query(&query)
            .expect("known statuses should be valid")
            .expect("statuses should exist");

        assert_eq!(
            statuses,
            vec![TaskStatus::Pending, TaskStatus::Running, TaskStatus::Failed]
        );
    }

    #[test]
    fn normalize_task_statuses_query_keeps_none_when_query_missing() {
        let query = TaskListQuery {
            project_id: None,
            statuses: None,
            active_only: None,
            limit: None,
        };

        assert_eq!(normalize_task_statuses_query(&query).unwrap(), None);
    }

    #[test]
    fn normalize_task_statuses_query_rejects_unknown_status_like_python_route() {
        let query = TaskListQuery {
            project_id: None,
            statuses: Some("pending, unknown,invalid,unknown".to_string()),
            active_only: Some(false),
            limit: Some(10),
        };

        assert_eq!(
            normalize_task_statuses_query(&query),
            Err(TaskListQueryRequestError::InvalidStatuses(vec![
                "invalid".to_string(),
                "unknown".to_string()
            ]))
        );
    }

    #[test]
    fn task_list_request_validates_limit_like_python_query() {
        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: Some("project-1".to_string()),
                statuses: None,
                active_only: None,
                limit: None,
            })
            .expect("default limit should be valid")
            .limit(),
            20
        );

        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: None,
                statuses: None,
                active_only: Some(true),
                limit: Some(25),
            })
            .expect("explicit in-range limit should be valid")
            .limit(),
            25
        );

        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: None,
                statuses: None,
                active_only: None,
                limit: Some(0),
            }),
            Err(TaskListQueryRequestError::LimitTooSmall)
        );

        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: None,
                statuses: None,
                active_only: None,
                limit: Some(-1),
            }),
            Err(TaskListQueryRequestError::LimitTooSmall)
        );

        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: None,
                statuses: None,
                active_only: None,
                limit: Some(101),
            }),
            Err(TaskListQueryRequestError::LimitTooLarge)
        );
    }
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
        .route(
            BACKGROUND_TASKS_LIST_CREATE_ROUTE,
            post(create_task).get(list_tasks),
        )
        .route(BACKGROUND_TASKS_DETAIL_ROUTE, get(get_task))
        .route(BACKGROUND_TASKS_STREAM_ROUTE, get(stream_task))
        .route(BACKGROUND_TASKS_CANCEL_ROUTE, post(cancel_task))
        .route(
            BACKGROUND_TASKS_WORKFLOW_STATE_ROUTE,
            patch(update_workflow_state),
        )
}
