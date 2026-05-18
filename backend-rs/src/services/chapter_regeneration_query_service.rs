use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use crate::models::regeneration_task;

pub fn datetime_to_string(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

pub async fn load_regeneration_tasks_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    limit: u64,
) -> Result<Value, String> {
    let tasks = regeneration_task::Entity::find()
        .filter(regeneration_task::Column::ChapterId.eq(chapter_id.to_string()))
        .order_by_desc(regeneration_task::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let task_items: Vec<Value> = tasks
        .iter()
        .map(|task| {
            json!({
                "task_id": task.id,
                "status": task.status,
                "version_number": task.version_number,
                "version_note": task.version_note,
                "original_word_count": task.original_word_count,
                "regenerated_word_count": task.regenerated_word_count,
                "created_at": datetime_to_string(task.created_at),
                "completed_at": datetime_to_string(task.completed_at),
            })
        })
        .collect();

    Ok(json!({
        "chapter_id": chapter_id,
        "total": task_items.len(),
        "tasks": task_items,
    }))
}
