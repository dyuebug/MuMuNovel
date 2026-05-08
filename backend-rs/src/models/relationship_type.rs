use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "relationship_types")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub category: String,
    #[sea_orm(nullable)]
    pub reverse_name: Option<String>,
    #[sea_orm(nullable)]
    pub intimacy_range: Option<String>,
    #[sea_orm(nullable)]
    pub icon: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
