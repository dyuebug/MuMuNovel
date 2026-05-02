use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::models::{chapter, project};

pub struct ChapterService;

impl ChapterService {
    async fn verify_project_access(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<bool, String> {
        let exists = project::Entity::find()
            .filter(project::Column::Id.eq(project_id))
            .filter(project::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(exists.is_some())
    }

    pub async fn create(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        title: &str,
        chapter_number: i32,
        content: Option<&str>,
        summary: Option<&str>,
        outline_id: Option<&str>,
        sub_index: Option<i32>,
    ) -> Result<Option<chapter::Model>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        let now = Utc::now();
        let model = chapter::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            title: Set(title.to_string()),
            chapter_number: Set(chapter_number),
            content: Set(content.map(|s| s.to_string())),
            summary: Set(summary.map(|s| s.to_string())),
            word_count: Set(content.map(|s| s.chars().count() as i32).unwrap_or(0)),
            status: Set("draft".to_string()),
            outline_id: Set(outline_id.map(|s| s.to_string())),
            sub_index: Set(sub_index.unwrap_or(1)),
            expansion_plan: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn list_by_project(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<chapter::Model>>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(project_id))
            .order_by_asc(chapter::Column::ChapterNumber)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn get(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
    ) -> Result<Option<chapter::Model>, String> {
        let c = chapter::Entity::find_by_id(chapter_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        match c {
            Some(ref ch) => {
                if !Self::verify_project_access(db, &ch.project_id, user_id).await? {
                    return Ok(None);
                }
                Ok(Some(ch.clone()))
            }
            None => Ok(None),
        }
    }

    pub async fn update(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        title: Option<&str>,
        content: Option<&str>,
        summary: Option<&str>,
        status: Option<&str>,
        chapter_number: Option<i32>,
        expansion_plan: Option<&str>,
    ) -> Result<Option<chapter::Model>, String> {
        let existing = Self::get(db, chapter_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };
        let mut active: chapter::ActiveModel = model.into();
        if let Some(v) = title { active.title = Set(v.to_string()); }
        if let Some(v) = content {
            let wc = v.chars().count() as i32;
            active.content = Set(Some(v.to_string()));
            active.word_count = Set(wc);
        }
        if let Some(v) = summary { active.summary = Set(Some(v.to_string())); }
        if let Some(v) = status { active.status = Set(v.to_string()); }
        if let Some(v) = chapter_number { active.chapter_number = Set(v); }
        if let Some(v) = expansion_plan { active.expansion_plan = Set(Some(v.to_string())); }
        active.updated_at = Set(Some(Utc::now()));
        active.update(db).await.map_err(|e| format!("{}", e)).map(Some)
    }

    pub async fn delete(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, chapter_id, user_id).await?;
        if existing.is_none() {
            return Ok(None);
        }
        chapter::Entity::delete_by_id(chapter_id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(Some(()))
    }
}
