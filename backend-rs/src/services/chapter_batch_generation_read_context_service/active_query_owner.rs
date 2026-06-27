use std::collections::HashMap;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{active_batch_generation_statuses, recover_generation_task_if_needed};
use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_task_payload_base_service::{
    build_batch_generation_task_view_payload_with_quality_context,
    BatchGenerationQualityStatusContext, BatchGenerationTaskViewPayloadVariant,
};
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::load_chapter_generation_snapshot_map;
use crate::services::project_service::{ProjectAccessQueryError, ProjectService};

const ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_DEFAULT: u64 = 20;
const ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MIN: i64 = 1;
const ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MAX: u64 = 100;

pub(crate) fn build_batch_generation_active_query_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_read_context_service::active_query_owner",
        "scope": "active_query_task_view_projection_and_route_read_models",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service/active_query_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/api/chapter_batch_generation.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "active_query_entrypoints": [
                "load_active_batch_generation_view_from_route_project",
                "load_active_user_batch_generation_task_list_view_from_route_query",
                "load_active_batch_generation_query",
                "load_active_user_batch_generation_task_list_view"
            ],
            "read_context_projection": [
                "BatchGenerationReadContext::from_task_and_snapshot_projection",
                "BatchGenerationReadContext::into_active_project_task_payload",
                "BatchGenerationReadContext::into_active_task_list_item_payload"
            ],
            "query_request_contract": [
                "ActiveBatchGenerationTaskListRouteQuery",
                "ActiveBatchGenerationTaskListQueryRequest",
                "ActiveBatchGenerationTaskListRouteQueryError",
                "ActiveProjectBatchGenerationRouteError"
            ],
            "task_list_limit": {
                "default": ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_DEFAULT,
                "min": ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MIN,
                "max": ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MAX
            }
        },
        "active_consumers": [
            "chapter_batch_generation_read_context_service",
            "chapter_batch_generation",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_read_context_service",
            "cargo test api::health",
            "cargo check"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "active_project_query_owner": "load_active_batch_generation_view_from_route_project",
            "active_user_task_list_owner": "load_active_user_batch_generation_task_list_view_from_route_query",
            "read_context_projection_owner": "BatchGenerationReadContext::from_task_and_snapshot_projection",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages",
            "status": "rust_batch_generation_active_query_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "source_map_policy": "batch_generation_read_context_owner_is_rust_only_and_surviving_python_query_status_surfaces_are_tracked_by_external_shared_runtime_projection_contracts",
            "route_payloads": [
                "has_active_task",
                "task",
                "total",
                "items"
            ]
        }
    })
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct ActiveBatchGenerationTaskListRouteQuery {
    pub(crate) limit: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActiveBatchGenerationTaskListQueryRequest {
    limit: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveBatchGenerationTaskListQueryRequestError {
    LimitTooSmall,
    LimitTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActiveBatchGenerationTaskListRouteQueryError {
    Request(ActiveBatchGenerationTaskListQueryRequestError),
    Query(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActiveProjectBatchGenerationRouteError {
    Query(ProjectAccessQueryError),
}

impl ActiveBatchGenerationTaskListQueryRequest {
    fn from_route_query(
        route_query: ActiveBatchGenerationTaskListRouteQuery,
    ) -> Result<Self, ActiveBatchGenerationTaskListQueryRequestError> {
        let Some(limit) = route_query.limit else {
            return Ok(Self {
                limit: ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_DEFAULT,
            });
        };

        if limit < ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MIN {
            return Err(ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall);
        }
        if limit > ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MAX as i64 {
            return Err(ActiveBatchGenerationTaskListQueryRequestError::LimitTooLarge);
        }

        Ok(Self {
            limit: limit as u64,
        })
    }

    pub(crate) fn limit(&self) -> u64 {
        self.limit
    }
}

pub(crate) fn build_active_batch_generation_task_list_query_request_from_route_query(
    route_query: ActiveBatchGenerationTaskListRouteQuery,
) -> Result<ActiveBatchGenerationTaskListQueryRequest, ActiveBatchGenerationTaskListQueryRequestError>
{
    ActiveBatchGenerationTaskListQueryRequest::from_route_query(route_query)
}

async fn load_active_batch_generation_tasks(
    db: &DatabaseConnection,
    user_id: &str,
    project_id: Option<&str>,
    limit: Option<u64>,
) -> Result<Vec<batch_generation_task::Model>, String> {
    let mut query = batch_generation_task::Entity::find()
        .filter(batch_generation_task::Column::UserId.eq(user_id))
        .filter(batch_generation_task::Column::Status.is_in(active_batch_generation_statuses()))
        .order_by_desc(batch_generation_task::Column::CreatedAt);

    if let Some(project_id) = project_id {
        query = query.filter(batch_generation_task::Column::ProjectId.eq(project_id));
    }
    if let Some(limit) = limit {
        query = query.limit(limit);
    }

    query.all(db).await.map_err(|error| error.to_string())
}

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationReadContext {
    pub(crate) task: batch_generation_task::Model,
    pub(crate) workflow_runtime_state: Option<Value>,
    pub(crate) quality_status_context: BatchGenerationQualityStatusContext,
}

impl BatchGenerationReadContext {
    pub(crate) fn from_task_and_snapshot_projection(
        task: batch_generation_task::Model,
        snapshot: Option<&batch_generation_snapshot::Model>,
        workflow_runtime_state: Option<Value>,
    ) -> Self {
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                snapshot,
                workflow_runtime_state.as_ref(),
            );

        Self {
            task,
            workflow_runtime_state,
            quality_status_context,
        }
    }

    pub(crate) fn into_payload_parts(
        self,
    ) -> (batch_generation_task::Model, serde_json::Map<String, Value>) {
        let BatchGenerationReadContext {
            task,
            workflow_runtime_state,
            quality_status_context,
        } = self;
        let payload = build_batch_generation_task_view_payload_with_quality_context(
            &task,
            workflow_runtime_state.as_ref(),
            Some(&quality_status_context),
            BatchGenerationTaskViewPayloadVariant::ActiveTaskListItem,
        );

        (task, payload)
    }

    pub(crate) fn into_active_project_task_payload(self) -> Value {
        let BatchGenerationReadContext {
            task,
            workflow_runtime_state,
            quality_status_context,
        } = self;

        Value::Object(
            build_batch_generation_task_view_payload_with_quality_context(
                &task,
                workflow_runtime_state.as_ref(),
                Some(&quality_status_context),
                BatchGenerationTaskViewPayloadVariant::ActiveProjectTask,
            ),
        )
    }

    pub(crate) fn into_active_task_list_item_payload(self) -> Value {
        let (_, payload) = self.into_payload_parts();
        Value::Object(payload)
    }
}

fn build_batch_generation_read_context_for_task_and_snapshot(
    task: batch_generation_task::Model,
    snapshot: Option<batch_generation_snapshot::Model>,
) -> BatchGenerationReadContext {
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.clone());

    BatchGenerationReadContext::from_task_and_snapshot_projection(
        task,
        snapshot.as_ref(),
        workflow_runtime_state,
    )
}

pub(crate) fn build_batch_generation_read_contexts_from_snapshot_owner_map(
    tasks: Vec<batch_generation_task::Model>,
    mut snapshots_by_task_id: HashMap<String, batch_generation_snapshot::Model>,
) -> Vec<BatchGenerationReadContext> {
    tasks
        .into_iter()
        .map(|task| {
            let snapshot = snapshots_by_task_id.remove(&task.id);
            build_batch_generation_read_context_for_task_and_snapshot(task, snapshot)
        })
        .collect()
}

pub(crate) async fn load_batch_generation_read_contexts_for_tasks(
    db: &DatabaseConnection,
    tasks: Vec<batch_generation_task::Model>,
) -> Result<Vec<BatchGenerationReadContext>, String> {
    let task_ids: Vec<String> = tasks.iter().map(|task| task.id.clone()).collect();
    let snapshots_by_task_id = load_chapter_generation_snapshot_map(db, &task_ids).await?;

    Ok(build_batch_generation_read_contexts_from_snapshot_owner_map(tasks, snapshots_by_task_id))
}

pub(crate) async fn load_active_batch_generation_read_contexts_for_tasks(
    db: &DatabaseConnection,
    tasks: Vec<batch_generation_task::Model>,
) -> Result<Vec<BatchGenerationReadContext>, String> {
    let mut active_tasks = Vec::with_capacity(tasks.len());

    for task in tasks {
        let (task, _) = recover_generation_task_if_needed(db, task).await?;
        if !active_batch_generation_statuses().contains(&task.status.as_str()) {
            continue;
        }

        active_tasks.push(task);
    }

    load_batch_generation_read_contexts_for_tasks(db, active_tasks).await
}

pub(crate) async fn load_active_batch_generation_task_list_item_payloads_for_tasks(
    db: &DatabaseConnection,
    tasks: Vec<batch_generation_task::Model>,
) -> Result<Vec<Value>, String> {
    Ok(
        load_active_batch_generation_read_contexts_for_tasks(db, tasks)
            .await?
            .into_iter()
            .map(BatchGenerationReadContext::into_active_task_list_item_payload)
            .collect(),
    )
}

pub(crate) async fn load_active_project_batch_generation_task_payload_for_tasks(
    db: &DatabaseConnection,
    tasks: Vec<batch_generation_task::Model>,
) -> Result<Option<Value>, String> {
    Ok(
        load_active_batch_generation_read_contexts_for_tasks(db, tasks)
            .await?
            .into_iter()
            .next()
            .map(BatchGenerationReadContext::into_active_project_task_payload),
    )
}

pub(crate) fn build_active_batch_generation_task_list_view_payload(items: Vec<Value>) -> Value {
    json!({
        "total": items.len(),
        "items": items,
    })
}

pub(crate) fn build_active_project_batch_generation_view_payload(
    task_payload: Option<Value>,
) -> Value {
    let has_active_task = task_payload.is_some();

    json!({
        "has_active_task": has_active_task,
        "task": task_payload,
    })
}

pub(crate) async fn load_active_user_batch_generation_task_list_view(
    db: &DatabaseConnection,
    user_id: &str,
    request: ActiveBatchGenerationTaskListQueryRequest,
) -> Result<Value, String> {
    let tasks =
        load_active_batch_generation_tasks(db, user_id, None, Some(request.limit())).await?;
    let items = load_active_batch_generation_task_list_item_payloads_for_tasks(db, tasks).await?;

    Ok(build_active_batch_generation_task_list_view_payload(items))
}

pub(crate) async fn load_active_user_batch_generation_task_list_view_from_route_query(
    db: &DatabaseConnection,
    user_id: &str,
    route_query: ActiveBatchGenerationTaskListRouteQuery,
) -> Result<Value, ActiveBatchGenerationTaskListRouteQueryError> {
    let request =
        build_active_batch_generation_task_list_query_request_from_route_query(route_query)
            .map_err(ActiveBatchGenerationTaskListRouteQueryError::Request)?;

    load_active_user_batch_generation_task_list_view(db, user_id, request)
        .await
        .map_err(ActiveBatchGenerationTaskListRouteQueryError::Query)
}

pub(crate) async fn load_active_batch_generation_query(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, ProjectAccessQueryError> {
    ProjectService::ensure_owned_access(db, project_id, user_id).await?;

    let tasks = load_active_batch_generation_tasks(db, user_id, Some(project_id), None)
        .await
        .map_err(ProjectAccessQueryError::Internal)?;
    let task_payload = load_active_project_batch_generation_task_payload_for_tasks(db, tasks)
        .await
        .map_err(ProjectAccessQueryError::Internal)?;

    Ok(build_active_project_batch_generation_view_payload(
        task_payload,
    ))
}

pub(crate) async fn load_active_batch_generation_view_from_route_project(
    db: &DatabaseConnection,
    user_id: &str,
    project_id: String,
) -> Result<Value, ActiveProjectBatchGenerationRouteError> {
    load_active_batch_generation_query(db, &project_id, user_id)
        .await
        .map_err(ActiveProjectBatchGenerationRouteError::Query)
}
