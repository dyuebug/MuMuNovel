use chrono::NaiveDateTime;
use serde_json::{json, Map, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_status_semantics_service::{
    batch_generation_stage_code, task_execution_mode, task_stage_code, task_type,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationCommandProgressSummary {
    pub(crate) batch_id: String,
    pub(crate) total_chapters: i32,
    pub(crate) completed_chapters: i32,
}

impl BatchGenerationCommandProgressSummary {
    pub(crate) fn batch_id(&self) -> &str {
        &self.batch_id
    }
}

pub(crate) fn build_batch_generation_command_summary_payload(
    progress: BatchGenerationCommandProgressSummary,
    message: impl Into<String>,
) -> Value {
    let mut payload = Map::new();
    payload.insert("total_chapters".to_string(), json!(progress.total_chapters));
    payload.insert(
        "completed_chapters".to_string(),
        json!(progress.completed_chapters),
    );
    payload.insert("batch_id".to_string(), json!(progress.batch_id));
    payload.insert("message".to_string(), json!(message.into()));
    Value::Object(payload)
}

pub(crate) fn build_batch_generation_task_runtime_payload(
    batch_id: impl Into<String>,
    task_type: impl Into<String>,
    project_id: impl Into<String>,
    status: impl Into<String>,
    current_chapter_id: Option<&str>,
    created_at: Option<NaiveDateTime>,
    checkpoint: Map<String, Value>,
    stage_code: impl Into<String>,
    execution_mode: impl Into<String>,
) -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert("batch_id".to_string(), json!(batch_id.into()));
    payload.insert("task_type".to_string(), json!(task_type.into()));
    payload.insert("project_id".to_string(), json!(project_id.into()));
    let status = status.into();
    payload.insert("status".to_string(), json!(status));
    payload.insert("stage_code".to_string(), json!(stage_code.into()));
    payload.insert("execution_mode".to_string(), json!(execution_mode.into()));
    payload.insert(
        "current_chapter_id".to_string(),
        json!(current_chapter_id.map(str::to_string)),
    );
    payload.insert("checkpoint".to_string(), Value::Object(checkpoint));
    payload.insert("created_at".to_string(), json!(to_iso(created_at)));
    payload
}

pub(crate) fn build_batch_generation_task_runtime_payload_from_runtime_parts(
    batch_id: impl Into<String>,
    task_type: impl Into<String>,
    project_id: impl Into<String>,
    status: impl Into<String>,
    current_chapter_id: Option<&str>,
    created_at: Option<NaiveDateTime>,
    workflow_runtime_state: Option<&Value>,
    checkpoint_override: Option<(&str, Value)>,
) -> Map<String, Value> {
    let batch_id = batch_id.into();
    let task_type = task_type.into();
    let project_id = project_id.into();
    let status = status.into();
    let stage_code = batch_generation_stage_code(&status);
    let execution_mode = task_execution_mode();
    let mut checkpoint =
        checkpoint_with_runtime_metadata(workflow_runtime_state, stage_code, execution_mode);

    if let Some((key, value)) = checkpoint_override {
        checkpoint.insert(key.to_string(), value);
    }

    build_batch_generation_task_runtime_payload(
        batch_id,
        task_type,
        project_id,
        status,
        current_chapter_id,
        created_at,
        checkpoint,
        stage_code,
        execution_mode,
    )
}

pub(crate) fn build_batch_generation_task_runtime_payload_from_task_state(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
) -> Map<String, Value> {
    let stage_code = task_stage_code(task);
    let execution_mode = task_execution_mode();
    let checkpoint =
        checkpoint_with_runtime_metadata(workflow_runtime_state, stage_code, execution_mode);

    build_batch_generation_task_runtime_payload(
        &task.id,
        task_type(task),
        &task.project_id,
        &task.status,
        task.current_chapter_id.as_deref(),
        task.created_at,
        checkpoint,
        stage_code,
        execution_mode,
    )
}

pub(crate) fn build_batch_generation_task_view_payload_from_task_state(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
) -> Map<String, Value> {
    let mut payload =
        build_batch_generation_task_runtime_payload_from_task_state(task, workflow_runtime_state);

    payload.insert("total".to_string(), json!(task.total_chapters));
    payload.insert("completed".to_string(), json!(task.completed_chapters));
    payload.insert(
        "current_chapter_number".to_string(),
        json!(task.current_chapter_number),
    );
    payload.insert("started_at".to_string(), json!(to_iso(task.started_at)));
    payload.insert("completed_at".to_string(), json!(to_iso(task.completed_at)));
    payload.insert("error_message".to_string(), json!(task.error_message));

    payload
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_batch_generation_command_summary_payload,
        build_batch_generation_task_runtime_payload,
        build_batch_generation_task_runtime_payload_from_runtime_parts,
        build_batch_generation_task_runtime_payload_from_task_state,
        build_batch_generation_task_view_payload_from_task_state, checkpoint_with_runtime_metadata,
        BatchGenerationCommandProgressSummary,
    };
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_status_semantics_service::task_type;

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
    fn should_build_batch_generation_command_summary_payload() {
        let payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: "task-3".to_string(),
                total_chapters: 5,
                completed_chapters: 2,
            },
            "Batch generation cancelled",
        );

        assert_eq!(payload["batch_id"], "task-3");
        assert_eq!(payload["total_chapters"], 5);
        assert_eq!(payload["completed_chapters"], 2);
        assert_eq!(payload["message"], "Batch generation cancelled");
    }

    #[test]
    fn should_build_batch_generation_command_progress_summary() {
        let payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: "task-7".to_string(),
                total_chapters: 6,
                completed_chapters: 4,
            },
            "Batch generation completed",
        );

        assert_eq!(payload["batch_id"], "task-7");
        assert_eq!(payload["total_chapters"], 6);
        assert_eq!(payload["completed_chapters"], 4);
        assert_eq!(payload["message"], "Batch generation completed");
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload() {
        let task = build_task("running");
        let checkpoint = checkpoint_with_runtime_metadata(
            Some(&json!({"progress": 42})),
            "6.writing.generating",
            "interactive",
        );

        let payload = build_batch_generation_task_runtime_payload(
            &task.id,
            task_type(&task),
            &task.project_id,
            &task.status,
            task.current_chapter_id.as_deref(),
            task.created_at,
            checkpoint,
            "6.writing.generating",
            "interactive",
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["stage_code"], "6.writing.generating");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["current_chapter_id"], "chapter-1");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.generating");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload_from_parts() {
        let checkpoint = checkpoint_with_runtime_metadata(
            Some(&json!({"progress": 42})),
            "6.writing.pending",
            "interactive",
        );

        let payload = build_batch_generation_task_runtime_payload(
            "task-9",
            "chapters_batch_generate",
            "project-9",
            "pending",
            Some("chapter-7"),
            None,
            checkpoint,
            "6.writing.pending",
            "interactive",
        );

        assert_eq!(payload["batch_id"], "task-9");
        assert_eq!(payload["task_type"], "chapters_batch_generate");
        assert_eq!(payload["project_id"], "project-9");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["current_chapter_id"], "chapter-7");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload_from_runtime_parts() {
        let payload = build_batch_generation_task_runtime_payload_from_runtime_parts(
            "task-9",
            "chapters_batch_generate",
            "project-9",
            "pending",
            Some("chapter-7"),
            None,
            Some(&json!({"progress": 42})),
            Some(("chapter_id", json!("chapter-8"))),
        );

        assert_eq!(payload["batch_id"], "task-9");
        assert_eq!(payload["task_type"], "chapters_batch_generate");
        assert_eq!(payload["project_id"], "project-9");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["current_chapter_id"], "chapter-7");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-8");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload_from_task_state() {
        let task = build_task("running");

        let payload = build_batch_generation_task_runtime_payload_from_task_state(
            &task,
            Some(&json!({"progress": 42})),
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["stage_code"], "6.writing.generating");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.generating");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
    }

    #[test]
    fn should_build_batch_generation_task_view_payload_from_task_state() {
        let mut task = build_task("running");
        task.started_at = Some(
            chrono::NaiveDate::from_ymd_opt(2026, 5, 21)
                .expect("valid date")
                .and_hms_opt(9, 30, 0)
                .expect("valid time"),
        );
        task.completed_at = Some(
            chrono::NaiveDate::from_ymd_opt(2026, 5, 21)
                .expect("valid date")
                .and_hms_opt(10, 30, 0)
                .expect("valid time"),
        );
        task.error_message = Some("boom".to_string());

        let payload = build_batch_generation_task_view_payload_from_task_state(
            &task,
            Some(&json!({"progress": 42})),
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["total"], 2);
        assert_eq!(payload["completed"], 1);
        assert_eq!(payload["current_chapter_number"], 1);
        assert_eq!(payload["started_at"], "2026-05-21T09:30:00+00:00");
        assert_eq!(payload["completed_at"], "2026-05-21T10:30:00+00:00");
        assert_eq!(payload["error_message"], "boom");
        assert_eq!(payload["checkpoint"]["progress"], 42);
    }

    #[test]
    fn should_build_batch_generation_task_runtime_payload_from_runtime_parts_with_checkpoint_override()
    {
        let task = build_task("running");

        let payload = build_batch_generation_task_runtime_payload_from_runtime_parts(
            &task.id,
            task_type(&task),
            &task.project_id,
            &task.status,
            task.current_chapter_id.as_deref(),
            task.created_at,
            Some(&json!({"progress": 42})),
            Some(("chapter_id", json!("chapter-9"))),
        );

        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-9");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.generating");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
    }
}
