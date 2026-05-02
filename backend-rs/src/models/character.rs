use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "characters")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub name: String,
    #[sea_orm(nullable)]
    pub age: Option<String>,
    #[sea_orm(nullable)]
    pub gender: Option<String>,
    pub is_organization: bool,
    #[sea_orm(nullable)]
    pub role_type: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub personality: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub background: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub appearance: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub relationships: Option<String>,
    #[sea_orm(nullable)]
    pub organization_type: Option<String>,
    #[sea_orm(nullable)]
    pub organization_purpose: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub organization_members: Option<String>,
    pub status: String,
    #[sea_orm(nullable)]
    pub status_changed_chapter: Option<i32>,
    #[sea_orm(column_type = "Text", nullable)]
    pub current_state: Option<String>,
    #[sea_orm(nullable)]
    pub state_updated_chapter: Option<i32>,
    #[sea_orm(nullable)]
    pub main_career_id: Option<String>,
    #[sea_orm(nullable)]
    pub main_career_stage: Option<i32>,
    #[sea_orm(column_type = "Text", nullable)]
    pub sub_careers: Option<String>,
    #[sea_orm(nullable)]
    pub avatar_url: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub traits: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
