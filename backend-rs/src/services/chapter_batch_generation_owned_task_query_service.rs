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
