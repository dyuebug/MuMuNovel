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
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::api::careers::execute_career_system_request;
use crate::api::careers::CareerSystemRequest;
use crate::api::characters::GenerateCharacterRequest;
use crate::api::inspiration::{
    execute_generate_options_task, execute_quick_generate_task, execute_refine_options_task,
};
use crate::api::organizations::GenerateOrganizationRequest;
use crate::api::outlines::{
    build_outline_batch_expand_execution_request, build_outline_expand_execution_request,
    execute_outline_batch_expand_request, execute_outline_expand_request, execute_outline_request,
};
use crate::api::polish::{
    execute_polish_batch_task, execute_polish_text_task, PolishBatchRequest, PolishRequest,
};
use crate::api::wizard::{
    execute_characters_request, execute_regenerate_world_building_request,
    execute_world_building_request, CharactersRequest, OutlineRequest,
    RegenerateWorldBuildingRequest, WorldBuildingRequest,
};
use crate::services::auth::Claims;
use crate::services::autopilot_coordinator_service::execute_novel_autopilot_task;
use crate::services::autopilot_invocation_audit_service::{
    create_queued_autopilot_invocation_audit, mark_autopilot_invocation_cancelled,
};
use crate::services::book_import_service::BookImportService;
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_regeneration_prepare_service::{
    build_full_chapter_regeneration_stream_request_from_route_payload,
    build_partial_regeneration_stream_workflow_request_from_route_payload,
    FullChapterRegenerationStreamRouteRequest, PartialRegenerationStreamRouteRequest,
};
use crate::services::chapter_regeneration_stream_workflow_service::{
    execute_chapter_regeneration_task, execute_partial_regeneration_task,
};
use crate::services::cooperative_cancellation_service::{
    global_cooperative_cancellation_registry, CooperativeCancellationScope,
    CooperativeCancellationToken,
};
use crate::services::novel_autopilot::{
    coordinator::{
        execute_novel_book_autopilot_tick, is_novel_autopilot_execution_cancelled,
        NovelAutopilotNextTickLease, NovelAutopilotTickOutcome,
    },
    output_observer::NovelAutopilotOutputObserver,
    repository::{NovelAutopilotRepository, NovelAutopilotRepositoryError},
    types::NovelAutopilotRunStatus,
};
use crate::tasks::checkpoint::touch_checkpoint_at;
use crate::tasks::registry::TaskRegistry;
use crate::tasks::stream::TaskStreamHub;
use crate::tasks::types::{
    TaskCreateRequest, TaskEvent, TaskListQuery, TaskRecord, TaskStatus, TaskWorkflowUpdate,
};
use crate::utils::sse::{SseChannel, SseTaskCapture, SseTaskOutputEvent};

const BACKGROUND_TASKS_LIST_CREATE_ROUTE: &str = "/background-tasks";
const BACKGROUND_TASKS_DETAIL_ROUTE: &str = "/background-tasks/{task_id}";
const BACKGROUND_TASKS_STREAM_ROUTE: &str = "/background-tasks/{task_id}/stream";
const BACKGROUND_TASKS_CANCEL_ROUTE: &str = "/background-tasks/{task_id}/cancel";
const BACKGROUND_TASKS_WORKFLOW_STATE_ROUTE: &str = "/background-tasks/{task_id}/workflow-state";
const TASK_LIST_LIMIT_DEFAULT: usize = 20;
const TASK_LIST_LIMIT_MIN: i64 = 1;
const TASK_LIST_LIMIT_MAX: usize = 100;
const NOVEL_BOOK_AUTOPILOT_TASK_TYPE: &str = "novel_book_autopilot";
const NOVEL_BOOK_AUTOPILOT_SCHEDULE_FAILED: &str = "novel_autopilot_schedule_failed";
const NOVEL_BOOK_AUTOPILOT_EXECUTION_FAILED: &str = "novel_autopilot_execution_failed";

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

fn prepare_task_execution_payload(record: &TaskRecord, payload: Value) -> Value {
    // Autopilot 的调用协议是严格 DTO；作用域与操作者始终以 TaskRecord 为准，不能注入其 payload。
    if record.task_type == "novel_autopilot" {
        payload
    } else {
        enrich_task_payload(record, payload)
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
        content: None,
        data: Some(record_json),
        error: None,
    }
}

async fn subscribe_task_with_latest_snapshot(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    authorized_record: TaskRecord,
) -> (tokio::sync::broadcast::Receiver<String>, TaskRecord) {
    let receiver = stream_hub.subscribe(task_id).await;
    let latest_record = registry.get(task_id).await.unwrap_or(authorized_record);
    (receiver, latest_record)
}

struct TaskStreamState {
    receiver: tokio::sync::broadcast::Receiver<String>,
    registry: TaskRegistry,
    task_id: String,
    close_after_emit: bool,
}

impl TaskStreamState {
    fn new(
        receiver: tokio::sync::broadcast::Receiver<String>,
        registry: TaskRegistry,
        task_id: String,
    ) -> Self {
        Self {
            receiver,
            registry,
            task_id,
            close_after_emit: false,
        }
    }
}

