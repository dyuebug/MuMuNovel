use std::convert::Infallible;

use axum::response::sse::Event;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::service::AIService;
use crate::services::chapter_regeneration_text_service::{
    finalize_chapter_regeneration_result, FinalizePartialRegenerationError,
};
use crate::utils::sse::{
    sse_chunk, sse_done, sse_error, sse_result, SseProgress,
};

pub type FullChapterRegenerationStream =
    ReceiverStream<Result<Event, Infallible>>;

pub struct FullChapterRegenerationStreamInput {
    pub task_label: String,
    pub chapter_id: String,
    pub chapter_word_count: usize,
    pub prompt: String,
    pub ai_service: AIService,
}

pub fn build_full_chapter_regeneration_stream(
    input: FullChapterRegenerationStreamInput,
) -> FullChapterRegenerationStream {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let FullChapterRegenerationStreamInput {
            task_label,
            chapter_id,
            chapter_word_count,
            prompt,
            ai_service,
        } = input;

        let mut tracker = SseProgress::new(&task_label);
        let _ = tx.send(Ok(tracker.start())).await;
        let _ = tx
            .send(Ok(tracker.preparing(Some("Building rewrite prompt..."))))
            .await;
        let _ = tx
            .send(Ok(tracker.generating(
                Some("Rewriting chapter..."),
                (20, 95),
                chapter_word_count,
                None,
            )))
            .await;

        let mut full_content = String::new();
        let mut rx_stream = ai_service.generate_text_stream(prompt, None, None);
        while let Some(chunk) = rx_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let chunk_content = chunk.content.unwrap_or_default();
                    full_content.push_str(&chunk_content);
                    let _ = tx.send(Ok(sse_chunk(&chunk_content))).await;
                }
                Err(error) => {
                    let _ = tx.send(Ok(sse_error(&error, 500))).await;
                    return;
                }
            }
        }

        let result =
            match finalize_chapter_regeneration_result(&full_content, &chapter_id) {
                Ok(result) => result,
                Err(FinalizePartialRegenerationError::EmptyContent) => {
                    let _ = tx
                        .send(Ok(sse_error(
                            "Rewrite result is empty after sanitization",
                            500,
                        )))
                        .await;
                    return;
                }
                Err(FinalizePartialRegenerationError::WorkflowMetaText) => {
                    let _ = tx
                        .send(Ok(sse_error(
                            "Rewrite result still contains workflow meta text",
                            500,
                        )))
                        .await;
                    return;
                }
            };

        let _ = tx.send(Ok(tracker.complete(Some("Rewrite complete")))).await;
        let _ = tx.send(Ok(sse_result(&result.payload))).await;
        let _ = tx.send(Ok(sse_done())).await;
    });

    ReceiverStream::new(rx)
}
