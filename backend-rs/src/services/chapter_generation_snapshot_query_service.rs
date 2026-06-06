use std::collections::HashMap;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::models::batch_generation_snapshot;

pub(crate) async fn load_chapter_generation_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<Option<batch_generation_snapshot::Model>, String> {
    batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.eq(task_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn load_chapter_generation_snapshot_map(
    db: &DatabaseConnection,
    task_ids: &[String],
) -> Result<HashMap<String, batch_generation_snapshot::Model>, String> {
    if task_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let snapshots = batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.is_in(task_ids.iter().cloned()))
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    Ok(snapshots
        .into_iter()
        .map(|snapshot| (snapshot.batch_task_id.clone(), snapshot))
        .collect())
}
