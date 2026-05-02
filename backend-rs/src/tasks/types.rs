use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        matches!(self, TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled)
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
    pub project_id: String,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
    pub stage_code: Option<String>,
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String,
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
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWorkflowUpdate {
    pub stage_code: Option<String>,
    pub execution_mode: Option<String>,
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
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
