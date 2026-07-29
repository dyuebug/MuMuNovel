use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "autopilot_invocation_audits")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub actor_user_id: String,
    pub schema_version: String,
    pub tool_name: String,
    pub tool_schema_version: String,
    pub confirmed_by_user: bool,
    pub execution_mode: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub provider_name: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub model_name: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub prompt_digest: Option<String>,
    pub input_digest: String,
    #[sea_orm(column_type = "Text")]
    pub input_summary: String,
    pub status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub result_summary: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub error_code: Option<String>,
    pub created_at: NaiveDateTime,
    pub started_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
