use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "organizations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub character_id: String,
    pub project_id: String,
    #[sea_orm(nullable)]
    pub parent_org_id: Option<String>,
    pub level: i32,
    pub power_level: i32,
    pub member_count: i32,
    #[sea_orm(column_type = "Text", nullable)]
    pub location: Option<String>,
    #[sea_orm(nullable)]
    pub motto: Option<String>,
    #[sea_orm(nullable)]
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
