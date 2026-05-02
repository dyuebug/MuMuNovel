use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::models::{character, project};

pub struct CharacterService;

impl CharacterService {
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
        is_organization: bool,
        role_type: Option<&str>,
        personality: Option<&str>,
        background: Option<&str>,
        appearance: Option<&str>,
        age: Option<&str>,
        gender: Option<&str>,
    ) -> Result<Option<character::Model>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }

        let now = Utc::now();
        let model = character::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            name: Set(name.to_string()),
            is_organization: Set(is_organization),
            role_type: Set(role_type.map(|s| s.to_string())),
            personality: Set(personality.map(|s| s.to_string())),
            background: Set(background.map(|s| s.to_string())),
            appearance: Set(appearance.map(|s| s.to_string())),
            age: Set(age.map(|s| s.to_string())),
            gender: Set(gender.map(|s| s.to_string())),
            relationships: Set(None),
            organization_type: Set(None),
            organization_purpose: Set(None),
            organization_members: Set(None),
            status: Set("active".to_string()),
            status_changed_chapter: Set(None),
            current_state: Set(None),
            state_updated_chapter: Set(None),
            main_career_id: Set(None),
            main_career_stage: Set(None),
            sub_careers: Set(None),
            avatar_url: Set(None),
            traits: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn list(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<character::Model>>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        character::Entity::find()
            .filter(character::Column::ProjectId.eq(project_id))
            .order_by_asc(character::Column::Name)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn get(
        db: &DatabaseConnection,
        character_id: &str,
        user_id: &str,
    ) -> Result<Option<character::Model>, String> {
        let c = character::Entity::find_by_id(character_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        match c {
            Some(ref ch) => {
                if !Self::verify_project_access(db, &ch.project_id, user_id).await? {
                    return Ok(None);
                }
                Ok(Some(ch.clone()))
            }
            None => Ok(None),
        }
    }

    pub async fn update(
        db: &DatabaseConnection,
        character_id: &str,
        user_id: &str,
        name: Option<&str>,
        role_type: Option<&str>,
        personality: Option<&str>,
        background: Option<&str>,
        appearance: Option<&str>,
        age: Option<&str>,
        gender: Option<&str>,
        status: Option<&str>,
        is_organization: Option<bool>,
    ) -> Result<Option<character::Model>, String> {
        let existing = Self::get(db, character_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };

        let mut active: character::ActiveModel = model.into();
        if let Some(v) = name { active.name = Set(v.to_string()); }
        if let Some(v) = role_type { active.role_type = Set(Some(v.to_string())); }
        if let Some(v) = personality { active.personality = Set(Some(v.to_string())); }
        if let Some(v) = background { active.background = Set(Some(v.to_string())); }
        if let Some(v) = appearance { active.appearance = Set(Some(v.to_string())); }
        if let Some(v) = age { active.age = Set(Some(v.to_string())); }
        if let Some(v) = gender { active.gender = Set(Some(v.to_string())); }
        if let Some(v) = status { active.status = Set(v.to_string()); }
        if let Some(v) = is_organization { active.is_organization = Set(v); }
        active.updated_at = Set(Some(Utc::now()));

        active.update(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn delete(
        db: &DatabaseConnection,
        character_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, character_id, user_id).await?;
        if existing.is_none() {
            return Ok(None);
        }
        character::Entity::delete_by_id(character_id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(Some(()))
    }
}
