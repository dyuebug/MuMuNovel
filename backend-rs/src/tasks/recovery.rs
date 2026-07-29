use chrono::Utc;
use tracing::info;

use crate::tasks::checkpoint::touch_checkpoint_at;
use crate::tasks::registry::TaskRegistry;
use crate::tasks::types::{TaskRecord, TaskStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskRecoveryPolicy {
    Restartable,
    CheckpointResumable,
    ManualConfirmation,
    NonResumable,
}

impl TaskRecoveryPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Restartable => "restartable",
            Self::CheckpointResumable => "checkpoint_resumable",
            Self::ManualConfirmation => "manual_confirmation",
            Self::NonResumable => "non_resumable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskRecoveryPolicyEntry {
    pub task_type: &'static str,
    pub policy: TaskRecoveryPolicy,
}

pub const TASK_RECOVERY_POLICIES: &[TaskRecoveryPolicyEntry] = &[
    TaskRecoveryPolicyEntry {
        task_type: "novel_autopilot",
        policy: TaskRecoveryPolicy::NonResumable,
    },
    TaskRecoveryPolicyEntry {
        task_type: "novel_book_autopilot",
        policy: TaskRecoveryPolicy::CheckpointResumable,
    },
    TaskRecoveryPolicyEntry {
        task_type: "chapter_analysis",
        policy: TaskRecoveryPolicy::Restartable,
    },
    TaskRecoveryPolicyEntry {
        task_type: "inspiration_generate_options",
        policy: TaskRecoveryPolicy::Restartable,
    },
    TaskRecoveryPolicyEntry {
        task_type: "inspiration_refine_options",
        policy: TaskRecoveryPolicy::Restartable,
    },
    TaskRecoveryPolicyEntry {
        task_type: "inspiration_quick_generate",
        policy: TaskRecoveryPolicy::Restartable,
    },
    TaskRecoveryPolicyEntry {
        task_type: "polish_text",
        policy: TaskRecoveryPolicy::Restartable,
    },
    TaskRecoveryPolicyEntry {
        task_type: "chapters_batch_generate",
        policy: TaskRecoveryPolicy::CheckpointResumable,
    },
    TaskRecoveryPolicyEntry {
        task_type: "chapter_single_generate",
        policy: TaskRecoveryPolicy::CheckpointResumable,
    },
    TaskRecoveryPolicyEntry {
        task_type: "chapter_regenerate",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "chapter_partial_regenerate",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "book_import_apply",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "book_import_retry_failed_steps",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "polish_batch",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "careers_generate_system",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "character_generate",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "organization_generate",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "world_regenerate",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "outline_generate",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "outline_expand",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "outline_batch_expand",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "wizard_world_building",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "wizard_career_system",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "wizard_characters",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
    TaskRecoveryPolicyEntry {
        task_type: "wizard_outline",
        policy: TaskRecoveryPolicy::ManualConfirmation,
    },
];

pub fn has_explicit_recovery_policy(task_type: &str) -> bool {
    TASK_RECOVERY_POLICIES
        .iter()
        .any(|entry| entry.task_type == task_type)
}

pub fn recovery_policy_for(task_type: &str) -> TaskRecoveryPolicy {
    TASK_RECOVERY_POLICIES
        .iter()
        .find(|entry| entry.task_type == task_type)
        .map(|entry| entry.policy)
        .unwrap_or(TaskRecoveryPolicy::NonResumable)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrphanRecoveryProjection {
    terminal_reason: &'static str,
    terminal_label: &'static str,
    error: &'static str,
    message: &'static str,
    review_required: bool,
    can_resume: bool,
}

fn has_usable_checkpoint(record: &TaskRecord) -> bool {
    record
        .checkpoint
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .is_some_and(|checkpoint| !checkpoint.is_empty())
}

fn recovery_projection(
    record: &TaskRecord,
    policy: TaskRecoveryPolicy,
) -> OrphanRecoveryProjection {
    match policy {
        TaskRecoveryPolicy::Restartable => OrphanRecoveryProjection {
            terminal_reason: "restart_required",
            terminal_label: "可重新发起",
            error: "服务重启导致任务执行上下文丢失",
            message: "服务重启后未恢复执行上下文，请从原业务入口重新发起任务",
            review_required: false,
            can_resume: false,
        },
        TaskRecoveryPolicy::CheckpointResumable if has_usable_checkpoint(record) => {
            OrphanRecoveryProjection {
                terminal_reason: "resume_available",
                terminal_label: "可从检查点恢复",
                error: "服务重启中断了任务执行",
                message: "任务已中断，可使用现有章节恢复入口从检查点继续执行",
                review_required: false,
                can_resume: true,
            }
        }
        TaskRecoveryPolicy::CheckpointResumable => OrphanRecoveryProjection {
            terminal_reason: "checkpoint_missing",
            terminal_label: "恢复检查点不可用",
            error: "服务重启中断了任务执行，但恢复检查点不可用",
            message: "未找到有效恢复检查点，请检查已有内容后重新发起任务",
            review_required: false,
            can_resume: false,
        },
        TaskRecoveryPolicy::ManualConfirmation => OrphanRecoveryProjection {
            terminal_reason: "manual_review",
            terminal_label: "需要人工确认",
            error: "服务重启中断了任务执行，任务可能已产生部分数据",
            message: "请先检查已生成内容和持久化结果，再决定是否重新执行",
            review_required: true,
            can_resume: false,
        },
        TaskRecoveryPolicy::NonResumable => OrphanRecoveryProjection {
            terminal_reason: "non_resumable",
            terminal_label: "不可恢复",
            error: "服务重启导致任务执行上下文丢失，且该任务不支持恢复",
            message: "该任务无法恢复，请从对应业务入口重新处理",
            review_required: false,
            can_resume: false,
        },
    }
}

async fn recover_orphan_task(
    registry: &TaskRegistry,
    task_id: &str,
) -> Option<(String, String, TaskRecoveryPolicy)> {
    let recovered = registry
        .update_if(
            task_id,
            |task| task.status.is_active(),
            |task| {
                let policy = recovery_policy_for(&task.task_type);
                let projection = recovery_projection(task, policy);
                let checkpoint_for_merge = task
                    .checkpoint
                    .as_ref()
                    .filter(|checkpoint| checkpoint.is_object());
                let now = Utc::now();
                let new_checkpoint = touch_checkpoint_at(
                    checkpoint_for_merge,
                    "orphan_recovery",
                    Some(task.progress),
                    Some(projection.message),
                    Some(&serde_json::json!({
                        "recovery_policy": policy.as_str(),
                        "terminal_reason": projection.terminal_reason,
                        "can_resume": projection.can_resume,
                        "review_required": projection.review_required,
                        "has_result": task.result.is_some(),
                    })),
                    now,
                );

                task.status = TaskStatus::Failed;
                task.error = Some(projection.error.into());
                task.message = projection.message.into();
                task.terminal_reason = Some(projection.terminal_reason.into());
                task.terminal_label = Some(projection.terminal_label.into());
                task.review_required = Some(projection.review_required);
                task.can_resume = Some(projection.can_resume);
                task.completed_at = Some(now);
                task.updated_at = now;
                task.checkpoint = Some(new_checkpoint);
            },
        )
        .await?;
    let policy = recovery_policy_for(&recovered.task_type);

    Some((recovered.task_id, recovered.task_type, policy))
}

pub async fn recover_orphan_tasks(registry: &TaskRegistry) -> usize {
    let orphan_task_ids: Vec<_> = registry
        .all_records()
        .await
        .into_iter()
        .filter(|record| record.status.is_active())
        .map(|record| record.task_id)
        .collect();

    if orphan_task_ids.is_empty() {
        info!("No orphan tasks found");
        return 0;
    }

    let mut recovered_count = 0;

    for task_id in orphan_task_ids {
        let Some((task_id, task_type, policy)) = recover_orphan_task(registry, &task_id).await
        else {
            continue;
        };

        recovered_count += 1;
        info!(
            task_id = %task_id,
            task_type = %task_type,
            recovery_policy = policy.as_str(),
            projected_status = %TaskStatus::Failed,
            "Recovered orphan task"
        );
    }

    info!(recovered_count, "Recovered orphan tasks");
    recovered_count
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::{Duration, Utc};
    use serde_json::json;

    use super::{
        has_explicit_recovery_policy, recover_orphan_task, recover_orphan_tasks,
        recovery_policy_for, recovery_projection, TaskRecoveryPolicy, TASK_RECOVERY_POLICIES,
    };
    use crate::tasks::registry::TaskRegistry;
    use crate::tasks::types::{TaskRecord, TaskStatus};

    fn record(task_id: &str, task_type: &str, status: TaskStatus) -> TaskRecord {
        let mut record = TaskRecord::new(
            task_id.to_string(),
            task_type.to_string(),
            "user-1".to_string(),
            "project-1".to_string(),
            "interactive".to_string(),
        );
        record.status = status;
        record.progress = 37;
        record.message = "existing message".to_string();
        record
    }

    #[test]
    fn registry_contains_exactly_25_unique_known_task_types() {
        let unique: HashSet<_> = TASK_RECOVERY_POLICIES
            .iter()
            .map(|entry| entry.task_type)
            .collect();

        assert_eq!(TASK_RECOVERY_POLICIES.len(), 25);
        assert_eq!(unique.len(), 25);
        assert!(TASK_RECOVERY_POLICIES
            .iter()
            .all(|entry| has_explicit_recovery_policy(entry.task_type)));
        assert_eq!(
            TASK_RECOVERY_POLICIES
                .iter()
                .filter(|entry| entry.policy == TaskRecoveryPolicy::Restartable)
                .count(),
            5
        );
        assert_eq!(
            TASK_RECOVERY_POLICIES
                .iter()
                .filter(|entry| entry.policy == TaskRecoveryPolicy::CheckpointResumable)
                .count(),
            3
        );
        assert_eq!(
            TASK_RECOVERY_POLICIES
                .iter()
                .filter(|entry| entry.policy == TaskRecoveryPolicy::ManualConfirmation)
                .count(),
            16
        );
    }

    #[test]
    fn durable_novel_autopilot_uses_checkpoint_resumable_policy() {
        assert_eq!(
            recovery_policy_for("novel_book_autopilot"),
            TaskRecoveryPolicy::CheckpointResumable
        );
    }

    #[tokio::test]
    async fn novel_autopilot_orphan_fails_as_explicit_non_resumable_without_replay() {
        let registry = TaskRegistry::new();
        registry
            .insert(record("autopilot", "novel_autopilot", TaskStatus::Running))
            .await;

        assert_eq!(recover_orphan_tasks(&registry).await, 1);
        let recovered = registry.get("autopilot").await.expect("recovered task");
        assert_eq!(recovered.status, TaskStatus::Failed);
        assert_eq!(recovered.terminal_reason.as_deref(), Some("non_resumable"));
        assert_eq!(recovered.terminal_label.as_deref(), Some("不可恢复"));
        assert_eq!(recovered.can_resume, Some(false));
        assert_eq!(
            recovered.message,
            "该任务无法恢复，请从对应业务入口重新处理"
        );
    }

    #[test]
    fn unknown_task_type_uses_non_resumable_fallback() {
        assert!(!has_explicit_recovery_policy("unknown"));
        assert!(!has_explicit_recovery_policy("future_unregistered_task"));
        assert_eq!(
            recovery_policy_for("unknown"),
            TaskRecoveryPolicy::NonResumable
        );
        assert_eq!(
            recovery_policy_for("future_unregistered_task"),
            TaskRecoveryPolicy::NonResumable
        );
    }

    #[test]
    fn each_policy_builds_actionable_terminal_semantics() {
        let restartable = record("restartable", "chapter_analysis", TaskStatus::Running);
        let restartable_projection =
            recovery_projection(&restartable, recovery_policy_for(&restartable.task_type));
        assert_eq!(restartable_projection.terminal_reason, "restart_required");
        assert!(!restartable_projection.review_required);
        assert!(!restartable_projection.can_resume);

        let mut resumable = record("resumable", "chapters_batch_generate", TaskStatus::Running);
        resumable.checkpoint = Some(json!({"completed_chapters": [1]}));
        let resumable_projection =
            recovery_projection(&resumable, recovery_policy_for(&resumable.task_type));
        assert_eq!(resumable_projection.terminal_reason, "resume_available");
        assert!(!resumable_projection.review_required);
        assert!(resumable_projection.can_resume);

        let manual = record("manual", "wizard_outline", TaskStatus::Running);
        let manual_projection =
            recovery_projection(&manual, recovery_policy_for(&manual.task_type));
        assert_eq!(manual_projection.terminal_reason, "manual_review");
        assert!(manual_projection.review_required);
        assert!(!manual_projection.can_resume);

        let unknown = record("unknown", "unknown", TaskStatus::Running);
        let unknown_projection =
            recovery_projection(&unknown, recovery_policy_for(&unknown.task_type));
        assert_eq!(unknown_projection.terminal_reason, "non_resumable");
        assert!(!unknown_projection.review_required);
        assert!(!unknown_projection.can_resume);
    }

    #[test]
    fn checkpoint_resume_requires_non_empty_object() {
        for checkpoint in [
            None,
            Some(serde_json::Value::Null),
            Some(json!("checkpoint")),
            Some(json!(1)),
            Some(json!([1, 2, 3])),
            Some(json!({})),
        ] {
            let mut task = record("checkpoint", "chapter_single_generate", TaskStatus::Running);
            task.checkpoint = checkpoint;
            let projection = recovery_projection(&task, recovery_policy_for(&task.task_type));
            assert_eq!(projection.terminal_reason, "checkpoint_missing");
            assert!(!projection.can_resume);
        }

        let mut task = record("checkpoint", "chapter_single_generate", TaskStatus::Running);
        task.checkpoint = Some(json!({"chapter_id": "chapter-1"}));
        let projection = recovery_projection(&task, recovery_policy_for(&task.task_type));
        assert_eq!(projection.terminal_reason, "resume_available");
        assert!(projection.can_resume);
    }

    #[tokio::test]
    async fn empty_registry_reports_zero_recovered_tasks() {
        let registry = TaskRegistry::new();

        assert_eq!(recover_orphan_tasks(&registry).await, 0);
    }

    #[tokio::test]
    async fn recovery_updates_pending_and_running_but_keeps_terminal_records_unchanged() {
        let registry = TaskRegistry::new();
        for (task_id, status) in [
            ("pending", TaskStatus::Pending),
            ("running", TaskStatus::Running),
            ("completed", TaskStatus::Completed),
            ("failed", TaskStatus::Failed),
            ("cancelled", TaskStatus::Cancelled),
        ] {
            registry
                .insert(record(task_id, "chapter_analysis", status))
                .await;
        }

        let recovered_count = recover_orphan_tasks(&registry).await;

        assert_eq!(recovered_count, 2);
        for task_id in ["pending", "running"] {
            let recovered = registry.get(task_id).await.expect("recovered task");
            assert_eq!(recovered.status, TaskStatus::Failed);
            assert_eq!(
                recovered.terminal_reason.as_deref(),
                Some("restart_required")
            );
            assert_eq!(recovered.review_required, Some(false));
            assert_eq!(recovered.can_resume, Some(false));
            assert_eq!(
                recovered.started_at, None,
                "startup recovery must not fabricate a task start timestamp"
            );
        }
        assert_eq!(
            registry.get("completed").await.expect("completed").status,
            TaskStatus::Completed
        );
        assert_eq!(
            registry
                .get("failed")
                .await
                .expect("failed")
                .terminal_reason,
            None
        );
        assert_eq!(
            registry.get("cancelled").await.expect("cancelled").status,
            TaskStatus::Cancelled
        );
    }

    #[tokio::test]
    async fn recovery_preserves_result_progress_custom_checkpoint_and_existing_started_at() {
        let registry = TaskRegistry::new();
        let mut task = record("resumable", "chapters_batch_generate", TaskStatus::Running);
        let started_at = Utc::now() - Duration::minutes(5);
        let old_updated_at = Utc::now() - Duration::minutes(1);
        task.started_at = Some(started_at);
        task.updated_at = old_updated_at;
        task.result = Some(json!({"partial": true}));
        task.checkpoint = Some(json!({"custom": "preserved"}));
        registry.insert(task).await;

        let recovered_count = recover_orphan_tasks(&registry).await;

        assert_eq!(recovered_count, 1);
        let recovered = registry.get("resumable").await.expect("recovered task");
        assert_eq!(recovered.status, TaskStatus::Failed);
        assert_eq!(recovered.progress, 37);
        assert_eq!(recovered.result, Some(json!({"partial": true})));
        assert_eq!(recovered.started_at, Some(started_at));
        assert!(recovered.updated_at > old_updated_at);
        assert_eq!(
            recovered.terminal_reason.as_deref(),
            Some("resume_available")
        );
        assert_eq!(recovered.can_resume, Some(true));
        let checkpoint = recovered
            .checkpoint
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .expect("diagnostic checkpoint object");
        assert_eq!(checkpoint.get("custom"), Some(&json!("preserved")));
        assert_eq!(checkpoint.get("event"), Some(&json!("orphan_recovery")));
        assert_eq!(
            checkpoint.get("recovery_policy"),
            Some(&json!("checkpoint_resumable"))
        );
        assert_eq!(
            checkpoint.get("terminal_reason"),
            Some(&json!("resume_available"))
        );
        assert_eq!(checkpoint.get("has_result"), Some(&json!(true)));
        let checkpoint_updated_at = chrono::DateTime::parse_from_rfc3339(
            checkpoint
                .get("updated_at")
                .and_then(serde_json::Value::as_str)
                .expect("checkpoint updated_at"),
        )
        .expect("valid checkpoint updated_at")
        .with_timezone(&Utc);
        assert_eq!(checkpoint_updated_at, recovered.updated_at);
        assert_eq!(recovered.completed_at, Some(recovered.updated_at));
    }

    #[tokio::test]
    async fn stale_orphan_candidate_does_not_overwrite_terminal_record() {
        let registry = TaskRegistry::new();
        registry
            .insert(record("stale", "polish_text", TaskStatus::Running))
            .await;
        let completed_at = Utc::now();
        registry
            .update("stale", |task| {
                task.status = TaskStatus::Completed;
                task.progress = 100;
                task.message = "completed concurrently".to_string();
                task.completed_at = Some(completed_at);
                task.updated_at = completed_at;
            })
            .await;

        assert_eq!(recover_orphan_task(&registry, "stale").await, None);

        let preserved = registry.get("stale").await.expect("terminal task");
        assert_eq!(preserved.status, TaskStatus::Completed);
        assert_eq!(preserved.progress, 100);
        assert_eq!(preserved.message, "completed concurrently");
        assert_eq!(preserved.completed_at, Some(completed_at));
        assert_eq!(preserved.terminal_reason, None);
        assert_eq!(preserved.checkpoint, None);
    }

    #[tokio::test]
    async fn repeated_recovery_is_idempotent() {
        let registry = TaskRegistry::new();
        registry
            .insert(record("once", "polish_text", TaskStatus::Running))
            .await;

        assert_eq!(recover_orphan_tasks(&registry).await, 1);
        let first_projection = registry.get("once").await.expect("first projection");

        assert_eq!(recover_orphan_tasks(&registry).await, 0);
        let second_projection = registry.get("once").await.expect("second projection");

        assert_eq!(second_projection.status, first_projection.status);
        assert_eq!(second_projection.updated_at, first_projection.updated_at);
        assert_eq!(
            second_projection.completed_at,
            first_projection.completed_at
        );
        assert_eq!(second_projection.message, first_projection.message);
        assert_eq!(second_projection.error, first_projection.error);
        assert_eq!(second_projection.checkpoint, first_projection.checkpoint);
    }

    #[test]
    fn orphan_recovery_log_contract_exposes_only_safe_metadata() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/tasks/recovery.rs"
        ));
        let message_index = source
            .find("\"Recovered orphan task\"")
            .expect("orphan recovery log message");
        let log_start = source[..message_index]
            .rfind("info!(")
            .expect("orphan recovery info macro");
        let log_end_offset = source[message_index..]
            .find(");")
            .expect("orphan recovery info macro end");
        let log_block = &source[log_start..message_index + log_end_offset + 2];

        let fields: Vec<_> = log_block
            .lines()
            .filter_map(|line| {
                let (name, _) = line.trim().split_once('=')?;
                Some(name.trim())
            })
            .collect();

        assert_eq!(
            fields,
            [
                "task_id",
                "task_type",
                "recovery_policy",
                "projected_status",
            ]
        );

        for forbidden in [
            "?record,",
            "%record,",
            "record.result",
            "record.checkpoint",
            "record.payload",
            "result =",
            "checkpoint =",
            "payload =",
        ] {
            assert!(
                !log_block.contains(forbidden),
                "orphan recovery log must not contain {forbidden}: {log_block}"
            );
        }
    }

    #[tokio::test]
    async fn malformed_checkpoint_is_replaced_with_safe_diagnostics() {
        let registry = TaskRegistry::new();
        let mut task = record("malformed", "chapter_single_generate", TaskStatus::Running);
        task.checkpoint = Some(json!(["invalid"]));
        registry.insert(task).await;

        let recovered_count = recover_orphan_tasks(&registry).await;

        assert_eq!(recovered_count, 1);
        let recovered = registry.get("malformed").await.expect("recovered task");
        assert_eq!(
            recovered.terminal_reason.as_deref(),
            Some("checkpoint_missing")
        );
        assert_eq!(recovered.can_resume, Some(false));
        let checkpoint = recovered
            .checkpoint
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .expect("replacement diagnostic object");
        assert_eq!(checkpoint.get("event"), Some(&json!("orphan_recovery")));
        assert_eq!(
            checkpoint.get("terminal_reason"),
            Some(&json!("checkpoint_missing"))
        );
    }
}
