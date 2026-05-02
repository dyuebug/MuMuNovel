use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "settings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub user_id: String,
    pub api_provider: String,
    pub api_key: String,
    pub api_base_url: String,
    #[sea_orm(column_type = "Text")]
    pub api_backup_urls: Option<String>,
    pub provider_type: String,
    pub fallback_strategy: String,
    pub azure_api_version: Option<String>,
    pub llm_model: String,
    pub temperature: f64,
    pub max_tokens: i32,
    #[sea_orm(column_type = "Text")]
    pub system_prompt: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub preferences: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
