use serde_json::{json, Value};

use crate::tasks::types::{TaskEvent, TaskRecord};

pub(crate) fn compatible_task_payload(record: &TaskRecord) -> Value {
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

pub(crate) fn enrich_task_payload(record: &TaskRecord, payload: Value) -> Value {
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

pub(crate) fn build_task_list_response(tasks: Vec<TaskRecord>) -> Value {
    json!({
        "success": true,
        "data": tasks,
        "items": tasks,
        "total": tasks.len(),
    })
}

pub(crate) fn build_missing_task_payload(task_id: &str) -> Value {
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

pub(crate) fn build_connected_task_event(task_id: &str, record: &TaskRecord) -> TaskEvent {
    let record_json = serde_json::to_value(record).unwrap_or_default();
    TaskEvent {
        event_type: "connected".into(),
        task_id: Some(task_id.to_string()),
        message: Some(record.message.clone()),
        progress: Some(record.progress),
        status: Some(record.status.to_string()),
        data: Some(record_json),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use crate::tasks::types::{TaskRecord, TaskStatus};

    use super::{
        build_connected_task_event, build_missing_task_payload, build_task_list_response,
        compatible_task_payload, enrich_task_payload,
    };

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
            created_at: Utc::now(),
            updated_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
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
            event.data.as_ref().and_then(|v| v.get("task_id")),
            Some(&json!("task-1"))
        );
    }
}
