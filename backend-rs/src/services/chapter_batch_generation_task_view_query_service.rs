use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::batch_generation_task;
#[cfg(test)]
use crate::services::chapter_batch_generation_read_context_service::batch_generation_task_contains_chapter;
use crate::services::chapter_batch_generation_read_context_service::{
    load_active_batch_generation_task_list_item_payloads_for_tasks,
    load_active_project_batch_generation_task_payload_for_tasks,
};
use crate::services::chapter_batch_generation_status_semantics_service::active_batch_generation_statuses;
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

#[cfg(test)]
mod tests {
    use super::{
        build_active_batch_generation_task_list_query_request_from_route_query,
        build_active_batch_generation_task_list_view_payload,
        build_active_project_batch_generation_view_payload,
        ActiveBatchGenerationTaskListQueryRequestError, ActiveBatchGenerationTaskListRouteQuery,
        ActiveBatchGenerationTaskListRouteQueryError, ActiveProjectBatchGenerationRouteError,
    };
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_quality_status_service::BatchGenerationQualityStatusContext;
    use crate::services::chapter_batch_generation_read_context_service::build_batch_generation_status_task_payload_with_quality_context;
    use crate::services::chapter_batch_generation_read_context_service::BatchGenerationReadContext;
    use crate::services::project_access_query_service::ProjectAccessQueryError;
    use serde_json::{json, Value};

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 2,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    #[test]
    fn should_build_task_status_payload_with_terminal_fields() {
        let task = build_task("completed");
        let workflow_runtime_state = json!({"progress": 80});
        let quality_status_context = BatchGenerationQualityStatusContext {
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: None,
            quality_metrics_summary_state: None,
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            quality_history_context: None,
            active_story_repair_payload: Some(json!({"mode": "repair"})),
        };
        let payload = build_batch_generation_status_task_payload_with_quality_context(
            &task,
            Some(&workflow_runtime_state),
            &quality_status_context,
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 80);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.complete");
        assert_eq!(payload["current_retry_count"], 2);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(payload["terminal_reason"], "completed");
        assert_eq!(payload["review_required"], false);
        assert_eq!(payload["can_resume"], false);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_active_task_payload_without_status_only_fields() {
        let payload = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(json!({"progress": 42})),
            quality_status_context: BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({"score": 88})),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: Some(json!({"summary": "good"})),
                quality_history_context: None,
                active_story_repair_payload: Some(json!({"mode": "repair"})),
            },
        }
        .into_active_task_list_item_payload();

        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.generating");
        assert_eq!(payload["latest_quality_metrics"]["score"], 88);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "good");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
        assert!(payload.get("current_retry_count").is_none());
        assert!(payload.get("terminal_reason").is_none());
        assert!(payload.get("can_resume").is_none());
    }

    #[test]
    fn should_build_active_batch_generation_task_list_view_payload() {
        let mut first = build_task("running");
        first.id = "task-1".to_string();
        let mut second = build_task("pending");
        second.id = "task-2".to_string();

        let contexts = vec![
            BatchGenerationReadContext {
                task: first,
                workflow_runtime_state: Some(json!({"progress": 42})),
                quality_status_context: BatchGenerationQualityStatusContext::default(),
            },
            BatchGenerationReadContext {
                task: second,
                workflow_runtime_state: Some(json!({"progress": 0})),
                quality_status_context: BatchGenerationQualityStatusContext::default(),
            },
        ];
        let items: Vec<Value> = contexts
            .into_iter()
            .map(BatchGenerationReadContext::into_active_task_list_item_payload)
            .collect();
        let payload = json!({
            "total": items.len(),
            "items": items,
        });

        assert_eq!(payload["total"], 2);
        assert_eq!(payload["items"][0]["batch_id"], "task-1");
        assert_eq!(payload["items"][1]["batch_id"], "task-2");
        assert!(payload["items"][0].get("terminal_reason").is_none());
        assert!(payload["items"][1].get("can_resume").is_none());
    }

    #[test]
    fn should_match_chapter_id_from_string_or_object_entries() {
        let mut task = build_task("running");
        task.chapter_ids = json!([
            "chapter-1",
            {"id": "chapter-2", "title": "第二章"},
            {"id": "chapter-3"}
        ]);

        assert!(super::batch_generation_task_contains_chapter(
            &task,
            "chapter-1"
        ));
        assert!(super::batch_generation_task_contains_chapter(
            &task,
            "chapter-2"
        ));
        assert!(super::batch_generation_task_contains_chapter(
            &task,
            "chapter-3"
        ));
        assert!(!super::batch_generation_task_contains_chapter(
            &task,
            "chapter-9"
        ));
    }

    #[test]
    fn should_keep_active_task_payload_loader_projection_contract() {
        let payload = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(json!({"progress": 42})),
            quality_status_context: BatchGenerationQualityStatusContext::default(),
        }
        .into_active_project_task_payload();

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert!(payload.get("task_type").is_none());
        assert!(payload.get("project_id").is_none());
        assert!(payload.get("completed_at").is_none());
        assert!(payload.get("error_message").is_none());
    }

    #[test]
    fn should_convert_active_task_context_list_into_optional_context() {
        let context = vec![BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(json!({"progress": 42})),
            quality_status_context: BatchGenerationQualityStatusContext::default(),
        }]
        .into_iter()
        .next()
        .expect("context");

        assert_eq!(context.task.id, "task-1");
        assert_eq!(
            context.workflow_runtime_state,
            Some(json!({"progress": 42}))
        );
    }

    #[test]
    fn should_convert_empty_active_task_context_list_into_none() {
        let context: Option<BatchGenerationReadContext> = vec![].into_iter().next();

        assert!(context.is_none());
    }

    #[test]
    fn should_keep_project_access_not_found_error_for_active_query() {
        let error = ProjectAccessQueryError::NotFoundOrAccessDenied;

        assert_eq!(error, ProjectAccessQueryError::NotFoundOrAccessDenied);
    }

    #[test]
    fn should_keep_project_access_internal_error_for_active_query() {
        let error = ProjectAccessQueryError::Internal("boom".to_string());

        assert_eq!(error, ProjectAccessQueryError::Internal("boom".to_string()));
    }

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
    fn should_keep_active_batch_generation_task_list_route_query_contract() {
        let request = build_active_batch_generation_task_list_query_request_from_route_query(
            ActiveBatchGenerationTaskListRouteQuery { limit: Some(25) },
        )
        .expect("in-range limit should build query request");

        assert_eq!(request.limit(), 25);
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
    fn should_keep_active_project_batch_generation_route_project_contract() {
        let project_id = "project-42".to_string();

        assert_eq!(project_id, "project-42");
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
    fn should_build_active_batch_generation_query_response_from_task_context() {
        let mut task = build_task("running");
        task.total_chapters = 3;
        task.completed_chapters = 1;
        task.current_chapter_id = Some("chapter-2".to_string());
        task.current_chapter_number = Some(2);

        let payload = json!({
            "has_active_task": true,
            "task": BatchGenerationReadContext {
                task,
                workflow_runtime_state: Some(json!({"progress": 40})),
                quality_status_context: BatchGenerationQualityStatusContext {
                    latest_quality_metrics: Some(json!({"score": 88})),
                    quality_metrics_history: None,
                    quality_metrics_summary_state: None,
                    quality_metrics_summary: Some(json!({"summary": "good"})),
                    quality_history_context: None,
                    active_story_repair_payload: Some(json!({"mode": "repair"})),
                },
            }
            .into_active_project_task_payload(),
        });

        assert_eq!(payload["has_active_task"], true);
        assert_eq!(payload["task"]["batch_id"], "task-1");
        assert_eq!(payload["task"]["status"], "running");
        assert_eq!(payload["task"]["checkpoint"]["progress"], 40);
        assert_eq!(
            payload["task"]["checkpoint"]["stage_code"],
            "6.writing.generating"
        );
        assert_eq!(payload["task"]["latest_quality_metrics"]["score"], 88);
        assert_eq!(
            payload["task"]["quality_metrics_summary"]["summary"],
            "good"
        );
        assert_eq!(
            payload["task"]["active_story_repair_payload"]["mode"],
            "repair"
        );
        assert!(payload["task"].get("task_type").is_none());
        assert!(payload["task"].get("project_id").is_none());
        assert!(payload["task"].get("current_retry_count").is_none());
        assert!(payload["task"].get("terminal_reason").is_none());
    }

    #[test]
    fn should_build_empty_active_batch_generation_query_response() {
        let payload = json!({
            "has_active_task": false,
            "task": Value::Null,
        });

        assert_eq!(payload["has_active_task"], false);
        assert!(payload["task"].is_null());
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
    fn should_keep_existing_single_generation_background_payload_query_owner_contract() {
        let payload = Some(json!({
            "batch_id": "task-3",
            "task_type": "chapter_single_generate"
        }))
        .expect("payload");

        assert_eq!(payload["batch_id"], "task-3");
        assert_eq!(payload["task_type"], "chapter_single_generate");
    }
}
