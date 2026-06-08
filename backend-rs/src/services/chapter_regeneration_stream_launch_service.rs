use std::convert::Infallible;

use axum::response::sse::Event;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::service::AIService;
use crate::services::chapter_candidate_output_service::{
    collect_generation_candidate_output, ChapterCandidateOutputProgress,
    ChapterCandidateOutputRequest,
};
use crate::services::chapter_regeneration_text_service::FinalizePartialRegenerationError;
use crate::utils::sse::{sse_chunk, sse_done, sse_error, sse_result, SseProgress};

pub type OwnedRegenerationStream = ReceiverStream<Result<Event, Infallible>>;

pub struct RegenerationChunkProgress {
    pub chunk_count: u32,
    pub full_content_len: usize,
}

impl From<ChapterCandidateOutputProgress> for RegenerationChunkProgress {
    fn from(progress: ChapterCandidateOutputProgress) -> Self {
        Self {
            chunk_count: progress.chunk_count as u32,
            full_content_len: progress.current_chars,
        }
    }
}

pub fn describe_regeneration_finalize_error(
    error: FinalizePartialRegenerationError,
) -> &'static str {
    match error {
        FinalizePartialRegenerationError::EmptyContent => {
            "Rewrite result is empty after sanitization"
        }
        FinalizePartialRegenerationError::WorkflowMetaText => {
            "Rewrite result still contains workflow meta text"
        }
    }
}

async fn emit_regeneration_finalize_error(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    error: FinalizePartialRegenerationError,
) {
    let _ = tx
        .send(Ok(sse_error(
            describe_regeneration_finalize_error(error),
            500,
        )))
        .await;
}

async fn execute_regeneration_text_stream<F>(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    ai_service: AIService,
    prompt: String,
    mut build_progress_event: F,
) -> Result<String, ()>
where
    F: FnMut(RegenerationChunkProgress) -> Option<Event>,
{
    match collect_generation_candidate_output(
        ChapterCandidateOutputRequest {
            ai_service,
            prompt,
            system_prompt: None,
            tools: None,
            candidate_index: 1,
            max_output_chars: None,
            runtime_state: None,
        },
        |chunk_content, progress| {
            let event = build_progress_event(progress.into());
            let tx = tx.clone();
            async move {
                let _ = tx.send(Ok(sse_chunk(&chunk_content))).await;
                if let Some(event) = event {
                    let _ = tx.send(Ok(event)).await;
                }
                Ok(())
            }
        },
    )
    .await
    {
        Ok(output) => Ok(output.full_content),
        Err(error) => {
            let _ = tx.send(Ok(sse_error(&error, 500))).await;
            Err(())
        }
    }
}

pub enum OwnedRegenerationInitialEvent {
    Preparing {
        message: Option<String>,
    },
    Generating {
        message: Option<String>,
        progress_range: (u32, u32),
        char_count: usize,
        retry_count: Option<u32>,
    },
}

pub struct OwnedRegenerationStreamLaunchInput {
    pub task_label: String,
    pub prompt: String,
    pub ai_service: AIService,
    pub initial_events: Vec<OwnedRegenerationInitialEvent>,
    pub completion_message: String,
}

fn apply_owned_regeneration_initial_event(
    tracker: &mut SseProgress,
    step: &OwnedRegenerationInitialEvent,
) -> Event {
    match step {
        OwnedRegenerationInitialEvent::Preparing { message } => {
            tracker.preparing(message.as_deref())
        }
        OwnedRegenerationInitialEvent::Generating {
            message,
            progress_range,
            char_count,
            retry_count,
        } => tracker.generating(
            message.as_deref(),
            *progress_range,
            *char_count,
            *retry_count,
        ),
    }
}

pub fn build_owned_regeneration_stream<BuildProgress, Finalize>(
    input: OwnedRegenerationStreamLaunchInput,
    mut build_progress_event: BuildProgress,
    finalize_payload: Finalize,
) -> OwnedRegenerationStream
where
    BuildProgress:
        FnMut(&mut SseProgress, RegenerationChunkProgress) -> Option<Event> + Send + 'static,
    Finalize: FnOnce(&str) -> Result<Value, FinalizePartialRegenerationError> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let OwnedRegenerationStreamLaunchInput {
            task_label,
            prompt,
            ai_service,
            initial_events,
            completion_message,
        } = input;

        let mut tracker = SseProgress::new(&task_label);
        let _ = tx.send(Ok(tracker.start())).await;
        for step in &initial_events {
            let event = apply_owned_regeneration_initial_event(&mut tracker, step);
            let _ = tx.send(Ok(event)).await;
        }

        let full_content =
            match execute_regeneration_text_stream(&tx, ai_service, prompt, |progress| {
                build_progress_event(&mut tracker, progress)
            })
            .await
            {
                Ok(full_content) => full_content,
                Err(()) => return,
            };

        let payload = match finalize_payload(&full_content) {
            Ok(payload) => payload,
            Err(error) => {
                emit_regeneration_finalize_error(&tx, error).await;
                return;
            }
        };

        let _ = tx
            .send(Ok(tracker.complete(Some(&completion_message))))
            .await;
        let _ = tx.send(Ok(sse_result(&payload))).await;
        let _ = tx.send(Ok(sse_done())).await;
    });

    ReceiverStream::new(rx)
}

#[cfg(test)]
mod tests {
    use crate::services::chapter_regeneration_text_service::FinalizePartialRegenerationError;
    use crate::utils::sse::SseProgress;

    use super::{
        apply_owned_regeneration_initial_event, describe_regeneration_finalize_error,
        OwnedRegenerationInitialEvent,
    };

    #[test]
    fn should_advance_tracker_for_preparing_initial_event() {
        let mut tracker = SseProgress::new("Rewrite");

        let _ = apply_owned_regeneration_initial_event(
            &mut tracker,
            &OwnedRegenerationInitialEvent::Preparing {
                message: Some("Preparing rewrite context...".to_string()),
            },
        );

        assert_eq!(tracker.current_progress(), 15);
    }

    #[test]
    fn should_advance_tracker_for_generating_initial_event() {
        let mut tracker = SseProgress::new("Rewrite");

        let _ = apply_owned_regeneration_initial_event(
            &mut tracker,
            &OwnedRegenerationInitialEvent::Generating {
                message: Some("Rewriting chapter...".to_string()),
                progress_range: (20, 95),
                char_count: 0,
                retry_count: None,
            },
        );

        assert_eq!(tracker.current_progress(), 20);
    }

    #[test]
    fn should_describe_regeneration_finalize_errors_with_existing_messages() {
        assert_eq!(
            describe_regeneration_finalize_error(FinalizePartialRegenerationError::EmptyContent),
            "Rewrite result is empty after sanitization"
        );
        assert_eq!(
            describe_regeneration_finalize_error(
                FinalizePartialRegenerationError::WorkflowMetaText,
            ),
            "Rewrite result still contains workflow meta text"
        );
    }
}
