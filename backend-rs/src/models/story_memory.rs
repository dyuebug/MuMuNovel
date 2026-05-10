use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "story_memories")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub chapter_id: Option<String>,
    pub memory_type: String,
    pub title: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub full_context: Option<String>,
    pub related_characters: Option<serde_json::Value>,
    pub related_locations: Option<serde_json::Value>,
    pub tags: Option<serde_json::Value>,
    pub importance_score: Option<f64>,
    pub story_timeline: i32,
    pub chapter_position: i32,
    pub text_length: i32,
    pub is_foreshadow: i32,
    pub foreshadow_resolved_at: Option<String>,
    pub foreshadow_strength: Option<f64>,
    pub vector_id: Option<String>,
    pub embedding_model: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}