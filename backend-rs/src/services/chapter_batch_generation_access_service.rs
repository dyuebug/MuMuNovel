use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::models::{chapter, project};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadAccessibleChapterForGenerationError {
    ChapterNotFound,
    ChapterNotFoundOrAccessDenied,
    Internal(String),
}

pub(crate) async fn load_accessible_chapter_for_generation(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, LoadAccessibleChapterForGenerationError> {
    let chapter_model = chapter::Entity::find_by_id(chapter_id)
        .one(db)
        .await
        .map_err(|error| LoadAccessibleChapterForGenerationError::Internal(error.to_string()))?
        .ok_or(LoadAccessibleChapterForGenerationError::ChapterNotFound)?;

    let has_access = project::Entity::find()
        .filter(project::Column::Id.eq(&chapter_model.project_id))
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map(|result| result.is_some())
        .map_err(|error| LoadAccessibleChapterForGenerationError::Internal(error.to_string()))?;
    if !has_access {
        return Err(LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied);
    }

    Ok(chapter_model)
}

pub(crate) async fn load_accessible_chapters_for_generation(
    db: &DatabaseConnection,
    chapter_ids: &[String],
    user_id: &str,
) -> Result<Vec<chapter::Model>, LoadAccessibleChapterForGenerationError> {
    let mut chapters = Vec::with_capacity(chapter_ids.len());
    for chapter_id in chapter_ids {
        chapters.push(load_accessible_chapter_for_generation(db, chapter_id, user_id).await?);
    }
    Ok(chapters)
}