async fn next_task_stream_data(mut state: TaskStreamState) -> Option<(String, TaskStreamState)> {
    if state.close_after_emit {
        return None;
    }

    loop {
        match state.receiver.recv().await {
            Ok(data) => return Some((data, state)),
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(
                    task_id = %state.task_id,
                    skipped,
                    "Background task SSE receiver lagged; resynchronizing latest task snapshot"
                );

                // Drop every retained event that predates the recovery snapshot. Subscribing to
                // the current sender tail before reading the registry preserves the same
                // subscribe-then-snapshot ordering used by the initial SSE connection.
                state.receiver = state.receiver.resubscribe();
                let Some(record) = state.registry.get(&state.task_id).await else {
                    continue;
                };
                state.close_after_emit = record.status.is_terminal();

                match serde_json::to_string(&build_connected_task_event(&state.task_id, &record)) {
                    Ok(data) => return Some((data, state)),
                    Err(error) => {
                        tracing::warn!(
                            task_id = %state.task_id,
                            error = %error,
                            "Failed to serialize background task SSE recovery snapshot"
                        );
                        if state.close_after_emit {
                            return None;
                        }
                    }
                }
            }
        }
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

fn task_type_allows_empty_project(task_type: &str) -> bool {
    matches!(
        task_type,
        "wizard_world_building"
            | "inspiration_generate_options"
            | "inspiration_refine_options"
            | "inspiration_quick_generate"
            | "book_import_apply"
            | "book_import_retry_failed_steps"
            | "polish_text"
            | "polish_batch"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuthenticatedTaskCreateError {
    ProjectRequired,
    AutopilotAuditUnavailable,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedTaskCreateResponse {
    pub(crate) status: StatusCode,
    pub(crate) payload: Value,
}

#[derive(Debug)]
enum AuthenticatedTaskPreparation {
    Existing(AuthenticatedTaskCreateResponse),
    Ready {
        response: AuthenticatedTaskCreateResponse,
        record: TaskRecord,
        execution_payload: Value,
    },
}

#[derive(Debug)]
pub(crate) enum NovelBookAutopilotTaskScheduleOutcome {
    Scheduled { task: Value },
    Superseded,
}

#[derive(Debug)]
pub(crate) enum NovelBookAutopilotTaskScheduleError {
    TaskCreate(AuthenticatedTaskCreateError),
    InvalidTaskResponse,
    Repository(NovelAutopilotRepositoryError),
}

impl NovelBookAutopilotTaskScheduleError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::TaskCreate(AuthenticatedTaskCreateError::ProjectRequired) => {
                "novel_autopilot_project_required"
            }
            Self::TaskCreate(AuthenticatedTaskCreateError::AutopilotAuditUnavailable) => {
                "novel_autopilot_task_audit_unavailable"
            }
            Self::InvalidTaskResponse => "novel_autopilot_task_response_invalid",
            Self::Repository(error) => error.code(),
        }
    }
}

async fn prepare_task_for_authenticated_user(
    db: &DatabaseConnection,
    registry: &TaskRegistry,
    user_id: &str,
    req: TaskCreateRequest,
) -> Result<AuthenticatedTaskPreparation, AuthenticatedTaskCreateError> {
    if !task_type_allows_empty_project(&req.task_type) && req.project_id.trim().is_empty() {
        return Err(AuthenticatedTaskCreateError::ProjectRequired);
    }

    let task_id = Uuid::new_v4().to_string();
    let fingerprint = req.payload.as_ref().map(|payload| {
        let json_str = serde_json::to_string(payload).unwrap_or_default();
        format!("{:x}", md5::compute(json_str.as_bytes()))
    });

    // Deduplication remains owned by the generic task lifecycle.
    if let Some(ref fingerprint) = fingerprint {
        if let Some(existing) = registry
            .find_active(user_id, &req.task_type, &req.project_id, Some(fingerprint))
            .await
        {
            let mut payload = compatible_task_payload(&existing);
            if let Some(map) = payload.as_object_mut() {
                map.insert("message".to_string(), json!("相同任务已在执行中"));
            }
            return Ok(AuthenticatedTaskPreparation::Existing(
                AuthenticatedTaskCreateResponse {
                    status: StatusCode::OK,
                    payload,
                },
            ));
        }
    }

    let mut record = TaskRecord::new(
        task_id,
        req.task_type,
        user_id.to_string(),
        req.project_id,
        req.execution_mode,
    );

    record.stage_code = req.stage_code;
    record.workflow_scope = req.workflow_scope;
    record.payload_fingerprint = fingerprint;
    if let Some(checkpoint) = req.checkpoint {
        record.checkpoint = Some(checkpoint);
    }

    let payload = req.payload.unwrap_or_else(|| json!({}));
    if record.task_type == "novel_autopilot" {
        create_queued_autopilot_invocation_audit(db, &record, &payload)
            .await
            .map_err(|error| {
                tracing::error!(
                    event = "autopilot_invocation_audit_queue_failed",
                    task_id = %record.task_id,
                    error_code = error.code(),
                    "autopilot task was not spawned because its durable audit could not be created"
                );
                AuthenticatedTaskCreateError::AutopilotAuditUnavailable
            })?;
    }

    registry.insert(record.clone()).await;
    let execution_payload = prepare_task_execution_payload(&record, payload);
    let response = AuthenticatedTaskCreateResponse {
        status: StatusCode::CREATED,
        payload: compatible_task_payload(&record),
    };
    Ok(AuthenticatedTaskPreparation::Ready {
        response,
        record,
        execution_payload,
    })
}

pub(crate) async fn create_task_for_authenticated_user(
    db: DatabaseConnection,
    registry: TaskRegistry,
    stream_hub: TaskStreamHub,
    book_import_service: Arc<BookImportService>,
    user_id: &str,
    req: TaskCreateRequest,
) -> Result<AuthenticatedTaskCreateResponse, AuthenticatedTaskCreateError> {
    match prepare_task_for_authenticated_user(&db, &registry, user_id, req).await? {
        AuthenticatedTaskPreparation::Existing(response) => Ok(response),
        AuthenticatedTaskPreparation::Ready {
            response,
            record,
            execution_payload,
        } => {
            spawn_task_execution(
                db,
                registry,
                stream_hub,
                book_import_service,
                None,
                record,
                execution_payload,
            );
            Ok(response)
        }
    }
}

pub(crate) async fn schedule_owned_novel_book_autopilot_tick(
    db: DatabaseConnection,
    registry: TaskRegistry,
    stream_hub: TaskStreamHub,
    book_import_service: Arc<BookImportService>,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    lease: &NovelAutopilotNextTickLease,
    decision: Option<&str>,
) -> Result<NovelBookAutopilotTaskScheduleOutcome, NovelBookAutopilotTaskScheduleError> {
    let mut payload = json!({
        "run_id": lease.run_id,
        "run_epoch": lease.epoch,
        "run_version": lease.version,
    });
    if let Some(decision) = decision {
        payload["decision"] = json!(decision);
    }
    if let Some(not_before) = lease.next_attempt_at {
        payload["not_before"] = json!(format!("{}Z", not_before.format("%Y-%m-%dT%H:%M:%S%.f")));
    }
    let preparation = prepare_task_for_authenticated_user(
        &db,
        &registry,
        &lease.user_id,
        TaskCreateRequest {
            task_type: NOVEL_BOOK_AUTOPILOT_TASK_TYPE.to_string(),
            project_id: lease.project_id.clone(),
            payload: Some(payload),
            stage_code: Some(lease.current_phase.clone()),
            execution_mode: "auto".to_string(),
            workflow_scope: Some(NOVEL_BOOK_AUTOPILOT_TASK_TYPE.to_string()),
            checkpoint: Some(json!({
                "run_id": lease.run_id,
                "epoch": lease.epoch,
                "version": lease.version,
                "not_before": lease.next_attempt_at.map(|value| format!(
                    "{}Z",
                    value.format("%Y-%m-%dT%H:%M:%S%.f")
                )),
            })),
        },
    )
    .await
    .map_err(NovelBookAutopilotTaskScheduleError::TaskCreate)?;

    let (response, pending_execution) = match preparation {
        AuthenticatedTaskPreparation::Existing(response) => (response, None),
        AuthenticatedTaskPreparation::Ready {
            response,
            record,
            execution_payload,
        } => (response, Some((record, execution_payload))),
    };
    let task_id = response
        .payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(NovelBookAutopilotTaskScheduleError::InvalidTaskResponse)?;

    let binding = NovelAutopilotRepository::set_active_background_task_owned(
        &db,
        &lease.run_id,
        &lease.user_id,
        lease.version,
        lease.epoch,
        Some(&task_id),
    )
    .await;
    match binding {
        Ok(_) => {
            if let Some((record, execution_payload)) = pending_execution {
                spawn_task_execution(
                    db,
                    registry,
                    stream_hub,
                    book_import_service,
                    Some(candidate_gateway_config),
                    record,
                    execution_payload,
                );
            }
            Ok(NovelBookAutopilotTaskScheduleOutcome::Scheduled {
                task: response.payload,
            })
        }
        Err(NovelAutopilotRepositoryError::StaleVersion)
        | Err(NovelAutopilotRepositoryError::StaleEpoch)
        | Err(NovelAutopilotRepositoryError::InvalidTransition) => {
            let latest = match NovelAutopilotRepository::find_owned(
                &db,
                &lease.run_id,
                &lease.user_id,
            )
            .await
            {
                Ok(latest) => latest,
                Err(error) => {
                    let _ =
                        cancel_task_runtime(&registry, &stream_hub, &task_id, &lease.user_id).await;
                    return Err(NovelBookAutopilotTaskScheduleError::Repository(error));
                }
            };
            if latest.active_background_task_id.as_deref() == Some(task_id.as_str()) {
                if let Some((record, execution_payload)) = pending_execution {
                    spawn_task_execution(
                        db,
                        registry,
                        stream_hub,
                        book_import_service,
                        Some(candidate_gateway_config),
                        record,
                        execution_payload,
                    );
                }
                return Ok(NovelBookAutopilotTaskScheduleOutcome::Scheduled {
                    task: response.payload,
                });
            }
            let _ = cancel_task_runtime(&registry, &stream_hub, &task_id, &lease.user_id).await;
            Ok(NovelBookAutopilotTaskScheduleOutcome::Superseded)
        }
        Err(error) => {
            let _ = cancel_task_runtime(&registry, &stream_hub, &task_id, &lease.user_id).await;
            Err(NovelBookAutopilotTaskScheduleError::Repository(error))
        }
    }
}

/// POST /api/background-tasks
pub async fn create_task(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Extension(book_import_service): Extension<Arc<BookImportService>>,
    Json(req): Json<TaskCreateRequest>,
) -> impl IntoResponse {
    if req.task_type == "novel_book_autopilot" {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "code": "owner_managed_task_type",
                "message": "Create durable novel autopilot tasks through the project Run API",
            })),
        )
            .into_response();
    }

    match create_task_for_authenticated_user(
        db,
        registry,
        stream_hub,
        book_import_service,
        &claims.sub,
        req,
    )
    .await
    {
        Ok(response) => (response.status, Json(response.payload)).into_response(),
        Err(AuthenticatedTaskCreateError::ProjectRequired) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "project_id is required for this task type"})),
        )
            .into_response(),
        Err(AuthenticatedTaskCreateError::AutopilotAuditUnavailable) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "Unable to create autopilot task"})),
        )
            .into_response(),
    }
}

fn spawn_task_execution(
    db: DatabaseConnection,
    registry: TaskRegistry,
    stream_hub: TaskStreamHub,
    book_import_service: Arc<BookImportService>,
    candidate_gateway_config: Option<ChapterCandidateRouteGatewayConfig>,
    record: TaskRecord,
    payload: serde_json::Value,
) {
    tokio::spawn(async move {
        let cancellation_registration = global_cooperative_cancellation_registry().register(
            CooperativeCancellationScope::BackgroundTask,
            record.task_id.clone(),
        );
        let cancellation_token = cancellation_registration.token();
        match wait_for_task_not_before(&payload, &cancellation_token).await {
            Ok(true) => {}
            Ok(false) => {
                cancellation_registration.cleanup();
                return;
            }
            Err(error) => {
                if mark_task_running(&registry, &stream_hub, &record.task_id, "任务调度时间无效")
                    .await
                {
                    best_effort_wait_for_human_after_execution_failure(
                        &db,
                        &record,
                        &payload,
                        NOVEL_BOOK_AUTOPILOT_EXECUTION_FAILED,
                    )
                    .await;
                    fail_task(&registry, &stream_hub, &record.task_id, &error).await;
                }
                cancellation_registration.cleanup();
                return;
            }
        }
        if !mark_task_running(&registry, &stream_hub, &record.task_id, "任务已开始执行").await
        {
            cancellation_registration.cleanup();
            return;
        }

        let result = tokio::select! {
            biased;
            _ = cancellation_token.cancelled() => None,
            result = execute_task(
                &db,
                &registry,
                &stream_hub,
                book_import_service.clone(),
                candidate_gateway_config.clone(),
                &record,
                payload,
                cancellation_token.clone(),
            ) => Some(result),
        };

        if let Some(Err(error)) = result {
            fail_task(&registry, &stream_hub, &record.task_id, &error).await;
        }
        cancellation_registration.cleanup();
    });
}

