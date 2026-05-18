use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::chapter_service::ChapterService;

pub enum LoadAnnotationsPayloadError {
    NotFound,
    Internal(String),
}

pub async fn load_annotations_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadAnnotationsPayloadError> {
    match ChapterService::get(db, chapter_id, user_id).await {
        Ok(Some(_)) => Ok(json!({
            "chapter_id": chapter_id,
            "annotations": [],
            "memory_mapping": [],
        })),
        Ok(None) => Err(LoadAnnotationsPayloadError::NotFound),
        Err(error) => Err(LoadAnnotationsPayloadError::Internal(error)),
    }
}
