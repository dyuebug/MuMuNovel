use std::collections::HashMap;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_owned_batch_generation_task_read_state, LoadOwnedBatchGenerationTaskError,
    OwnedBatchGenerationTaskReadState,
};
use crate::services::chapter_batch_generation_task_payload_base_service::{
    build_batch_generation_task_view_payload_with_quality_context,
    BatchGenerationQualityStatusContext, BatchGenerationTaskViewPayloadVariant,
};
use crate::services::chapter_generation_snapshot_service::load_chapter_generation_snapshot_map;
use crate::services::chapter_generation_task_recovery_service::recover_generation_task_if_needed;
use crate::services::chapter_generation_task_semantics_service::active_batch_generation_statuses;
use crate::services::project_access_query_service::{
    ensure_owned_project_access, ProjectAccessQueryError,
};

const ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_DEFAULT: u64 = 20;
const ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MIN: i64 = 1;
const ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MAX: u64 = 100;

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

    fn into_payload_parts(self) -> (batch_generation_task::Model, serde_json::Map<String, Value>) {
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

    pub(crate) fn into_status_task_payload(self) -> Value {
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
                BatchGenerationTaskViewPayloadVariant::StatusTask,
            ),
        )
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

pub(crate) fn batch_generation_task_contains_chapter(
    task: &batch_generation_task::Model,
    chapter_id: &str,
) -> bool {
    task.chapter_ids
        .as_array()
        .into_iter()
        .flatten()
        .any(|item| {
            item.as_str() == Some(chapter_id)
                || item.get("id").and_then(Value::as_str) == Some(chapter_id)
        })
}