async fn wait_for_task_not_before(
    payload: &Value,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<bool, String> {
    let Some(not_before) = payload.get("not_before").and_then(Value::as_str) else {
        return Ok(true);
    };
    let not_before = chrono::DateTime::parse_from_rfc3339(not_before)
        .map_err(|_| "novel_autopilot_not_before_invalid".to_string())?
        .with_timezone(&Utc);

    loop {
        let now = Utc::now();
        if not_before <= now {
            return Ok(true);
        }
        let wait_duration = (not_before - now)
            .to_std()
            .unwrap_or_default()
            .min(std::time::Duration::from_secs(60));
        tokio::select! {
            biased;
            _ = cancellation_token.cancelled() => return Ok(false),
            _ = tokio::time::sleep(wait_duration) => {}
        }
    }
}

async fn mark_task_running(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    message: &str,
) -> bool {
    let now = Utc::now();
    let updated = registry
        .update_if(
            task_id,
            |record| record.status == TaskStatus::Pending,
            |record| {
                record.status = TaskStatus::Running;
                record.started_at = Some(now);
                record.updated_at = now;
                record.message = message.to_string();
                if record.progress <= 0 {
                    record.progress = 1;
                }
            },
        )
        .await;

    if let Some(record) = updated {
        stream_hub
            .fanout(
                task_id,
                &TaskEvent {
                    event_type: "progress".into(),
                    task_id: Some(task_id.to_string()),
                    message: Some(record.message),
                    progress: Some(record.progress),
                    status: Some(record.status.to_string()),
                    content: None,
                    data: None,
                    error: None,
                },
            )
            .await;
        true
    } else {
        false
    }
}

async fn complete_task(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    result: serde_json::Value,
    message: Option<String>,
) {
    let now = Utc::now();
    let updated = registry
        .update_if(
            task_id,
            |record| record.status.is_active(),
            |record| {
                record.status = TaskStatus::Completed;
                record.progress = 100;
                record.result = Some(result.clone());
                record.error = None;
                record.completed_at = Some(now);
                record.updated_at = now;
                record.message = message.unwrap_or_else(|| "任务执行完成".to_string());
            },
        )
        .await;

    if let Some(record) = updated {
        stream_hub
            .fanout(
                task_id,
                &TaskEvent {
                    event_type: "result".into(),
                    task_id: Some(task_id.to_string()),
                    message: Some(record.message.clone()),
                    progress: Some(record.progress),
                    status: Some(record.status.to_string()),
                    content: None,
                    data: record.result.clone(),
                    error: None,
                },
            )
            .await;
        stream_hub
            .fanout_terminal(
                task_id,
                &TaskEvent {
                    event_type: "done".into(),
                    task_id: Some(task_id.to_string()),
                    message: Some(record.message),
                    progress: Some(100),
                    status: Some("completed".into()),
                    content: None,
                    data: record.result,
                    error: None,
                },
            )
            .await;
    }
}

fn novel_book_autopilot_completion_message(task_result: &Value) -> String {
    let dispatch_status = task_result.get("dispatch_status").and_then(Value::as_str);
    let candidate_id = task_result.get("candidate_id").and_then(Value::as_str);
    let reason_code = task_result.get("reason_code").and_then(Value::as_str);

    if dispatch_status == Some("waiting_human") && candidate_id.is_some() {
        return "候选已保存，等待人工复核".to_string();
    }

    if dispatch_status == Some("waiting_human") {
        if reason_code.is_some_and(is_provider_failure_reason_code) {
            return "模型 Provider 调用失败，未生成可供人工接受的候选".to_string();
        }
        if reason_code.is_some_and(is_result_invalid_reason_code) {
            return "模型返回的章节结果无效，未生成可供人工接受的候选".to_string();
        }
        if reason_code.is_some_and(is_context_invalid_reason_code) {
            return "章节上下文无效，未生成可供人工接受的候选".to_string();
        }
    }

    "整本小说自动创作编排步骤已完成".to_string()
}

fn is_provider_failure_reason_code(reason_code: &str) -> bool {
    reason_code.starts_with("chapter_analysis_provider_")
        || reason_code.starts_with("chapter_repair_provider_")
}

fn is_result_invalid_reason_code(reason_code: &str) -> bool {
    matches!(
        reason_code,
        "chapter_analysis_result_invalid" | "chapter_repair_result_invalid"
    )
}

fn is_context_invalid_reason_code(reason_code: &str) -> bool {
    matches!(
        reason_code,
        "chapter_analysis_context_invalid" | "chapter_repair_context_invalid"
    )
}

async fn fail_task(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    error: &str,
) {
    let now = Utc::now();
    let updated = registry
        .update_if(
            task_id,
            |record| record.status.is_active(),
            |record| {
                record.status = TaskStatus::Failed;
                record.error = Some(error.to_string());
                record.completed_at = Some(now);
                record.updated_at = now;
                record.message = error.to_string();
            },
        )
        .await;

    if let Some(record) = updated {
        stream_hub
            .fanout_terminal(
                task_id,
                &TaskEvent {
                    event_type: "error".into(),
                    task_id: Some(task_id.to_string()),
                    message: Some(record.message.clone()),
                    progress: Some(record.progress),
                    status: Some(record.status.to_string()),
                    content: None,
                    data: None,
                    error: record.error,
                },
            )
            .await;
    }
}

async fn fanout_channel_output_events_if_active(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    output_events: Vec<SseTaskOutputEvent>,
) -> bool {
    if output_events.is_empty() {
        return registry
            .get(task_id)
            .await
            .is_some_and(|record| record.status.is_active());
    }

    let is_active = registry
        .get(task_id)
        .await
        .is_some_and(|record| record.status.is_active());
    if !is_active {
        return false;
    }

    for output_event in output_events {
        stream_hub
            .fanout(
                task_id,
                &TaskEvent {
                    event_type: output_event.event_type().to_string(),
                    task_id: Some(task_id.to_string()),
                    message: None,
                    progress: None,
                    status: None,
                    content: Some(output_event.content().to_string()),
                    data: None,
                    error: None,
                },
            )
            .await;
    }

    true
}

async fn sync_channel_state_to_task(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    state_capture: Arc<Mutex<SseTaskCapture>>,
    result_capture: Arc<Mutex<Option<serde_json::Value>>>,
) -> Result<serde_json::Value, String> {
    let (state, output_events) = {
        let mut capture = state_capture.lock().await;
        let output_events = capture.drain_output_events();
        (capture.clone(), output_events)
    };
    let result = result_capture.lock().await.clone().or(state.result.clone());

    fanout_channel_output_events_if_active(registry, stream_hub, task_id, output_events).await;

    if let Some(error) = state.error {
        return Err(error);
    }

    let now = Utc::now();
    let updated = registry
        .update_if(
            task_id,
            |record| record.status.is_active(),
            |record| {
                if let Some(message) = &state.message {
                    record.message = message.clone();
                }
                if let Some(progress) = state.progress {
                    record.progress = progress as i32;
                }
                record.updated_at = now;
            },
        )
        .await;

    if let Some(record) = updated {
        stream_hub
            .fanout(
                task_id,
                &TaskEvent {
                    event_type: "progress".into(),
                    task_id: Some(task_id.to_string()),
                    message: Some(record.message),
                    progress: Some(record.progress),
                    status: Some(record.status.to_string()),
                    content: None,
                    data: None,
                    error: None,
                },
            )
            .await;
    }

    result.ok_or_else(|| "后台任务未返回结果".to_string())
}

fn spawn_channel_progress_bridge(
    registry: TaskRegistry,
    stream_hub: TaskStreamHub,
    task_id: String,
    state_capture: Arc<Mutex<SseTaskCapture>>,
    cancellation_token: CooperativeCancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_message: Option<String> = None;
        let mut last_progress: Option<i32> = None;
        let mut last_status: Option<String> = None;

        loop {
            tokio::select! {
                biased;
                _ = cancellation_token.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_millis(250)) => {}
            }

            let (state, output_events) = {
                let mut capture = state_capture.lock().await;
                let output_events = capture.drain_output_events();
                (capture.clone(), output_events)
            };
            if !fanout_channel_output_events_if_active(
                &registry,
                &stream_hub,
                &task_id,
                output_events,
            )
            .await
            {
                break;
            }

            let progress = state.progress.map(|value| value.clamp(0, 100) as i32);
            let has_update =
                state.message.is_some() || progress.is_some() || state.status.is_some();
            let changed = state.message != last_message
                || progress != last_progress
                || state.status != last_status;
            let should_stop = state.done || state.error.is_some();

            if has_update && changed {
                let now = Utc::now();
                let updated = registry
                    .update_if(
                        &task_id,
                        |record| record.status.is_active(),
                        |record| {
                            if let Some(message) = &state.message {
                                record.message = message.clone();
                            }
                            if let Some(progress) = progress {
                                record.progress = record.progress.max(progress);
                            }
                            record.updated_at = now;
                        },
                    )
                    .await;

                if let Some(record) = updated {
                    stream_hub
                        .fanout(
                            &task_id,
                            &TaskEvent {
                                event_type: "progress".into(),
                                task_id: Some(task_id.clone()),
                                message: Some(record.message),
                                progress: Some(record.progress),
                                status: Some(record.status.to_string()),
                                content: None,
                                data: None,
                                error: None,
                            },
                        )
                        .await;
                } else {
                    break;
                }

                last_message = state.message.clone();
                last_progress = progress;
                last_status = state.status.clone();
            }

            if should_stop {
                break;
            }
        }
    })
}

