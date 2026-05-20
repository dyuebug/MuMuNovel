use sea_orm::{DatabaseConnection, EntityTrait};

use crate::models::batch_generation_task;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadOwnedBatchGenerationTaskError {
    TaskNotFound,
    Internal(String),
}

pub(crate) async fn load_owned_task(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Option<batch_generation_task::Model>, String> {
    let task = batch_generation_task::Entity::find_by_id(batch_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?;
    Ok(task.filter(|task| task.user_id == user_id))
}

pub(crate) async fn load_required_owned_task(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<batch_generation_task::Model, LoadOwnedBatchGenerationTaskError> {
    load_owned_task(db, batch_id, user_id)
        .await
        .map_err(LoadOwnedBatchGenerationTaskError::Internal)?
        .ok_or(LoadOwnedBatchGenerationTaskError::TaskNotFound)
}

pub(crate) fn map_owned_batch_generation_task_error<T, TNotFound, TInternal>(
    error: LoadOwnedBatchGenerationTaskError,
    map_not_found: TNotFound,
    map_internal: TInternal,
) -> T
where
    TNotFound: FnOnce() -> T,
    TInternal: FnOnce(String) -> T,
{
    match error {
        LoadOwnedBatchGenerationTaskError::TaskNotFound => map_not_found(),
        LoadOwnedBatchGenerationTaskError::Internal(error) => map_internal(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{map_owned_batch_generation_task_error, LoadOwnedBatchGenerationTaskError};

    #[test]
    fn should_map_owned_batch_generation_task_not_found_error() {
        let error = map_owned_batch_generation_task_error(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
            || "not-found".to_string(),
            |error| format!("internal:{error}"),
        );

        assert_eq!(error, "not-found");
    }

    #[test]
    fn should_map_owned_batch_generation_task_internal_error() {
        let error = map_owned_batch_generation_task_error(
            LoadOwnedBatchGenerationTaskError::Internal("boom".to_string()),
            || "not-found".to_string(),
            |error| format!("internal:{error}"),
        );

        assert_eq!(error, "internal:boom");
    }
}
