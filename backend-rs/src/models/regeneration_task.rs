use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "regeneration_tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub chapter_id: String,
    pub analysis_id: Option<String>,
    pub user_id: String,
    pub project_id: String,
    #[sea_orm(column_type = "Text")]
    pub modification_instructions: String,
    pub original_suggestions: Option<Value>,
    pub selected_suggestion_indices: Option<Value>,
    #[sea_orm(column_type = "Text", nullable)]
    pub custom_instructions: Option<String>,
    pub style_id: Option<i32>,
    pub target_word_count: Option<i32>,
    pub focus_areas: Option<Value>,
    pub preserve_elements: Option<Value>,
    pub status: Option<String>,
    pub progress: Option<i32>,
    #[sea_orm(column_type = "Text", nullable)]
    pub error_message: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub original_content: Option<String>,
    pub original_word_count: Option<i32>,
    #[sea_orm(column_type = "Text", nullable)]
    pub regenerated_content: Option<String>,
    pub regenerated_word_count: Option<i32>,
    pub version_number: Option<i32>,
    pub version_note: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub started_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
