use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::ai::AIConfig;
use crate::models::{chapter, project};
use crate::services::chapter_generation_prompt_context_provider_service::{
    resolve_default_prompt_context_provider_payload, PromptContextProviderPayload,
};

use super::settings_service::SettingsService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadAccessibleChapterForGenerationError {
    ChapterNotFound,
    ChapterNotFoundOrAccessDenied,
    Internal(String),
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedGenerationExecutionConfig {
    pub(crate) ai_config: AIConfig,
    pub(crate) provider_payload: PromptContextProviderPayload,
}

async fn build_user_ai_config(
    db: &DatabaseConnection,
    user_id: &str,
    model_override: Option<&str>,
) -> Result<AIConfig, String> {
    SettingsService::build_ai_config(db, user_id, None, model_override, None).await
}

pub(crate) async fn prepare_generation_execution_config(
    db: &DatabaseConnection,
    user_id: &str,
    model_override: Option<&str>,
) -> Result<PreparedGenerationExecutionConfig, String> {
    let ai_config = build_user_ai_config(db, user_id, model_override).await?;

    Ok(PreparedGenerationExecutionConfig {
        ai_config,
        provider_payload: resolve_default_prompt_context_provider_payload(),
    })
}

pub(crate) async fn verify_project_access(
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

    let has_access = verify_project_access(db, &chapter_model.project_id, user_id)
        .await
        .map_err(LoadAccessibleChapterForGenerationError::Internal)?;
    if !has_access {
        return Err(LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied);
    }

    Ok(chapter_model)
}
