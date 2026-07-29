use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(self, TaskStatus::Pending | TaskStatus::Running)
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: String,
    pub task_type: String,
    pub user_id: String,
    pub project_id: String,
    pub status: TaskStatus,
    pub progress: i32,
    pub message: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub stage_code: Option<String>,
    pub execution_mode: String,
    pub workflow_scope: Option<String>,
    pub checkpoint: Option<serde_json::Value>,
    pub payload_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_resume: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl TaskRecord {
    pub fn new(
        task_id: String,
        task_type: String,
        user_id: String,
        project_id: String,
        execution_mode: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            task_id,
            task_type,
            user_id,
            project_id,
            status: TaskStatus::Pending,
            progress: 0,
            message: String::new(),
            result: None,
            error: None,
            stage_code: None,
            execution_mode,
            workflow_scope: None,
            checkpoint: None,
            payload_fingerprint: None,
            terminal_reason: None,
            terminal_label: None,
            review_required: None,
            can_resume: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCreateRequest {
    pub task_type: String,
    #[serde(default)]
    #[serde(alias = "projectId")]
    pub project_id: String,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
    #[serde(alias = "stageCode")]
    pub stage_code: Option<String>,
    #[serde(default = "default_execution_mode")]
    #[serde(alias = "executionMode")]
    pub execution_mode: String,
    #[serde(alias = "workflowScope")]
    pub workflow_scope: Option<String>,
    pub checkpoint: Option<serde_json::Value>,
}

fn default_execution_mode() -> String {
    "interactive".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListQuery {
    pub project_id: Option<String>,
    pub statuses: Option<String>,
    pub active_only: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWorkflowUpdate {
    #[serde(alias = "stageCode")]
    pub stage_code: Option<String>,
    #[serde(alias = "executionMode")]
    pub execution_mode: Option<String>,
    #[serde(alias = "workflowScope")]
    pub workflow_scope: Option<String>,
    pub checkpoint: Option<serde_json::Value>,
    pub message: Option<String>,
    pub progress: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::TaskRecord;

    #[test]
    fn task_record_without_recovery_fields_remains_backward_compatible() {
        let record = TaskRecord::new(
            "task-1".to_string(),
            "chapter_analysis".to_string(),
            "user-1".to_string(),
            "project-1".to_string(),
            "interactive".to_string(),
        );
        let serialized = serde_json::to_value(&record).expect("serialize task record");

        assert_eq!(serialized.get("terminal_reason"), None);
        assert_eq!(serialized.get("terminal_label"), None);
        assert_eq!(serialized.get("review_required"), None);
        assert_eq!(serialized.get("can_resume"), None);

        let restored: TaskRecord =
            serde_json::from_value(serialized).expect("deserialize version-1 task record");
        assert_eq!(restored.terminal_reason, None);
        assert_eq!(restored.terminal_label, None);
        assert_eq!(restored.review_required, None);
        assert_eq!(restored.can_resume, None);
    }

    #[test]
    fn task_record_serializes_non_empty_recovery_fields() {
        let mut record = TaskRecord::new(
            "task-1".to_string(),
            "chapter_single_generate".to_string(),
            "user-1".to_string(),
            "project-1".to_string(),
            "interactive".to_string(),
        );
        record.terminal_reason = Some("resume_available".to_string());
        record.terminal_label = Some("可从检查点恢复".to_string());
        record.review_required = Some(false);
        record.can_resume = Some(true);

        let serialized = serde_json::to_value(&record).expect("serialize task record");
        assert_eq!(serialized["terminal_reason"], "resume_available");
        assert_eq!(serialized["terminal_label"], "可从检查点恢复");
        assert_eq!(serialized["review_required"], Value::Bool(false));
        assert_eq!(serialized["can_resume"], Value::Bool(true));
    }
}
