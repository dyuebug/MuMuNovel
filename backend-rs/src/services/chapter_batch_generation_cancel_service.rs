use serde_json::Value;
use sea_orm::DatabaseConnection;

use crate::services::chapter_batch_generation_owned_task_query_service::load_owned_task;
use crate::services::chapter_batch_generation_task_command_service::{
    cancel_batch_generation_task, CancelBatchGenerationResult,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelBatchGenerationWorkflowError {
    TaskNotFound,
    Domain(String),
    Internal(String),
}

#[derive(Debug)]
pub struct CancelBatchGenerationWorkflowResult {
    pub response_payload: Value,
}

pub async fn cancel_owned_batch_generation_task(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<CancelBatchGenerationWorkflowResult, CancelBatchGenerationWorkflowError> {
    let task = load_owned_task(db, batch_id, user_id)
        .await
        .map_err(CancelBatchGenerationWorkflowError::Internal)?
        .ok_or(CancelBatchGenerationWorkflowError::TaskNotFound)?;

    let CancelBatchGenerationResult { response_payload } =
        cancel_batch_generation_task(db, task)
            .await
            .map_err(CancelBatchGenerationWorkflowError::Domain)?;

    Ok(CancelBatchGenerationWorkflowResult { response_payload })
}
