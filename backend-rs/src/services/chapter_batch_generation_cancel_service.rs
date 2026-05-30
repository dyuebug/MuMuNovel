use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_owned_task, LoadOwnedBatchGenerationTaskError,
};
use crate::services::chapter_batch_generation_runtime_state_service::BatchGenerationRuntimePersistencePlan;
use crate::services::chapter_batch_generation_task_payload_base_service::{
    build_batch_generation_command_summary_payload, BatchGenerationCommandProgressSummary,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CancelBatchGenerationWorkflowError {
    Task(LoadOwnedBatchGenerationTaskError),
    Domain(String),
}

pub(crate) async fn cancel_owned_batch_generation_task(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, CancelBatchGenerationWorkflowError> {
    let task = load_owned_task(db, batch_id, user_id)
        .await
        .map_err(|error| {
            CancelBatchGenerationWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::Internal(error),
            )
        })?
        .ok_or(CancelBatchGenerationWorkflowError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        ))?;

    if matches!(task.status.as_str(), "completed" | "failed" | "cancelled") {
        return Err(CancelBatchGenerationWorkflowError::Domain(format!(
            "Cannot cancel task in status {}",
            task.status
        )));
    }

    BatchGenerationRuntimePersistencePlan::cancelled(
        task.completed_chapters,
        task.total_chapters,
    )
    .persist(db, &task.id)
    .await
    .map_err(CancelBatchGenerationWorkflowError::Domain)?;

    Ok(build_batch_generation_command_summary_payload(
        BatchGenerationCommandProgressSummary {
            batch_id: task.id.clone(),
            total_chapters: task.total_chapters,
            completed_chapters: task.completed_chapters,
        },
        "Batch generation cancelled",
    ))
}

#[cfg(test)]
mod tests {
    use super::CancelBatchGenerationWorkflowError;
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
    use serde_json::json;

    #[test]
    fn should_keep_cancel_batch_generation_workflow_error_shape() {
        let error = CancelBatchGenerationWorkflowError::Domain(
            "Cannot cancel task in status completed".to_string(),
        );

        assert!(matches!(
            error,
            CancelBatchGenerationWorkflowError::Domain(detail)
                if detail == "Cannot cancel task in status completed"
        ));
    }

    #[test]
    fn should_keep_cancel_batch_generation_workflow_task_error_shape() {
        let error = CancelBatchGenerationWorkflowError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        );

        assert!(matches!(
            error,
            CancelBatchGenerationWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::TaskNotFound
            )
        ));
    }

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

    #[test]
    fn should_reject_terminal_batch_generation_task_status_for_cancel() {
        let task = build_task("completed");
        let error = if matches!(task.status.as_str(), "completed" | "failed" | "cancelled") {
            format!("Cannot cancel task in status {}", task.status)
        } else {
            panic!("completed task should not be cancellable");
        };

        assert_eq!(error, "Cannot cancel task in status completed");
    }

    #[test]
    fn should_build_cancel_response_summary_from_task_projection() {
        let mut task = build_task("running");
        task.id = "task-8".to_string();
        task.completed_chapters = 1;
        task.total_chapters = 4;

        let summary = super::BatchGenerationCommandProgressSummary {
            batch_id: task.id.clone(),
            total_chapters: task.total_chapters,
            completed_chapters: task.completed_chapters,
        };

        assert_eq!(summary.batch_id(), "task-8");
        let payload = super::build_batch_generation_command_summary_payload(
            summary,
            "Batch generation cancelled",
        );
        assert_eq!(payload["batch_id"], "task-8");
        assert_eq!(payload["completed_chapters"], 1);
        assert_eq!(payload["total_chapters"], 4);
    }

    #[test]
    fn should_build_cancel_response_payload_from_task_projection() {
        let mut task = build_task("running");
        task.id = "task-9".to_string();
        task.completed_chapters = 2;
        task.total_chapters = 5;

        let payload = super::build_batch_generation_command_summary_payload(
            super::BatchGenerationCommandProgressSummary {
                batch_id: task.id.clone(),
                total_chapters: task.total_chapters,
                completed_chapters: task.completed_chapters,
            },
            "Batch generation cancelled",
        );

        assert_eq!(payload["batch_id"], "task-9");
        assert_eq!(payload["completed_chapters"], 2);
        assert_eq!(payload["total_chapters"], 5);
    }

    #[test]
    fn should_build_cancel_response_payload_from_shared_progress_summary_owner() {
        let payload = super::build_batch_generation_command_summary_payload(
            super::BatchGenerationCommandProgressSummary {
                batch_id: "task-7".to_string(),
                total_chapters: 6,
                completed_chapters: 3,
            },
            "Batch generation cancelled",
        );

        assert_eq!(payload["batch_id"], "task-7");
        assert_eq!(payload["completed_chapters"], 3);
        assert_eq!(payload["total_chapters"], 6);
    }

    #[test]
    fn should_build_cancel_response_payload_from_command_state_projection() {
        let mut task = build_task("running");
        task.id = "task-1".to_string();
        task.total_chapters = 4;
        task.completed_chapters = 1;

        let payload = super::build_batch_generation_command_summary_payload(
            super::BatchGenerationCommandProgressSummary {
                batch_id: task.id.clone(),
                total_chapters: task.total_chapters,
                completed_chapters: task.completed_chapters,
            },
            "Batch generation cancelled",
        );

        assert_eq!(payload["message"], "Batch generation cancelled");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["completed_chapters"], 1);
        assert_eq!(payload["total_chapters"], 4);
    }

    #[test]
    fn should_build_cancel_batch_generation_response_payload() {
        let mut task = build_task("running");
        task.id = "task-1".to_string();
        task.total_chapters = 5;
        task.completed_chapters = 2;

        let payload = super::build_batch_generation_command_summary_payload(
            super::BatchGenerationCommandProgressSummary {
                batch_id: task.id.clone(),
                total_chapters: task.total_chapters,
                completed_chapters: task.completed_chapters,
            },
            "Batch generation cancelled",
        );

        assert_eq!(payload["message"], "Batch generation cancelled");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["completed_chapters"], 2);
        assert_eq!(payload["total_chapters"], 5);
    }
}
