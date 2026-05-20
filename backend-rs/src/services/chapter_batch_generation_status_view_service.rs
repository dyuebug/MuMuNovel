use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Select,
};
use serde_json::{json, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_owned_task, load_required_owned_task, map_owned_batch_generation_task_error,
};
use crate::services::chapter_batch_generation_quality_status_service::build_quality_status_context;
use crate::services::chapter_batch_generation_runtime_state_service::load_batch_generation_snapshot;
use crate::services::chapter_batch_generation_status_payload_adapter_service::{
    active_task_payload, task_status_payload,
};
use crate::services::chapter_batch_generation_status_semantics_service::active_batch_generation_statuses;

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationTaskViewContext {
    pub(crate) task: batch_generation_task::Model,
    pub(crate) workflow_runtime_state: Option<Value>,
    pub(crate) latest_quality_metrics: Option<Value>,
    pub(crate) quality_metrics_summary: Option<Value>,
    pub(crate) active_story_repair_payload: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadBatchGenerationTaskViewContextError {
    TaskNotFound,
    Internal(String),
}

async fn build_batch_generation_task_view_context(
    db: &DatabaseConnection,
    task: batch_generation_task::Model,
) -> Result<BatchGenerationTaskViewContext, String> {
    let snapshot = load_batch_generation_snapshot(db, &task.id).await?;
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.clone());
    let quality_context =
        build_quality_status_context(snapshot.as_ref(), workflow_runtime_state.as_ref());

    Ok(BatchGenerationTaskViewContext {
        task,
        workflow_runtime_state,
        latest_quality_metrics: quality_context.latest_quality_metrics,
        quality_metrics_summary: quality_context.quality_metrics_summary,
        active_story_repair_payload: quality_context.active_story_repair_payload,
    })
}

async fn build_optional_batch_generation_task_view_context(
    db: &DatabaseConnection,
    task: Option<batch_generation_task::Model>,
) -> Result<Option<BatchGenerationTaskViewContext>, String> {
    let Some(task) = task else {
        return Ok(None);
    };

    build_batch_generation_task_view_context(db, task)
        .await
        .map(Some)
}

async fn build_batch_generation_task_view_contexts(
    db: &DatabaseConnection,
    tasks: Vec<batch_generation_task::Model>,
) -> Result<Vec<BatchGenerationTaskViewContext>, String> {
    let mut contexts = Vec::with_capacity(tasks.len());
    for task in tasks {
        contexts.push(build_batch_generation_task_view_context(db, task).await?);
    }
    Ok(contexts)
}

fn build_active_batch_generation_task_view_payload(context: BatchGenerationTaskViewContext) -> Value {
    active_task_payload(
        &context.task,
        context.workflow_runtime_state,
        context.latest_quality_metrics,
        context.quality_metrics_summary,
        context.active_story_repair_payload,
    )
}

pub(crate) fn build_batch_generation_status_query_response(
    context: BatchGenerationTaskViewContext,
) -> Value {
    task_status_payload(
        &context.task,
        context.workflow_runtime_state,
        context.latest_quality_metrics,
        context.quality_metrics_summary,
        context.active_story_repair_payload,
    )
}

pub(crate) fn build_active_batch_generation_query_response(
    task: Option<BatchGenerationTaskViewContext>,
) -> Value {
    match task {
        Some(context) => json!({
            "has_active_task": true,
            "task": build_active_batch_generation_task_view_payload(context),
        }),
        None => json!({
            "has_active_task": false,
            "task": null,
        }),
    }
}

pub(crate) fn build_active_batch_generation_task_list_query_response(
    contexts: Vec<BatchGenerationTaskViewContext>,
) -> Value {
    let items: Vec<Value> = contexts
        .into_iter()
        .map(build_active_batch_generation_task_view_payload)
        .collect();

    json!({
        "total": items.len(),
        "items": items,
    })
}

pub(crate) fn build_batch_generation_progress_event(
    state: &BatchGenerationStreamState,
) -> Value {
    json!({
        "type": "progress",
        "message": state.message,
        "progress": state.progress,
        "status": state.event_status,
    })
}

pub(crate) fn build_batch_generation_result_event(state: &BatchGenerationStreamState) -> Value {
    json!({
        "type": "result",
        "data": {
            "generation_task_id": state.task.id,
            "chapter_id": state.task.current_chapter_id,
            "content_source": "chapter",
        }
    })
}

pub(crate) fn build_batch_generation_failed_event(state: &BatchGenerationStreamState) -> Value {
    json!({
        "type": "error",
        "error": state
            .task
            .error_message
            .clone()
            .unwrap_or_else(|| "Generation task failed.".to_string()),
        "code": 500
    })
}

