use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use tokio::sync::RwLock;
use tracing::info;

use crate::tasks::types::{TaskRecord, TaskStatus};

const MAX_TASKS: usize = 2000;
const TERMINAL_TTL_SECS: i64 = 7200;

#[derive(Clone)]
pub struct TaskRegistry {
    tasks: Arc<RwLock<HashMap<String, TaskRecord>>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn insert(&self, record: TaskRecord) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(record.task_id.clone(), record);
    }

    pub async fn get(&self, task_id: &str) -> Option<TaskRecord> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).cloned()
    }

    pub async fn update<F>(&self, task_id: &str, updater: F) -> Option<TaskRecord>
    where
        F: FnOnce(&mut TaskRecord),
    {
        let mut tasks = self.tasks.write().await;
        if let Some(record) = tasks.get_mut(task_id) {
            updater(record);
            Some(record.clone())
        } else {
            None
        }
    }

    pub async fn remove(&self, task_id: &str) -> Option<TaskRecord> {
        let mut tasks = self.tasks.write().await;
        tasks.remove(task_id)
    }

    pub async fn list_for_user(
        &self,
        user_id: &str,
        project_id: Option<&str>,
        statuses: Option<&[TaskStatus]>,
        active_only: bool,
        limit: Option<usize>,
    ) -> Vec<TaskRecord> {
        let tasks = self.tasks.read().await;
        let mut result: Vec<TaskRecord> = tasks
            .values()
            .filter(|t| t.user_id == user_id)
            .filter(|t| {
                if let Some(pid) = project_id {
                    t.project_id == pid
                } else {
                    true
                }
            })
            .filter(|t| {
                if active_only {
                    t.status.is_active()
                } else {
                    true
                }
            })
            .filter(|t| {
                if let Some(allowed) = statuses {
                    allowed.contains(&t.status)
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        result.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        if let Some(n) = limit {
            result.truncate(n);
        }
        result
    }

    pub async fn find_active(
        &self,
        user_id: &str,
        task_type: &str,
        project_id: &str,
        fingerprint: Option<&str>,
    ) -> Option<TaskRecord> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|t| t.user_id == user_id && t.task_type == task_type && t.project_id == project_id)
            .filter(|t| t.status.is_active())
            .filter(|t| {
                if let Some(fp) = fingerprint {
                    t.payload_fingerprint.as_deref() == Some(fp)
                } else {
                    true
                }
            })
            .max_by_key(|t| t.created_at)
            .cloned()
    }

    pub async fn count_active_for_user(&self, user_id: &str) -> usize {
        let tasks = self.tasks.read().await;
        tasks.values().filter(|t| t.user_id == user_id && t.status.is_active()).count()
    }

    pub async fn all_records(&self) -> Vec<TaskRecord> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }

    pub async fn load_records(&self, records: Vec<TaskRecord>) {
        let mut tasks = self.tasks.write().await;
        for record in records {
            tasks.insert(record.task_id.clone(), record);
        }
    }

    /// Remove terminal tasks older than TTL, and enforce max task count
    pub async fn prune_old_tasks(&self) {
        let now = Utc::now();
        let mut tasks = self.tasks.write().await;

        // Remove expired terminal tasks
        let before = tasks.len();
        tasks.retain(|_, t| {
            if !t.status.is_terminal() {
                return true;
            }
            if let Some(completed_at) = t.completed_at {
                let age = now - completed_at;
                age.num_seconds() < TERMINAL_TTL_SECS
            } else {
                // Terminal but no completed_at — keep for now
                true
            }
        });
        let removed_expired = before - tasks.len();

        // Enforce max task count
        if tasks.len() > MAX_TASKS {
            let mut entries: Vec<_> = tasks.iter().map(|(k, v)| (k.clone(), v.updated_at)).collect();
            entries.sort_by(|a, b| a.1.cmp(&b.1));
            let to_remove = entries.len() - MAX_TASKS;
            let ids_to_remove: Vec<String> = entries.iter().take(to_remove).map(|(id, _)| id.clone()).collect();
            for id in &ids_to_remove {
                tasks.remove(id);
            }
        }

        let total_removed = before - tasks.len();
        if total_removed > 0 {
            info!(
                "Pruned {} tasks ({} expired, {} overflow), {} remaining",
                total_removed,
                removed_expired,
                total_removed - removed_expired,
                tasks.len()
            );
        }
    }
}
