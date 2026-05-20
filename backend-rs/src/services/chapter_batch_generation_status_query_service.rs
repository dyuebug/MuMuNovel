use sea_orm::DatabaseConnection;
use serde_json::Value;

use super::chapter_batch_generation_status_view_service::{
    build_batch_generation_status_query_response, load_required_batch_generation_task_view_context,
    LoadBatchGenerationTaskViewContextError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadBatchGenerationStatusQueryError {
    TaskNotFound,
    Internal(String),
}

fn map_load_task_view_context_error(
    error: LoadBatchGenerationTaskViewContextError,
) -> LoadBatchGenerationStatusQueryError {
    match error {
        LoadBatchGenerationTaskViewContextError::TaskNotFound => {
            LoadBatchGenerationStatusQueryError::TaskNotFound
        }
        LoadBatchGenerationTaskViewContextError::Internal(error) => {
            LoadBatchGenerationStatusQueryError::Internal(error)
        }
    }
}

pub(crate) async fn load_batch_generation_status_query(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, LoadBatchGenerationStatusQueryError> {
    let context = load_required_batch_generation_task_view_context(db, batch_id, user_id)
        .await
        .map_err(map_load_task_view_context_error)?;

    Ok(build_batch_generation_status_query_response(context))
}

#[cfg(test)]
mod tests {
    use crate::services::chapter_batch_generation_status_view_service::LoadBatchGenerationTaskViewContextError;

    use super::{
        map_load_task_view_context_error, LoadBatchGenerationStatusQueryError,
    };

    #[test]
    fn should_map_task_view_context_not_found_error() {
        let error =
            map_load_task_view_context_error(LoadBatchGenerationTaskViewContextError::TaskNotFound);

        assert_eq!(error, LoadBatchGenerationStatusQueryError::TaskNotFound);
    }

    #[test]
    fn should_map_task_view_context_internal_error() {
        let error = map_load_task_view_context_error(
            LoadBatchGenerationTaskViewContextError::Internal("boom".to_string()),
        );

        assert_eq!(
            error,
            LoadBatchGenerationStatusQueryError::Internal("boom".to_string())
        );
    }
}
