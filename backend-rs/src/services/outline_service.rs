use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::models::{outline, project};

pub struct OutlineService;

impl OutlineService {
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
        title: &str,
        content: Option<&str>,
        order_index: Option<i32>,
        structure: Option<&str>,
    ) -> Result<Option<outline::Model>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }

        let now = Utc::now().naive_utc();
        let model = outline::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            title: Set(title.to_string()),
            content: Set(content.map(|s| s.to_string())),
            structure: Set(structure.map(|s| s.to_string())),
            order_index: Set(order_index),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn list(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<outline::Model>>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        outline::Entity::find()
            .filter(outline::Column::ProjectId.eq(project_id))
            .order_by_asc(outline::Column::OrderIndex)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn get(
        db: &DatabaseConnection,
        outline_id: &str,
        user_id: &str,
    ) -> Result<Option<outline::Model>, String> {
        let o = outline::Entity::find_by_id(outline_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        match o {
            Some(ref outline) => {
                if !Self::verify_project_access(db, &outline.project_id, user_id).await? {
                    return Ok(None);
                }
                Ok(Some(outline.clone()))
            }
            None => Ok(None),
        }
    }

    pub async fn update(
        db: &DatabaseConnection,
        outline_id: &str,
        user_id: &str,
        title: Option<&str>,
        content: Option<&str>,
        order_index: Option<i32>,
        structure: Option<&str>,
    ) -> Result<Option<outline::Model>, String> {
        let existing = Self::get(db, outline_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };

        let mut active: outline::ActiveModel = model.into();
        if let Some(v) = title { active.title = Set(v.to_string()); }
        if let Some(v) = content { active.content = Set(Some(v.to_string())); }
        if let Some(v) = order_index { active.order_index = Set(Some(v)); }
        if let Some(v) = structure { active.structure = Set(Some(v.to_string())); }
        active.updated_at = Set(Some(Utc::now().naive_utc()));

        active.update(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn delete(
        db: &DatabaseConnection,
        outline_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, outline_id, user_id).await?;
        if existing.is_none() {
            return Ok(None);
        }
        outline::Entity::delete_by_id(outline_id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(Some(()))
    }
}
