use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::models::{organization, project};

pub struct OrganizationService;

impl OrganizationService {
    async fn verify_project_access(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<bool, String> {
        let exists = project::Entity::find()
            .filter(project::Column::Id.eq(project_id))
            .filter(project::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(exists.is_some())
    }

    pub async fn create(
        db: &DatabaseConnection,
        project_id: &str,
        character_id: &str,
        user_id: &str,
        parent_org_id: Option<&str>,
        level: Option<i32>,
        power_level: Option<i32>,
        location: Option<&str>,
        motto: Option<&str>,
        color: Option<&str>,
    ) -> Result<Option<organization::Model>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        let now = Utc::now().naive_utc();
        let model = organization::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            character_id: Set(character_id.to_string()),
            project_id: Set(project_id.to_string()),
            parent_org_id: Set(parent_org_id.map(|s| s.to_string())),
            level: Set(level.unwrap_or(0)),
            power_level: Set(power_level.unwrap_or(50)),
            member_count: Set(0),
            location: Set(location.map(|s| s.to_string())),
            motto: Set(motto.map(|s| s.to_string())),
            color: Set(color.map(|s| s.to_string())),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn list(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<organization::Model>>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(project_id))
            .order_by_asc(organization::Column::CharacterId)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn get(
        db: &DatabaseConnection,
        org_id: &str,
        user_id: &str,
    ) -> Result<Option<organization::Model>, String> {
        let o = organization::Entity::find_by_id(org_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        match o {
            Some(ref org) => {
                if !Self::verify_project_access(db, &org.project_id, user_id).await? {
                    return Ok(None);
                }
                Ok(Some(org.clone()))
            }
            None => Ok(None),
        }
    }

    pub async fn update(
        db: &DatabaseConnection,
        org_id: &str,
        user_id: &str,
        parent_org_id: Option<&str>,
        level: Option<i32>,
        power_level: Option<i32>,
        location: Option<&str>,
        motto: Option<&str>,
        color: Option<&str>,
    ) -> Result<Option<organization::Model>, String> {
        let existing = Self::get(db, org_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };
        let mut active: organization::ActiveModel = model.into();
        if let Some(v) = parent_org_id { active.parent_org_id = Set(Some(v.to_string())); }
        if let Some(v) = level { active.level = Set(v); }
        if let Some(v) = power_level { active.power_level = Set(v); }
        if let Some(v) = location { active.location = Set(Some(v.to_string())); }
        if let Some(v) = motto { active.motto = Set(Some(v.to_string())); }
        if let Some(v) = color { active.color = Set(Some(v.to_string())); }
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn delete(
        db: &DatabaseConnection,
        org_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, org_id, user_id).await?;
        if existing.is_none() {
            return Ok(None);
        }
        organization::Entity::delete_by_id(org_id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(Some(()))
    }
}
