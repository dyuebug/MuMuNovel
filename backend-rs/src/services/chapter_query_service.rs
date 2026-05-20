use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::chapter;
use crate::services::chapter_service::ChapterService;

pub enum LoadNavigationPayloadError {
    NotFound,
    Internal(String),
}

pub enum LoadCanGeneratePayloadError {
    NotFound,
    Internal(String),
}

fn navigation_payload(
    previous: Option<chapter::Model>,
    current: Option<chapter::Model>,
    next: Option<chapter::Model>,
) -> Value {
    json!({
        "previous": previous,
        "current": current,
        "next": next,
    })
}

fn can_generate_payload(can_generate: bool) -> Value {
    json!({
        "can_generate": can_generate,
    })
}

pub async fn load_navigation_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadNavigationPayloadError> {
    match ChapterService::navigation(db, chapter_id, user_id).await {
        Ok(Some((previous, current, next))) => Ok(navigation_payload(previous, current, next)),
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
        Ok(Some(can_generate)) => Ok(can_generate_payload(can_generate)),
        Ok(None) => Err(LoadCanGeneratePayloadError::NotFound),
        Err(error) => Err(LoadCanGeneratePayloadError::Internal(error)),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::chapter;

    use super::{can_generate_payload, navigation_payload};

    fn chapter_model(id: &str, number: i32) -> chapter::Model {
        chapter::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_number: number,
            title: format!("第{}章", number),
            content: Some("正文".to_string()),
            summary: None,
            word_count: 2,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn should_build_navigation_payload() {
        let payload = navigation_payload(
            Some(chapter_model("chapter-1", 1)),
            Some(chapter_model("chapter-2", 2)),
            None,
        );

        assert_eq!(payload["previous"]["id"], "chapter-1");
        assert_eq!(payload["current"]["id"], "chapter-2");
        assert!(payload["next"].is_null());
    }

    #[test]
    fn should_build_can_generate_payload() {
        assert_eq!(can_generate_payload(true)["can_generate"], true);
        assert_eq!(can_generate_payload(false)["can_generate"], false);
    }
}
