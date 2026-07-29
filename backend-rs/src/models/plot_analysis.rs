use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "plot_analysis")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub chapter_id: String,
    pub source_content_digest: Option<String>,
    pub plot_stage: Option<String>,
    pub conflict_level: Option<i32>,
    pub conflict_types: Option<serde_json::Value>,
    pub emotional_tone: Option<String>,
    pub emotional_intensity: Option<f64>,
    pub emotional_curve: Option<serde_json::Value>,
    pub hooks: Option<serde_json::Value>,
    pub hooks_count: i32,
    pub hooks_avg_strength: Option<f64>,
    pub foreshadows: Option<serde_json::Value>,
    pub foreshadows_planted: i32,
    pub foreshadows_resolved: i32,
    pub plot_points: Option<serde_json::Value>,
    pub plot_points_count: i32,
    pub character_states: Option<serde_json::Value>,
    pub scenes: Option<serde_json::Value>,
    pub pacing: Option<String>,
    pub overall_quality_score: Option<f64>,
    pub pacing_score: Option<f64>,
    pub engagement_score: Option<f64>,
    pub coherence_score: Option<f64>,
    #[sea_orm(column_type = "Text", nullable)]
    pub analysis_report: Option<String>,
    pub suggestions: Option<serde_json::Value>,
    pub word_count: Option<i32>,
    pub dialogue_ratio: Option<f64>,
    pub description_ratio: Option<f64>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
