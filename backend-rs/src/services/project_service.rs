use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::models::project;

pub struct ProjectService;

impl ProjectService {
    pub async fn create(
        db: &DatabaseConnection,
        user_id: &str,
        title: &str,
        description: Option<&str>,
        theme: Option<&str>,
        genre: Option<&str>,
        outline_mode: Option<&str>,
        target_words: Option<i32>,
    ) -> Result<project::Model, String> {
        let now = Utc::now();
        let model = project::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_string()),
            title: Set(title.to_string()),
            description: Set(description.map(|s| s.to_string())),
            theme: Set(theme.map(|s| s.to_string())),
            genre: Set(genre.map(|s| s.to_string())),
            target_words: Set(target_words.unwrap_or(0)),
            current_words: Set(0),
            status: Set("planning".to_string()),
            wizard_status: Set("incomplete".to_string()),
            wizard_step: Set(0),
            outline_mode: Set(outline_mode.unwrap_or("one-to-many").to_string()),
            world_time_period: Set(None),
            world_location: Set(None),
            world_atmosphere: Set(None),
            world_rules: Set(None),
            chapter_count: Set(None),
            narrative_perspective: Set(None),
            character_count: Set(5),
            default_creative_mode: Set(None),
            default_story_focus: Set(None),
            default_plot_stage: Set(None),
            default_story_creation_brief: Set(None),
            default_quality_preset: Set(None),
            default_quality_notes: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e))
    }

    pub async fn list(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<Vec<project::Model>, String> {
        project::Entity::find()
            .filter(project::Column::UserId.eq(user_id))
            .order_by_desc(project::Column::UpdatedAt)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))
    }

    pub async fn get(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<project::Model>, String> {
        project::Entity::find()
            .filter(project::Column::Id.eq(project_id))
            .filter(project::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))
    }

    pub async fn update(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        theme: Option<&str>,
        genre: Option<&str>,
        status: Option<&str>,
        target_words: Option<i32>,
        outline_mode: Option<&str>,
        narrative_perspective: Option<&str>,
        default_creative_mode: Option<&str>,
        default_story_focus: Option<&str>,
        default_plot_stage: Option<&str>,
        default_story_creation_brief: Option<&str>,
        default_quality_preset: Option<&str>,
        default_quality_notes: Option<&str>,
    ) -> Result<Option<project::Model>, String> {
        let existing = Self::get(db, project_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };

        let mut active: project::ActiveModel = model.into();
        if let Some(v) = title { active.title = Set(v.to_string()); }
        if let Some(v) = description { active.description = Set(Some(v.to_string())); }
        if let Some(v) = theme { active.theme = Set(Some(v.to_string())); }
        if let Some(v) = genre { active.genre = Set(Some(v.to_string())); }
        if let Some(v) = status { active.status = Set(v.to_string()); }
        if let Some(v) = target_words { active.target_words = Set(v); }
        if let Some(v) = outline_mode { active.outline_mode = Set(v.to_string()); }
        if let Some(v) = narrative_perspective { active.narrative_perspective = Set(Some(v.to_string())); }
        if let Some(v) = default_creative_mode { active.default_creative_mode = Set(Some(v.to_string())); }
        if let Some(v) = default_story_focus { active.default_story_focus = Set(Some(v.to_string())); }
        if let Some(v) = default_plot_stage { active.default_plot_stage = Set(Some(v.to_string())); }
        if let Some(v) = default_story_creation_brief { active.default_story_creation_brief = Set(Some(v.to_string())); }
        if let Some(v) = default_quality_preset { active.default_quality_preset = Set(Some(v.to_string())); }
        if let Some(v) = default_quality_notes { active.default_quality_notes = Set(Some(v.to_string())); }
        active.updated_at = Set(Some(Utc::now()));

        active.update(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn delete(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, project_id, user_id).await?;
        if existing.is_none() {
            return Ok(None);
        }
        project::Entity::delete_by_id(project_id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(Some(()))
    }
}
