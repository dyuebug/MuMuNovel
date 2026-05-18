use serde_json::Value;
use sea_orm::DatabaseConnection;

use crate::ai::AIConfig;
use crate::services::chapter_generation_prompt_context_provider_service::{
    resolve_default_prompt_context_provider_payload, PromptContextProviderPayload,
};

use super::chapter_batch_generation_access_service::{
    build_user_ai_config, verify_project_access,
};
use super::chapter_batch_generation_create_service::{
    prepare_batch_generation_create_request, BatchGenerationCreateRequest,
    PrepareBatchGenerationCreateRequestError,
};
use super::chapter_batch_generation_task_command_service::create_batch_generation_task_plan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateBatchGenerationWorkflowDomainError {
    InvalidCount,
    ChaptersNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateBatchGenerationWorkflowError {
    ProjectNotFoundOrAccessDenied,
    Domain(CreateBatchGenerationWorkflowDomainError),
    Config(String),
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct BatchGenerationCreateWorkflowRequest {
    pub start_chapter_number: i32,
    pub count: i32,
    pub style_id: Option<i32>,
    pub target_word_count: Option<i32>,
    pub enable_analysis: Option<bool>,
    pub max_retries: Option<i32>,
    pub model_override: Option<String>,
}

#[derive(Debug)]
pub struct CreateBatchGenerationWorkflowResult {
    pub response_payload: Value,
    pub ai_config: AIConfig,
    pub provider_payload: PromptContextProviderPayload,
    pub chapter_ids: Vec<String>,
    pub target_word_count: i32,
    pub created_task_id: String,
}

fn map_prepare_error(
    error: PrepareBatchGenerationCreateRequestError,
) -> CreateBatchGenerationWorkflowError {
    match error {
        PrepareBatchGenerationCreateRequestError::InvalidCount => {
            CreateBatchGenerationWorkflowError::Domain(
                CreateBatchGenerationWorkflowDomainError::InvalidCount,
            )
        }
        PrepareBatchGenerationCreateRequestError::ChaptersNotFound => {
            CreateBatchGenerationWorkflowError::Domain(
                CreateBatchGenerationWorkflowDomainError::ChaptersNotFound,
            )
        }
        PrepareBatchGenerationCreateRequestError::Internal(error) => {
            CreateBatchGenerationWorkflowError::Internal(error)
        }
    }
}

pub async fn create_batch_generation_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    request: &BatchGenerationCreateWorkflowRequest,
) -> Result<CreateBatchGenerationWorkflowResult, CreateBatchGenerationWorkflowError> {
    let has_access = verify_project_access(db, project_id, user_id)
        .await
        .map_err(CreateBatchGenerationWorkflowError::Internal)?;
    if !has_access {
        return Err(CreateBatchGenerationWorkflowError::ProjectNotFoundOrAccessDenied);
    }

    let prepared = prepare_batch_generation_create_request(
        db,
        project_id,
        &BatchGenerationCreateRequest {
            start_chapter_number: request.start_chapter_number,
            count: request.count,
            target_word_count: request.target_word_count,
        },
    )
    .await
    .map_err(map_prepare_error)?;

    let ai_config = build_user_ai_config(db, user_id, request.model_override.as_deref())
        .await
        .map_err(CreateBatchGenerationWorkflowError::Config)?;

    let plan = create_batch_generation_task_plan(
        db,
        project_id,
        user_id,
        request.start_chapter_number,
        &prepared.chapters_to_generate,
        request.style_id,
        prepared.normalized_target_word_count,
        request.enable_analysis.unwrap_or(false),
        request.max_retries.unwrap_or(3),
    )
    .await
    .map_err(CreateBatchGenerationWorkflowError::Internal)?;

    Ok(CreateBatchGenerationWorkflowResult {
        response_payload: plan.response_payload,
        ai_config,
        provider_payload: resolve_default_prompt_context_provider_payload(),
        chapter_ids: plan.chapter_ids,
        target_word_count: plan.target_word_count,
        created_task_id: plan.created_task.id,
    })
}
