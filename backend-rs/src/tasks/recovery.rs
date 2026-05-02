use chrono::Utc;
use tracing::info;

use crate::tasks::checkpoint::touch_checkpoint;
use crate::tasks::registry::TaskRegistry;
use crate::tasks::types::TaskStatus;

pub async fn recover_orphan_tasks(registry: &TaskRegistry) {
    let orphans: Vec<_> = registry
        .all_records()
        .await
        .into_iter()
        .filter(|r| r.status == TaskStatus::Pending || r.status == TaskStatus::Running)
        .collect();

    if orphans.is_empty() {
        info!("No orphan tasks found");
        return;
    }

    let count = orphans.len();

    for record in orphans {
        let new_checkpoint = touch_checkpoint(
            record.checkpoint.as_ref(),
            "orphan_recovery",
            Some(record.progress),
            Some(&record.message),
            Some(&serde_json::json!({
                "error": "服务重启导致任务上下文丢失",
                "has_result": false,
            })),
        );

        registry
            .update(&record.task_id, |r| {
                r.status = TaskStatus::Failed;
                r.error = Some("服务重启导致任务上下文丢失".into());
                r.message = "服务重启后未恢复执行上下文，请重新发起任务".into();
                r.completed_at = Some(Utc::now());
                if r.started_at.is_none() {
                    r.started_at = Some(Utc::now());
                }
                r.checkpoint = Some(new_checkpoint);
            })
            .await;
    }

    info!("Recovered {} orphan tasks (marked as failed)", count);
}
