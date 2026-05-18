use sea_orm::DatabaseConnection;

use crate::services::chapter_batch_generation_owned_task_query_service::load_owned_task;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchGenerationStatusStreamAccessError {
    TaskNotFound,
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchGenerationStatusStreamAccessGate;

pub async fn ensure_batch_generation_status_stream_access(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<BatchGenerationStatusStreamAccessGate, BatchGenerationStatusStreamAccessError> {
    load_owned_task(db, batch_id, user_id)
        .await
        .map_err(BatchGenerationStatusStreamAccessError::Internal)?
        .ok_or(BatchGenerationStatusStreamAccessError::TaskNotFound)?;

    Ok(BatchGenerationStatusStreamAccessGate)
}
