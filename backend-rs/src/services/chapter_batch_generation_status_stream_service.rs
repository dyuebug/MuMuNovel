use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;

use crate::services::chapter_batch_generation_status_view_service::{
    build_batch_generation_cancelled_event, build_batch_generation_failed_event,
    build_batch_generation_not_found_event, build_batch_generation_progress_event,
    build_batch_generation_result_event, build_batch_generation_timeout_event,
    load_batch_generation_stream_state,
};

const STATUS_POLL_ATTEMPTS: usize = 300;
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub type BatchGenerationStatusStream = ReceiverStream<Result<Event, Infallible>>;

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

    fn has_changed(
        &self,
        status: &str,
        completed: i32,
        progress: i32,
        message: &str,
    ) -> bool {
        self.status != status
            || self.completed != completed
            || self.progress != progress
            || self.message != message
    }

    fn update(
        &mut self,
        status: String,
        completed: i32,
        progress: i32,
        message: String,
    ) {
        self.status = status;
        self.completed = completed;
        self.progress = progress;
        self.message = message;
    }
}

async fn send_json_event(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    payload: serde_json::Value,
) {
    let _ = tx.send(Ok(Event::default().data(payload.to_string()))).await;
}

pub fn build_batch_generation_status_stream(
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

                if state.status == "completed" {
                    send_json_event(&tx, build_batch_generation_result_event(&state)).await;
                    send_json_event(&tx, serde_json::json!({"type":"done"})).await;
                    return;
                }

                if state.status == "failed" {
                    send_json_event(&tx, build_batch_generation_failed_event(&state)).await;
                    return;
                }

                if state.status == "cancelled" {
                    send_json_event(&tx, build_batch_generation_cancelled_event()).await;
                    return;
                }

                cursor.update(
                    state.status,
                    state.completed,
                    state.progress,
                    state.message,
                );
            }

            sleep(STATUS_POLL_INTERVAL).await;
        }

        send_json_event(&tx, build_batch_generation_timeout_event()).await;
    });

    ReceiverStream::new(rx)
}
