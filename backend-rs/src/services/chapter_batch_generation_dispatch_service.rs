use sea_orm::DatabaseConnection;

use crate::ai::AIConfig;
use crate::services::chapter_batch_generation_runtime_state_service::{
    execute_batch_generation_runtime, execute_single_generation_runtime,
};
use crate::services::chapter_batch_generation_task_command_service::ResumeExecutionPlan;
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;

use super::chapter_single_generation_request_service::SingleChapterGenerationExecutionInput;

pub(crate) fn dispatch_single_chapter_generation_runtime(
    db: DatabaseConnection,
    task_id: String,
    user_id: String,
    execution_input: SingleChapterGenerationExecutionInput,
) {
    tokio::spawn(async move {
        execute_single_generation_runtime(&db, &task_id, &user_id, execution_input).await;
    });
}

pub(crate) fn dispatch_batch_generation_runtime(
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

pub(crate) fn dispatch_resume_generation_runtime(
    db: DatabaseConnection,
    task_id: String,
    execution: ResumeExecutionPlan,
    ai_config: AIConfig,
    provider_payload: PromptContextProviderPayload,
) {
    match execution {
        ResumeExecutionPlan::SingleChapter {
            chapter_id,
            target_word_count,
            user_id,
        } => {
            dispatch_single_chapter_generation_runtime(
                db,
                task_id,
                user_id,
                SingleChapterGenerationExecutionInput {
                    chapter_id,
                    target_word_count,
                    ai_config,
                    provider_payload,
                },
            );
        }
        ResumeExecutionPlan::Batch {
            chapter_ids,
            target_word_count,
            user_id,
        } => {
            dispatch_batch_generation_runtime(
                db,
                task_id,
                user_id,
                chapter_ids,
                target_word_count,
                ai_config,
                provider_payload,
            );
        }
    }
}
