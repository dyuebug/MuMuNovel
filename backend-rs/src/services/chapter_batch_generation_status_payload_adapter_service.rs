use chrono::NaiveDateTime;
use serde_json::{json, Map, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_quality_status_service::terminal_semantics;
use crate::services::chapter_batch_generation_status_semantics_service::{
    task_execution_mode, task_stage_code, task_type,
};
use crate::services::chapter_batch_generation_status_view_service::BatchGenerationTaskViewContext;

pub fn to_iso(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.and_utc().to_rfc3339())
}

pub fn checkpoint_with_runtime_metadata(
    workflow_runtime_state: Option<&Value>,
    stage_code: &str,
    execution_mode: &str,
) -> Map<String, Value> {
    let mut checkpoint = workflow_runtime_state
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    checkpoint.insert("stage_code".to_string(), json!(stage_code));
    checkpoint.insert("execution_mode".to_string(), json!(execution_mode));
    checkpoint
}

pub fn task_status_payload(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<Value>,
    latest_quality_metrics: Option<Value>,
    quality_metrics_summary: Option<Value>,
    active_story_repair_payload: Option<Value>,
) -> Value {
    let stage_code = task_stage_code(task);
    let execution_mode = task_execution_mode(task);
    let failed_chapters = task.failed_chapters.clone();
    let checkpoint = checkpoint_with_runtime_metadata(
        workflow_runtime_state.as_ref(),
        stage_code,
        execution_mode,
    );
    let (terminal_reason, terminal_label, review_required, can_resume) =
        terminal_semantics(task, Some(&failed_chapters));

    json!({
        "batch_id": task.id,
        "task_type": task_type(task),
        "project_id": task.project_id,
        "status": task.status,
        "stage_code": stage_code,
        "execution_mode": execution_mode,
        "total": task.total_chapters,
        "completed": task.completed_chapters,
        "current_chapter_id": task.current_chapter_id,
        "current_chapter_number": task.current_chapter_number,
        "current_retry_count": task.current_retry_count,
        "max_retries": task.max_retries,
        "failed_chapters": failed_chapters,
        "created_at": to_iso(task.created_at),
        "started_at": to_iso(task.started_at),
        "completed_at": to_iso(task.completed_at),
        "error_message": task.error_message,
        "checkpoint": checkpoint,
        "latest_quality_metrics": latest_quality_metrics,
        "quality_metrics_summary": quality_metrics_summary,
        "active_story_repair_payload": active_story_repair_payload,
        "terminal_reason": terminal_reason,
        "terminal_label": terminal_label,
        "review_required": review_required,
        "can_resume": can_resume,
    })
}

pub fn active_task_payload(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<Value>,
    latest_quality_metrics: Option<Value>,
    quality_metrics_summary: Option<Value>,
    active_story_repair_payload: Option<Value>,
) -> Value {
    let stage_code = task_stage_code(task);
    let execution_mode = task_execution_mode(task);
    let checkpoint = checkpoint_with_runtime_metadata(
        workflow_runtime_state.as_ref(),
        stage_code,
        execution_mode,
    );

    json!({
        "batch_id": task.id,
        "task_type": task_type(task),
        "project_id": task.project_id,
        "status": task.status,
        "stage_code": stage_code,
        "execution_mode": execution_mode,
        "total": task.total_chapters,
        "completed": task.completed_chapters,
        "current_chapter_id": task.current_chapter_id,
        "current_chapter_number": task.current_chapter_number,
        "checkpoint": checkpoint,
        "latest_quality_metrics": latest_quality_metrics,
        "quality_metrics_summary": quality_metrics_summary,
        "active_story_repair_payload": active_story_repair_payload,
        "created_at": to_iso(task.created_at),
        "started_at": to_iso(task.started_at),
        "completed_at": to_iso(task.completed_at),
        "error_message": task.error_message,
    })
}

pub fn build_task_status_response(context: BatchGenerationTaskViewContext) -> Value {
    task_status_payload(
        &context.task,
        context.workflow_runtime_state,
        context.latest_quality_metrics,
        context.quality_metrics_summary,
        context.active_story_repair_payload,
    )
}

pub fn build_active_batch_generation_response(context: BatchGenerationTaskViewContext) -> Value {
    json!({
        "has_active_task": true,
        "task": active_task_payload(
            &context.task,
            context.workflow_runtime_state,
            context.latest_quality_metrics,
            context.quality_metrics_summary,
            context.active_story_repair_payload,
        ),
    })
}

pub fn build_active_batch_generation_task_list_response(
    contexts: Vec<BatchGenerationTaskViewContext>,
) -> Value {
    let items: Vec<Value> = contexts
        .into_iter()
        .map(|context| {
            active_task_payload(
                &context.task,
                context.workflow_runtime_state,
                context.latest_quality_metrics,
                context.quality_metrics_summary,
                context.active_story_repair_payload,
            )
        })
        .collect();

    json!({
        "total": items.len(),
        "items": items,
    })
}
