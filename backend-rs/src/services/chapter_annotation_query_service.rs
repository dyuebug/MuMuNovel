use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::chapter_service::ChapterService;

pub enum LoadAnnotationsPayloadError {
    NotFound,
    Internal(String),
}

fn annotations_payload(chapter_id: &str) -> Value {
    json!({
        "chapter_id": chapter_id,
        "annotations": [],
        "memory_mapping": [],
    })
}

pub async fn load_annotations_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadAnnotationsPayloadError> {
    match ChapterService::get(db, chapter_id, user_id).await {
        Ok(Some(_)) => Ok(annotations_payload(chapter_id)),
        Ok(None) => Err(LoadAnnotationsPayloadError::NotFound),
        Err(error) => Err(LoadAnnotationsPayloadError::Internal(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::annotations_payload;

    #[test]
    fn should_build_empty_annotations_payload() {
        let payload = annotations_payload("chapter-1");

        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["annotations"].as_array().map(Vec::len), Some(0));
        assert_eq!(payload["memory_mapping"].as_array().map(Vec::len), Some(0));
    }
}
