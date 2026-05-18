use axum::{http::StatusCode, Json};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::chapter;

use super::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};

pub type ChapterHttpResult<T> = Result<T, (StatusCode, Json<Value>)>;

pub async fn load_accessible_chapter_or_404(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> ChapterHttpResult<chapter::Model> {
    match load_accessible_chapter(db, chapter_id, user_id).await {
        Ok(chapter) => Ok(chapter),
        Err(LoadAccessibleChapterError::NotFoundOrAccessDenied) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found or access denied"})),
        )),
        Err(LoadAccessibleChapterError::Internal(error)) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        )),
    }
}
