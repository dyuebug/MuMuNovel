use chrono::NaiveDateTime;
use serde_json::{json, Map, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_quality_status_service::terminal_semantics;
use crate::services::chapter_batch_generation_status_semantics_service::{
    task_execution_mode, task_stage_code, task_type,
};

pub(crate) fn to_iso(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.and_utc().to_rfc3339())
}

pub(crate) fn checkpoint_with_runtime_metadata(
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

fn base_task_payload(
    task: &batch_generation_task::Model,
    checkpoint: Map<String, Value>,
) -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert("batch_id".to_string(), json!(task.id));
    payload.insert("task_type".to_string(), json!(task_type(task)));
    payload.insert("project_id".to_string(), json!(task.project_id));
    payload.insert("status".to_string(), json!(task.status));
    payload.insert("stage_code".to_string(), json!(task_stage_code(task)));
    payload.insert(
        "execution_mode".to_string(),
        json!(task_execution_mode(task)),
    );
    payload.insert("total".to_string(), json!(task.total_chapters));
    payload.insert("completed".to_string(), json!(task.completed_chapters));
    payload.insert(
        "current_chapter_id".to_string(),
        json!(task.current_chapter_id),
    );
    payload.insert(
        "current_chapter_number".to_string(),
        json!(task.current_chapter_number),
    );
    payload.insert("checkpoint".to_string(), Value::Object(checkpoint));
    payload.insert("created_at".to_string(), json!(to_iso(task.created_at)));
    payload.insert("started_at".to_string(), json!(to_iso(task.started_at)));
    payload.insert("completed_at".to_string(), json!(to_iso(task.completed_at)));
    payload.insert("error_message".to_string(), json!(task.error_message));
    payload
}

pub(crate) fn task_status_payload(
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
    let mut payload = base_task_payload(task, checkpoint);

    payload.insert(
        "current_retry_count".to_string(),
        json!(task.current_retry_count),
    );
    payload.insert("max_retries".to_string(), json!(task.max_retries));
    payload.insert("failed_chapters".to_string(), failed_chapters);
    payload.insert(
        "latest_quality_metrics".to_string(),
        json!(latest_quality_metrics),
    );
    payload.insert(
        "quality_metrics_summary".to_string(),
        json!(quality_metrics_summary),
    );
    payload.insert(
        "active_story_repair_payload".to_string(),
        json!(active_story_repair_payload),
    );
    payload.insert("terminal_reason".to_string(), json!(terminal_reason));
    payload.insert("terminal_label".to_string(), json!(terminal_label));
    payload.insert("review_required".to_string(), json!(review_required));
    payload.insert("can_resume".to_string(), json!(can_resume));

    Value::Object(payload)
}

pub(crate) fn active_task_payload(
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
    let mut payload = base_task_payload(task, checkpoint);

    payload.insert(
        "latest_quality_metrics".to_string(),
        json!(latest_quality_metrics),
    );
    payload.insert(
        "quality_metrics_summary".to_string(),
        json!(quality_metrics_summary),
    );
    payload.insert(
        "active_story_repair_payload".to_string(),
        json!(active_story_repair_payload),
    );

    Value::Object(payload)
}

#[cfg(test)]
mod tests {
    use super::{active_task_payload, checkpoint_with_runtime_metadata, task_status_payload};
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
    fn should_build_checkpoint_with_runtime_metadata_without_runtime_state() {
        let checkpoint = checkpoint_with_runtime_metadata(None, "6.writing.pending", "batch");

        assert_eq!(checkpoint["stage_code"], "6.writing.pending");
        assert_eq!(checkpoint["execution_mode"], "batch");
    }

    #[test]
    fn should_preserve_checkpoint_fields_and_override_runtime_metadata() {
        let runtime_state = json!({
            "progress": 42,
            "stage_code": "stale-stage",
            "execution_mode": "stale-mode"
        });

        let checkpoint =
            checkpoint_with_runtime_metadata(Some(&runtime_state), "6.writing.completed", "single");

        assert_eq!(checkpoint["progress"], 42);
        assert_eq!(checkpoint["stage_code"], "6.writing.completed");
        assert_eq!(checkpoint["execution_mode"], "single");
    }

    #[test]
    fn should_build_task_status_payload_with_terminal_fields() {
        let task = build_task("completed");
        let payload = task_status_payload(
            &task,
            Some(json!({"progress": 80})),
            Some(json!({"score": 91})),
            Some(json!({"summary": "ok"})),
            Some(json!({"mode": "repair"})),
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 80);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.completed");
        assert_eq!(payload["current_retry_count"], 2);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(payload["terminal_reason"], "completed");
        assert_eq!(payload["review_required"], false);
        assert_eq!(payload["can_resume"], false);
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_active_task_payload_without_status_only_fields() {
        let task = build_task("running");
        let payload = active_task_payload(
            &task,
            Some(json!({"progress": 42})),
            Some(json!({"score": 88})),
            Some(json!({"summary": "good"})),
            Some(json!({"mode": "repair"})),
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.generating");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
        assert!(payload.get("current_retry_count").is_none());
        assert!(payload.get("terminal_reason").is_none());
        assert!(payload.get("can_resume").is_none());
    }

}
