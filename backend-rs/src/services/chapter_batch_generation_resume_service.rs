use serde_json::Value;
use sea_orm::DatabaseConnection;

use crate::ai::AIConfig;
use crate::services::chapter_batch_generation_access_service::build_user_ai_config;
use crate::services::chapter_batch_generation_owned_task_query_service::load_owned_task;
use crate::services::chapter_batch_generation_task_command_service::{
    prepare_batch_generation_resume, ResumeExecutionPlan,
};
use crate::services::chapter_generation_prompt_context_provider_service::{
    resolve_default_prompt_context_provider_payload, PromptContextProviderPayload,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareBatchGenerationResumeRequestError {
    NotFound,
    Domain(String),
    Config(String),
    Internal(String),
}

pub struct PreparedBatchGenerationResumeRequest {
    pub response_payload: Value,
    pub ai_config: AIConfig,
    pub provider_payload: PromptContextProviderPayload,
    pub execution: ResumeExecutionPlan,
}

pub async fn prepare_batch_generation_resume_request(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<PreparedBatchGenerationResumeRequest, PrepareBatchGenerationResumeRequestError> {
    let task = load_owned_task(db, batch_id, user_id)
        .await
        .map_err(PrepareBatchGenerationResumeRequestError::Internal)?
        .ok_or(PrepareBatchGenerationResumeRequestError::NotFound)?;

    let plan = prepare_batch_generation_resume(db, task, user_id)
        .await
        .map_err(PrepareBatchGenerationResumeRequestError::Domain)?;

    let ai_config = build_user_ai_config(db, user_id, None)
        .await
        .map_err(PrepareBatchGenerationResumeRequestError::Config)?;

    Ok(PreparedBatchGenerationResumeRequest {
        response_payload: plan.response_payload,
        ai_config,
        provider_payload: resolve_default_prompt_context_provider_payload(),
        execution: plan.execution,
    })
}
