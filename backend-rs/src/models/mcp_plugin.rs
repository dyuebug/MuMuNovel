use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "mcp_plugins")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    pub plugin_name: String,
    pub display_name: String,
    #[sea_orm(column_type = "Text")]
    pub description: Option<String>,
    pub plugin_type: String,
    pub server_url: Option<String>,
    pub command: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub args: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub env: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub headers: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub config: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub tools: Option<String>,
    pub enabled: bool,
    pub status: String,
    #[sea_orm(column_type = "Text")]
    pub last_error: Option<String>,
    pub last_test_at: Option<NaiveDateTime>,
    pub category: String,
    pub sort_order: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
