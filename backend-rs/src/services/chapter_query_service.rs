use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::chapter_service::ChapterService;

pub enum LoadNavigationPayloadError {
    NotFound,
    Internal(String),
}

pub enum LoadCanGeneratePayloadError {
    NotFound,
    Internal(String),
}

pub async fn load_navigation_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadNavigationPayloadError> {
    match ChapterService::navigation(db, chapter_id, user_id).await {
        Ok(Some((previous, current, next))) => Ok(json!({
            "previous": previous,
            "current": current,
            "next": next,
        })),
        Ok(None) => Err(LoadNavigationPayloadError::NotFound),
        Err(error) => Err(LoadNavigationPayloadError::Internal(error)),
    }
}

pub async fn load_can_generate_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadCanGeneratePayloadError> {
    match ChapterService::can_generate(db, chapter_id, user_id).await {
        Ok(Some(can_generate)) => Ok(json!({
            "can_generate": can_generate,
        })),
        Ok(None) => Err(LoadCanGeneratePayloadError::NotFound),
        Err(error) => Err(LoadCanGeneratePayloadError::Internal(error)),
    }
}
