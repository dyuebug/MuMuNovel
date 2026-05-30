use sea_orm::DatabaseConnection;

use crate::services::project_service::ProjectService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectAccessQueryError {
    NotFoundOrAccessDenied,
    Internal(String),
}

pub async fn ensure_owned_project_access(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<(), ProjectAccessQueryError> {
    ProjectService::get(db, project_id, user_id)
        .await
        .map_err(ProjectAccessQueryError::Internal)?
        .ok_or(ProjectAccessQueryError::NotFoundOrAccessDenied)
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::ProjectAccessQueryError;

    #[test]
    fn project_access_query_error_equality_is_stable() {
        assert_eq!(
            ProjectAccessQueryError::NotFoundOrAccessDenied,
            ProjectAccessQueryError::NotFoundOrAccessDenied
        );
        assert_eq!(
            ProjectAccessQueryError::Internal("boom".to_string()),
            ProjectAccessQueryError::Internal("boom".to_string())
        );
    }
}
