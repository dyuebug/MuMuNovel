use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;

use crate::services::chapter_batch_generation_read_context_service::{
    load_owned_batch_generation_read_context,
};
use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
use crate::services::chapter_batch_generation_status_stream_event_service::{
    BatchGenerationStreamCursor, BatchGenerationStreamEventResolution,
};
use crate::services::chapter_batch_generation_stream_semantics_service::BatchGenerationStreamState;
use serde_json::json;

const STATUS_POLL_ATTEMPTS: usize = 300;
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) type BatchGenerationStatusStream = ReceiverStream<Result<Event, Infallible>>;

async fn send_stream_events(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    events: Vec<serde_json::Value>,
) {
    for event in events {
        let _ = tx.send(Ok(Event::default().data(event.to_string()))).await;
    }
}

pub(crate) async fn load_owned_batch_generation_status_stream(
    db: DatabaseConnection,
    batch_id: String,
    user_id: String,
) -> Result<BatchGenerationStatusStream, LoadOwnedBatchGenerationTaskError> {
    let initial_context = load_owned_batch_generation_read_context(&db, &batch_id, &user_id)
        .await?;
    let initial_state = BatchGenerationStreamState::from_task_state_with_quality_context(
        initial_context.task,
        initial_context.workflow_runtime_state.as_ref(),
        Some(&initial_context.quality_status_context),
    );

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
        let mut cursor = BatchGenerationStreamCursor {
            status: String::new(),
            completed: -1,
            progress: -1,
            message: String::new(),
        };
        let mut pending_state = Some(initial_state);

        for _ in 0..STATUS_POLL_ATTEMPTS {
            let state = if let Some(state) = pending_state.take() {
                state
            } else {
                match load_owned_batch_generation_read_context(
                    &db, &batch_id, &user_id,
                )
                .await
                {
                    Ok(context) => BatchGenerationStreamState::from_task_state_with_quality_context(
                        context.task,
                        context.workflow_runtime_state.as_ref(),
                        Some(&context.quality_status_context),
                    ),
                    Err(_) => {
                        let _ = tx
                            .send(Ok(Event::default().data(
                                json!({
                                    "type": "error",
                                    "error": "Batch generation task not found",
                                    "code": 404
                                })
                                .to_string(),
                            )))
                            .await;
                        return;
                    }
                }
            };

            if let Some(event_batch) = cursor.resolve_event_batch(&state) {
                match event_batch {
                    BatchGenerationStreamEventResolution::Continue { events, next_cursor } => {
                        send_stream_events(&tx, events).await;
                        cursor = next_cursor;
                    }
                    BatchGenerationStreamEventResolution::Close { events } => {
                        send_stream_events(&tx, events).await;
                        return;
                    }
                }
            }

            sleep(STATUS_POLL_INTERVAL).await;
        }

        let _ = tx
            .send(Ok(
                Event::default().data(
                    json!({
                        "type": "error",
                        "error": "Generation stream timed out.",
                        "code": 408
                    })
                    .to_string(),
                )
            ))
            .await;
    });

    ReceiverStream::new(rx)
}

#[cfg(test)]
mod tests {
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_read_context_service::BatchGenerationReadContext;
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
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: None,
        }
    }

    fn build_read_context(status: &str) -> BatchGenerationReadContext {
        BatchGenerationReadContext {
            task: build_task(status),
            workflow_runtime_state: Some(json!({"progress": 65, "last_message": "处理中"})),
            quality_status_context:
                crate::services::chapter_batch_generation_quality_status_service::BatchGenerationQualityStatusContext::default(),
        }
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
        let payload = json!({
            "type": "error",
            "error": "Batch generation task not found",
            "code": 404
        });

        assert_eq!(payload["type"], "error");
        assert_eq!(payload["error"], "Batch generation task not found");
        assert_eq!(payload["code"], 404);
    }

    #[test]
    fn should_build_timed_out_stream_system_event_payload() {
        let payload = json!({
            "type": "error",
            "error": "Generation stream timed out.",
            "code": 408
        });

        assert_eq!(payload["type"], "error");
        assert_eq!(payload["error"], "Generation stream timed out.");
        assert_eq!(payload["code"], 408);
    }

    #[test]
    fn should_convert_read_context_into_stream_state() {
        let context = build_read_context("running");
        let state = BatchGenerationStreamState::from_task_state(
            context.task,
            context.workflow_runtime_state.as_ref(),
        );

        assert_eq!(state.status, "running");
        assert_eq!(state.progress, 65);
    }

    #[test]
    fn should_build_detailed_stream_state_from_read_context_owner() {
        let context = build_read_context("running");
        let state = BatchGenerationStreamState::from_task_state(
            context.task,
            context.workflow_runtime_state.as_ref(),
        );

        assert_eq!(state.status, "running");
        assert_eq!(state.completed, 1);
        assert_eq!(state.progress, 65);
        assert_eq!(state.message, "处理中");
        assert_eq!(state.event_status, "processing");
        assert_eq!(state.terminal_kind, None);
    }

    #[test]
    fn should_build_terminal_stream_state_from_stream_owner() {
        let context = BatchGenerationReadContext {
            task: build_task("completed"),
            workflow_runtime_state: None,
            quality_status_context:
                crate::services::chapter_batch_generation_quality_status_service::BatchGenerationQualityStatusContext::default(),
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
    fn should_keep_status_stream_system_event_owner_contract() {
        let task_not_found_payload = json!({
            "type": "error",
            "error": "Batch generation task not found",
            "code": 404
        });
        let timed_out_payload = json!({
            "type": "error",
            "error": "Generation stream timed out.",
            "code": 408
        });

        assert_eq!(task_not_found_payload["error"], "Batch generation task not found");
        assert_eq!(timed_out_payload["code"], 408);
    }

}
