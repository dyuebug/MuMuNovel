use sea_orm::DatabaseConnection;

use crate::ai::AIConfig;
use crate::models::chapter;
use crate::services::chapter_generation_prompt_context_provider_service::{
    resolve_default_prompt_context_provider_payload, PromptContextProviderPayload,
};

use super::chapter_batch_generation_access_service::{
    build_user_ai_config, load_accessible_chapter_for_generation,
    LoadAccessibleChapterForGenerationError,
};

#[derive(Debug, Clone)]
pub struct SingleChapterGenerationRequest {
    pub target_word_count: Option<i32>,
    pub model: Option<String>,
}

#[derive(Debug)]
pub enum PrepareSingleChapterGenerationRequestError {
    ChapterNotFound,
    ChapterNotFoundOrAccessDenied,
    Config(String),
    Internal(String),
}

#[derive(Debug)]
pub struct PreparedSingleChapterGenerationRequest {
    pub chapter_model: chapter::Model,
    pub ai_config: AIConfig,
    pub provider_payload: PromptContextProviderPayload,
    pub target_word_count: i32,
}

pub fn normalize_single_chapter_generation_target_word_count(
    target_word_count: Option<i32>,
) -> i32 {
    target_word_count.unwrap_or(3000).max(1)
}

pub fn load_chapter_generation_target(request: &SingleChapterGenerationRequest) -> i32 {
    normalize_single_chapter_generation_target_word_count(request.target_word_count)
}

pub async fn prepare_single_chapter_generation_request(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: &SingleChapterGenerationRequest,
) -> Result<PreparedSingleChapterGenerationRequest, PrepareSingleChapterGenerationRequestError> {
    let chapter_model = load_accessible_chapter_for_generation(db, chapter_id, user_id)
        .await
        .map_err(|error| match error {
            LoadAccessibleChapterForGenerationError::ChapterNotFound => {
                PrepareSingleChapterGenerationRequestError::ChapterNotFound
            }
            LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied => {
                PrepareSingleChapterGenerationRequestError::ChapterNotFoundOrAccessDenied
            }
            LoadAccessibleChapterForGenerationError::Internal(error) => {
                PrepareSingleChapterGenerationRequestError::Internal(error)
            }
        })?;
    let ai_config = build_user_ai_config(db, user_id, request.model.as_deref())
        .await
        .map_err(PrepareSingleChapterGenerationRequestError::Config)?;

    Ok(PreparedSingleChapterGenerationRequest {
        chapter_model,
        ai_config,
        provider_payload: resolve_default_prompt_context_provider_payload(),
        target_word_count: load_chapter_generation_target(request),
    })
}
