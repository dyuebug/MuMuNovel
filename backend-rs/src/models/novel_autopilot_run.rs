use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "novel_autopilot_runs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub user_id: String,
    pub schema_version: String,
    pub status: String,
    pub current_phase: String,
    pub current_step: Option<String>,
    pub active_scope_key: Option<String>,
    pub current_chapter_id: Option<String>,
    pub current_chapter_number: Option<i32>,
    pub total_chapters: i32,
    pub completed_chapters: i32,
    pub failed_chapters: Value,
    pub pending_rewrites: Value,
    pub total_word_count: i64,
    pub execution_scope: String,
    pub human_gate_mode: String,
    pub gate_interval: Option<i32>,
    pub config_snapshot: Value,
    pub max_chapters: Option<i32>,
    pub max_tokens: Option<i64>,
    pub max_estimated_cost: Option<f64>,
    pub max_runtime_seconds: Option<i64>,
    pub used_tokens: i64,
    pub estimated_cost: f64,
    pub epoch: i64,
    pub version: i64,
    pub consecutive_provider_failures: i32,
    pub consecutive_quality_failures: i32,
    pub last_error_code: Option<String>,
    pub next_attempt_at: Option<NaiveDateTime>,
    pub guidance_digest: Option<String>,
    pub active_background_task_id: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub final_export_ref: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub started_at: Option<NaiveDateTime>,
    pub paused_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
