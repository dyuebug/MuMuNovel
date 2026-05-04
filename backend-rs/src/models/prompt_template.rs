use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "prompt_templates")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    pub template_key: String,
    pub template_name: String,
    #[sea_orm(column_type = "Text")]
    pub template_content: String,
    #[sea_orm(column_type = "Text")]
    pub description: Option<String>,
    pub category: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub parameters: Option<String>,
    pub is_active: bool,
    pub is_system_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