pub(crate) fn build_batch_generation_cancelled_event() -> Value {
    json!({
        "type": "error",
        "error": "Generation task was cancelled.",
        "code": 499
    })
}

pub(crate) fn build_batch_generation_not_found_event() -> Value {
    json!({
        "type": "error",
        "error": "Batch generation task not found",
        "code": 404
    })
}

pub(crate) fn build_batch_generation_timeout_event() -> Value {
    json!({
        "type": "error",
        "error": "Generation stream timed out.",
        "code": 408
    })
}

pub(crate) fn build_batch_generation_terminal_events(
    state: &BatchGenerationStreamState,
) -> Option<Vec<Value>> {
    match state.status.as_str() {
        "completed" => Some(vec![
            build_batch_generation_result_event(state),
            json!({"type":"done"}),
        ]),
        "failed" => Some(vec![build_batch_generation_failed_event(state)]),
        "cancelled" => Some(vec![build_batch_generation_cancelled_event()]),
        _ => None,
    }
}

pub(crate) async fn load_required_batch_generation_task_view_context(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<BatchGenerationTaskViewContext, LoadBatchGenerationTaskViewContextError> {
    let task = load_required_owned_task(db, batch_id, user_id)
        .await
        .map_err(|error| {
            map_owned_batch_generation_task_error(
                error,
                || LoadBatchGenerationTaskViewContextError::TaskNotFound,
                LoadBatchGenerationTaskViewContextError::Internal,
            )
        })?;

    build_batch_generation_task_view_context(db, task)
        .await
        .map_err(LoadBatchGenerationTaskViewContextError::Internal)
}

fn build_active_batch_generation_task_query(
    user_id: &str,
) -> Select<batch_generation_task::Entity> {
    batch_generation_task::Entity::find()
        .filter(batch_generation_task::Column::UserId.eq(user_id))
        .filter(batch_generation_task::Column::Status.is_in(active_batch_generation_statuses()))
        .order_by_desc(batch_generation_task::Column::CreatedAt)
}

pub(crate) async fn load_active_project_batch_generation_task_view_context(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Option<BatchGenerationTaskViewContext>, String> {
    let task = build_active_batch_generation_task_query(user_id)
        .filter(batch_generation_task::Column::ProjectId.eq(project_id))
        .order_by_desc(batch_generation_task::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| error.to_string())?;

    build_optional_batch_generation_task_view_context(db, task).await
}

pub(crate) async fn load_active_user_batch_generation_task_view_contexts(
    db: &DatabaseConnection,
    user_id: &str,
    limit: u64,
) -> Result<Vec<BatchGenerationTaskViewContext>, String> {
    let tasks = build_active_batch_generation_task_query(user_id)
        .limit(limit)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    build_batch_generation_task_view_contexts(db, tasks).await
}

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationStreamState {
    pub(crate) task: batch_generation_task::Model,
    pub(crate) status: String,
    pub(crate) completed: i32,
    pub(crate) progress: i32,
    pub(crate) message: String,
    pub(crate) event_status: &'static str,
}

fn default_stream_progress(status: &str) -> i32 {
    match status {
        "pending" => 10,
        "running" => 65,
        "completed" => 100,
        "failed" => 100,
        "cancelled" => 100,
        _ => 15,
    }
}

fn default_stream_message(status: &str) -> &'static str {
    match status {
        "pending" => "等待开始生成...",
        "running" => "正在生成正文...",
        "completed" => "生成完成",
        "failed" => "生成失败",
        "cancelled" => "生成已取消",
        _ => "任务处理中",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationStreamSemantics {
    progress: i32,
    message: String,
    event_status: &'static str,
}

fn map_batch_generation_stream_event_status(status: &str) -> &'static str {
    match status {
        "failed" => "error",
        "completed" => "success",
        _ => "processing",
    }
}

fn resolve_batch_generation_stream_semantics(
    status: &str,
    checkpoint: Option<&Value>,
) -> BatchGenerationStreamSemantics {
    let progress = checkpoint
        .and_then(|item| item.get("progress"))
        .and_then(Value::as_i64)
        .map(|value| value.clamp(0, 100) as i32)
        .unwrap_or_else(|| default_stream_progress(status));
    let message = checkpoint
        .and_then(|item| item.get("last_message"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_stream_message(status))
        .to_string();

    BatchGenerationStreamSemantics {
        progress,
        message,
        event_status: map_batch_generation_stream_event_status(status),
    }
}

pub(crate) async fn load_batch_generation_stream_state(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Option<BatchGenerationStreamState>, String> {
    let task = load_owned_task(db, batch_id, user_id).await?;
    let Some(task) = task else {
        return Ok(None);
    };

    let snapshot = load_batch_generation_snapshot(db, &task.id).await?;
    let checkpoint = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.as_ref());
    let status = task.status.clone();
    let completed = task.completed_chapters;
    let semantics = resolve_batch_generation_stream_semantics(&status, checkpoint);

    Ok(Some(BatchGenerationStreamState {
        task,
        status,
        completed,
        progress: semantics.progress,
        message: semantics.message,
        event_status: semantics.event_status,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        build_active_batch_generation_query_response,
        build_active_batch_generation_task_list_query_response,
        build_batch_generation_cancelled_event, build_batch_generation_failed_event,
        build_batch_generation_not_found_event, build_batch_generation_progress_event,
        build_batch_generation_result_event, build_batch_generation_status_query_response,
        build_batch_generation_terminal_events, build_batch_generation_timeout_event,
        map_batch_generation_stream_event_status, resolve_batch_generation_stream_semantics,
        BatchGenerationTaskViewContext, BatchGenerationStreamState,
    };
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_quality_status_service::terminal_semantics;
    use serde_json::json;

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
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    #[test]
    fn should_map_manual_review_failed_task_to_terminal_review_state() {
        let mut task = build_task("failed");
        task.failed_chapters = json!([{
            "quality_gate_decision": "manual_review",
            "quality_gate_label": "manual review"
        }]);

        let payload = build_batch_generation_status_query_response(BatchGenerationTaskViewContext {
            task,
            workflow_runtime_state: None,
            latest_quality_metrics: None,
            quality_metrics_summary: None,
            active_story_repair_payload: None,
        });

        assert_eq!(payload["terminal_reason"], "manual_review");
        assert_eq!(payload["terminal_label"], "manual review");
        assert_eq!(payload["review_required"], true);
        assert_eq!(payload["can_resume"], false);
    }

    #[test]
    fn should_fallback_to_default_manual_review_label() {
        let task = build_task("failed");
        let semantics = terminal_semantics(
            &task,
            Some(&json!([{
                "quality_gate_decision": "manual_review"
            }])),
        );

        assert_eq!(semantics.0, Some("manual_review"));
        assert_eq!(semantics.1.as_deref(), Some("需人工复核"));
        assert!(semantics.2);
        assert!(!semantics.3);
    }

    #[test]
    fn should_resolve_batch_generation_stream_semantics_with_checkpoint_fallbacks() {
        let running = resolve_batch_generation_stream_semantics("running", None);
        assert_eq!(running.progress, 65);
        assert_eq!(running.message, "正在生成正文...");
        assert_eq!(running.event_status, "processing");

        let completed = resolve_batch_generation_stream_semantics(
            "completed",
            Some(&json!({
                "progress": 120,
                "last_message": "  "
            })),
        );
        assert_eq!(completed.progress, 100);
        assert_eq!(completed.message, "生成完成");
        assert_eq!(completed.event_status, "success");
    }

    #[test]
    fn should_resolve_batch_generation_stream_semantics_for_terminal_and_unknown_statuses() {
        let failed = resolve_batch_generation_stream_semantics("failed", None);
        assert_eq!(failed.progress, 100);
        assert_eq!(failed.message, "生成失败");
        assert_eq!(failed.event_status, "error");

        let cancelled = resolve_batch_generation_stream_semantics(
            "cancelled",
            Some(&json!({
                "progress": -5,
                "last_message": "已停止"
            })),
        );
        assert_eq!(cancelled.progress, 0);
        assert_eq!(cancelled.message, "已停止");
        assert_eq!(cancelled.event_status, "processing");

        let unknown = resolve_batch_generation_stream_semantics("queued", None);
        assert_eq!(unknown.progress, 15);
        assert_eq!(unknown.message, "任务处理中");
        assert_eq!(unknown.event_status, "processing");
    }

    #[test]
    fn should_map_batch_generation_stream_event_status() {
        assert_eq!(map_batch_generation_stream_event_status("failed"), "error");
        assert_eq!(
            map_batch_generation_stream_event_status("completed"),
            "success"
        );
        assert_eq!(
            map_batch_generation_stream_event_status("running"),
            "processing"
        );
    }

    #[test]
    fn should_build_batch_generation_stream_events() {
        let state = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            event_status: "success",
        };

        let progress_event = build_batch_generation_progress_event(&state);
        let result_event = build_batch_generation_result_event(&state);
        let failed_event = build_batch_generation_failed_event(&BatchGenerationStreamState {
            task: {
                let mut task = build_task("failed");
                task.error_message = Some("boom".to_string());
                task
            },
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "生成失败".to_string(),
            event_status: "error",
        });

        assert_eq!(progress_event["type"], "progress");
        assert_eq!(progress_event["status"], "success");
        assert_eq!(result_event["type"], "result");
        assert_eq!(result_event["data"]["content_source"], "chapter");
        assert_eq!(failed_event["error"], "boom");
        assert_eq!(build_batch_generation_cancelled_event()["code"], 499);
        assert_eq!(build_batch_generation_not_found_event()["code"], 404);
        assert_eq!(build_batch_generation_timeout_event()["code"], 408);
    }

    #[test]
    fn should_build_terminal_batch_generation_events() {
        let completed = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            event_status: "success",
        };
        let mut failed = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "生成失败".to_string(),
            event_status: "error",
        };
        failed.task.error_message = Some("boom".to_string());
        let cancelled = BatchGenerationStreamState {
            task: build_task("cancelled"),
            status: "cancelled".to_string(),
            completed: 0,
            progress: 100,
            message: "生成已取消".to_string(),
            event_status: "processing",
        };

        let completed_events =
            build_batch_generation_terminal_events(&completed).expect("completed events");
        assert_eq!(completed_events.len(), 2);
        assert_eq!(completed_events[0]["type"], "result");
        assert_eq!(completed_events[1]["type"], "done");

        let failed_events = build_batch_generation_terminal_events(&failed).expect("failed events");
        assert_eq!(failed_events.len(), 1);
        assert_eq!(failed_events[0]["type"], "error");
        assert_eq!(failed_events[0]["error"], "boom");

        let cancelled_events =
            build_batch_generation_terminal_events(&cancelled).expect("cancelled events");
        assert_eq!(cancelled_events.len(), 1);
        assert_eq!(cancelled_events[0]["code"], 499);

        let running = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 65,
            message: "处理中".to_string(),
            event_status: "processing",
        };
        assert!(build_batch_generation_terminal_events(&running).is_none());
    }

    #[test]
    fn should_build_active_batch_generation_query_response_from_task_context() {
        let mut task = build_task("running");
        task.total_chapters = 3;
        task.completed_chapters = 1;
        task.current_chapter_id = Some("chapter-2".to_string());
        task.current_chapter_number = Some(2);

        let payload = build_active_batch_generation_query_response(Some(
            BatchGenerationTaskViewContext {
                task,
                workflow_runtime_state: Some(json!({"progress": 40})),
                latest_quality_metrics: Some(json!({"score": 88})),
                quality_metrics_summary: Some(json!({"summary": "good"})),
                active_story_repair_payload: Some(json!({"mode": "repair"})),
            },
        ));

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
        let payload = build_active_batch_generation_query_response(None);

        assert_eq!(payload["has_active_task"], false);
        assert!(payload["task"].is_null());
    }

    #[test]
    fn should_build_active_batch_generation_task_list_query_response_from_contexts() {
        let mut first_task = build_task("running");
        first_task.id = "task-1".to_string();
        first_task.project_id = "project-1".to_string();
        first_task.total_chapters = 3;
        first_task.completed_chapters = 1;
        first_task.current_chapter_id = Some("chapter-2".to_string());
        first_task.current_chapter_number = Some(2);

        let first = BatchGenerationTaskViewContext {
            task: first_task,
            workflow_runtime_state: Some(json!({"progress": 42})),
            latest_quality_metrics: Some(json!({"score": 88})),
            quality_metrics_summary: Some(json!({"summary": "good"})),
            active_story_repair_payload: Some(json!({"mode": "repair"})),
        };

        let mut second_task = build_task("pending");
        second_task.id = "task-2".to_string();
        second_task.project_id = "project-2".to_string();
        second_task.current_chapter_id = None;
        second_task.current_chapter_number = None;
        let second = BatchGenerationTaskViewContext {
            task: second_task,
            workflow_runtime_state: None,
            latest_quality_metrics: None,
            quality_metrics_summary: None,
            active_story_repair_payload: None,
        };

        let payload =
            build_active_batch_generation_task_list_query_response(vec![first, second]);

        assert_eq!(payload["total"], 2);
        assert_eq!(payload["items"][0]["batch_id"], "task-1");
        assert_eq!(payload["items"][0]["checkpoint"]["progress"], 42);
        assert_eq!(
            payload["items"][0]["quality_metrics_summary"]["summary"],
            "good"
        );
        assert_eq!(payload["items"][1]["batch_id"], "task-2");
        assert_eq!(payload["items"][1]["status"], "pending");
        assert_eq!(
            payload["items"][1]["checkpoint"]["stage_code"],
            "6.writing.pending"
        );
        assert!(payload["items"][1]["latest_quality_metrics"].is_null());
        assert!(payload["items"][1].get("terminal_reason").is_none());
        assert!(payload["items"][1].get("current_retry_count").is_none());
    }

    #[test]
    fn should_build_empty_active_batch_generation_task_list_query_response() {
        let payload = build_active_batch_generation_task_list_query_response(vec![]);

        assert_eq!(payload["total"], 0);
        assert_eq!(payload["items"], json!([]));
    }
}
