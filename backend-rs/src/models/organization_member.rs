use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "organization_members")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub organization_id: String,
    pub character_id: String,
    pub position: String,
    pub rank: i32,
    pub status: String,
    #[sea_orm(nullable)]
    pub joined_at: Option<String>,
    #[sea_orm(nullable)]
    pub left_at: Option<String>,
    pub loyalty: i32,
    pub contribution: i32,
    pub source: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub notes: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
