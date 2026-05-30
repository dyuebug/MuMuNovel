use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{character, organization, organization_member};
use crate::services::project_consistency_query_service::{
    ensure_project_consistency_access, load_project_consistency_counts,
    LoadProjectConsistencyContextError,
};

#[derive(Debug)]
pub(crate) enum ProjectConsistencyWriteWorkflowError {
    Context(LoadProjectConsistencyContextError),
    Internal(String),
}

pub(crate) fn normalize_project_consistency_auto_fix(raw: Option<&str>) -> bool {
    raw.map(|value| value != "false" && value != "0")
        .unwrap_or(true)
}

pub(crate) async fn fix_project_organizations_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, ProjectConsistencyWriteWorkflowError> {
    ensure_project_consistency_access(db, project_id, user_id)
        .await
        .map_err(ProjectConsistencyWriteWorkflowError::Context)?;

    let (fixed, total) = fix_missing_organization_records(db, project_id)
        .await
        .map_err(ProjectConsistencyWriteWorkflowError::Internal)?;

    Ok(json!({
        "message": "组织记录修复完成",
        "fixed": fixed,
        "total": total,
    }))
}

pub(crate) async fn fix_project_member_counts_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, ProjectConsistencyWriteWorkflowError> {
    ensure_project_consistency_access(db, project_id, user_id)
        .await
        .map_err(ProjectConsistencyWriteWorkflowError::Context)?;

    let (fixed, total) = fix_organization_member_counts(db, project_id)
        .await
        .map_err(ProjectConsistencyWriteWorkflowError::Internal)?;

    Ok(json!({
        "message": "成员计数修复完成",
        "fixed": fixed,
        "total": total,
    }))
}

pub(crate) async fn check_project_consistency_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    auto_fix: bool,
) -> Result<Value, ProjectConsistencyWriteWorkflowError> {
    let counts_without_fix = if auto_fix {
        None
    } else {
        Some(
            load_project_consistency_counts(db, project_id, user_id)
                .await
                .map_err(ProjectConsistencyWriteWorkflowError::Context)?,
        )
    };

    let (org_fixed, org_total) = if auto_fix {
        ensure_project_consistency_access(db, project_id, user_id)
            .await
            .map_err(ProjectConsistencyWriteWorkflowError::Context)?;
        fix_missing_organization_records(db, project_id)
            .await
            .map_err(ProjectConsistencyWriteWorkflowError::Internal)?
    } else {
        let counts = counts_without_fix
            .as_ref()
            .expect("counts_without_fix should exist when auto_fix is false");
        (0, counts.organization_character_total)
    };

    let (member_fixed, member_total) = if auto_fix {
        fix_organization_member_counts(db, project_id)
            .await
            .map_err(ProjectConsistencyWriteWorkflowError::Internal)?
    } else {
        let counts = counts_without_fix
            .as_ref()
            .expect("counts_without_fix should exist when auto_fix is false");
        (0, counts.organization_total)
    };

    Ok(json!({
        "project_id": project_id,
        "checks": {
            "organization_records": {
                "checked": org_total,
                "fixed": org_fixed,
                "status": if org_fixed == 0 { "ok" } else { "fixed" },
            },
            "member_counts": {
                "checked": member_total,
                "fixed": member_fixed,
                "status": if member_fixed == 0 { "ok" } else { "fixed" },
            },
        },
    }))
}

async fn fix_missing_organization_records(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<(usize, usize), String> {
    let org_characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .filter(character::Column::IsOrganization.eq(true))
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut fixed = 0usize;
    for character_model in &org_characters {
        let existing = organization::Entity::find()
            .filter(organization::Column::CharacterId.eq(&character_model.id))
            .one(db)
            .await
            .map_err(|error| error.to_string())?;
        if existing.is_some() {
            continue;
        }

        let now = Utc::now().naive_utc();
        organization::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            character_id: Set(character_model.id.clone()),
            project_id: Set(project_id.to_string()),
            parent_org_id: Set(None),
            level: Set(1),
            power_level: Set(50),
            member_count: Set(0),
            location: Set(None),
            motto: Set(None),
            color: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .map_err(|error| error.to_string())?;
        fixed += 1;
    }

    Ok((fixed, org_characters.len()))
}

async fn fix_organization_member_counts(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<(usize, usize), String> {
    let organizations = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut fixed = 0usize;
    for org in &organizations {
        let actual_count = organization_member::Entity::find()
            .filter(organization_member::Column::OrganizationId.eq(&org.id))
            .filter(organization_member::Column::Status.eq("active"))
            .count(db)
            .await
            .map_err(|error| error.to_string())? as i32;

        if org.member_count == actual_count {
            continue;
        }

        let mut active: organization::ActiveModel = org.clone().into();
        active.member_count = Set(actual_count);
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await.map_err(|error| error.to_string())?;
        fixed += 1;
    }

    Ok((fixed, organizations.len()))
}

#[cfg(test)]
mod tests {
    use super::{normalize_project_consistency_auto_fix, ProjectConsistencyWriteWorkflowError};
    use crate::services::project_consistency_query_service::LoadProjectConsistencyContextError;

    #[test]
    fn normalize_project_consistency_auto_fix_keeps_existing_query_semantics() {
        assert!(normalize_project_consistency_auto_fix(None));
        assert!(normalize_project_consistency_auto_fix(Some("true")));
        assert!(normalize_project_consistency_auto_fix(Some("1")));
        assert!(!normalize_project_consistency_auto_fix(Some("false")));
        assert!(!normalize_project_consistency_auto_fix(Some("0")));
    }

    #[test]
    fn project_consistency_write_workflow_error_shapes_stay_stable() {
        let context = LoadProjectConsistencyContextError::ProjectNotFound;
        let org_error = ProjectConsistencyWriteWorkflowError::Context(context.clone());
        let member_error = ProjectConsistencyWriteWorkflowError::Context(context.clone());
        let check_error = ProjectConsistencyWriteWorkflowError::Context(context);
        let internal = ProjectConsistencyWriteWorkflowError::Internal("db exploded".to_string());

        assert!(matches!(
            org_error,
            ProjectConsistencyWriteWorkflowError::Context(
                LoadProjectConsistencyContextError::ProjectNotFound
            )
        ));
        assert!(matches!(
            member_error,
            ProjectConsistencyWriteWorkflowError::Context(
                LoadProjectConsistencyContextError::ProjectNotFound
            )
        ));
        assert!(matches!(
            check_error,
            ProjectConsistencyWriteWorkflowError::Context(
                LoadProjectConsistencyContextError::ProjectNotFound
            )
        ));
        assert!(matches!(
            internal,
            ProjectConsistencyWriteWorkflowError::Internal(detail) if detail == "db exploded"
        ));
    }
}