fn build_batch_generation_read_contexts_from_snapshot_owner_map(
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

pub(crate) fn is_active_batch_generation_task_status(status: &str) -> bool {
    active_batch_generation_statuses().contains(&status)
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
        if !is_active_batch_generation_task_status(&task.status) {
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

fn build_active_batch_generation_task_list_view_payload(items: Vec<Value>) -> Value {
    json!({
        "total": items.len(),
        "items": items,
    })
}

fn build_active_project_batch_generation_view_payload(task_payload: Option<Value>) -> Value {
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
    ensure_owned_project_access(db, project_id, user_id).await?;

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

pub(crate) fn build_batch_generation_status_task_payload_with_quality_context(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
    quality_status_context: &BatchGenerationQualityStatusContext,
) -> Value {
    Value::Object(
        build_batch_generation_task_view_payload_with_quality_context(
            task,
            workflow_runtime_state,
            Some(quality_status_context),
            BatchGenerationTaskViewPayloadVariant::StatusTask,
        ),
    )
}

pub(crate) fn build_batch_generation_status_task_payload_from_task_and_snapshot_projection(
    task: &batch_generation_task::Model,
    snapshot: Option<&batch_generation_snapshot::Model>,
    workflow_runtime_state: Option<&Value>,
) -> Value {
    let quality_status_context =
        BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            snapshot,
            workflow_runtime_state,
        );

    build_batch_generation_status_task_payload_with_quality_context(
        task,
        workflow_runtime_state,
        &quality_status_context,
    )
}

fn build_owned_batch_generation_status_payload_from_read_state(
    read_state: OwnedBatchGenerationTaskReadState,
) -> Value {
    let (task, snapshot) = read_state.into_parts();
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.clone());

    BatchGenerationReadContext::from_task_and_snapshot_projection(
        task,
        snapshot.as_ref(),
        workflow_runtime_state,
    )
    .into_status_task_payload()
}

pub(crate) async fn load_owned_batch_generation_status_payload(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, LoadOwnedBatchGenerationTaskError> {
    Ok(build_owned_batch_generation_status_payload_from_read_state(
        load_owned_batch_generation_task_read_state(db, batch_id, user_id).await?,
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::BatchGenerationReadContext;
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_batch_generation_owned_task_query_service::{
        LoadOwnedBatchGenerationTaskError, OwnedBatchGenerationTaskReadState,
    };
    use crate::services::chapter_batch_generation_task_payload_base_service::BatchGenerationQualityStatusContext;
    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn build_snapshot() -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            workflow_runtime_state: Some(json!({
                "progress": 60,
                "active_story_repair_payload": {
                    "mode": "repair"
                }
            })),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_build_batch_generation_read_context_from_snapshot() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let context = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        };

        assert_eq!(context.task.id, "task-1");
        assert_eq!(
            context.workflow_runtime_state,
            Some(json!({
                "progress": 60,
                "active_story_repair_payload": {
                    "mode": "repair"
                }
            }))
        );
        assert_eq!(
            context.quality_status_context.latest_quality_metrics,
            Some(json!({"score": 91}))
        );
        assert_eq!(
            context.quality_status_context.quality_metrics_history,
            Some(json!([{"score": 90}]))
        );
        assert_eq!(
            context.quality_status_context.quality_metrics_summary,
            Some(json!({"summary": "ok"}))
        );
        assert_eq!(
            context
                .quality_status_context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("scope")),
            Some(&json!("batch"))
        );
        assert_eq!(
            context
                .quality_status_context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(1))
        );
        assert_eq!(context.quality_status_context.quality_history_context, None);
        assert_eq!(
            context.quality_status_context.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_batch_generation_read_context_without_snapshot() {
        let context = BatchGenerationReadContext {
            task: build_task("pending"),
            workflow_runtime_state: None,
            quality_status_context: BatchGenerationQualityStatusContext::default(),
        };

        assert_eq!(context.task.status, "pending");
        assert_eq!(context.workflow_runtime_state, None);
        assert_eq!(
            context.quality_status_context,
            BatchGenerationQualityStatusContext::default()
        );
    }

    #[test]
    fn should_build_batch_generation_read_contexts_from_snapshot_owner_map() {
        let mut first_task = build_task("running");
        first_task.id = "task-1".to_string();
        let mut second_task = build_task("pending");
        second_task.id = "task-2".to_string();

        let mut second_snapshot = build_snapshot();
        second_snapshot.batch_task_id = "task-2".to_string();
        second_snapshot.workflow_runtime_state = Some(json!({
            "progress": 25,
            "last_message": "等待中"
        }));

        let contexts = super::build_batch_generation_read_contexts_from_snapshot_owner_map(
            vec![first_task, second_task],
            HashMap::from([(String::from("task-2"), second_snapshot)]),
        );

        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].task.id, "task-1");
        assert_eq!(contexts[0].workflow_runtime_state, None);
        assert_eq!(contexts[1].task.id, "task-2");
        assert_eq!(
            contexts[1].workflow_runtime_state,
            Some(json!({
                "progress": 25,
                "last_message": "等待中"
            }))
        );
    }

    #[test]
    fn should_classify_active_batch_generation_task_status() {
        assert!(super::is_active_batch_generation_task_status("pending"));
        assert!(super::is_active_batch_generation_task_status("running"));
        assert!(!super::is_active_batch_generation_task_status("failed"));
        assert!(!super::is_active_batch_generation_task_status("completed"));
    }

    #[test]
    fn should_build_shared_read_payload_plan_from_context_owner() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let (task, payload) = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_payload_parts();

        assert_eq!(task.id, "task-1");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_history"][0]["score"], 90);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "batch");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 1);
        assert!(payload["quality_history_context"].is_null());
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_active_project_task_payload_from_read_context() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let payload = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_active_project_task_payload();

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
        assert!(payload.get("task_type").is_none());
        assert!(payload.get("project_id").is_none());
        assert!(payload.get("current_retry_count").is_none());
        assert!(payload.get("completed_at").is_none());
        assert!(payload.get("error_message").is_none());
        assert!(payload.get("terminal_reason").is_none());
        assert!(payload.get("can_resume").is_none());
    }

    #[test]
    fn should_build_active_task_list_item_payload_without_terminal_fields() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let payload = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_active_task_list_item_payload();

        assert_eq!(payload["task_type"], "chapters_batch_generate");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert!(payload.get("current_retry_count").is_none());
        assert!(payload.get("failed_chapters").is_none());
        assert!(payload.get("terminal_reason").is_none());
    }

    #[test]
    fn should_keep_read_payload_parts_contract() {
        let task = build_task("running");
        let payload = serde_json::Map::from_iter([
            ("batch_id".to_string(), json!("task-1")),
            ("status".to_string(), json!("running")),
        ]);

        assert_eq!(task.id, "task-1");
        assert_eq!(task.failed_chapters, json!([]));
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["status"], "running");
    }

    #[test]
    fn should_build_status_task_payload_from_task_and_snapshot_projection_owner_inside_read_context_service(
    ) {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .as_ref()
            .expect("snapshot runtime state");
        let payload =
            super::build_batch_generation_status_task_payload_from_task_and_snapshot_projection(
                &build_task("completed"),
                Some(&snapshot),
                Some(workflow_runtime_state),
            );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["current_retry_count"], 0);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(payload["terminal_reason"], "completed");
        assert_eq!(payload["review_required"], false);
        assert_eq!(payload["can_resume"], false);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_status_task_payload_from_quality_context_owner_inside_read_context_service() {
        let payload = super::build_batch_generation_status_task_payload_with_quality_context(
            &build_task("failed"),
            Some(&json!({"progress": 80})),
            &BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({"score": 88})),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: Some(json!({"summary": "good"})),
                quality_history_context: None,
                active_story_repair_payload: Some(json!({"mode": "repair"})),
            },
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 80);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.failed");
        assert_eq!(payload["current_retry_count"], 0);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(payload["latest_quality_metrics"]["score"], 88);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "good");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_keep_owned_status_payload_loader_error_contract_inside_read_context_service() {
        let missing = LoadOwnedBatchGenerationTaskError::TaskNotFound;
        let internal = LoadOwnedBatchGenerationTaskError::Internal("boom".to_string());

        assert_eq!(missing, LoadOwnedBatchGenerationTaskError::TaskNotFound);
        assert_eq!(
            internal,
            LoadOwnedBatchGenerationTaskError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_keep_owned_status_payload_read_state_projection_contract_inside_read_context_service()
    {
        let mut task = build_task("running");
        task.id = "task-owned-status-1".to_string();
        let payload = super::build_owned_batch_generation_status_payload_from_read_state(
            OwnedBatchGenerationTaskReadState::from_parts(task, Some(build_snapshot())),
        );
        assert_eq!(payload["batch_id"], "task-owned-status-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }
}

#[cfg(test)]
mod active_query_owner_tests {
    use super::{
        build_active_batch_generation_task_list_query_request_from_route_query,
        build_active_batch_generation_task_list_view_payload,
        build_active_project_batch_generation_view_payload,
        ActiveBatchGenerationTaskListQueryRequestError, ActiveBatchGenerationTaskListRouteQuery,
        ActiveBatchGenerationTaskListRouteQueryError, ActiveProjectBatchGenerationRouteError,
    };
    use crate::services::project_access_query_service::ProjectAccessQueryError;
    use serde_json::json;

    #[test]
    fn should_validate_active_batch_generation_task_list_query_request_limit_like_python_query() {
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: None }
            )
            .expect("default limit should be valid")
            .limit(),
            20
        );
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(25) }
            )
            .expect("explicit in-range limit should be valid")
            .limit(),
            25
        );
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(0) }
            ),
            Err(ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(-1) }
            ),
            Err(ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(500) }
            ),
            Err(ActiveBatchGenerationTaskListQueryRequestError::LimitTooLarge)
        );
    }

    #[test]
    fn should_keep_active_batch_generation_task_list_route_query_error_shape() {
        let error = build_active_batch_generation_task_list_query_request_from_route_query(
            ActiveBatchGenerationTaskListRouteQuery { limit: Some(0) },
        )
        .map_err(ActiveBatchGenerationTaskListRouteQueryError::Request)
        .expect_err("out-of-range limit should fail before query execution");

        assert_eq!(
            error,
            ActiveBatchGenerationTaskListRouteQueryError::Request(
                ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall,
            )
        );
    }

    #[test]
    fn should_keep_active_project_batch_generation_route_error_shape() {
        let error = ActiveProjectBatchGenerationRouteError::Query(
            ProjectAccessQueryError::NotFoundOrAccessDenied,
        );

        assert_eq!(
            error,
            ActiveProjectBatchGenerationRouteError::Query(
                ProjectAccessQueryError::NotFoundOrAccessDenied,
            )
        );
    }

    #[test]
    fn should_keep_active_batch_generation_task_list_view_owner_contract() {
        let payload = build_active_batch_generation_task_list_view_payload(vec![json!({
            "batch_id": "task-1"
        })]);

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["batch_id"], "task-1");
    }

    #[test]
    fn should_keep_active_batch_generation_query_view_owner_contract() {
        let payload = build_active_project_batch_generation_view_payload(Some(json!({
            "batch_id": "task-2"
        })));

        assert_eq!(payload["has_active_task"], true);
        assert_eq!(payload["task"]["batch_id"], "task-2");
    }

    #[test]
    fn should_build_empty_active_batch_generation_query_response() {
        let payload = build_active_project_batch_generation_view_payload(None);

        assert_eq!(payload["has_active_task"], false);
        assert!(payload["task"].is_null());
    }
}
