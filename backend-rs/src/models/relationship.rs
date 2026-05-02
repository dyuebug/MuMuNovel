use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "character_relationships")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub character_from_id: String,
    pub character_to_id: String,
    #[sea_orm(nullable)]
    pub relationship_type_id: Option<i32>,
    #[sea_orm(nullable)]
    pub relationship_name: Option<String>,
    pub intimacy_level: i32,
    pub status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    #[sea_orm(nullable)]
    pub started_at: Option<String>,
    #[sea_orm(nullable)]
    pub ended_at: Option<String>,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
