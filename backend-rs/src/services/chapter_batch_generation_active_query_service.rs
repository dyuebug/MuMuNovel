use sea_orm::DatabaseConnection;
use serde_json::Value;

use super::chapter_batch_generation_access_service::verify_project_access;
use super::chapter_batch_generation_status_view_service::{
    build_active_batch_generation_query_response,
    load_active_project_batch_generation_task_view_context,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadActiveBatchGenerationQueryError {
    ProjectNotFoundOrAccessDenied,
    Internal(String),
}

pub(crate) async fn load_active_batch_generation_query(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, LoadActiveBatchGenerationQueryError> {
    let has_access = verify_project_access(db, project_id, user_id)
        .await
        .map_err(LoadActiveBatchGenerationQueryError::Internal)?;
    if !has_access {
        return Err(LoadActiveBatchGenerationQueryError::ProjectNotFoundOrAccessDenied);
    }

    let task = load_active_project_batch_generation_task_view_context(db, project_id, user_id)
        .await
        .map_err(LoadActiveBatchGenerationQueryError::Internal)?;

    Ok(build_active_batch_generation_query_response(task))
}

#[cfg(test)]
mod tests {}
