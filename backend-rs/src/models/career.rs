use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "careers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub name: String,
    #[sea_orm(column_name = "type")]
    pub career_type: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    #[sea_orm(nullable)]
    pub category: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub stages: String,
    pub max_stage: i32,
    #[sea_orm(column_type = "Text", nullable)]
    pub requirements: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub special_abilities: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub worldview_rules: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub attribute_bonuses: Option<String>,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
