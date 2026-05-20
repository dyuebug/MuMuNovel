use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::services::chapter_batch_generation_dispatch_service::dispatch_single_chapter_generation_runtime;

use super::chapter_batch_generation_task_command_service::create_single_generation_background_task_plan;
use super::chapter_single_generation_request_service::{
    prepare_single_chapter_generation_request, PrepareSingleChapterGenerationRequestError,
    SingleChapterGenerationRequest,
};

#[derive(Debug)]
pub(crate) enum CreateSingleGenerationBackgroundWorkflowError {
    ChapterNotFound,
    ChapterNotFoundOrAccessDenied,
    Config(String),
    Internal(String),
}

fn map_prepare_error(
    error: PrepareSingleChapterGenerationRequestError,
) -> CreateSingleGenerationBackgroundWorkflowError {
    match error {
        PrepareSingleChapterGenerationRequestError::ChapterNotFound => {
            CreateSingleGenerationBackgroundWorkflowError::ChapterNotFound
        }
        PrepareSingleChapterGenerationRequestError::ChapterNotFoundOrAccessDenied => {
            CreateSingleGenerationBackgroundWorkflowError::ChapterNotFoundOrAccessDenied
        }
        PrepareSingleChapterGenerationRequestError::Config(error) => {
            CreateSingleGenerationBackgroundWorkflowError::Config(error)
        }
        PrepareSingleChapterGenerationRequestError::Internal(error) => {
            CreateSingleGenerationBackgroundWorkflowError::Internal(error)
        }
    }
}

pub(crate) async fn start_owned_single_generation_background_workflow(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: SingleChapterGenerationRequest,
) -> Result<Value, CreateSingleGenerationBackgroundWorkflowError> {
    let prepared = prepare_single_chapter_generation_request(db, chapter_id, user_id, &request)
        .await
        .map_err(map_prepare_error)?;

    let plan = create_single_generation_background_task_plan(
        db,
        user_id,
        &prepared.chapter_model,
        prepared.execution_input.target_word_count,
    )
    .await
    .map_err(CreateSingleGenerationBackgroundWorkflowError::Internal)?;

    let response_payload = plan.response_payload;
    let created_task_id = plan.created_task_id;
    let mut execution_input = prepared.execution_input;
    execution_input.target_word_count = plan.target_word_count;

    dispatch_single_chapter_generation_runtime(
        db.clone(),
        created_task_id,
        user_id.to_string(),
        execution_input,
    );

    Ok(response_payload)
}
