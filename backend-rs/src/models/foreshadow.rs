use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "foreshadows")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub title: String,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    #[sea_orm(column_type = "Text")]
    pub hint_text: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub resolution_text: Option<String>,
    pub source_type: String,
    pub source_memory_id: Option<String>,
    pub source_analysis_id: Option<String>,
    pub plant_chapter_id: Option<String>,
    pub plant_chapter_number: Option<i32>,
    pub target_resolve_chapter_id: Option<String>,
    pub target_resolve_chapter_number: Option<i32>,
    pub actual_resolve_chapter_id: Option<String>,
    pub actual_resolve_chapter_number: Option<i32>,
    pub status: String,
    pub is_long_term: bool,
    pub importance: f64,
    pub strength: i32,
    pub subtlety: i32,
    pub urgency: i32,
    pub related_characters: Option<serde_json::Value>,
    pub related_foreshadow_ids: Option<serde_json::Value>,
    pub tags: Option<serde_json::Value>,
    pub category: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub notes: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub resolution_notes: Option<String>,
    pub auto_remind: bool,
    pub remind_before_chapters: i32,
    pub include_in_context: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub planted_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
