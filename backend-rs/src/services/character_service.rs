use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::models::{character, organization, project, relationship as charrel};

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

        let now = Utc::now().naive_utc();
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
        active.updated_at = Set(Some(Utc::now().naive_utc()));

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

    /// Create a character with full wizard fields
    #[allow(clippy::too_many_arguments)]
    pub async fn create_full(
        db: &DatabaseConnection,
        project_id: &str,
        name: &str,
        is_organization: bool,
        role_type: Option<&str>,
        personality: Option<&str>,
        background: Option<&str>,
        appearance: Option<&str>,
        age: Option<&str>,
        gender: Option<&str>,
        traits: Option<&str>,
        organization_type: Option<&str>,
        organization_purpose: Option<&str>,
        relationships_text: Option<&str>,
    ) -> Result<character::Model, String> {
        let now = Utc::now().naive_utc();
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
            relationships: Set(relationships_text.map(|s| s.to_string())),
            organization_type: Set(organization_type.map(|s| s.to_string())),
            organization_purpose: Set(organization_purpose.map(|s| s.to_string())),
            organization_members: Set(None),
            status: Set("active".to_string()),
            status_changed_chapter: Set(None),
            current_state: Set(None),
            state_updated_chapter: Set(None),
            main_career_id: Set(None),
            main_career_stage: Set(None),
            sub_careers: Set(None),
            avatar_url: Set(None),
            traits: Set(traits.map(|s| s.to_string())),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e))
    }

    /// Update character's career assignment (embedded fields)
    pub async fn assign_career(
        db: &DatabaseConnection,
        character_id: &str,
        main_career_id: Option<&str>,
        main_stage: Option<i32>,
        sub_careers_json: Option<&str>,
    ) -> Result<(), String> {
        let model = character::Entity::find_by_id(character_id)
            .one(db).await.map_err(|e| format!("{}", e))?
            .ok_or("角色不存在")?;
        let mut active: character::ActiveModel = model.into();
        active.main_career_id = Set(main_career_id.map(|s| s.to_string()));
        active.main_career_stage = Set(main_stage);
        active.sub_careers = Set(sub_careers_json.map(|s| s.to_string()));
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(())
    }

    /// Create an organization record linked to a character
    pub async fn create_organization(
        db: &DatabaseConnection,
        character_id: &str,
        project_id: &str,
        power_level: i32,
        location: Option<&str>,
        motto: Option<&str>,
        color: Option<&str>,
    ) -> Result<organization::Model, String> {
        let now = Utc::now().naive_utc();
        let model = organization::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            character_id: Set(character_id.to_string()),
            project_id: Set(project_id.to_string()),
            parent_org_id: Set(None),
            level: Set(0),
            power_level: Set(power_level),
            member_count: Set(0),
            location: Set(location.map(|s| s.to_string())),
            motto: Set(motto.map(|s| s.to_string())),
            color: Set(color.map(|s| s.to_string())),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e))
    }

    /// Create a relationship between two characters
    #[allow(clippy::too_many_arguments)]
    pub async fn create_relationship(
        db: &DatabaseConnection,
        project_id: &str,
        from_id: &str,
        to_id: &str,
        relationship_type_id: Option<i32>,
        relationship_name: Option<&str>,
        intimacy_level: i32,
        description: Option<&str>,
        started_at: Option<&str>,
    ) -> Result<charrel::Model, String> {
        let now = Utc::now().naive_utc();
        let model = charrel::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            character_from_id: Set(from_id.to_string()),
            character_to_id: Set(to_id.to_string()),
            relationship_type_id: Set(relationship_type_id),
            relationship_name: Set(relationship_name.map(|s| s.to_string())),
            intimacy_level: Set(intimacy_level),
            status: Set("active".to_string()),
            description: Set(description.map(|s| s.to_string())),
            started_at: Set(started_at.map(|s| s.to_string())),
            ended_at: Set(None),
            source: Set("ai".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e))
    }
}
