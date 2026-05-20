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
        let now = Utc::now().naive_utc();
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
        model
            .insert(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    /// Create a pending chapter (used by wizard outline one-to-one mode)
    pub async fn create_pending(
        db: &DatabaseConnection,
        project_id: &str,
        title: &str,
        chapter_number: i32,
    ) -> Result<chapter::Model, String> {
        let now = Utc::now().naive_utc();
        let model = chapter::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            title: Set(title.to_string()),
            chapter_number: Set(chapter_number),
            content: Set(None),
            summary: Set(None),
            word_count: Set(0),
            status: Set("pending".to_string()),
            outline_id: Set(None),
            sub_index: Set(1),
            expansion_plan: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e))
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
        if let Some(v) = title {
            active.title = Set(v.to_string());
        }
        if let Some(v) = content {
            let wc = v.chars().count() as i32;
            active.content = Set(Some(v.to_string()));
            active.word_count = Set(wc);
        }
        if let Some(v) = summary {
            active.summary = Set(Some(v.to_string()));
        }
        if let Some(v) = status {
            active.status = Set(v.to_string());
        }
        if let Some(v) = chapter_number {
            active.chapter_number = Set(v);
        }
        if let Some(v) = expansion_plan {
            active.expansion_plan = Set(Some(v.to_string()));
        }
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active
            .update(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
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

    pub async fn navigation(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
    ) -> Result<
        Option<(
            Option<chapter::Model>,
            Option<chapter::Model>,
            Option<chapter::Model>,
        )>,
        String,
    > {
        let current = Self::get(db, chapter_id, user_id).await?;
        let Some(ref ch) = current else {
            return Ok(None);
        };

        let all = chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(&ch.project_id))
            .order_by_asc(chapter::Column::ChapterNumber)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))?;

        let pos = all.iter().position(|c| c.id == *chapter_id);
        let prev = pos.and_then(|p| p.checked_sub(1)).map(|i| all[i].clone());
        let next = pos
            .and_then(|p| p.checked_add(1))
            .filter(|i| *i < all.len())
            .map(|i| all[i].clone());

        Ok(Some((prev, current.map(|c| c.clone()), next)))
    }

    pub async fn update_expansion_plan(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        plan: &str,
    ) -> Result<Option<chapter::Model>, String> {
        let existing = Self::get(db, chapter_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };
        let mut active: chapter::ActiveModel = model.into();
        active.expansion_plan = Set(Some(plan.to_string()));
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active
            .update(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn can_generate(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
    ) -> Result<Option<bool>, String> {
        let ch = Self::get(db, chapter_id, user_id).await?;
        let Some(ch) = ch else {
            return Ok(None);
        };

        let has_content = ch.content.as_ref().map_or(false, |c| !c.trim().is_empty());
        if has_content {
            return Ok(Some(true));
        }

        // Check if previous chapter exists (for continuity)
        if ch.chapter_number > 1 {
            let prev_exists = chapter::Entity::find()
                .filter(chapter::Column::ProjectId.eq(&ch.project_id))
                .filter(chapter::Column::ChapterNumber.eq(ch.chapter_number - 1))
                .one(db)
                .await
                .map_err(|e| format!("{}", e))?;
            Ok(Some(prev_exists.is_some()))
        } else {
            Ok(Some(true))
        }
    }
}
