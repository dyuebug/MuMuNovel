use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::models::{chapter, project};
use crate::services::project_consistency_query_service::ProjectQueryContextError;
use crate::services::project_service::ProjectService;

#[derive(Debug, Clone)]
pub struct ProjectExportContext {
    pub project: project::Model,
    pub chapters: Vec<chapter::Model>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadProjectExportContextError {
    Context(ProjectQueryContextError),
    ProjectHasNoChapters,
}

pub async fn load_project_export_context(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<ProjectExportContext, LoadProjectExportContextError> {
    let project = ProjectService::get(db, project_id, user_id)
        .await
        .map_err(ProjectQueryContextError::Internal)
        .map_err(LoadProjectExportContextError::Context)?
        .ok_or(LoadProjectExportContextError::Context(
            ProjectQueryContextError::ProjectNotFound,
        ))?;

    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(db)
        .await
        .map_err(|error| {
            LoadProjectExportContextError::Context(ProjectQueryContextError::Internal(
                error.to_string(),
            ))
        })?;

    Ok(ProjectExportContext { project, chapters })
}

pub async fn load_project_export_context_with_non_empty_chapters(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<ProjectExportContext, LoadProjectExportContextError> {
    let context = load_project_export_context(db, project_id, user_id).await?;
    if context.chapters.is_empty() {
        return Err(LoadProjectExportContextError::ProjectHasNoChapters);
    }

    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::LoadProjectExportContextError;
    use crate::services::project_consistency_query_service::ProjectQueryContextError;

    #[test]
    fn project_export_context_error_equality_is_stable() {
        assert_eq!(
            LoadProjectExportContextError::Context(ProjectQueryContextError::ProjectNotFound),
            LoadProjectExportContextError::Context(ProjectQueryContextError::ProjectNotFound)
        );
        assert_eq!(
            LoadProjectExportContextError::ProjectHasNoChapters,
            LoadProjectExportContextError::ProjectHasNoChapters
        );
        assert_eq!(
            LoadProjectExportContextError::Context(ProjectQueryContextError::Internal(
                "boom".to_string(),
            )),
            LoadProjectExportContextError::Context(ProjectQueryContextError::Internal(
                "boom".to_string(),
            ))
        );
    }
}
