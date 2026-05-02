use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::models::{career, project};

pub struct CareerService;

impl CareerService {
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
        name: &str,
        career_type: &str,
        stages: &str,
        description: Option<&str>,
        category: Option<&str>,
        max_stage: Option<i32>,
    ) -> Result<Option<career::Model>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        let now = Utc::now();
        let model = career::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            name: Set(name.to_string()),
            career_type: Set(career_type.to_string()),
            stages: Set(stages.to_string()),
            description: Set(description.map(|s| s.to_string())),
            category: Set(category.map(|s| s.to_string())),
            max_stage: Set(max_stage.unwrap_or(10)),
            requirements: Set(None),
            special_abilities: Set(None),
            worldview_rules: Set(None),
            attribute_bonuses: Set(None),
            source: Set("manual".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn list(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<career::Model>>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        career::Entity::find()
            .filter(career::Column::ProjectId.eq(project_id))
            .order_by_asc(career::Column::Name)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn get(
        db: &DatabaseConnection,
        career_id: &str,
        user_id: &str,
    ) -> Result<Option<career::Model>, String> {
        let c = career::Entity::find_by_id(career_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        match c {
            Some(ref career) => {
                if !Self::verify_project_access(db, &career.project_id, user_id).await? {
                    return Ok(None);
                }
                Ok(Some(career.clone()))
            }
            None => Ok(None),
        }
    }

    pub async fn update(
        db: &DatabaseConnection,
        career_id: &str,
        user_id: &str,
        name: Option<&str>,
        description: Option<&str>,
        stages: Option<&str>,
        max_stage: Option<i32>,
        category: Option<&str>,
    ) -> Result<Option<career::Model>, String> {
        let existing = Self::get(db, career_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };
        let mut active: career::ActiveModel = model.into();
        if let Some(v) = name { active.name = Set(v.to_string()); }
        if let Some(v) = description { active.description = Set(Some(v.to_string())); }
        if let Some(v) = stages { active.stages = Set(v.to_string()); }
        if let Some(v) = max_stage { active.max_stage = Set(v); }
        if let Some(v) = category { active.category = Set(Some(v.to_string())); }
        active.updated_at = Set(Some(Utc::now()));
        active.update(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn delete(
        db: &DatabaseConnection,
        career_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, career_id, user_id).await?;
        if existing.is_none() {
            return Ok(None);
        }
        career::Entity::delete_by_id(career_id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(Some(()))
    }
}
