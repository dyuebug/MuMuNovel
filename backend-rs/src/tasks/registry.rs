use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
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

    pub async fn update_if<P, F>(
        &self,
        task_id: &str,
        predicate: P,
        updater: F,
    ) -> Option<TaskRecord>
    where
        P: FnOnce(&TaskRecord) -> bool,
        F: FnOnce(&mut TaskRecord),
    {
        let mut tasks = self.tasks.write().await;
        let record = tasks.get_mut(task_id)?;
        if !predicate(record) {
            return None;
        }

        updater(record);
        Some(record.clone())
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
            .filter(|t| {
                t.user_id == user_id && t.task_type == task_type && t.project_id == project_id
            })
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
            let mut entries: Vec<_> = tasks
                .iter()
                .map(|(k, v)| (k.clone(), v.updated_at))
                .collect();
            entries.sort_by(|a, b| a.1.cmp(&b.1));
            let to_remove = entries.len() - MAX_TASKS;
            let ids_to_remove: Vec<String> = entries
                .iter()
                .take(to_remove)
                .map(|(id, _)| id.clone())
                .collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Barrier;

    fn record(task_id: &str) -> TaskRecord {
        TaskRecord::new(
            task_id.to_string(),
            "polish_text".to_string(),
            "user-1".to_string(),
            "project-1".to_string(),
            "interactive".to_string(),
        )
    }

    #[tokio::test]
    async fn update_if_does_not_evaluate_callbacks_for_missing_task() {
        let registry = TaskRegistry::new();
        let predicate_calls = Arc::new(AtomicUsize::new(0));
        let updater_calls = Arc::new(AtomicUsize::new(0));

        let predicate_counter = Arc::clone(&predicate_calls);
        let updater_counter = Arc::clone(&updater_calls);
        let updated = registry
            .update_if(
                "missing",
                move |_| {
                    predicate_counter.fetch_add(1, Ordering::SeqCst);
                    true
                },
                move |_| {
                    updater_counter.fetch_add(1, Ordering::SeqCst);
                },
            )
            .await;

        assert!(updated.is_none());
        assert_eq!(predicate_calls.load(Ordering::SeqCst), 0);
        assert_eq!(updater_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn update_if_rejected_predicate_skips_updater_and_preserves_record() {
        let registry = TaskRegistry::new();
        let original = record("rejected");
        let original_updated_at = original.updated_at;
        registry.insert(original).await;
        let predicate_calls = Arc::new(AtomicUsize::new(0));
        let updater_calls = Arc::new(AtomicUsize::new(0));

        let predicate_counter = Arc::clone(&predicate_calls);
        let updater_counter = Arc::clone(&updater_calls);
        let updated = registry
            .update_if(
                "rejected",
                move |task| {
                    predicate_counter.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(task.status, TaskStatus::Pending);
                    false
                },
                move |task| {
                    updater_counter.fetch_add(1, Ordering::SeqCst);
                    task.status = TaskStatus::Running;
                },
            )
            .await;

        assert!(updated.is_none());
        assert_eq!(predicate_calls.load(Ordering::SeqCst), 1);
        assert_eq!(updater_calls.load(Ordering::SeqCst), 0);
        let preserved = registry.get("rejected").await.expect("preserved task");
        assert_eq!(preserved.status, TaskStatus::Pending);
        assert_eq!(preserved.updated_at, original_updated_at);
    }

    #[tokio::test]
    async fn update_if_accepted_predicate_runs_updater_once_and_returns_latest_record() {
        let registry = TaskRegistry::new();
        registry.insert(record("accepted")).await;
        let predicate_calls = Arc::new(AtomicUsize::new(0));
        let updater_calls = Arc::new(AtomicUsize::new(0));

        let predicate_counter = Arc::clone(&predicate_calls);
        let updater_counter = Arc::clone(&updater_calls);
        let updated = registry
            .update_if(
                "accepted",
                move |task| {
                    predicate_counter.fetch_add(1, Ordering::SeqCst);
                    task.status == TaskStatus::Pending
                },
                move |task| {
                    updater_counter.fetch_add(1, Ordering::SeqCst);
                    task.status = TaskStatus::Running;
                    task.message = "admitted".to_string();
                },
            )
            .await
            .expect("accepted update");

        assert_eq!(predicate_calls.load(Ordering::SeqCst), 1);
        assert_eq!(updater_calls.load(Ordering::SeqCst), 1);
        assert_eq!(updated.status, TaskStatus::Running);
        assert_eq!(updated.message, "admitted");
        let stored = registry.get("accepted").await.expect("stored task");
        assert_eq!(stored.status, updated.status);
        assert_eq!(stored.message, updated.message);
    }

    #[tokio::test]
    async fn update_if_serializes_competing_pending_admissions() {
        let registry = TaskRegistry::new();
        registry.insert(record("contended")).await;
        let barrier = Arc::new(Barrier::new(3));

        let first_registry = registry.clone();
        let first_barrier = Arc::clone(&barrier);
        let first = tokio::spawn(async move {
            first_barrier.wait().await;
            first_registry
                .update_if(
                    "contended",
                    |task| task.status == TaskStatus::Pending,
                    |task| task.status = TaskStatus::Running,
                )
                .await
                .is_some()
        });

        let second_registry = registry.clone();
        let second_barrier = Arc::clone(&barrier);
        let second = tokio::spawn(async move {
            second_barrier.wait().await;
            second_registry
                .update_if(
                    "contended",
                    |task| task.status == TaskStatus::Pending,
                    |task| task.status = TaskStatus::Running,
                )
                .await
                .is_some()
        });

        barrier.wait().await;
        let admitted = usize::from(first.await.expect("first admission task"))
            + usize::from(second.await.expect("second admission task"));

        assert_eq!(admitted, 1);
        assert_eq!(
            registry
                .get("contended")
                .await
                .expect("contended task")
                .status,
            TaskStatus::Running
        );
    }
}
