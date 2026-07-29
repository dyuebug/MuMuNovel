use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "novel_autopilot_step_runs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub run_id: String,
    pub step_key: String,
    pub step_type: String,
    pub phase: String,
    pub chapter_id: Option<String>,
    pub chapter_number: Option<i32>,
    pub attempt: i32,
    pub run_epoch: i64,
    pub status: String,
    pub background_task_id: Option<String>,
    pub input_digest: String,
    pub result_digest: Option<String>,
    pub quality_decision: Option<String>,
    pub error_code: Option<String>,
    pub started_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
