use sea_orm::DatabaseConnection;

use crate::models::chapter;

use super::chapter_service::ChapterService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadAccessibleChapterError {
    NotFoundOrAccessDenied,
    Internal(String),
}

pub async fn load_accessible_chapter(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, LoadAccessibleChapterError> {
    match ChapterService::get(db, chapter_id, user_id).await {
        Ok(Some(chapter)) => Ok(chapter),
        Ok(None) => Err(LoadAccessibleChapterError::NotFoundOrAccessDenied),
        Err(error) => Err(LoadAccessibleChapterError::Internal(error)),
    }
}
