use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::service::AIService;
use crate::ai::AIConfig;
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::chapter_generation_service::{
    generate_and_persist_chapter_content_with_provider_payload,
};
use crate::utils::sse::{sse_done, sse_error, sse_result, SseProgress};

pub type SingleChapterGenerationStream =
    ReceiverStream<Result<Event, Infallible>>;

pub fn build_single_chapter_generation_stream(
    db: DatabaseConnection,
    user_id: String,
    chapter_id: String,
    target_word_count: i32,
    ai_config: AIConfig,
    provider_payload: PromptContextProviderPayload,
) -> SingleChapterGenerationStream {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);

    tokio::spawn(async move {
        let mut tracker = SseProgress::new("Chapter Generation");
        let _ = tx.send(Ok(tracker.start())).await;
        let _ = tx
            .send(Ok(tracker.preparing(Some("Preparing chapter generation..."))))
            .await;
        let _ = tx
            .send(Ok(tracker.generating(
                Some("Generating chapter content..."),
                (15, 95),
                target_word_count as usize,
                None,
            )))
            .await;

        let ai_service = AIService::new(ai_config);
        match generate_and_persist_chapter_content_with_provider_payload(
            &db,
            &ai_service,
            &user_id,
            &chapter_id,
            target_word_count,
            provider_payload,
        )
        .await
        {
            Ok(payload) => {
                let _ = tx
                    .send(Ok(tracker.complete(Some("Generation complete"))))
                    .await;
                let _ = tx.send(Ok(sse_result(&payload))).await;
                let _ = tx.send(Ok(sse_done())).await;
            }
            Err(error) => {
                let _ = tx.send(Ok(sse_error(&error, 500))).await;
            }
        }
    });

    ReceiverStream::new(rx)
}
