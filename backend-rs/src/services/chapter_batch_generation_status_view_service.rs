use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde_json::{json, Value};

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_owned_task_query_service::load_owned_task;
use crate::services::chapter_batch_generation_quality_status_service::{
    build_quality_status_context,
};
pub use crate::services::chapter_batch_generation_status_semantics_service::{
    task_execution_mode, task_stage_code, task_type,
};
pub use crate::services::chapter_batch_generation_status_payload_adapter_service::{
    active_task_payload, build_active_batch_generation_response,
    build_active_batch_generation_task_list_response, build_task_status_response,
    checkpoint_with_runtime_metadata, task_status_payload, to_iso,
};

pub async fn load_batch_generation_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<Option<batch_generation_snapshot::Model>, String> {
    batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.eq(task_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone)]
pub struct BatchGenerationTaskViewContext {
    pub task: batch_generation_task::Model,
    pub workflow_runtime_state: Option<Value>,
    pub latest_quality_metrics: Option<Value>,
    pub quality_metrics_summary: Option<Value>,
    pub active_story_repair_payload: Option<Value>,
}

pub async fn build_batch_generation_task_view_context(
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

pub async fn load_batch_generation_task_view_context(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Option<BatchGenerationTaskViewContext>, String> {
    let task = load_owned_task(db, batch_id, user_id).await?;
    let Some(task) = task else {
        return Ok(None);
    };

    build_batch_generation_task_view_context(db, task)
        .await
        .map(Some)
}

pub async fn load_active_project_batch_generation_task_view_context(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Option<BatchGenerationTaskViewContext>, String> {
    let task = batch_generation_task::Entity::find()
        .filter(batch_generation_task::Column::ProjectId.eq(project_id))
        .filter(batch_generation_task::Column::UserId.eq(user_id))
        .filter(batch_generation_task::Column::Status.is_in(["pending", "running"]))
        .order_by_desc(batch_generation_task::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| error.to_string())?;
    let Some(task) = task else {
        return Ok(None);
    };

    build_batch_generation_task_view_context(db, task)
        .await
        .map(Some)
}

pub async fn load_active_user_batch_generation_task_view_contexts(
    db: &DatabaseConnection,
    user_id: &str,
    limit: u64,
) -> Result<Vec<BatchGenerationTaskViewContext>, String> {
    let tasks = batch_generation_task::Entity::find()
        .filter(batch_generation_task::Column::UserId.eq(user_id))
        .filter(batch_generation_task::Column::Status.is_in(["pending", "running"]))
        .order_by_desc(batch_generation_task::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut contexts = Vec::with_capacity(tasks.len());
    for task in tasks {
        contexts.push(build_batch_generation_task_view_context(db, task).await?);
    }
    Ok(contexts)
}

#[derive(Debug, Clone)]
pub struct BatchGenerationStreamState {
    pub task: batch_generation_task::Model,
    pub status: String,
    pub completed: i32,
    pub progress: i32,
    pub message: String,
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

pub async fn load_batch_generation_stream_state(
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
    }))
}

pub fn build_batch_generation_progress_event(state: &BatchGenerationStreamState) -> Value {
    json!({
        "type": "progress",
        "message": state.message,
        "progress": state.progress,
        "status": map_batch_generation_stream_event_status(&state.status),
    })
}

pub fn build_batch_generation_result_event(state: &BatchGenerationStreamState) -> Value {
    json!({
        "type": "result",
        "data": {
            "generation_task_id": state.task.id,
            "chapter_id": state.task.current_chapter_id,
            "content_source": "chapter",
        }
    })
}

pub fn build_batch_generation_failed_event(state: &BatchGenerationStreamState) -> Value {
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

pub fn build_batch_generation_cancelled_event() -> Value {
    json!({
        "type": "error",
        "error": "Generation task was cancelled.",
        "code": 499
    })
}

pub fn build_batch_generation_not_found_event() -> Value {
    json!({
        "type": "error",
        "error": "Batch generation task not found",
        "code": 404
    })
}

pub fn build_batch_generation_timeout_event() -> Value {
    json!({
        "type": "error",
        "error": "Generation stream timed out.",
        "code": 408
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_active_batch_generation_task_list_response, build_batch_generation_cancelled_event,
        build_batch_generation_failed_event, build_batch_generation_not_found_event,
        build_batch_generation_progress_event, build_batch_generation_result_event,
        build_batch_generation_timeout_event, map_batch_generation_stream_event_status,
        resolve_batch_generation_stream_semantics, task_status_payload, terminal_semantics,
        BatchGenerationStreamState, BatchGenerationTaskViewContext,
    };
    use crate::models::batch_generation_task;
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

        let payload = task_status_payload(&task, None, None, None, None);

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
    fn should_build_active_batch_generation_task_list_response() {
        let task = build_task("running");
        let response = build_active_batch_generation_task_list_response(vec![
            BatchGenerationTaskViewContext {
                task: task.clone(),
                workflow_runtime_state: Some(json!({
                    "phase": "generating",
                    "progress": 42,
                    "last_message": "处理中"
                })),
                latest_quality_metrics: Some(json!({"score": 90})),
                quality_metrics_summary: Some(json!({"summary": "ok"})),
                active_story_repair_payload: Some(json!({"mode": "repair"})),
            },
        ]);

        assert_eq!(response["total"], 1);
        assert_eq!(response["items"][0]["batch_id"], task.id);
        assert_eq!(response["items"][0]["status"], "running");
        assert_eq!(response["items"][0]["checkpoint"]["progress"], 42);
        assert_eq!(response["items"][0]["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_batch_generation_stream_events() {
        let state = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
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
    fn should_map_batch_generation_stream_event_status() {
        assert_eq!(map_batch_generation_stream_event_status("failed"), "error");
        assert_eq!(map_batch_generation_stream_event_status("completed"), "success");
        assert_eq!(map_batch_generation_stream_event_status("running"), "processing");
    }
}
