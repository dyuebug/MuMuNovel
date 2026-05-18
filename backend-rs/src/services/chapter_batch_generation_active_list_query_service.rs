use serde_json::Value;
use sea_orm::DatabaseConnection;

use super::chapter_batch_generation_status_payload_adapter_service::build_active_batch_generation_task_list_response;
use super::chapter_batch_generation_status_view_service::load_active_user_batch_generation_task_view_contexts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadActiveBatchGenerationTaskListQueryError {
    Internal(String),
}

pub struct ActiveBatchGenerationTaskListQueryResult {
    pub response_payload: Value,
}

pub async fn load_active_batch_generation_task_list_query(
    db: &DatabaseConnection,
    user_id: &str,
    limit: u64,
) -> Result<
    ActiveBatchGenerationTaskListQueryResult,
    LoadActiveBatchGenerationTaskListQueryError,
> {
    let tasks = load_active_user_batch_generation_task_view_contexts(db, user_id, limit)
        .await
        .map_err(LoadActiveBatchGenerationTaskListQueryError::Internal)?;

    Ok(ActiveBatchGenerationTaskListQueryResult {
        response_payload: build_active_batch_generation_task_list_response(tasks),
    })
}
