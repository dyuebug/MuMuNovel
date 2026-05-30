use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_owned_task, LoadOwnedBatchGenerationTaskError,
};
use crate::services::chapter_batch_generation_quality_status_service::{
    insert_batch_generation_terminal_status_payload, BatchGenerationQualityStatusContext,
};
use crate::services::chapter_batch_generation_snapshot_service::load_batch_generation_snapshot;
use crate::services::chapter_batch_generation_task_payload_base_service::build_batch_generation_task_view_payload_from_task_state;

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationReadContext {
    pub(crate) task: batch_generation_task::Model,
    pub(crate) workflow_runtime_state: Option<Value>,
    pub(crate) quality_status_context: BatchGenerationQualityStatusContext,
}

impl BatchGenerationReadContext {
    fn into_payload_parts(self) -> (batch_generation_task::Model, serde_json::Map<String, Value>) {
        let BatchGenerationReadContext {
            task,
            workflow_runtime_state,
            quality_status_context,
        } = self;
        let mut payload = build_batch_generation_task_view_payload_from_task_state(
            &task,
            workflow_runtime_state.as_ref(),
        );
        quality_status_context.insert_into_payload(&mut payload);

        (task, payload)
    }

    pub(crate) fn into_status_task_payload(self) -> Value {
        let BatchGenerationReadContext {
            task,
            workflow_runtime_state,
            quality_status_context,
        } = self;
        let mut payload =
            build_batch_generation_task_view_payload_from_task_state(&task, workflow_runtime_state.as_ref());
        quality_status_context.insert_into_payload(&mut payload);

        payload.insert(
            "current_retry_count".to_string(),
            json!(task.current_retry_count),
        );
        payload.insert("max_retries".to_string(), json!(task.max_retries));
        payload.insert(
            "failed_chapters".to_string(),
            task.failed_chapters.clone(),
        );
        insert_batch_generation_terminal_status_payload(
            &mut payload,
            &task,
            Some(&task.failed_chapters),
            Some(&quality_status_context),
        );

        Value::Object(payload)
    }

    pub(crate) fn into_active_task_payload(self) -> Value {
        let (_, payload) = self.into_payload_parts();
        Value::Object(payload)
    }
}

pub(crate) async fn load_batch_generation_read_context_for_task(
    db: &DatabaseConnection,
    task: batch_generation_task::Model,
) -> Result<BatchGenerationReadContext, String> {
    let snapshot = load_batch_generation_snapshot(db, &task.id).await?;
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.clone());
    let quality_context = BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
        snapshot.as_ref(),
        workflow_runtime_state.as_ref(),
    );

    Ok(BatchGenerationReadContext {
        task,
        workflow_runtime_state,
        quality_status_context: quality_context,
    })
}

