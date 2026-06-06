use std::convert::Infallible;

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_owned_batch_generation_task_read_state, LoadOwnedBatchGenerationTaskError,
};
use crate::services::chapter_batch_generation_quality_status_service::BatchGenerationQualityStatusContext;
use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;

use crate::services::chapter_batch_generation_status_stream_event_service::{
    batch_generation_stream_connected_event_payload, batch_generation_stream_data_event,
    batch_generation_stream_heartbeat_event, batch_generation_stream_task_not_found_event_payload,
    batch_generation_stream_timeout_event_payload, BatchGenerationStreamCursor,
    BatchGenerationStreamEventResolution,
};
use crate::services::chapter_batch_generation_stream_semantics_service::BatchGenerationStreamState;

const STATUS_POLL_ATTEMPTS: usize = 300;
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const STATUS_HEARTBEAT_POLL_INTERVAL: usize = 15;

pub(crate) type BatchGenerationStatusStream = ReceiverStream<Result<Event, Infallible>>;

fn build_batch_generation_stream_state_from_task_and_snapshot(
    task: batch_generation_task::Model,
    snapshot: Option<batch_generation_snapshot::Model>,
) -> BatchGenerationStreamState {
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.as_ref());
    let quality_status_context =
        BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            snapshot.as_ref(),
            workflow_runtime_state,
        );

    BatchGenerationStreamState::from_task_state_with_quality_context(
        task,
        workflow_runtime_state,
        Some(&quality_status_context),
    )
}

async fn load_owned_batch_generation_stream_state(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<BatchGenerationStreamState, LoadOwnedBatchGenerationTaskError> {
    let (task, snapshot) = load_owned_batch_generation_task_read_state(db, batch_id, user_id)
        .await?
        .into_parts();

    Ok(build_batch_generation_stream_state_from_task_and_snapshot(
        task, snapshot,
    ))
}

async fn send_stream_event(tx: &mpsc::Sender<Result<Event, Infallible>>, event: Event) {
    let _ = tx.send(Ok(event)).await;
}

async fn send_stream_events(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    events: Vec<serde_json::Value>,
) {
    for event in events {
        send_stream_event(tx, batch_generation_stream_data_event(event)).await;
    }
}

pub(crate) async fn load_owned_batch_generation_status_stream(
    db: DatabaseConnection,
    batch_id: String,
    user_id: String,
) -> Result<BatchGenerationStatusStream, LoadOwnedBatchGenerationTaskError> {
    let initial_state = load_owned_batch_generation_stream_state(&db, &batch_id, &user_id).await?;

    Ok(build_batch_generation_status_stream(
        db,
        batch_id,
        user_id,
        initial_state,
    ))
}

pub(crate) fn build_batch_generation_status_stream(
    db: DatabaseConnection,
    batch_id: String,
    user_id: String,
    initial_state: BatchGenerationStreamState,
) -> BatchGenerationStatusStream {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);

    tokio::spawn(async move {
        let mut cursor = BatchGenerationStreamCursor { observation: None };
        let mut idle_poll_count = 0usize;
        let mut pending_state = Some(initial_state);
        send_stream_event(
            &tx,
            batch_generation_stream_data_event(batch_generation_stream_connected_event_payload()),
        )
        .await;

        for _ in 0..STATUS_POLL_ATTEMPTS {
            let state = if let Some(state) = pending_state.take() {
                state
            } else {
                match load_owned_batch_generation_stream_state(&db, &batch_id, &user_id).await {
                    Ok(state) => state,
                    Err(_) => {
                        send_stream_event(
                            &tx,
                            batch_generation_stream_data_event(
                                batch_generation_stream_task_not_found_event_payload(),
                            ),
                        )
                        .await;
                        return;
                    }
                }
            };

            if let Some(event_batch) = cursor.resolve_event_batch(&state) {
                match event_batch {
                    BatchGenerationStreamEventResolution::Continue {
                        events,
                        next_cursor,
                    } => {
                        send_stream_events(&tx, events).await;
                        cursor = next_cursor;
                        idle_poll_count = 0;
                    }
                    BatchGenerationStreamEventResolution::Close { events } => {
                        send_stream_events(&tx, events).await;
                        return;
                    }
                }
            } else {
                idle_poll_count += 1;
                if idle_poll_count >= STATUS_HEARTBEAT_POLL_INTERVAL {
                    send_stream_event(&tx, batch_generation_stream_heartbeat_event()).await;
                    idle_poll_count = 0;
                }
            }

            sleep(STATUS_POLL_INTERVAL).await;
        }

        send_stream_event(
            &tx,
            batch_generation_stream_data_event(batch_generation_stream_timeout_event_payload()),
        )
        .await;
    });

    ReceiverStream::new(rx)
}

