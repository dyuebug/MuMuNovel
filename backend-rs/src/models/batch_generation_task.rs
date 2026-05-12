use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "batch_generation_tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub user_id: String,
    pub start_chapter_number: i32,
    pub chapter_count: i32,
    pub chapter_ids: Value,
    pub style_id: Option<i32>,
    pub target_word_count: Option<i32>,
    pub enable_analysis: Option<bool>,
    pub status: Option<String>,
    pub total_chapters: Option<i32>,
    pub completed_chapters: Option<i32>,
    pub failed_chapters: Option<Value>,
    pub current_chapter_id: Option<String>,
    pub current_chapter_number: Option<i32>,
    pub current_retry_count: Option<i32>,
    pub max_retries: Option<i32>,
    pub created_at: Option<NaiveDateTime>,
    pub started_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
    pub error_message: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
