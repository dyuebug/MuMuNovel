use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "prompt_submissions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub submitter_id: String,
    pub submitter_name: Option<String>,
    pub source_instance: String,
    pub name: String,
    #[sea_orm(column_type = "Text")]
    pub description: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub prompt_content: String,
    pub category: String,
    #[sea_orm(column_type = "Text")]
    pub tags: Option<String>,
    pub author_display_name: Option<String>,
    pub is_anonymous: bool,
    pub status: String,
    pub reviewer_id: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub review_note: Option<String>,
    pub reviewed_at: Option<NaiveDateTime>,
    pub workshop_item_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
