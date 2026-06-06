use chrono::{Duration, NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

use crate::models::batch_generation_task;

pub(crate) fn resolve_generation_task_auto_recovery_error(
    task: &batch_generation_task::Model,
    now: NaiveDateTime,
) -> Option<String> {
    if task.status == "running" {
        if let Some(started_at) = task.started_at {
            if now - started_at > Duration::minutes(15) {
                return Some("任务超时（超过15分钟未完成，已自动恢复）".to_string());
            }
        }
    } else if task.status == "pending" {
        if let Some(created_at) = task.created_at {
            if now - created_at > Duration::minutes(3) {
                return Some("任务启动超时（超过3分钟未启动，已自动恢复）".to_string());
            }
        }
    }

    None
}

pub(crate) async fn recover_generation_task_if_needed(
    db: &DatabaseConnection,
    task: batch_generation_task::Model,
) -> Result<(batch_generation_task::Model, bool), String> {
    let now = Utc::now().naive_utc();
    let Some(error_message) = resolve_generation_task_auto_recovery_error(&task, now) else {
        return Ok((task, false));
    };

    let mut active: batch_generation_task::ActiveModel = task.clone().into();
    active.status = Set("failed".to_string());
    active.error_message = Set(Some(error_message));
    active.completed_at = Set(Some(now));

    active
        .update(db)
        .await
        .map(|updated| (updated, true))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use serde_json::json;

    use crate::models::batch_generation_task;

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-recovery-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn naive_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(year, month, day)
            .expect("valid date")
            .and_hms_opt(hour, minute, second)
            .expect("valid time")
    }

    #[test]
    fn should_resolve_running_generation_task_auto_recovery_error() {
        let mut task = build_task("running");
        let now = Utc::now().naive_utc();
        task.started_at = Some(now - Duration::minutes(16));

        let error = super::resolve_generation_task_auto_recovery_error(&task, now);

        assert_eq!(
            error.as_deref(),
            Some("任务超时（超过15分钟未完成，已自动恢复）")
        );
    }

    #[test]
    fn should_resolve_pending_generation_task_auto_recovery_error() {
        let mut task = build_task("pending");
        let now = Utc::now().naive_utc();
        task.created_at = Some(now - Duration::minutes(4));

        let error = super::resolve_generation_task_auto_recovery_error(&task, now);

        assert_eq!(
            error.as_deref(),
            Some("任务启动超时（超过3分钟未启动，已自动恢复）")
        );
    }

    #[test]
    fn should_not_resolve_generation_task_auto_recovery_error_within_time_budget() {
        let mut running = build_task("running");
        let mut pending = build_task("pending");
        let now = naive_datetime(2026, 5, 31, 21, 0, 0);
        running.started_at = Some(now - Duration::minutes(10));
        pending.created_at = Some(now - Duration::minutes(2));

        assert_eq!(
            super::resolve_generation_task_auto_recovery_error(&running, now),
            None
        );
        assert_eq!(
            super::resolve_generation_task_auto_recovery_error(&pending, now),
            None
        );
    }
}
