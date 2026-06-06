use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::Serialize;
use uuid::Uuid;

use crate::models::{
    analysis_task, batch_generation_snapshot, batch_generation_task, career, chapter,
    chapter_draft_attempt, character, character_career, foreshadow, generation_history,
    organization, organization_member, outline, plot_analysis, project, project_default_style,
    regeneration_task, relationship, story_memory, writing_style,
};

pub struct ProjectService;

#[derive(Debug, Clone, Serialize, Default)]
pub struct WizardCleanupDeletedCounts {
    pub characters: u64,
    pub outlines: u64,
    pub chapters: u64,
}

#[allow(clippy::too_many_arguments)]
pub struct CreateProjectParams {
    pub user_id: String,
    pub title: String,
    pub description: Option<String>,
    pub theme: Option<String>,
    pub genre: Option<String>,
    pub world_time_period: Option<String>,
    pub world_location: Option<String>,
    pub world_atmosphere: Option<String>,
    pub world_rules: Option<String>,
    pub narrative_perspective: Option<String>,
    pub target_words: i32,
    pub chapter_count: Option<i32>,
    pub character_count: i32,
    pub outline_mode: String,
    pub default_creative_mode: Option<String>,
    pub default_story_focus: Option<String>,
    pub default_plot_stage: Option<String>,
    pub default_story_creation_brief: Option<String>,
    pub default_quality_preset: Option<String>,
    pub default_quality_notes: Option<String>,
}