pub(crate) async fn best_effort_wait_for_human_after_schedule_failure(
    db: &DatabaseConnection,
    lease: &NovelAutopilotNextTickLease,
    error_code: &str,
) {
    match NovelAutopilotRepository::transition_owned(
        db,
        &lease.run_id,
        &lease.user_id,
        lease.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    {
        Ok(_) => {
            tracing::warn!(
                event = "novel_book_autopilot_schedule_failure_waiting_human",
                error_code,
                run_id = %lease.run_id,
                run_epoch = lease.epoch,
                run_version = lease.version,
                "durable novel autopilot entered waiting_human after next tick scheduling failed"
            );
        }
        Err(
            NovelAutopilotRepositoryError::StaleVersion
            | NovelAutopilotRepositoryError::StaleEpoch
            | NovelAutopilotRepositoryError::InvalidTransition,
        ) => {
            tracing::info!(
                event = "novel_book_autopilot_schedule_failure_superseded",
                error_code,
                run_id = %lease.run_id,
                run_epoch = lease.epoch,
                run_version = lease.version,
                "durable novel autopilot schedule failure was superseded by a newer run state"
            );
        }
        Err(error) => {
            tracing::error!(
                event = "novel_book_autopilot_schedule_failure_fallback_failed",
                error_code,
                repository_error_code = error.code(),
                run_id = %lease.run_id,
                run_epoch = lease.epoch,
                run_version = lease.version,
                "durable novel autopilot could not enter waiting_human after scheduling failure"
            );
        }
    }
}

async fn best_effort_wait_for_human_after_execution_failure(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: &Value,
    error_code: &str,
) {
    let Some(run_id) = payload.get("run_id").and_then(Value::as_str) else {
        tracing::error!(
            event = "novel_book_autopilot_execution_failure_scope_invalid",
            error_code,
            background_task_id = %record.task_id,
            "durable novel autopilot execution failure payload has no run id"
        );
        return;
    };
    let Some(run_epoch) = payload.get("run_epoch").and_then(Value::as_i64) else {
        tracing::error!(
            event = "novel_book_autopilot_execution_failure_scope_invalid",
            error_code,
            run_id,
            background_task_id = %record.task_id,
            "durable novel autopilot execution failure payload has no run epoch"
        );
        return;
    };

    match NovelAutopilotRepository::fail_active_task_and_wait_owned(
        db,
        run_id,
        &record.user_id,
        run_epoch,
        &record.task_id,
        error_code,
    )
    .await
    {
        Ok(waiting) => {
            tracing::warn!(
                event = "novel_book_autopilot_execution_failure_waiting_human",
                error_code,
                run_id = %waiting.id,
                run_epoch = waiting.epoch,
                run_version = waiting.version,
                background_task_id = %record.task_id,
                "durable novel autopilot execution failure converged to waiting_human"
            );
        }
        Err(
            NovelAutopilotRepositoryError::StaleVersion
            | NovelAutopilotRepositoryError::StaleEpoch
            | NovelAutopilotRepositoryError::InvalidTransition,
        ) => {
            tracing::info!(
                event = "novel_book_autopilot_execution_failure_superseded",
                error_code,
                run_id,
                run_epoch,
                background_task_id = %record.task_id,
                "durable novel autopilot execution failure was superseded by a newer owner"
            );
        }
        Err(error) => {
            tracing::error!(
                event = "novel_book_autopilot_execution_failure_convergence_failed",
                error_code,
                repository_error_code = error.code(),
                run_id,
                run_epoch,
                background_task_id = %record.task_id,
                "durable novel autopilot execution failure could not converge its run"
            );
        }
    }
}

async fn execute_task(
    db: &DatabaseConnection,
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    book_import_service: Arc<BookImportService>,
    candidate_gateway_config: Option<ChapterCandidateRouteGatewayConfig>,
    record: &TaskRecord,
    payload: serde_json::Value,
    cancellation_token: CooperativeCancellationToken,
) -> Result<(), String> {
    match record.task_type.as_str() {
        "novel_autopilot" => {
            let result = execute_novel_autopilot_task(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("小说自动驾驶已执行确认操作".to_string()),
            )
            .await;
        }
        "novel_book_autopilot" => {
            let candidate_gateway_config = candidate_gateway_config
                .as_ref()
                .ok_or_else(|| NOVEL_BOOK_AUTOPILOT_SCHEDULE_FAILED.to_string())?;
            let output_observer =
                NovelAutopilotOutputObserver::new(stream_hub.clone(), record.task_id.clone());
            let outcome = match execute_novel_book_autopilot_tick(
                db,
                record,
                payload.clone(),
                candidate_gateway_config,
                &output_observer,
                cancellation_token.clone(),
            )
            .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    if !is_novel_autopilot_execution_cancelled(&error) {
                        best_effort_wait_for_human_after_execution_failure(
                            db,
                            record,
                            &payload,
                            NOVEL_BOOK_AUTOPILOT_EXECUTION_FAILED,
                        )
                        .await;
                    }
                    return Err(error);
                }
            };
            let task_result = match outcome {
                NovelAutopilotTickOutcome::Completed { task_result }
                | NovelAutopilotTickOutcome::AwaitingHuman { task_result } => task_result,
                NovelAutopilotTickOutcome::ScheduleNext {
                    mut task_result,
                    lease,
                } => {
                    match schedule_owned_novel_book_autopilot_tick(
                        db.clone(),
                        registry.clone(),
                        stream_hub.clone(),
                        book_import_service.clone(),
                        candidate_gateway_config.clone(),
                        &lease,
                        None,
                    )
                    .await
                    {
                        Ok(NovelBookAutopilotTaskScheduleOutcome::Scheduled { task }) => {
                            if let Some(result) = task_result.as_object_mut() {
                                result.insert("next_task".to_string(), task);
                            }
                        }
                        Ok(NovelBookAutopilotTaskScheduleOutcome::Superseded) => {
                            if let Some(result) = task_result.as_object_mut() {
                                result.insert(
                                    "next_dispatch_status".to_string(),
                                    json!("superseded"),
                                );
                            }
                        }
                        Err(error) => {
                            let error_code = error.code();
                            tracing::error!(
                                event = "novel_book_autopilot_next_tick_schedule_failed",
                                error_code,
                                run_id = %lease.run_id,
                                run_epoch = lease.epoch,
                                run_version = lease.version,
                                parent_task_id = %record.task_id,
                                "durable novel autopilot could not schedule its next tick"
                            );
                            best_effort_wait_for_human_after_schedule_failure(
                                db, &lease, error_code,
                            )
                            .await;
                            return Err(NOVEL_BOOK_AUTOPILOT_SCHEDULE_FAILED.to_string());
                        }
                    }
                    task_result
                }
            };
            let completion_message = novel_book_autopilot_completion_message(&task_result);
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                task_result,
                Some(completion_message),
            )
            .await;
        }
        "wizard_world_building" => {
            let result = run_wizard_world_building(
                db,
                registry,
                stream_hub,
                record,
                payload,
                cancellation_token.clone(),
            )
            .await?;
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
            let result = run_wizard_career_system(
                db,
                registry,
                stream_hub,
                record,
                payload,
                cancellation_token.clone(),
            )
            .await?;
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
            let result = run_wizard_characters(
                db,
                registry,
                stream_hub,
                record,
                payload,
                cancellation_token.clone(),
            )
            .await?;
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
            let result = run_wizard_outline(
                db,
                registry,
                stream_hub,
                record,
                payload,
                cancellation_token.clone(),
            )
            .await?;
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
            let result = run_world_regenerate(
                db,
                registry,
                stream_hub,
                record,
                payload,
                cancellation_token.clone(),
            )
            .await?;
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
        "inspiration_generate_options" => {
            let result = run_inspiration_generate_options(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("灵感选项生成完成".to_string()),
            )
            .await;
        }
        "inspiration_refine_options" => {
            let result = run_inspiration_refine_options(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("灵感选项优化完成".to_string()),
            )
            .await;
        }
        "inspiration_quick_generate" => {
            let result = run_inspiration_quick_generate(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("灵感补全完成".to_string()),
            )
            .await;
        }
        "chapter_partial_regenerate" => {
            let result = run_chapter_partial_regenerate(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("局部重写完成".to_string()),
            )
            .await;
        }
        "chapter_regenerate" => {
            let result = run_chapter_regenerate(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("章节重生成完成".to_string()),
            )
            .await;
        }
        "book_import_apply" => {
            let result = run_book_import_apply(
                db,
                book_import_service.as_ref(),
                registry,
                stream_hub,
                record,
                payload,
                cancellation_token.clone(),
            )
            .await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("拆书导入完成".to_string()),
            )
            .await;
        }
        "book_import_retry_failed_steps" => {
            let result = run_book_import_retry_failed_steps(
                db,
                book_import_service.as_ref(),
                registry,
                stream_hub,
                record,
                payload,
                cancellation_token.clone(),
            )
            .await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("拆书失败步骤重试完成".to_string()),
            )
            .await;
        }
        "polish_text" => {
            let result = run_polish_text(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("AI 去味完成".to_string()),
            )
            .await;
        }
        "polish_batch" => {
            let result = run_polish_batch(db, record, payload).await?;
            complete_task(
                registry,
                stream_hub,
                &record.task_id,
                result,
                Some("批量 AI 去味完成".to_string()),
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
    cancellation_token: CooperativeCancellationToken,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<WorldBuildingRequest>(payload)
        .map_err(|error| format!("无效的世界观任务参数: {}", error))?;
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let progress_bridge_handle = spawn_channel_progress_bridge(
        registry.clone(),
        stream_hub.clone(),
        record.task_id.clone(),
        state_capture.clone(),
        cancellation_token.clone(),
    );

    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_world_building_request(db, &channel, &record.user_id, body).await;

    drop(channel);
    let _ = drain_handle.await;
    progress_bridge_handle.abort();
    let _ = progress_bridge_handle.await;
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
    cancellation_token: CooperativeCancellationToken,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<CareerSystemRequest>(payload)
        .map_err(|error| format!("无效的职业体系任务参数: {}", error))?;
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let progress_bridge_handle = spawn_channel_progress_bridge(
        registry.clone(),
        stream_hub.clone(),
        record.task_id.clone(),
        state_capture.clone(),
        cancellation_token.clone(),
    );
    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_career_system_request(db, &channel, &record.user_id, body).await;

    drop(channel);
    let _ = drain_handle.await;
    progress_bridge_handle.abort();
    let _ = progress_bridge_handle.await;
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
    cancellation_token: CooperativeCancellationToken,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<CharactersRequest>(payload)
        .map_err(|error| format!("无效的角色任务参数: {}", error))?;
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let progress_bridge_handle = spawn_channel_progress_bridge(
        registry.clone(),
        stream_hub.clone(),
        record.task_id.clone(),
        state_capture.clone(),
        cancellation_token.clone(),
    );
    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_characters_request(db, &channel, &record.user_id, body).await;

    drop(channel);
    let _ = drain_handle.await;
    progress_bridge_handle.abort();
    let _ = progress_bridge_handle.await;
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
    cancellation_token: CooperativeCancellationToken,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<OutlineRequest>(payload)
        .map_err(|error| format!("无效的大纲任务参数: {}", error))?;
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let progress_bridge_handle = spawn_channel_progress_bridge(
        registry.clone(),
        stream_hub.clone(),
        record.task_id.clone(),
        state_capture.clone(),
        cancellation_token.clone(),
    );
    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_outline_request(db, &channel, &record.user_id, body).await;

    drop(channel);
    let _ = drain_handle.await;
    progress_bridge_handle.abort();
    let _ = progress_bridge_handle.await;
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
    cancellation_token: CooperativeCancellationToken,
) -> Result<serde_json::Value, String> {
    let body =
        serde_json::from_value::<RegenerateWorldBuildingRequest>(payload).unwrap_or_default();
    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let progress_bridge_handle = spawn_channel_progress_bridge(
        registry.clone(),
        stream_hub.clone(),
        record.task_id.clone(),
        state_capture.clone(),
        cancellation_token.clone(),
    );
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
    progress_bridge_handle.abort();
    let _ = progress_bridge_handle.await;
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

async fn run_inspiration_generate_options(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    execute_generate_options_task(db, &record.user_id, payload).await
}

async fn run_inspiration_refine_options(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    execute_refine_options_task(db, &record.user_id, payload).await
}

async fn run_inspiration_quick_generate(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    execute_quick_generate_task(db, &record.user_id, payload).await
}

async fn run_chapter_partial_regenerate(
    db: &DatabaseConnection,
    record: &TaskRecord,
    mut payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let chapter_id = payload
        .get("chapter_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "chapter_id is required for chapter_partial_regenerate".to_string())?
        .to_string();

    if let Some(map) = payload.as_object_mut() {
        map.remove("chapter_id");
    }

    let route_request = serde_json::from_value::<PartialRegenerationStreamRouteRequest>(payload)
        .map_err(|error| format!("无效的局部重写任务参数: {}", error))?;
    let request =
        build_partial_regeneration_stream_workflow_request_from_route_payload(route_request);

    execute_partial_regeneration_task(db, &record.user_id, &chapter_id, request)
        .await
        .map_err(|error| match error {
            crate::services::chapter_regeneration_stream_workflow_service::CreateRegenerationStreamWorkflowError::Chapter(_) => {
                "章节访问失败".to_string()
            }
            crate::services::chapter_regeneration_stream_workflow_service::CreateRegenerationStreamWorkflowError::Prepare(_) => {
                "局部重写准备失败".to_string()
            }
            crate::services::chapter_regeneration_stream_workflow_service::CreateRegenerationStreamWorkflowError::TaskLifecycle(_) => {
                "局部重写任务状态失败".to_string()
            }
        })
}

async fn run_chapter_regenerate(
    db: &DatabaseConnection,
    record: &TaskRecord,
    mut payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let chapter_id = take_required_string(&mut payload, "chapter_id", "chapter_regenerate")?;
    let route_request =
        serde_json::from_value::<FullChapterRegenerationStreamRouteRequest>(payload)
            .map_err(|error| format!("无效的章节重生成任务参数: {}", error))?;
    let request = build_full_chapter_regeneration_stream_request_from_route_payload(route_request);

    execute_chapter_regeneration_task(db, &record.user_id, &chapter_id, request)
        .await
        .map_err(|error| match error {
            crate::services::chapter_regeneration_stream_workflow_service::CreateRegenerationStreamWorkflowError::Chapter(_) => {
                "章节访问失败".to_string()
            }
            crate::services::chapter_regeneration_stream_workflow_service::CreateRegenerationStreamWorkflowError::Prepare(_) => {
                "章节重生成准备失败".to_string()
            }
            crate::services::chapter_regeneration_stream_workflow_service::CreateRegenerationStreamWorkflowError::TaskLifecycle(_) => {
                "章节重生成任务状态失败".to_string()
            }
        })
}

async fn run_book_import_apply(
    db: &DatabaseConnection,
    service: &BookImportService,
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    record: &TaskRecord,
    mut payload: serde_json::Value,
    cancellation_token: CooperativeCancellationToken,
) -> Result<serde_json::Value, String> {
    let book_import_task_id =
        take_required_string(&mut payload, "book_import_task_id", "book_import_apply")?;
    let project_suggestion = payload
        .get("project_suggestion")
        .cloned()
        .unwrap_or(Value::Null);
    let chapters = payload
        .get("chapters")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let outlines = payload
        .get("outlines")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let import_mode = payload
        .get("import_mode")
        .and_then(Value::as_str)
        .unwrap_or("append")
        .to_string();

    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let progress_bridge_handle = spawn_channel_progress_bridge(
        registry.clone(),
        stream_hub.clone(),
        record.task_id.clone(),
        state_capture.clone(),
        cancellation_token.clone(),
    );
    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    service
        .apply_import_stream(
            db,
            &book_import_task_id,
            &record.user_id,
            &project_suggestion,
            &chapters,
            &outlines,
            &import_mode,
            &channel,
        )
        .await;

    drop(channel);
    let _ = drain_handle.await;
    progress_bridge_handle.abort();
    let _ = progress_bridge_handle.await;
    sync_channel_state_to_task(
        registry,
        stream_hub,
        &record.task_id,
        state_capture,
        result_capture,
    )
    .await
}

async fn run_book_import_retry_failed_steps(
    db: &DatabaseConnection,
    service: &BookImportService,
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    record: &TaskRecord,
    mut payload: serde_json::Value,
    cancellation_token: CooperativeCancellationToken,
) -> Result<serde_json::Value, String> {
    let book_import_task_id = take_required_string(
        &mut payload,
        "book_import_task_id",
        "book_import_retry_failed_steps",
    )?;
    let steps = payload
        .get("steps")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if steps.is_empty() {
        return Err("steps is required for book_import_retry_failed_steps".to_string());
    }

    let (tx, mut rx) = mpsc::channel(256);
    let result_capture = Arc::new(Mutex::new(None));
    let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
    let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());
    let progress_bridge_handle = spawn_channel_progress_bridge(
        registry.clone(),
        stream_hub.clone(),
        record.task_id.clone(),
        state_capture.clone(),
        cancellation_token.clone(),
    );
    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    service
        .retry_stream(db, &book_import_task_id, &record.user_id, &steps, &channel)
        .await;

    drop(channel);
    let _ = drain_handle.await;
    progress_bridge_handle.abort();
    let _ = progress_bridge_handle.await;
    sync_channel_state_to_task(
        registry,
        stream_hub,
        &record.task_id,
        state_capture,
        result_capture,
    )
    .await
}

async fn run_polish_text(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<PolishRequest>(payload)
        .map_err(|error| format!("无效的 AI 去味任务参数: {}", error))?;
    execute_polish_text_task(db, &record.user_id, body).await
}

async fn run_polish_batch(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = serde_json::from_value::<PolishBatchRequest>(payload)
        .map_err(|error| format!("无效的批量 AI 去味任务参数: {}", error))?;
    execute_polish_batch_task(db, &record.user_id, body).await
}

fn take_required_string(
    payload: &mut Value,
    field_name: &str,
    task_type: &str,
) -> Result<String, String> {
    let value = payload
        .get(field_name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{} is required for {}", field_name, task_type))?
        .to_string();

    if let Some(map) = payload.as_object_mut() {
        map.remove(field_name);
    }

    Ok(value)
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
    use std::sync::Arc;

    use axum::{
        extract::{Extension, Path},
        http::StatusCode,
        response::IntoResponse,
    };
    use chrono::{DateTime, NaiveDate, Utc};
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
        Schema, Set, Statement,
    };
    use serde_json::json;
    use tokio::sync::{mpsc, Barrier, Mutex};

    use super::{
        adapt_character_generation_task_request, adapt_organization_generation_task_request,
        build_background_tasks_route_owner_contract, build_connected_task_event,
        build_missing_task_payload, build_task_list_response, cancel_active_task, cancel_task,
        compatible_task_payload, complete_task, enrich_task_payload, execute_task, fail_task,
        map_task_list_query_request_error, mark_task_running, next_task_stream_data,
        normalize_task_statuses_query, novel_book_autopilot_completion_message,
        prepare_task_execution_payload, spawn_channel_progress_bridge, spawn_task_execution,
        subscribe_task_with_latest_snapshot, sync_channel_state_to_task,
        task_type_allows_empty_project, wait_for_task_not_before, TaskListQueryRequestError,
        TaskListRequest, TaskStreamState, BACKGROUND_TASKS_CANCEL_ROUTE,
        BACKGROUND_TASKS_DETAIL_ROUTE, BACKGROUND_TASKS_LIST_CREATE_ROUTE,
        BACKGROUND_TASKS_STREAM_ROUTE, BACKGROUND_TASKS_WORKFLOW_STATE_ROUTE,
    };
    use crate::models::{autopilot_invocation_audit, project};
    use crate::services::auth::Claims;
    use crate::services::autopilot_invocation_audit_service::{
        create_queued_autopilot_invocation_audit, list_project_autopilot_invocation_audits,
        mark_autopilot_invocation_running,
    };
    use crate::services::autopilot_safety_gate_fixture as safety_fixture;
    use crate::services::book_import_service::BookImportService;
    use crate::services::cooperative_cancellation_service::{
        CooperativeCancellationRegistry, CooperativeCancellationScope,
    };
    use crate::tasks::registry::TaskRegistry;
    use crate::tasks::stream::TaskStreamHub;
    use crate::tasks::types::{TaskEvent, TaskListQuery, TaskRecord, TaskStatus};
    use crate::utils::sse::{SseChannel, SseTaskCapture};

    async fn setup_project_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect project sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create projects table");
        db.execute(
            builder.build(&schema.create_table_from_entity(autopilot_invocation_audit::Entity)),
        )
        .await
        .expect("create autopilot invocation audits table");
        db
    }

    #[tokio::test]
    async fn autopilot_not_before_allows_missing_and_elapsed_deadlines() {
        let cancellation_registry = CooperativeCancellationRegistry::default();
        let registration = cancellation_registry.register(
            CooperativeCancellationScope::BackgroundTask,
            "not-before-elapsed",
        );
        let token = registration.token();

        assert_eq!(wait_for_task_not_before(&json!({}), &token).await, Ok(true));
        let elapsed = (Utc::now() - chrono::Duration::seconds(1)).to_rfc3339();
        assert_eq!(
            wait_for_task_not_before(&json!({"not_before": elapsed}), &token).await,
            Ok(true)
        );
        registration.cleanup();
    }

    #[tokio::test]
    async fn autopilot_not_before_wait_is_cooperatively_cancellable() {
        let cancellation_registry = CooperativeCancellationRegistry::default();
        let registration = cancellation_registry.register(
            CooperativeCancellationScope::BackgroundTask,
            "not-before-cancelled",
        );
        let token = registration.token();
        let future = (Utc::now() + chrono::Duration::minutes(1)).to_rfc3339();
        assert!(cancellation_registry.cancel(
            CooperativeCancellationScope::BackgroundTask,
            "not-before-cancelled"
        ));

        assert_eq!(
            wait_for_task_not_before(&json!({"not_before": future}), &token).await,
            Ok(false)
        );
        registration.cleanup();
    }

    #[tokio::test]
    async fn autopilot_not_before_rejects_malformed_deadline() {
        let cancellation_registry = CooperativeCancellationRegistry::default();
        let registration = cancellation_registry.register(
            CooperativeCancellationScope::BackgroundTask,
            "not-before-invalid",
        );
        let token = registration.token();

        assert_eq!(
            wait_for_task_not_before(&json!({"not_before": "invalid"}), &token).await,
            Err("novel_autopilot_not_before_invalid".to_string())
        );
        registration.cleanup();
    }

    #[tokio::test]
    async fn spawned_task_stays_pending_until_not_before_then_starts_once() {
        let db = setup_project_db().await;
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        let mut record = task_record();
        record.task_id = "not-before-execution-boundary".to_string();
        record.task_type = "not_before_test_unsupported".to_string();
        record.status = TaskStatus::Pending;
        record.progress = 0;
        record.started_at = None;
        registry.insert(record.clone()).await;
        let mut receiver = stream_hub.subscribe(&record.task_id).await;
        let not_before = (Utc::now() + chrono::Duration::milliseconds(250)).to_rfc3339();

        spawn_task_execution(
            db,
            registry.clone(),
            stream_hub,
            Arc::new(BookImportService::new()),
            None,
            record.clone(),
            json!({"not_before": not_before}),
        );

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), receiver.recv())
                .await
                .is_err(),
            "task emitted an execution event before its persisted deadline"
        );
        let waiting = registry
            .get(&record.task_id)
            .await
            .expect("waiting task remains registered");
        assert_eq!(waiting.status, TaskStatus::Pending);
        assert_eq!(waiting.progress, 0);
        assert!(waiting.started_at.is_none());

        let running_payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), receiver.recv())
                .await
                .expect("task should start after its persisted deadline")
                .expect("running event channel remains available");
        let running_event: TaskEvent =
            serde_json::from_str(&running_payload).expect("running event is valid JSON");
        assert_eq!(running_event.event_type, "progress");
        assert_eq!(running_event.status.as_deref(), Some("running"));

        let terminal_payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), receiver.recv())
                .await
                .expect("test executor should reach a terminal state")
                .expect("terminal event is delivered");
        let terminal_event: TaskEvent =
            serde_json::from_str(&terminal_payload).expect("terminal event is valid JSON");
        assert_eq!(terminal_event.event_type, "error");
        assert_eq!(terminal_event.status.as_deref(), Some("failed"));
        assert!(receiver.recv().await.is_err());

        let terminal = registry
            .get(&record.task_id)
            .await
            .expect("terminal task remains registered");
        assert_eq!(terminal.status, TaskStatus::Failed);
        assert!(terminal.started_at.is_some());
    }

    #[test]
    fn novel_autopilot_completion_message_distinguishes_no_candidate_failures() {
        assert_eq!(
            novel_book_autopilot_completion_message(&json!({
                "dispatch_status": "waiting_human",
                "candidate_id": "step-1",
                "reason_code": "chapter_generation_attempts_exhausted",
            })),
            "候选已保存，等待人工复核"
        );
        assert_eq!(
            novel_book_autopilot_completion_message(&json!({
                "dispatch_status": "waiting_human",
                "reason_code": "chapter_repair_provider_timeout",
            })),
            "模型 Provider 调用失败，未生成可供人工接受的候选"
        );
        assert_eq!(
            novel_book_autopilot_completion_message(&json!({
                "dispatch_status": "waiting_human",
                "reason_code": "chapter_analysis_result_invalid",
            })),
            "模型返回的章节结果无效，未生成可供人工接受的候选"
        );
        assert_eq!(
            novel_book_autopilot_completion_message(&json!({
                "dispatch_status": "waiting_human",
                "reason_code": "chapter_repair_context_invalid",
            })),
            "章节上下文无效，未生成可供人工接受的候选"
        );
    }

    async fn insert_project(db: &DatabaseConnection, id: &str, user_id: &str, status: &str) {
        let created_at = NaiveDate::from_ymd_opt(2026, 7, 16)
            .expect("valid date")
            .and_hms_opt(8, 0, 0)
            .expect("valid time");
        project::ActiveModel {
            id: Set(id.to_string()),
            user_id: Set(user_id.to_string()),
            title: Set(format!("Workflow {id}")),
            target_words: Set(100_000),
            current_words: Set(0),
            status: Set(status.to_string()),
            wizard_status: Set("completed".to_string()),
            wizard_step: Set(0),
            outline_mode: Set("linear".to_string()),
            character_count: Set(0),
            created_at: Set(created_at),
            updated_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert project");
    }

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
            terminal_reason: None,
            terminal_label: None,
            review_required: None,
            can_resume: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn global_background_task_types_allow_empty_project() {
        for task_type in [
            "wizard_world_building",
            "inspiration_generate_options",
            "inspiration_refine_options",
            "inspiration_quick_generate",
            "book_import_apply",
            "book_import_retry_failed_steps",
            "polish_text",
            "polish_batch",
        ] {
            assert!(
                task_type_allows_empty_project(task_type),
                "{task_type} should be executable without a project"
            );
        }
    }

    #[test]
    fn project_scoped_background_task_types_require_project() {
        for task_type in [
            "novel_autopilot",
            "chapter_regenerate",
            "chapter_partial_regenerate",
            "world_regenerate",
            "outline_generate",
            "character_generate",
        ] {
            assert!(
                !task_type_allows_empty_project(task_type),
                "{task_type} should require a project"
            );
        }
    }

    #[tokio::test]
    async fn novel_autopilot_executor_projects_confirmed_receipt_into_generic_task_result() {
        let db = setup_project_db().await;
        insert_project(&db, "project-1", "owner-1", "foundation").await;
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        let mut record = task_record();
        record.task_type = "novel_autopilot".to_string();
        record.user_id = "owner-1".to_string();
        record.project_id = "project-1".to_string();
        let payload = json!({
            "tool_name": "transition_project_workflow",
            "arguments": r#"{"project_id":"project-1","expected_phase":"foundation","target_phase":"world_building"}"#,
            "confirmed_by_user": true
        });
        create_queued_autopilot_invocation_audit(&db, &record, &payload)
            .await
            .expect("queued audit before direct executor test");
        registry.insert(record.clone()).await;
        let cancellation_registration = CooperativeCancellationRegistry::default().register(
            CooperativeCancellationScope::BackgroundTask,
            record.task_id.clone(),
        );
        let service = BookImportService::new();

        execute_task(
            &db,
            &registry,
            &stream_hub,
            Arc::new(service),
            None,
            &record,
            payload,
            cancellation_registration.token(),
        )
        .await
        .expect("autopilot executor succeeds");
        cancellation_registration.cleanup();

        let stored = registry.get(&record.task_id).await.expect("completed task");
        assert_eq!(stored.status, TaskStatus::Completed);
        assert_eq!(stored.progress, 100);
        assert_eq!(
            stored
                .result
                .as_ref()
                .and_then(|result| result.get("schema_version"))
                .and_then(serde_json::Value::as_str),
            Some("autopilot-tool-contract/v1")
        );
        assert_eq!(
            stored
                .result
                .as_ref()
                .and_then(|result| result.get("tool_name"))
                .and_then(serde_json::Value::as_str),
            Some("transition_project_workflow")
        );
    }

    #[tokio::test]
    async fn novel_autopilot_terminal_audit_failure_keeps_generic_task_as_failed_terminal_owner() {
        let db = setup_project_db().await;
        insert_project(
            &db,
            safety_fixture::PROJECT_ID,
            safety_fixture::OWNER_ID,
            safety_fixture::EXPECTED_PHASE,
        )
        .await;
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        let mut record = task_record();
        record.task_id = safety_fixture::TASK_ID.to_string();
        record.task_type = "novel_autopilot".to_string();
        record.user_id = safety_fixture::OWNER_ID.to_string();
        record.project_id = safety_fixture::PROJECT_ID.to_string();
        record.status = TaskStatus::Pending;
        record.progress = 0;
        record.message = "等待执行".to_string();
        let payload = safety_fixture::confirmed_transition_payload(safety_fixture::PROJECT_ID);
        create_queued_autopilot_invocation_audit(&db, &record, &payload)
            .await
            .expect("queue audit before generic runner execution");
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            format!(
                "CREATE TRIGGER g2_delete_audit_before_generic_terminal \
                 AFTER UPDATE OF status ON projects \
                 BEGIN DELETE FROM autopilot_invocation_audits WHERE task_id = '{}'; END",
                safety_fixture::TASK_ID
            ),
        ))
        .await
        .expect("install terminal audit failure trigger");
        registry.insert(record.clone()).await;

        spawn_task_execution(
            db.clone(),
            registry.clone(),
            stream_hub,
            Arc::new(BookImportService::new()),
            None,
            record.clone(),
            payload,
        );

        let terminal = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                let stored = registry
                    .get(&record.task_id)
                    .await
                    .expect("generic task remains in registry");
                match stored.status {
                    TaskStatus::Failed => return stored,
                    TaskStatus::Completed | TaskStatus::Cancelled => {
                        panic!("generic task must fail when terminal audit projection fails")
                    }
                    TaskStatus::Pending | TaskStatus::Running => {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                }
            }
        })
        .await
        .expect("generic task should reach a terminal state");

        assert_eq!(terminal.status, TaskStatus::Failed);
        assert_eq!(
            terminal.error.as_deref(),
            Some("autopilot task execution failed")
        );
        assert_eq!(terminal.result, None);
        let workflow = project::Entity::find_by_id(safety_fixture::PROJECT_ID)
            .one(&db)
            .await
            .expect("read workflow after generic task terminal failure")
            .expect("fixture project remains available");
        assert_eq!(workflow.status, safety_fixture::EXPECTED_PHASE);
        let audits = list_project_autopilot_invocation_audits(&db, safety_fixture::PROJECT_ID, 10)
            .await
            .expect("fallback audit remains readable");
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].status, "failed");
        assert_eq!(
            audits[0].error_code.as_deref(),
            Some(safety_fixture::TERMINAL_AUDIT_FAILURE_CODE)
        );
    }
    #[tokio::test]
    async fn cancel_task_route_marks_running_novel_autopilot_audit_cancelled() {
        let db = setup_project_db().await;
        insert_project(&db, "project-1", "owner-1", "foundation").await;
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        let mut record = task_record();
        record.task_id = "autopilot-cancel-route-task".to_string();
        record.task_type = "novel_autopilot".to_string();
        record.user_id = "owner-1".to_string();
        record.project_id = "project-1".to_string();
        let payload = json!({
            "tool_name": "transition_project_workflow",
            "arguments": r#"{"project_id":"project-1","expected_phase":"foundation","target_phase":"world_building"}"#,
            "confirmed_by_user": true,
        });
        create_queued_autopilot_invocation_audit(&db, &record, &payload)
            .await
            .expect("queued audit before cancellation route test");
        mark_autopilot_invocation_running(&db, &record.task_id)
            .await
            .expect("running audit before cancellation route test");
        registry.insert(record.clone()).await;

        let response = cancel_task(
            Extension(Claims {
                sub: "owner-1".to_string(),
                username: "owner".to_string(),
                is_admin: false,
                exp: 0,
                iat: 0,
            }),
            Extension(db.clone()),
            Extension(registry.clone()),
            Extension(stream_hub),
            Path(record.task_id.clone()),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            registry
                .get(&record.task_id)
                .await
                .expect("cancelled task record")
                .status,
            TaskStatus::Cancelled
        );
        let audits = list_project_autopilot_invocation_audits(&db, "project-1", 20)
            .await
            .expect("read cancelled audit");
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].task_id, record.task_id);
        assert_eq!(audits[0].status, "cancelled");
        assert_eq!(audits[0].error_code.as_deref(), Some("cancelled_by_user"));
        assert!(audits[0].completed_at.is_some());
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

    #[tokio::test]
    async fn concurrent_running_admission_and_cancellation_leave_task_cancelled() {
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        let mut record = task_record();
        record.status = TaskStatus::Pending;
        record.progress = 0;
        record.message = "等待执行".to_string();
        registry.insert(record).await;

        let barrier = Arc::new(Barrier::new(3));
        let mark_barrier = barrier.clone();
        let cancel_barrier = barrier.clone();
        let mark_future = async {
            mark_barrier.wait().await;
            mark_task_running(&registry, &stream_hub, "task-1", "任务已开始执行").await
        };
        let cancel_future = async {
            cancel_barrier.wait().await;
            cancel_active_task(&registry, "task-1", "user-1").await
        };
        let release_future = async {
            barrier.wait().await;
        };

        let (running_admitted, cancelled, ()) =
            tokio::join!(mark_future, cancel_future, release_future);
        let cancelled = cancelled.expect("pending or running task should be cancelled");
        let stored = registry.get("task-1").await.expect("task should remain");

        assert_eq!(cancelled.status, TaskStatus::Cancelled);
        assert_eq!(stored.status, TaskStatus::Cancelled);
        assert_eq!(stored.message, "任务已取消");
        assert_eq!(stored.completed_at, cancelled.completed_at);
        assert_eq!(stored.updated_at, cancelled.updated_at);
        assert_eq!(
            stored
                .checkpoint
                .as_ref()
                .and_then(|checkpoint| checkpoint["event"].as_str()),
            Some("cancelled")
        );
        assert!(
            !mark_task_running(&registry, &stream_hub, "task-1", "不应重新运行").await,
            "cancelled task must remain terminal after concurrent admission"
        );
        if running_admitted {
            assert!(stored.started_at.is_some());
        }
    }

    #[tokio::test]
    async fn cancelled_task_is_not_reactivated_or_overwritten_by_executor_completion() {
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        let record = task_record();
        registry.insert(record).await;

        let cancelled = cancel_active_task(&registry, "task-1", "user-1")
            .await
            .expect("running task should be cancelled");
        let completed_at = cancelled
            .completed_at
            .expect("cancelled task should record completion time");
        let checkpoint_updated_at = cancelled
            .checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint["updated_at"].as_str())
            .expect("cancel checkpoint should record updated_at")
            .parse::<DateTime<Utc>>()
            .expect("checkpoint updated_at should be RFC3339");

        assert_eq!(cancelled.status, TaskStatus::Cancelled);
        assert_eq!(cancelled.updated_at, completed_at);
        assert_eq!(checkpoint_updated_at, completed_at);
        let mut receiver = stream_hub.subscribe("task-1").await;

        assert!(
            !mark_task_running(&registry, &stream_hub, "task-1", "不应重新运行").await,
            "cancelled task must not be admitted for execution"
        );
        complete_task(
            &registry,
            &stream_hub,
            "task-1",
            json!({"late": "result"}),
            Some("不应完成".to_string()),
        )
        .await;
        fail_task(&registry, &stream_hub, "task-1", "不应失败").await;

        assert!(
            matches!(
                receiver.try_recv(),
                Err(tokio::sync::broadcast::error::TryRecvError::Empty)
            ),
            "rejected late lifecycle events must not be broadcast"
        );

        let unchanged = registry.get("task-1").await.expect("task should remain");
        assert_eq!(unchanged.status, TaskStatus::Cancelled);
        assert_eq!(unchanged.message, "任务已取消");
        assert_eq!(unchanged.completed_at, Some(completed_at));
        assert_eq!(unchanged.updated_at, completed_at);
        assert_eq!(unchanged.result, None);
        assert_eq!(unchanged.error, None);
        assert_eq!(unchanged.terminal_reason, None);
        assert_eq!(unchanged.terminal_label, None);
        assert_eq!(unchanged.review_required, None);
        assert_eq!(unchanged.can_resume, None);
    }

    #[tokio::test]
    async fn recovered_failed_task_remains_terminal_with_recovery_semantics() {
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        let mut recovered = task_record();
        recovered.status = TaskStatus::Failed;
        recovered.error = Some("服务重启后需要人工确认".to_string());
        recovered.message = "恢复策略已投影".to_string();
        recovered.terminal_reason = Some("manual_review".to_string());
        recovered.terminal_label = Some("需要人工确认".to_string());
        recovered.review_required = Some(true);
        recovered.can_resume = Some(false);
        recovered.completed_at = Some(Utc::now());
        let original = recovered.clone();
        registry.insert(recovered).await;

        assert!(
            !mark_task_running(&registry, &stream_hub, "task-1", "不应重新运行").await,
            "recovered failed task must not be admitted for execution"
        );
        complete_task(
            &registry,
            &stream_hub,
            "task-1",
            json!({"late": "result"}),
            Some("不应完成".to_string()),
        )
        .await;
        fail_task(&registry, &stream_hub, "task-1", "不应覆盖恢复诊断").await;

        let unchanged = registry.get("task-1").await.expect("task should remain");
        assert_eq!(unchanged.status, TaskStatus::Failed);
        assert_eq!(unchanged.message, original.message);
        assert_eq!(unchanged.error, original.error);
        assert_eq!(unchanged.completed_at, original.completed_at);
        assert_eq!(unchanged.updated_at, original.updated_at);
        assert_eq!(unchanged.result, None);
        assert_eq!(unchanged.terminal_reason, original.terminal_reason);
        assert_eq!(unchanged.terminal_label, original.terminal_label);
        assert_eq!(unchanged.review_required, original.review_required);
        assert_eq!(unchanged.can_resume, original.can_resume);
    }

    #[tokio::test]
    async fn channel_success_waits_for_complete_task_to_own_terminal_projection() {
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        registry.insert(task_record()).await;
        let state_capture = Arc::new(Mutex::new({
            let mut capture = SseTaskCapture::default();
            capture.message = Some("流式处理完成".to_string());
            capture.progress = Some(100);
            capture.status = Some("success".to_string());
            capture.result = Some(json!({"chapter": "done"}));
            capture.done = true;
            capture
        }));
        let result_capture = Arc::new(Mutex::new(None));

        let result = sync_channel_state_to_task(
            &registry,
            &stream_hub,
            "task-1",
            state_capture,
            result_capture,
        )
        .await
        .expect("channel result should be returned");

        let active = registry.get("task-1").await.expect("task should remain");
        assert_eq!(active.status, TaskStatus::Running);
        assert_eq!(active.progress, 100);
        assert_eq!(active.completed_at, None);
        assert_eq!(active.result, None);

        complete_task(
            &registry,
            &stream_hub,
            "task-1",
            result.clone(),
            Some("任务执行完成".to_string()),
        )
        .await;

        let completed = registry.get("task-1").await.expect("task should remain");
        assert_eq!(completed.status, TaskStatus::Completed);
        assert_eq!(completed.result, Some(result));
        assert!(completed.completed_at.is_some());
        assert_eq!(completed.updated_at, completed.completed_at.unwrap());
        assert_eq!(completed.terminal_reason, None);
        assert_eq!(completed.terminal_label, None);
        assert_eq!(completed.review_required, None);
        assert_eq!(completed.can_resume, None);
    }

    #[tokio::test]
    async fn channel_state_sync_does_not_mutate_cancelled_terminal_record() {
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        registry.insert(task_record()).await;
        let cancelled = cancel_active_task(&registry, "task-1", "user-1")
            .await
            .expect("running task should be cancelled");
        let state_capture = Arc::new(Mutex::new({
            let mut capture = SseTaskCapture::default();
            capture.message = Some("迟到的成功事件".to_string());
            capture.progress = Some(100);
            capture.status = Some("success".to_string());
            capture.result = Some(json!({"late": true}));
            capture.done = true;
            capture
        }));

        let result = sync_channel_state_to_task(
            &registry,
            &stream_hub,
            "task-1",
            state_capture,
            Arc::new(Mutex::new(None)),
        )
        .await
        .expect("captured result remains available to the executor");
        assert_eq!(result, json!({"late": true}));

        let unchanged = registry.get("task-1").await.expect("task should remain");
        assert_eq!(unchanged.status, TaskStatus::Cancelled);
        assert_eq!(unchanged.message, cancelled.message);
        assert_eq!(unchanged.progress, cancelled.progress);
        assert_eq!(unchanged.updated_at, cancelled.updated_at);
        assert_eq!(unchanged.completed_at, cancelled.completed_at);
        assert_eq!(unchanged.result, None);
    }

    #[tokio::test]
    async fn channel_state_sync_flushes_transient_output_without_persisting_it() {
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        registry.insert(task_record()).await;
        let mut receiver = stream_hub.subscribe("task-1").await;
        let (tx, _rx) = mpsc::channel(8);
        let result_capture = Arc::new(Mutex::new(None));
        let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
        let channel = SseChannel::with_captures(tx, result_capture.clone(), state_capture.clone());

        channel.reasoning_chunk("显式推理").await;
        channel.chunk("生成正文").await;
        channel.result(&json!({"saved": true})).await;

        let result = sync_channel_state_to_task(
            &registry,
            &stream_hub,
            "task-1",
            state_capture,
            result_capture,
        )
        .await
        .expect("captured result should be returned");
        assert_eq!(result, json!({"saved": true}));

        let reasoning: TaskEvent = serde_json::from_str(
            &receiver
                .recv()
                .await
                .expect("reasoning event should be broadcast"),
        )
        .expect("reasoning event should be valid JSON");
        assert_eq!(reasoning.event_type, "reasoning_chunk");
        assert_eq!(reasoning.content.as_deref(), Some("显式推理"));

        let content: TaskEvent = serde_json::from_str(
            &receiver
                .recv()
                .await
                .expect("content event should be broadcast"),
        )
        .expect("content event should be valid JSON");
        assert_eq!(content.event_type, "chunk");
        assert_eq!(content.content.as_deref(), Some("生成正文"));

        let record = registry.get("task-1").await.expect("task should remain");
        assert_eq!(record.result, None);
        assert!(!record.message.contains("显式推理"));
        assert!(!record.message.contains("生成正文"));
    }

    #[tokio::test]
    async fn channel_progress_bridge_stops_after_terminal_update_is_rejected() {
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        registry.insert(task_record()).await;
        let cancelled = cancel_active_task(&registry, "task-1", "user-1")
            .await
            .expect("running task should be cancelled");
        let state_capture = Arc::new(Mutex::new({
            let mut capture = SseTaskCapture::default();
            capture.message = Some("迟到的进度事件".to_string());
            capture.progress = Some(99);
            capture.status = Some("processing".to_string());
            capture
        }));

        let cancellation_registry = CooperativeCancellationRegistry::default();
        let registration =
            cancellation_registry.register(CooperativeCancellationScope::BackgroundTask, "task-1");
        let mut bridge = spawn_channel_progress_bridge(
            registry.clone(),
            stream_hub,
            "task-1".to_string(),
            state_capture,
            registration.token(),
        );
        if tokio::time::timeout(std::time::Duration::from_secs(1), &mut bridge)
            .await
            .is_err()
        {
            bridge.abort();
            panic!("terminal task must stop the channel progress bridge");
        }

        let unchanged = registry.get("task-1").await.expect("task should remain");
        assert_eq!(unchanged.status, TaskStatus::Cancelled);
        assert_eq!(unchanged.message, cancelled.message);
        assert_eq!(unchanged.progress, cancelled.progress);
        assert_eq!(unchanged.updated_at, cancelled.updated_at);
        assert_eq!(unchanged.completed_at, cancelled.completed_at);
        assert_eq!(unchanged.result, None);
    }

    #[tokio::test]
    async fn channel_progress_bridge_exits_when_cancellation_token_is_signalled() {
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        registry.insert(task_record()).await;
        let cancellation_registry = CooperativeCancellationRegistry::default();
        let registration =
            cancellation_registry.register(CooperativeCancellationScope::BackgroundTask, "task-1");
        let token = registration.token();
        let bridge = spawn_channel_progress_bridge(
            registry,
            stream_hub,
            "task-1".to_string(),
            Arc::new(Mutex::new(SseTaskCapture::default())),
            token.clone(),
        );

        assert!(token.cancel());
        tokio::time::timeout(std::time::Duration::from_secs(1), bridge)
            .await
            .expect("cancelled bridge should exit before timeout")
            .expect("cancelled bridge should not panic");
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
    fn prepare_task_execution_payload_preserves_strict_novel_autopilot_invocation() {
        let mut record = task_record();
        record.task_type = "novel_autopilot".to_string();
        let payload = json!({
            "tool_name": "transition_project_workflow",
            "arguments": "{\"project_id\":\"project-1\"}",
            "confirmed_by_user": true,
        });

        let prepared = prepare_task_execution_payload(&record, payload.clone());

        assert_eq!(prepared, payload);
        assert!(prepared.get("project_id").is_none());
        assert!(prepared.get("user_id").is_none());
    }

    #[test]
    fn prepare_task_execution_payload_enriches_non_autopilot_task() {
        let payload = prepare_task_execution_payload(&task_record(), json!({"hello": "world"}));

        assert_eq!(payload["hello"], "world");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["user_id"], "user-1");
    }
    #[test]
    fn compatible_task_payload_exposes_recovery_semantics_at_top_level_and_data() {
        let mut record = task_record();
        record.status = TaskStatus::Failed;
        record.terminal_reason = Some("manual_review".to_string());
        record.terminal_label = Some("需要人工确认".to_string());
        record.review_required = Some(true);
        record.can_resume = Some(false);

        let payload = compatible_task_payload(&record);

        for target in [&payload, &payload["data"]] {
            assert_eq!(target["terminal_reason"], "manual_review");
            assert_eq!(target["terminal_label"], "需要人工确认");
            assert_eq!(target["review_required"], true);
            assert_eq!(target["can_resume"], false);
        }
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

    #[tokio::test]
    async fn task_stream_subscription_refreshes_snapshot_after_authorization_gap() {
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();
        let authorized_record = task_record();
        registry.insert(authorized_record.clone()).await;

        complete_task(
            &registry,
            &stream_hub,
            "task-1",
            json!({"chapter": "done"}),
            Some("任务执行完成".to_string()),
        )
        .await;

        let (mut receiver, latest_record) = subscribe_task_with_latest_snapshot(
            &registry,
            &stream_hub,
            "task-1",
            authorized_record,
        )
        .await;
        let connected = build_connected_task_event("task-1", &latest_record);

        assert_eq!(latest_record.status, TaskStatus::Completed);
        assert_eq!(connected.status.as_deref(), Some("completed"));
        assert_eq!(connected.progress, Some(100));
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn lagged_task_stream_resynchronizes_and_drops_stale_buffer() {
        let registry = TaskRegistry::new();
        registry.insert(task_record()).await;
        registry
            .update("task-1", |record| {
                record.progress = 90;
                record.message = "接近完成".to_string();
            })
            .await
            .expect("task should exist");

        let (sender, receiver) = tokio::sync::broadcast::channel(2);
        sender.send("stale-progress-1".to_string()).unwrap();
        sender.send("stale-progress-2".to_string()).unwrap();
        sender.send("stale-progress-3".to_string()).unwrap();

        let state = TaskStreamState::new(receiver, registry.clone(), "task-1".to_string());
        let (payload, state) = next_task_stream_data(state)
            .await
            .expect("lagged active stream should emit a recovery snapshot");
        let event: TaskEvent =
            serde_json::from_str(&payload).expect("snapshot event should be valid JSON");

        assert_eq!(event.event_type, "connected");
        assert_eq!(event.status.as_deref(), Some("running"));
        assert_eq!(event.progress, Some(90));

        sender.send("fresh-progress".to_string()).unwrap();
        let (payload, state) = next_task_stream_data(state)
            .await
            .expect("resubscribed stream should keep receiving new events");
        assert_eq!(payload, "fresh-progress");

        registry
            .update("task-1", |record| {
                record.status = TaskStatus::Completed;
                record.progress = 100;
                record.message = "任务执行完成".to_string();
                record.result = Some(json!({"chapter": "done"}));
                record.completed_at = Some(Utc::now());
            })
            .await
            .expect("task should exist");
        sender.send("stale-progress-4".to_string()).unwrap();
        sender.send("stale-progress-5".to_string()).unwrap();
        sender.send("stale-progress-6".to_string()).unwrap();

        let (payload, terminal_state) = next_task_stream_data(state)
            .await
            .expect("lagged terminal stream should emit the latest terminal snapshot");
        let event: TaskEvent =
            serde_json::from_str(&payload).expect("terminal snapshot should be valid JSON");

        assert_eq!(event.event_type, "connected");
        assert_eq!(event.status.as_deref(), Some("completed"));
        assert_eq!(event.progress, Some(100));
        assert_eq!(
            event
                .data
                .as_ref()
                .and_then(|data| data.get("result"))
                .and_then(|result| result.get("chapter")),
            Some(&json!("done"))
        );
        assert!(next_task_stream_data(terminal_state).await.is_none());
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

async fn cancel_active_task(
    registry: &TaskRegistry,
    task_id: &str,
    user_id: &str,
) -> Option<TaskRecord> {
    let now = Utc::now();
    registry
        .update_if(
            task_id,
            |record| record.user_id == user_id && record.status.is_active(),
            |record| {
                let checkpoint = touch_checkpoint_at(
                    record.checkpoint.as_ref(),
                    "cancelled",
                    Some(record.progress),
                    Some("任务已取消"),
                    Some(&json!({"error": "用户取消"})),
                    now,
                );
                record.status = TaskStatus::Cancelled;
                record.message = "任务已取消".into();
                record.completed_at = Some(now);
                record.updated_at = now;
                record.checkpoint = Some(checkpoint);
            },
        )
        .await
}

pub(crate) async fn cancel_task_runtime(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: &str,
    user_id: &str,
) -> Option<TaskRecord> {
    let updated = cancel_active_task(registry, task_id, user_id).await?;
    global_cooperative_cancellation_registry()
        .cancel(CooperativeCancellationScope::BackgroundTask, task_id);
    let event = TaskEvent {
        event_type: "cancelled".into(),
        task_id: Some(task_id.to_string()),
        message: Some("任务已取消".into()),
        progress: None,
        status: Some("cancelled".into()),
        content: None,
        data: None,
        error: None,
    };
    stream_hub.fanout_terminal(task_id, &event).await;
    Some(updated)
}

/// POST /api/background-tasks/:task_id/cancel
pub async fn cancel_task(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    match registry.get(&task_id).await {
        Some(record) if record.user_id == claims.sub && record.status.is_active() => {
            let Some(updated) =
                cancel_task_runtime(&registry, &stream_hub, &task_id, &claims.sub).await
            else {
                return (
                    StatusCode::OK,
                    Json(json!({"success": false, "message": "任务不存在或已完成"})),
                )
                    .into_response();
            };

            if updated.task_type == "novel_autopilot" {
                if let Err(error) = mark_autopilot_invocation_cancelled(&db, &task_id).await {
                    tracing::error!(
                        event = "autopilot_invocation_audit_cancel_update_failed",
                        task_id = %task_id,
                        error_code = error.code(),
                        "autopilot invocation audit cancellation could not be persisted"
                    );
                }
            }

            (
                StatusCode::OK,
                Json({
                    let mut payload = compatible_task_payload(&updated);
                    if let Some(map) = payload.as_object_mut() {
                        map.insert("message".to_string(), json!("任务已取消"));
                    }
                    payload
                }),
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
                    content: None,
                    data: None,
                    error: None,
                };
                stream_hub.fanout(&task_id, &event).await;

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

    // Subscribe before refreshing the snapshot so a transition after authorization is represented
    // either by the latest connected snapshot or by the queued broadcast stream.
    let (rx, record) =
        subscribe_task_with_latest_snapshot(&registry, &stream_hub, &task_id, record).await;
    let status_event = build_connected_task_event(&task_id, &record);
    let initial_json = serde_json::to_string(&status_event).unwrap_or_default();

    let events = futures::stream::unfold(
        TaskStreamState::new(rx, registry.clone(), task_id.clone()),
        next_task_stream_data,
    )
    .map(|data| {
        Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default().data(data))
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
