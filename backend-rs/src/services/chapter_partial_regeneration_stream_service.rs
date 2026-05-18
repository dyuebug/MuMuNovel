use std::convert::Infallible;

use axum::response::sse::Event;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::services::chapter_regeneration_prepare_service::PreparedPartialRegenerationStream;
use crate::services::chapter_regeneration_text_service::{
    finalize_partial_regeneration_result, FinalizePartialRegenerationError,
};
use crate::utils::sse::{
    sse_chunk, sse_done, sse_error, sse_result, SseProgress,
};

pub type PartialChapterRegenerationStream =
    ReceiverStream<Result<Event, Infallible>>;

pub fn build_partial_chapter_regeneration_stream(
    stream_prepared: PreparedPartialRegenerationStream,
    start_position: usize,
    end_position: usize,
) -> PartialChapterRegenerationStream {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let prepared = stream_prepared.prepared;
        let ai_service = stream_prepared.ai_service;
        let target_words = prepared.target_words;
        let original_word_count = prepared.original_word_count;
        let prompt = prepared.prompt;

        let mut tracker = SseProgress::new("Partial Rewrite");
        let _ = tx.send(Ok(tracker.start())).await;
        let _ = tx
            .send(Ok(tracker.preparing(Some("Preparing rewrite context..."))))
            .await;
        let _ = tx
            .send(Ok(tracker.preparing(Some("Starting generation..."))))
            .await;

        let mut full_content = String::new();
        let mut chunk_count = 0u32;

        let mut rx_stream = ai_service.generate_text_stream(prompt, None, None);
        while let Some(chunk) = rx_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let chunk_content = chunk.content.unwrap_or_default();
                    full_content.push_str(&chunk_content);
                    chunk_count += 1;
                    let _ = tx.send(Ok(sse_chunk(&chunk_content))).await;
                    if chunk_count % 5 == 0 {
                        let _ = tx
                            .send(Ok(tracker.generating(
                                Some(&format!(
                                    "Generating rewrite... {}/{} chars",
                                    full_content.len(),
                                    target_words
                                )),
                                (35, 95),
                                full_content.len(),
                                None,
                            )))
                            .await;
                    }
                }
                Err(error) => {
                    let _ = tx.send(Ok(sse_error(&error, 500))).await;
                    return;
                }
            }
        }

        let result = match finalize_partial_regeneration_result(
            &full_content,
            original_word_count,
            start_position,
            end_position,
        ) {
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
