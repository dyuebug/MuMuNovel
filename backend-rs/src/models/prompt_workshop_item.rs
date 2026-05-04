use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "prompt_workshop_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    #[sea_orm(column_type = "Text")]
    pub description: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub prompt_content: String,
    pub category: String,
    #[sea_orm(column_type = "Text")]
    pub tags: Option<String>,
    pub author_id: Option<String>,
    pub author_name: Option<String>,
    pub source_instance: Option<String>,
    pub is_official: bool,
    pub download_count: i32,
    pub like_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
