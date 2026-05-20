use sea_orm::DatabaseConnection;

use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_required_owned_task, map_owned_batch_generation_task_error,
};
use crate::services::chapter_batch_generation_task_command_service::cancel_batch_generation_task;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CancelBatchGenerationWorkflowError {
    TaskNotFound,
    Domain(String),
    Internal(String),
}

pub(crate) async fn cancel_owned_batch_generation_task(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, CancelBatchGenerationWorkflowError> {
    let task = load_required_owned_task(db, batch_id, user_id)
        .await
        .map_err(|error| {
            map_owned_batch_generation_task_error(
                error,
                || CancelBatchGenerationWorkflowError::TaskNotFound,
                CancelBatchGenerationWorkflowError::Internal,
            )
        })?;

    let response_payload = cancel_batch_generation_task(db, task)
        .await
        .map_err(CancelBatchGenerationWorkflowError::Domain)?;

    Ok(response_payload)
}
