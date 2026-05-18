use serde_json::{json, Value};
use sea_orm::DatabaseConnection;

use super::chapter_batch_generation_access_service::verify_project_access;
use super::chapter_batch_generation_status_payload_adapter_service::build_active_batch_generation_response;
use super::chapter_batch_generation_status_view_service::{
    load_active_project_batch_generation_task_view_context, BatchGenerationTaskViewContext,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadActiveBatchGenerationQueryError {
    ProjectNotFoundOrAccessDenied,
    Internal(String),
}

pub struct ActiveBatchGenerationQueryResult {
    pub response_payload: Value,
}

pub async fn load_active_batch_generation_query(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<ActiveBatchGenerationQueryResult, LoadActiveBatchGenerationQueryError> {
    let has_access = verify_project_access(db, project_id, user_id)
        .await
        .map_err(LoadActiveBatchGenerationQueryError::Internal)?;
    if !has_access {
        return Err(
            LoadActiveBatchGenerationQueryError::ProjectNotFoundOrAccessDenied,
        );
    }

    let task = load_active_project_batch_generation_task_view_context(
        db, project_id, user_id,
    )
    .await
    .map_err(LoadActiveBatchGenerationQueryError::Internal)?;

    Ok(ActiveBatchGenerationQueryResult {
        response_payload: build_active_batch_generation_query_response(task),
    })
}

pub fn build_active_batch_generation_query_response(
    task: Option<BatchGenerationTaskViewContext>,
) -> Value {
    match task {
        Some(task) => build_active_batch_generation_response(task),
        None => json!({
            "has_active_task": false,
            "task": null,
        }),
    }
}
