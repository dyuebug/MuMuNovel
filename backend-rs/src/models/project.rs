use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    pub title: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub theme: Option<String>,
    #[sea_orm(nullable)]
    pub genre: Option<String>,
    pub target_words: i32,
    pub current_words: i32,
    pub status: String,
    pub wizard_status: String,
    pub wizard_step: i32,
    pub outline_mode: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub world_time_period: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub world_location: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub world_atmosphere: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub world_rules: Option<String>,
    #[sea_orm(nullable)]
    pub chapter_count: Option<i32>,
    #[sea_orm(nullable)]
    pub narrative_perspective: Option<String>,
    pub character_count: i32,
    #[sea_orm(nullable)]
    pub default_creative_mode: Option<String>,
    #[sea_orm(nullable)]
    pub default_story_focus: Option<String>,
    #[sea_orm(nullable)]
    pub default_plot_stage: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub default_story_creation_brief: Option<String>,
    #[sea_orm(nullable)]
    pub default_quality_preset: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub default_quality_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
