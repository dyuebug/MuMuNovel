use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "chapter_draft_attempts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub chapter_id: Option<String>,
    pub batch_task_id: Option<String>,
    pub source: String,
    pub attempt_state: String,
    pub quality_gate_action: Option<String>,
    pub quality_gate_decision: Option<String>,
    pub word_count: i32,
    #[sea_orm(column_type = "Text", nullable)]
    pub summary_preview: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub content_preview: Option<String>,
    pub quality_metrics: Option<serde_json::Value>,
    pub repair_payload: Option<serde_json::Value>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}