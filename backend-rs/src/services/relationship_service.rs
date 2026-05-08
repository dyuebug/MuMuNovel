use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::models::{project, relationship};

pub struct RelationshipService;

impl RelationshipService {
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
        user_id: &str,
        character_from_id: &str,
        character_to_id: &str,
        relationship_type_id: Option<i32>,
        relationship_name: Option<&str>,
        intimacy_level: Option<i32>,
        description: Option<&str>,
    ) -> Result<Option<relationship::Model>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        let now = Utc::now().naive_utc();
        let model = relationship::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            character_from_id: Set(character_from_id.to_string()),
            character_to_id: Set(character_to_id.to_string()),
            relationship_type_id: Set(relationship_type_id),
            relationship_name: Set(relationship_name.map(|s| s.to_string())),
            intimacy_level: Set(intimacy_level.unwrap_or(50)),
            status: Set("active".to_string()),
            description: Set(description.map(|s| s.to_string())),
            started_at: Set(None),
            ended_at: Set(None),
            source: Set("manual".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model
            .insert(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn list(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<relationship::Model>>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        relationship::Entity::find()
            .filter(relationship::Column::ProjectId.eq(project_id))
            .order_by_asc(relationship::Column::CharacterFromId)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn get(
        db: &DatabaseConnection,
        rel_id: &str,
        user_id: &str,
    ) -> Result<Option<relationship::Model>, String> {
        let r = relationship::Entity::find_by_id(rel_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        match r {
            Some(ref rel) => {
                if !Self::verify_project_access(db, &rel.project_id, user_id).await? {
                    return Ok(None);
                }
                Ok(Some(rel.clone()))
            }
            None => Ok(None),
        }
    }

    pub async fn update(
        db: &DatabaseConnection,
        rel_id: &str,
        user_id: &str,
        relationship_name: Option<&str>,
        intimacy_level: Option<i32>,
        status: Option<&str>,
        description: Option<&str>,
    ) -> Result<Option<relationship::Model>, String> {
        let existing = Self::get(db, rel_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };
        let mut active: relationship::ActiveModel = model.into();
        if let Some(v) = relationship_name {
            active.relationship_name = Set(Some(v.to_string()));
        }
        if let Some(v) = intimacy_level {
            active.intimacy_level = Set(v);
        }
        if let Some(v) = status {
            active.status = Set(v.to_string());
        }
        if let Some(v) = description {
            active.description = Set(Some(v.to_string()));
        }
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active
            .update(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn delete(
        db: &DatabaseConnection,
        rel_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, rel_id, user_id).await?;
        if existing.is_none() {
            return Ok(None);
        }
        relationship::Entity::delete_by_id(rel_id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(Some(()))
    }
}