pub(crate) async fn load_owned_batch_generation_read_context(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<BatchGenerationReadContext, LoadOwnedBatchGenerationTaskError> {
    let task = load_owned_task(db, batch_id, user_id)
        .await
        .map_err(LoadOwnedBatchGenerationTaskError::Internal)?
        .ok_or(LoadOwnedBatchGenerationTaskError::TaskNotFound)?;

    load_batch_generation_read_context_for_task(db, task)
        .await
        .map_err(LoadOwnedBatchGenerationTaskError::Internal)
}

pub(crate) async fn load_owned_batch_generation_status_payload(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, LoadOwnedBatchGenerationTaskError> {
    load_owned_batch_generation_read_context(db, batch_id, user_id)
        .await
        .map(BatchGenerationReadContext::into_status_task_payload)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
    use crate::services::chapter_batch_generation_quality_status_service::BatchGenerationQualityStatusContext;
    use crate::services::chapter_batch_generation_stream_semantics_service::{
        BatchGenerationStreamState, BatchGenerationStreamTerminalKind,
    };
    use super::BatchGenerationReadContext;

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn build_snapshot() -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            workflow_runtime_state: Some(json!({
                "progress": 60,
                "active_story_repair_payload": {
                    "mode": "repair"
                }
            })),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_build_batch_generation_read_context_from_snapshot() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let context = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        };

        assert_eq!(context.task.id, "task-1");
        assert_eq!(
            context.workflow_runtime_state,
            Some(json!({
                "progress": 60,
                "active_story_repair_payload": {
                    "mode": "repair"
                }
            }))
        );
        assert_eq!(
            context.quality_status_context.latest_quality_metrics,
            Some(json!({"score": 91}))
        );
        assert_eq!(
            context.quality_status_context.quality_metrics_summary,
            Some(json!({"summary": "ok"}))
        );
        assert_eq!(
            context.quality_status_context.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_batch_generation_read_context_without_snapshot() {
        let context = BatchGenerationReadContext {
            task: build_task("pending"),
            workflow_runtime_state: None,
            quality_status_context: BatchGenerationQualityStatusContext::default(),
        };

        assert_eq!(context.task.status, "pending");
        assert_eq!(context.workflow_runtime_state, None);
        assert_eq!(
            context.quality_status_context,
            BatchGenerationQualityStatusContext::default()
        );
    }

    #[test]
    fn should_build_shared_read_payload_plan_from_context_owner() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let (task, payload) = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_payload_parts();

        assert_eq!(task.id, "task-1");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_stream_state_from_read_context_owner() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let context = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        };
        let state = BatchGenerationStreamState::from_task_state(
            context.task,
            context.workflow_runtime_state.as_ref(),
        );

        assert_eq!(state.status, "running");
        assert_eq!(state.completed, 1);
        assert_eq!(state.progress, 60);
        assert_eq!(state.message, "正在生成正文...");
        assert_eq!(state.event_status, "processing");
        assert_eq!(state.terminal_kind, None);
    }

    #[test]
    fn should_build_terminal_stream_state_from_read_context_owner() {
        let context = BatchGenerationReadContext {
            task: build_task("completed"),
            workflow_runtime_state: None,
            quality_status_context: BatchGenerationQualityStatusContext::default(),
        };
        let state = BatchGenerationStreamState::from_task_state(
            context.task,
            context.workflow_runtime_state.as_ref(),
        );

        assert_eq!(state.progress, 100);
        assert_eq!(state.message, "生成完成");
        assert_eq!(state.event_status, "success");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
    }

    #[test]
    fn should_build_status_task_payload_from_read_context() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let payload = BatchGenerationReadContext {
            task: build_task("completed"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_status_task_payload();

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["current_retry_count"], 0);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(payload["terminal_reason"], "completed");
        assert_eq!(payload["review_required"], false);
        assert_eq!(payload["can_resume"], false);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_active_task_payload_from_read_context() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let payload = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_active_task_payload();

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
        assert!(payload.get("current_retry_count").is_none());
        assert!(payload.get("terminal_reason").is_none());
        assert!(payload.get("can_resume").is_none());
    }

    #[test]
    fn should_build_status_task_payload_with_terminal_fields() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let payload = BatchGenerationReadContext {
            task: build_task("completed"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_status_task_payload();

        assert_eq!(payload["current_retry_count"], 0);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(payload["terminal_reason"], "completed");
        assert_eq!(payload["review_required"], false);
        assert_eq!(payload["can_resume"], false);
    }

    #[test]
    fn should_build_active_task_payload_without_terminal_fields() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let payload = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_active_task_payload();

        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert!(payload.get("current_retry_count").is_none());
        assert!(payload.get("failed_chapters").is_none());
        assert!(payload.get("terminal_reason").is_none());
    }

    #[test]
    fn should_convert_owned_read_context_value_into_optional_context() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let context = Some(BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        })
        .expect("context");

        assert_eq!(context.task.id, "task-1");
        assert_eq!(
            context.quality_status_context.latest_quality_metrics,
            Some(json!({"score": 91}))
        );
    }

    #[test]
    fn should_convert_missing_owned_read_context_value_into_required_error() {
        let error: Result<BatchGenerationReadContext, LoadOwnedBatchGenerationTaskError> =
            None.ok_or(LoadOwnedBatchGenerationTaskError::TaskNotFound);
        let error = error.expect_err("missing should become task not found");

        assert_eq!(error, LoadOwnedBatchGenerationTaskError::TaskNotFound);
    }

    #[test]
    fn should_keep_owned_read_context_loader_error_contract() {
        let missing = LoadOwnedBatchGenerationTaskError::TaskNotFound;
        let internal = LoadOwnedBatchGenerationTaskError::Internal("boom".to_string());

        assert_eq!(missing, LoadOwnedBatchGenerationTaskError::TaskNotFound);
        assert_eq!(
            internal,
            LoadOwnedBatchGenerationTaskError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_keep_owned_status_payload_loader_error_contract() {
        let missing = LoadOwnedBatchGenerationTaskError::TaskNotFound;
        let internal = LoadOwnedBatchGenerationTaskError::Internal("boom".to_string());

        assert_eq!(missing, LoadOwnedBatchGenerationTaskError::TaskNotFound);
        assert_eq!(
            internal,
            LoadOwnedBatchGenerationTaskError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_keep_read_payload_parts_contract() {
        let task = build_task("running");
        let payload = serde_json::Map::from_iter([
            ("batch_id".to_string(), json!("task-1")),
            ("status".to_string(), json!("running")),
        ]);

        assert_eq!(task.id, "task-1");
        assert_eq!(task.failed_chapters, json!([]));
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["status"], "running");
    }
}
