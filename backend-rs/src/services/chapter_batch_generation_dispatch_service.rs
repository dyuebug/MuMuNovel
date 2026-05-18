use sea_orm::DatabaseConnection;

use crate::ai::AIConfig;
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::chapter_batch_generation_runtime_state_service::{
    execute_batch_generation_runtime, execute_single_generation_runtime,
};

pub fn dispatch_single_chapter_generation_runtime(
    db: DatabaseConnection,
    task_id: String,
    user_id: String,
    chapter_id: String,
    target_word_count: i32,
    ai_config: AIConfig,
    provider_payload: PromptContextProviderPayload,
) {
    tokio::spawn(async move {
        execute_single_generation_runtime(
            &db,
            &task_id,
            &user_id,
            &chapter_id,
            target_word_count,
            ai_config,
            provider_payload,
        )
        .await;
    });
}

pub fn dispatch_batch_generation_runtime(
    db: DatabaseConnection,
    task_id: String,
    user_id: String,
    chapter_ids: Vec<String>,
    target_word_count: i32,
    ai_config: AIConfig,
    provider_payload: PromptContextProviderPayload,
) {
    tokio::spawn(async move {
        execute_batch_generation_runtime(
            &db,
            &task_id,
            &user_id,
            &chapter_ids,
            target_word_count,
            ai_config,
            provider_payload,
        )
        .await;
    });
}
