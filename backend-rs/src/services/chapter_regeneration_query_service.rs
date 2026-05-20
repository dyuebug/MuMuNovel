use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use crate::models::regeneration_task;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};

pub enum LoadRegenerationTasksPayloadError {
    NotFoundOrAccessDenied,
    Internal(String),
}

pub fn datetime_to_string(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

pub fn normalize_regeneration_tasks_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(10).clamp(1, 50)
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

pub async fn load_owned_regeneration_tasks_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    limit: Option<u64>,
) -> Result<Value, LoadRegenerationTasksPayloadError> {
    load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(|error| match error {
            LoadAccessibleChapterError::NotFoundOrAccessDenied => {
                LoadRegenerationTasksPayloadError::NotFoundOrAccessDenied
            }
            LoadAccessibleChapterError::Internal(detail) => {
                LoadRegenerationTasksPayloadError::Internal(detail)
            }
        })?;

    load_regeneration_tasks_payload(db, chapter_id, normalize_regeneration_tasks_limit(limit))
        .await
        .map_err(LoadRegenerationTasksPayloadError::Internal)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use super::{datetime_to_string, normalize_regeneration_tasks_limit};

    #[test]
    fn should_normalize_regeneration_tasks_limit() {
        assert_eq!(normalize_regeneration_tasks_limit(None), 10);
        assert_eq!(normalize_regeneration_tasks_limit(Some(0)), 1);
        assert_eq!(normalize_regeneration_tasks_limit(Some(25)), 25);
        assert_eq!(normalize_regeneration_tasks_limit(Some(99)), 50);
    }

    #[test]
    fn should_format_regeneration_task_datetime() {
        let datetime = NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse");

        assert_eq!(
            datetime_to_string(Some(datetime)),
            Some("2026-05-17T12:30:45".to_string())
        );
        assert_eq!(datetime_to_string(None), None);
    }
}
