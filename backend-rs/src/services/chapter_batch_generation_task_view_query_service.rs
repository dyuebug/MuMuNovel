use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_read_context_service::{
    load_batch_generation_read_context_for_task,
};
use crate::services::chapter_batch_generation_status_semantics_service::active_batch_generation_statuses;
use crate::services::project_access_query_service::{
    ensure_owned_project_access, ProjectAccessQueryError,
};

const ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_DEFAULT: u64 = 20;
const ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MAX: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActiveBatchGenerationTaskListQueryRequest {
    limit: u64,
}

impl ActiveBatchGenerationTaskListQueryRequest {
    pub(crate) fn from_route_limit(limit: Option<u64>) -> Self {
        Self {
            limit: limit
                .unwrap_or(ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_DEFAULT)
                .clamp(1, ACTIVE_BATCH_GENERATION_TASK_LIST_LIMIT_MAX),
        }
    }

    pub(crate) fn limit(&self) -> u64 {
        self.limit
    }
}

pub(crate) async fn load_active_user_batch_generation_task_list_view(
    db: &DatabaseConnection,
    user_id: &str,
    request: ActiveBatchGenerationTaskListQueryRequest,
) -> Result<Value, String> {
    let tasks = batch_generation_task::Entity::find()
        .filter(batch_generation_task::Column::UserId.eq(user_id))
        .filter(batch_generation_task::Column::Status.is_in(active_batch_generation_statuses()))
        .order_by_desc(batch_generation_task::Column::CreatedAt)
        .limit(request.limit())
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut items = Vec::with_capacity(tasks.len());
    for task in tasks {
        items.push(
            load_batch_generation_read_context_for_task(db, task)
                .await?
                .into_active_task_payload(),
        );
    }

    Ok(json!({
        "total": items.len(),
        "items": items,
    }))
}

pub(crate) async fn load_active_batch_generation_query(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, ProjectAccessQueryError> {
    ensure_owned_project_access(db, project_id, user_id).await?;

    let task = batch_generation_task::Entity::find()
        .filter(batch_generation_task::Column::UserId.eq(user_id))
        .filter(batch_generation_task::Column::Status.is_in(active_batch_generation_statuses()))
        .order_by_desc(batch_generation_task::Column::CreatedAt)
        .filter(batch_generation_task::Column::ProjectId.eq(project_id))
        .one(db)
        .await
        .map_err(|error| ProjectAccessQueryError::Internal(error.to_string()))?;

    let task_payload = match task {
        Some(task) => Some(
            load_batch_generation_read_context_for_task(db, task)
                .await
                .map(|context| context.into_active_task_payload())
                .map_err(ProjectAccessQueryError::Internal)?,
        ),
        None => None,
    };

    Ok(json!({
        "has_active_task": task_payload.is_some(),
        "task": task_payload,
    }))
}

#[cfg(test)]
mod tests {
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_quality_status_service::BatchGenerationQualityStatusContext;
    use crate::services::chapter_batch_generation_read_context_service::BatchGenerationReadContext;
    use crate::services::project_access_query_service::ProjectAccessQueryError;
    use serde_json::{json, Value};
    use super::ActiveBatchGenerationTaskListQueryRequest;

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
        let payload = BatchGenerationReadContext {
            task: build_task("completed"),
            workflow_runtime_state: Some(json!({"progress": 80})),
            quality_status_context: BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({"score": 91})),
                quality_metrics_summary: Some(json!({"summary": "ok"})),
                active_story_repair_payload: Some(json!({"mode": "repair"})),
            },
        }
        .into_status_task_payload();

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 80);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.completed");
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
                quality_metrics_summary: Some(json!({"summary": "good"})),
                active_story_repair_payload: Some(json!({"mode": "repair"})),
            },
        }
        .into_active_task_payload();

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
            .map(BatchGenerationReadContext::into_active_task_payload)
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
    fn should_keep_active_task_payload_loader_projection_contract() {
        let payload = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(json!({"progress": 42})),
            quality_status_context: BatchGenerationQualityStatusContext::default(),
        }
        .into_active_task_payload();

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["checkpoint"]["progress"], 42);
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
    fn should_normalize_active_batch_generation_task_list_query_request_limit() {
        assert_eq!(
            ActiveBatchGenerationTaskListQueryRequest::from_route_limit(None).limit(),
            20
        );
        assert_eq!(
            ActiveBatchGenerationTaskListQueryRequest::from_route_limit(Some(0)).limit(),
            1
        );
        assert_eq!(
            ActiveBatchGenerationTaskListQueryRequest::from_route_limit(Some(25)).limit(),
            25
        );
        assert_eq!(
            ActiveBatchGenerationTaskListQueryRequest::from_route_limit(Some(500)).limit(),
            100
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
                    quality_metrics_summary: Some(json!({"summary": "good"})),
                    active_story_repair_payload: Some(json!({"mode": "repair"})),
                },
            }
            .into_active_task_payload(),
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
        let payload = json!({
            "total": 1,
            "items": [json!({"batch_id": "task-1"})],
        });

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["batch_id"], "task-1");
    }

    #[test]
    fn should_keep_active_batch_generation_query_view_owner_contract() {
        let payload = json!({
            "has_active_task": true,
            "task": json!({"batch_id": "task-2"}),
        });

        assert_eq!(payload["has_active_task"], true);
        assert_eq!(payload["task"]["batch_id"], "task-2");
    }
}
