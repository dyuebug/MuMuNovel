use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};

use crate::models::{character, organization};
use crate::services::project_service::ProjectService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectQueryContextError {
    ProjectNotFound,
    Internal(String),
}

pub type LoadProjectConsistencyContextError = ProjectQueryContextError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConsistencyCounts {
    pub organization_character_total: usize,
    pub organization_total: usize,
}

pub async fn ensure_project_consistency_access(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<(), LoadProjectConsistencyContextError> {
    ProjectService::get(db, project_id, user_id)
        .await
        .map_err(ProjectQueryContextError::Internal)?
        .ok_or(ProjectQueryContextError::ProjectNotFound)?;

    Ok(())
}

pub async fn load_project_consistency_counts(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<ProjectConsistencyCounts, LoadProjectConsistencyContextError> {
    ensure_project_consistency_access(db, project_id, user_id).await?;

    let organization_character_total = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .filter(character::Column::IsOrganization.eq(true))
        .count(db)
        .await
        .map_err(|error| ProjectQueryContextError::Internal(error.to_string()))?
        as usize;

    let organization_total = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .count(db)
        .await
        .map_err(|error| ProjectQueryContextError::Internal(error.to_string()))?
        as usize;

    Ok(ProjectConsistencyCounts {
        organization_character_total,
        organization_total,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        LoadProjectConsistencyContextError, ProjectConsistencyCounts, ProjectQueryContextError,
    };

    #[test]
    fn project_consistency_types_have_stable_equality() {
        assert_eq!(
            LoadProjectConsistencyContextError::ProjectNotFound,
            ProjectQueryContextError::ProjectNotFound
        );
        assert_eq!(
            LoadProjectConsistencyContextError::Internal("boom".to_string()),
            ProjectQueryContextError::Internal("boom".to_string())
        );
        assert_eq!(
            ProjectConsistencyCounts {
                organization_character_total: 2,
                organization_total: 1,
            },
            ProjectConsistencyCounts {
                organization_character_total: 2,
                organization_total: 1,
            }
        );
    }
}