impl Default for CreateProjectParams {
    fn default() -> Self {
        Self {
            user_id: String::new(),
            title: String::new(),
            description: None,
            theme: None,
            genre: None,
            world_time_period: None,
            world_location: None,
            world_atmosphere: None,
            world_rules: None,
            narrative_perspective: None,
            target_words: 0,
            chapter_count: None,
            character_count: 5,
            outline_mode: "one-to-many".into(),
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
        }
    }
}

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
        default_creative_mode: Option<&str>,
        default_story_focus: Option<&str>,
        default_plot_stage: Option<&str>,
        default_story_creation_brief: Option<&str>,
        default_quality_preset: Option<&str>,
        default_quality_notes: Option<&str>,
    ) -> Result<project::Model, String> {
        let now = Utc::now().naive_utc();
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
            default_creative_mode: Set(default_creative_mode.map(str::to_string)),
            default_story_focus: Set(default_story_focus.map(str::to_string)),
            default_plot_stage: Set(default_plot_stage.map(str::to_string)),
            default_story_creation_brief: Set(default_story_creation_brief.map(str::to_string)),
            default_quality_preset: Set(default_quality_preset.map(str::to_string)),
            default_quality_notes: Set(default_quality_notes.map(str::to_string)),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e))
    }

    /// Full project creation with all wizard fields — used by wizard generator
    pub async fn create_full(
        db: &DatabaseConnection,
        params: CreateProjectParams,
    ) -> Result<project::Model, String> {
        let now = Utc::now().naive_utc();
        let model = project::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(params.user_id),
            title: Set(params.title),
            description: Set(params.description),
            theme: Set(params.theme),
            genre: Set(params.genre),
            target_words: Set(params.target_words),
            current_words: Set(0),
            status: Set("planning".to_string()),
            wizard_status: Set("incomplete".to_string()),
            wizard_step: Set(1),
            outline_mode: Set(params.outline_mode),
            world_time_period: Set(params.world_time_period),
            world_location: Set(params.world_location),
            world_atmosphere: Set(params.world_atmosphere),
            world_rules: Set(params.world_rules),
            chapter_count: Set(params.chapter_count),
            narrative_perspective: Set(params.narrative_perspective),
            character_count: Set(params.character_count),
            default_creative_mode: Set(params.default_creative_mode),
            default_story_focus: Set(params.default_story_focus),
            default_plot_stage: Set(params.default_plot_stage),
            default_story_creation_brief: Set(params.default_story_creation_brief),
            default_quality_preset: Set(params.default_quality_preset),
            default_quality_notes: Set(params.default_quality_notes),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e))
    }

    /// Auto-assign first global writing style to a project
    pub async fn assign_default_style(
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<(), String> {
        let style = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.is_null())
            .filter(writing_style::Column::OrderIndex.eq(1))
            .limit(1)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        if let Some(style) = style {
            let active = project_default_style::ActiveModel {
                project_id: Set(project_id.to_string()),
                style_id: Set(style.id),
                ..Default::default()
            };
            active.insert(db).await.map_err(|e| format!("{}", e))?;
        }
        Ok(())
    }

    /// Finalize project after wizard outline completes
    pub async fn complete_wizard(
        db: &DatabaseConnection,
        project_id: &str,
        chapter_count: i32,
        narrative_perspective: Option<&str>,
        target_words: i32,
    ) -> Result<(), String> {
        let model = project::Entity::find_by_id(project_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?
            .ok_or("项目不存在")?;
        let mut active: project::ActiveModel = model.into();
        active.chapter_count = Set(Some(chapter_count));
        if let Some(np) = narrative_perspective {
            active.narrative_perspective = Set(Some(np.to_string()));
        }
        active.target_words = Set(target_words);
        active.status = Set("writing".to_string());
        active.wizard_status = Set("completed".to_string());
        active.wizard_step = Set(4);
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(())
    }

    /// Update project wizard step after each wizard phase completes
    pub async fn update_wizard_step(
        db: &DatabaseConnection,
        project_id: &str,
        step: i32,
    ) -> Result<(), String> {
        let model = project::Entity::find_by_id(project_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?
            .ok_or("项目不存在")?;
        let mut active: project::ActiveModel = model.into();
        active.wizard_step = Set(step);
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(())
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
        world_time_period: Option<&str>,
        world_location: Option<&str>,
        world_atmosphere: Option<&str>,
        world_rules: Option<&str>,
        chapter_count: Option<i32>,
        narrative_perspective: Option<&str>,
        character_count: Option<i32>,
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
        if let Some(v) = title {
            active.title = Set(v.to_string());
        }
        if let Some(v) = description {
            active.description = Set(Some(v.to_string()));
        }
        if let Some(v) = theme {
            active.theme = Set(Some(v.to_string()));
        }
        if let Some(v) = genre {
            active.genre = Set(Some(v.to_string()));
        }
        if let Some(v) = status {
            active.status = Set(v.to_string());
        }
        if let Some(v) = target_words {
            active.target_words = Set(v);
        }
        if let Some(v) = world_time_period {
            active.world_time_period = Set(Some(v.to_string()));
        }
        if let Some(v) = world_location {
            active.world_location = Set(Some(v.to_string()));
        }
        if let Some(v) = world_atmosphere {
            active.world_atmosphere = Set(Some(v.to_string()));
        }
        if let Some(v) = world_rules {
            active.world_rules = Set(Some(v.to_string()));
        }
        if let Some(v) = chapter_count {
            active.chapter_count = Set(Some(v));
        }
        if let Some(v) = narrative_perspective {
            active.narrative_perspective = Set(Some(v.to_string()));
        }
        if let Some(v) = character_count {
            active.character_count = Set(v);
        }
        if let Some(v) = default_creative_mode {
            active.default_creative_mode = Set(Some(v.to_string()));
        }
        if let Some(v) = default_story_focus {
            active.default_story_focus = Set(Some(v.to_string()));
        }
        if let Some(v) = default_plot_stage {
            active.default_plot_stage = Set(Some(v.to_string()));
        }
        if let Some(v) = default_story_creation_brief {
            active.default_story_creation_brief = Set(Some(v.to_string()));
        }
        if let Some(v) = default_quality_preset {
            active.default_quality_preset = Set(Some(v.to_string()));
        }
        if let Some(v) = default_quality_notes {
            active.default_quality_notes = Set(Some(v.to_string()));
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

    pub async fn cleanup_wizard_data(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<WizardCleanupDeletedCounts>, String> {
        if Self::get(db, project_id, user_id).await?.is_none() {
            return Ok(None);
        }

        let character_ids = character::Entity::find()
            .filter(character::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(|e| format!("{}", e))?
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();

        let organization_ids = organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(|e| format!("{}", e))?
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();

        let batch_task_ids = batch_generation_task::Entity::find()
            .filter(batch_generation_task::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(|e| format!("{}", e))?
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();

        let txn = db.begin().await.map_err(|e| format!("{}", e))?;

        if !batch_task_ids.is_empty() {
            batch_generation_snapshot::Entity::delete_many()
                .filter(batch_generation_snapshot::Column::BatchTaskId.is_in(batch_task_ids))
                .exec(&txn)
                .await
                .map_err(|e| format!("{}", e))?;
        }

        chapter_draft_attempt::Entity::delete_many()
            .filter(chapter_draft_attempt::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        regeneration_task::Entity::delete_many()
            .filter(regeneration_task::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        plot_analysis::Entity::delete_many()
            .filter(plot_analysis::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        generation_history::Entity::delete_many()
            .filter(generation_history::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        analysis_task::Entity::delete_many()
            .filter(analysis_task::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        batch_generation_task::Entity::delete_many()
            .filter(batch_generation_task::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        story_memory::Entity::delete_many()
            .filter(story_memory::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        relationship::Entity::delete_many()
            .filter(relationship::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        foreshadow::Entity::delete_many()
            .filter(foreshadow::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        if !organization_ids.is_empty() {
            organization_member::Entity::delete_many()
                .filter(organization_member::Column::OrganizationId.is_in(organization_ids))
                .exec(&txn)
                .await
                .map_err(|e| format!("{}", e))?;
        }

        organization::Entity::delete_many()
            .filter(organization::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        if !character_ids.is_empty() {
            character_career::Entity::delete_many()
                .filter(character_career::Column::CharacterId.is_in(character_ids))
                .exec(&txn)
                .await
                .map_err(|e| format!("{}", e))?;
        }

        career::Entity::delete_many()
            .filter(career::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        let deleted_chapters = chapter::Entity::delete_many()
            .filter(chapter::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        let deleted_outlines = outline::Entity::delete_many()
            .filter(outline::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        let deleted_characters = character::Entity::delete_many()
            .filter(character::Column::ProjectId.eq(project_id))
            .exec(&txn)
            .await
            .map_err(|e| format!("{}", e))?;

        let project_model = project::Entity::find_by_id(project_id)
            .one(&txn)
            .await
            .map_err(|e| format!("{}", e))?
            .ok_or_else(|| "项目不存在".to_string())?;

        let mut active: project::ActiveModel = project_model.into();
        active.status = Set("planning".to_string());
        active.wizard_status = Set("incomplete".to_string());
        active.wizard_step = Set(0);
        active.world_time_period = Set(None);
        active.world_location = Set(None);
        active.world_atmosphere = Set(None);
        active.world_rules = Set(None);
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(&txn).await.map_err(|e| format!("{}", e))?;

        txn.commit().await.map_err(|e| format!("{}", e))?;

        Ok(Some(WizardCleanupDeletedCounts {
            characters: deleted_characters.rows_affected,
            outlines: deleted_outlines.rows_affected,
            chapters: deleted_chapters.rows_affected,
        }))
    }
}
