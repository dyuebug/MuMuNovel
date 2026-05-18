use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::ai::AIConfig;
use crate::models::{chapter, project};

use super::settings_service::SettingsService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadAccessibleChapterForGenerationError {
    ChapterNotFound,
    ChapterNotFoundOrAccessDenied,
    Internal(String),
}

pub async fn build_user_ai_config(
    db: &DatabaseConnection,
    user_id: &str,
    model_override: Option<&str>,
) -> Result<AIConfig, String> {
    SettingsService::build_ai_config(db, user_id, None, model_override, None).await
}

pub async fn verify_project_access(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<bool, String> {
    project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map(|result| result.is_some())
        .map_err(|error| error.to_string())
}

pub async fn load_accessible_chapter_for_generation(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, LoadAccessibleChapterForGenerationError> {
    let chapter_model = chapter::Entity::find_by_id(chapter_id)
        .one(db)
        .await
        .map_err(|error| {
            LoadAccessibleChapterForGenerationError::Internal(error.to_string())
        })?
        .ok_or(LoadAccessibleChapterForGenerationError::ChapterNotFound)?;

    let has_access = verify_project_access(db, &chapter_model.project_id, user_id)
        .await
        .map_err(LoadAccessibleChapterForGenerationError::Internal)?;
    if !has_access {
        return Err(
            LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
        );
    }

    Ok(chapter_model)
}
