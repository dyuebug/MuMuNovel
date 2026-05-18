use serde_json::Value;
use sea_orm::DatabaseConnection;

use super::chapter_batch_generation_status_payload_adapter_service::build_task_status_response;
use super::chapter_batch_generation_status_view_service::load_batch_generation_task_view_context;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadBatchGenerationStatusQueryError {
    TaskNotFound,
    Internal(String),
}

pub struct BatchGenerationStatusQueryResult {
    pub response_payload: Value,
}

pub async fn load_batch_generation_status_query(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<BatchGenerationStatusQueryResult, LoadBatchGenerationStatusQueryError> {
    let context = load_batch_generation_task_view_context(db, batch_id, user_id)
        .await
        .map_err(LoadBatchGenerationStatusQueryError::Internal)?;
    let Some(context) = context else {
        return Err(LoadBatchGenerationStatusQueryError::TaskNotFound);
    };

    Ok(BatchGenerationStatusQueryResult {
        response_payload: build_task_status_response(context),
    })
}
