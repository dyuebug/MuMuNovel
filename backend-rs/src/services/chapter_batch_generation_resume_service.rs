use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::ai::AIConfig;
use crate::services::chapter_batch_generation_access_service::prepare_generation_execution_config;
use crate::services::chapter_batch_generation_dispatch_service::dispatch_resume_generation_runtime;
use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_required_owned_task, map_owned_batch_generation_task_error,
};
use crate::services::chapter_batch_generation_task_command_service::{
    prepare_batch_generation_resume, ResumeExecutionPlan,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrepareBatchGenerationResumeRequestError {
    NotFound,
    Domain(String),
    Config(String),
    Internal(String),
}

pub(crate) async fn resume_owned_batch_generation_task(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, PrepareBatchGenerationResumeRequestError> {
    let task = load_required_owned_task(db, batch_id, user_id)
        .await
        .map_err(|error| {
            map_owned_batch_generation_task_error(
                error,
                || PrepareBatchGenerationResumeRequestError::NotFound,
                PrepareBatchGenerationResumeRequestError::Internal,
            )
        })?;

    let plan = prepare_batch_generation_resume(db, task, user_id)
        .await
        .map_err(PrepareBatchGenerationResumeRequestError::Domain)?;

    let execution_config = prepare_generation_execution_config(db, user_id, None)
        .await
        .map_err(PrepareBatchGenerationResumeRequestError::Config)?;

    let response_payload = plan.response_payload;
    let execution: ResumeExecutionPlan = plan.execution;
    let ai_config: AIConfig = execution_config.ai_config;
    let provider_payload = execution_config.provider_payload;

    dispatch_resume_generation_runtime(
        db.clone(),
        batch_id.to_string(),
        execution,
        ai_config,
        provider_payload,
    );

    Ok(response_payload)
}
