use sea_orm::DatabaseConnection;
use serde_json::Value;

use super::chapter_batch_generation_status_view_service::{
    build_active_batch_generation_task_list_query_response,
    load_active_user_batch_generation_task_view_contexts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadActiveBatchGenerationTaskListQueryError {
    Internal(String),
}

fn normalize_active_batch_generation_task_list_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(20).clamp(1, 100)
}

pub(crate) async fn load_owned_active_batch_generation_task_list_query(
    db: &DatabaseConnection,
    user_id: &str,
    limit: Option<u64>,
) -> Result<Value, LoadActiveBatchGenerationTaskListQueryError> {
    let tasks = load_active_user_batch_generation_task_view_contexts(
        db,
        user_id,
        normalize_active_batch_generation_task_list_limit(limit),
    )
    .await
    .map_err(LoadActiveBatchGenerationTaskListQueryError::Internal)?;

    Ok(build_active_batch_generation_task_list_query_response(tasks))
}

#[cfg(test)]
mod tests {
    use super::normalize_active_batch_generation_task_list_limit;

    #[test]
    fn should_normalize_active_batch_generation_task_list_limit() {
        assert_eq!(normalize_active_batch_generation_task_list_limit(None), 20);
        assert_eq!(
            normalize_active_batch_generation_task_list_limit(Some(0)),
            1
        );
        assert_eq!(
            normalize_active_batch_generation_task_list_limit(Some(25)),
            25
        );
        assert_eq!(
            normalize_active_batch_generation_task_list_limit(Some(500)),
            100
        );
    }
}
