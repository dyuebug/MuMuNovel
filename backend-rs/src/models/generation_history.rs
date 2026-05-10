use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "generation_history")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub chapter_id: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub prompt: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub generated_content: Option<String>,
    pub model: Option<String>,
    pub tokens_used: Option<i32>,
    pub generation_time: Option<f64>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}