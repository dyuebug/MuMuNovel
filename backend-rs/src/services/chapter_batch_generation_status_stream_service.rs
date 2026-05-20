use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;

use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_required_owned_task, map_owned_batch_generation_task_error,
};
use crate::services::chapter_batch_generation_status_view_service::{
    build_batch_generation_not_found_event, build_batch_generation_progress_event,
    build_batch_generation_terminal_events, build_batch_generation_timeout_event,
    load_batch_generation_stream_state,
};

const STATUS_POLL_ATTEMPTS: usize = 300;
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) type BatchGenerationStatusStream = ReceiverStream<Result<Event, Infallible>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BatchGenerationStatusStreamAccessError {
    TaskNotFound,
    Internal(String),
}

struct BatchGenerationStatusCursor {
    status: String,
    completed: i32,
    progress: i32,
    message: String,
}

impl BatchGenerationStatusCursor {
    fn new() -> Self {
        Self {
            status: String::new(),
            completed: -1,
            progress: -1,
            message: String::new(),
        }
    }

    fn has_changed(&self, status: &str, completed: i32, progress: i32, message: &str) -> bool {
        self.status != status
            || self.completed != completed
            || self.progress != progress
            || self.message != message
    }

    fn update(&mut self, status: String, completed: i32, progress: i32, message: String) {
        self.status = status;
        self.completed = completed;
        self.progress = progress;
        self.message = message;
    }
}

async fn send_json_event(tx: &mpsc::Sender<Result<Event, Infallible>>, payload: Value) {
    let _ = tx
        .send(Ok(Event::default().data(payload.to_string())))
        .await;
}

async fn ensure_batch_generation_status_stream_access(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<(), BatchGenerationStatusStreamAccessError> {
    load_required_owned_task(db, batch_id, user_id)
        .await
        .map_err(|error| {
            map_owned_batch_generation_task_error(
                error,
                || BatchGenerationStatusStreamAccessError::TaskNotFound,
                BatchGenerationStatusStreamAccessError::Internal,
            )
        })?;

    Ok(())
}

fn build_batch_generation_status_stream(
    db: DatabaseConnection,
    batch_id: String,
    user_id: String,
) -> BatchGenerationStatusStream {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);

    tokio::spawn(async move {
        let mut cursor = BatchGenerationStatusCursor::new();

        for _ in 0..STATUS_POLL_ATTEMPTS {
            let state = match load_batch_generation_stream_state(&db, &batch_id, &user_id).await {
                Ok(Some(state)) => state,
                _ => {
                    send_json_event(&tx, build_batch_generation_not_found_event()).await;
                    return;
                }
            };

            if cursor.has_changed(
                &state.status,
                state.completed,
                state.progress,
                &state.message,
            ) {
                send_json_event(&tx, build_batch_generation_progress_event(&state)).await;

                if let Some(events) = build_batch_generation_terminal_events(&state) {
                    for event in events {
                        send_json_event(&tx, event).await;
                    }
                    return;
                }

                cursor.update(state.status, state.completed, state.progress, state.message);
            }

            sleep(STATUS_POLL_INTERVAL).await;
        }

        send_json_event(&tx, build_batch_generation_timeout_event()).await;
    });

    ReceiverStream::new(rx)
}

pub(crate) async fn create_owned_batch_generation_status_stream(
    db: DatabaseConnection,
    batch_id: String,
    user_id: String,
) -> Result<BatchGenerationStatusStream, BatchGenerationStatusStreamAccessError> {
    ensure_batch_generation_status_stream_access(&db, &batch_id, &user_id).await?;

    Ok(build_batch_generation_status_stream(db, batch_id, user_id))
}

#[cfg(test)]
mod tests {
    use crate::services::chapter_batch_generation_owned_task_query_service::{
        map_owned_batch_generation_task_error, LoadOwnedBatchGenerationTaskError,
    };

    use super::BatchGenerationStatusStreamAccessError;

    #[test]
    fn should_map_owned_task_not_found_error_for_stream_access() {
        let error = map_owned_batch_generation_task_error(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
            || BatchGenerationStatusStreamAccessError::TaskNotFound,
            BatchGenerationStatusStreamAccessError::Internal,
        );

        assert_eq!(error, BatchGenerationStatusStreamAccessError::TaskNotFound);
    }

    #[test]
    fn should_map_owned_task_internal_error_for_stream_access() {
        let error = map_owned_batch_generation_task_error(
            LoadOwnedBatchGenerationTaskError::Internal("boom".to_string()),
            || BatchGenerationStatusStreamAccessError::TaskNotFound,
            BatchGenerationStatusStreamAccessError::Internal,
        );

        assert_eq!(
            error,
            BatchGenerationStatusStreamAccessError::Internal("boom".to_string())
        );
    }
}