#[cfg(test)]
mod tests {
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_batch_generation_owned_task_query_service::OwnedBatchGenerationTaskReadState;
    use crate::services::chapter_batch_generation_status_stream_event_service::batch_generation_stream_heartbeat_comment;
    use crate::services::chapter_batch_generation_stream_semantics_service::{
        BatchGenerationStreamState, BatchGenerationStreamTerminalKind,
    };
    use serde_json::json;

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

    fn build_stream_state(status: &str) -> BatchGenerationStreamState {
        BatchGenerationStreamState {
            task: build_task(status),
            status: status.to_string(),
            completed: 1,
            progress: 65,
            message: "处理中".to_string(),
            phase: "generating".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        }
    }

    #[test]
    fn should_build_python_compatible_stream_connected_event_payload() {
        let payload = super::batch_generation_stream_connected_event_payload();

        assert_eq!(payload["type"], "progress");
        assert_eq!(payload["message"], "正在连接批量生成任务流");
        assert_eq!(payload["progress"], 0);
        assert_eq!(payload["status"], "processing");
    }

    #[test]
    fn should_resolve_stream_poll_from_pending_initial_state_first() {
        let initial_state = build_stream_state("running");
        let mut pending_state = Some(initial_state.clone());

        let state = match pending_state.take() {
            Some(state) => state,
            None => panic!("pending state should be present when checked"),
        };

        assert_eq!(state.status, "running");
        assert_eq!(state.progress, 65);

        assert!(pending_state.is_none());
    }

    #[test]
    fn should_close_stream_poll_when_loaded_state_is_missing() {
        let pending_state: Option<BatchGenerationStreamState> = None;

        assert!(pending_state.is_none());
    }

    #[test]
    fn should_build_task_not_found_stream_system_event_payload() {
        let payload = super::batch_generation_stream_task_not_found_event_payload();

        assert_eq!(payload["type"], "error");
        assert_eq!(payload["error"], "批量生成任务不存在");
        assert_eq!(payload["code"], 404);
    }

    #[test]
    fn should_build_timed_out_stream_system_event_payload() {
        let payload = super::batch_generation_stream_timeout_event_payload();

        assert_eq!(payload["type"], "error");
        assert_eq!(payload["error"], "批量生成任务流等待超时");
        assert_eq!(payload["code"], 408);
    }

    #[test]
    fn should_build_python_compatible_stream_heartbeat_comment() {
        assert_eq!(batch_generation_stream_heartbeat_comment(), "heartbeat");
    }

    #[test]
    fn should_build_stream_state_from_task_and_snapshot_owner_inside_status_stream_service() {
        let state = super::build_batch_generation_stream_state_from_task_and_snapshot(
            build_task("running"),
            Some(build_snapshot()),
        );

        assert_eq!(state.status, "running");
        assert_eq!(state.completed, 1);
        assert_eq!(state.progress, 60);
        assert_eq!(state.message, "正在生成正文...");
        assert_eq!(state.event_status, "processing");
        assert_eq!(
            state.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_terminal_stream_state_from_task_and_snapshot_owner_inside_status_stream_service(
    ) {
        let state = super::build_batch_generation_stream_state_from_task_and_snapshot(
            build_task("completed"),
            None,
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
    fn should_build_stream_state_from_shared_owned_read_state_owner_inside_status_stream_service() {
        let (task, snapshot) = OwnedBatchGenerationTaskReadState::from_parts(
            build_task("running"),
            Some(build_snapshot()),
        )
        .into_parts();
        let state =
            super::build_batch_generation_stream_state_from_task_and_snapshot(task, snapshot);

        assert_eq!(state.status, "running");
        assert_eq!(state.progress, 60);
        assert_eq!(state.event_status, "processing");
        assert_eq!(
            state.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_terminal_stream_state_from_stream_owner() {
        let state = BatchGenerationStreamState::from_task_state(build_task("completed"), None);

        assert_eq!(state.progress, 100);
        assert_eq!(state.message, "生成完成");
        assert_eq!(state.event_status, "success");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
    }

    #[test]
    fn should_keep_status_stream_system_event_owner_contract() {
        let task_not_found_payload = super::batch_generation_stream_task_not_found_event_payload();
        let timed_out_payload = super::batch_generation_stream_timeout_event_payload();

        assert_eq!(task_not_found_payload["error"], "批量生成任务不存在");
        assert_eq!(timed_out_payload["code"], 408);
        assert_eq!(batch_generation_stream_heartbeat_comment(), "heartbeat");
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
}
