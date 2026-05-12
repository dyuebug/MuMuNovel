use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};

use crate::ai::service::AIService;
use crate::models::{chapter, project};
use crate::services::prompt_template_service::PromptTemplateService;

fn chapter_template_key(outline_mode: &str, has_previous: bool) -> &'static str {
    match (outline_mode, has_previous) {
        ("one-to-many", false) => "CHAPTER_GENERATION_ONE_TO_MANY",
        ("one-to-many", true) => "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
        ("one-to-one", false) | (_, false) => "CHAPTER_GENERATION_ONE_TO_ONE",
        _ => "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
    }
}

pub struct ChapterGenerationService;

impl ChapterGenerationService {
    async fn load_context(
        db: &DatabaseConnection,
        user_id: &str,
        chapter_id: &str,
    ) -> Result<(chapter::Model, project::Model, Option<chapter::Model>), String> {
        let chapter_model = chapter::Entity::find_by_id(chapter_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Chapter not found".to_string())?;

        let project_model = project::Entity::find_by_id(&chapter_model.project_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Project not found".to_string())?;

        if project_model.user_id != user_id {
            return Err("Chapter not found or access denied".to_string());
        }

        let previous_chapter = chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(&chapter_model.project_id))
            .filter(chapter::Column::ChapterNumber.lt(chapter_model.chapter_number))
            .order_by_desc(chapter::Column::ChapterNumber)
            .one(db)
            .await
            .map_err(|error| error.to_string())?;

        Ok((chapter_model, project_model, previous_chapter))
    }

    fn build_prompt(
        chapter_model: &chapter::Model,
        project_model: &project::Model,
        previous_chapter: Option<&chapter::Model>,
        target_word_count: i32,
    ) -> Result<String, String> {
        let template_key =
            chapter_template_key(&project_model.outline_mode, previous_chapter.is_some());
        let template = PromptTemplateService::system_template_info(template_key)
            .ok_or_else(|| format!("找不到章节模板: {}", template_key))?;

        let mut params = HashMap::new();
        params.insert("project_title".to_string(), project_model.title.clone());
        params.insert(
            "genre".to_string(),
            project_model.genre.clone().unwrap_or_default(),
        );
        params.insert(
            "chapter_number".to_string(),
            chapter_model.chapter_number.to_string(),
        );
        params.insert("chapter_title".to_string(), chapter_model.title.clone());
        params.insert(
            "target_word_count".to_string(),
            target_word_count.to_string(),
        );
        params.insert(
            "narrative_perspective".to_string(),
            project_model
                .narrative_perspective
                .clone()
                .unwrap_or_else(|| "第三人称".to_string()),
        );
        params.insert(
            "chapter_outline".to_string(),
            chapter_model
                .expansion_plan
                .clone()
                .unwrap_or_else(|| "暂无大纲".to_string()),
        );
        params.insert(
            "world_time_period".to_string(),
            project_model.world_time_period.clone().unwrap_or_default(),
        );
        params.insert(
            "world_location".to_string(),
            project_model.world_location.clone().unwrap_or_default(),
        );
        params.insert(
            "world_atmosphere".to_string(),
            project_model.world_atmosphere.clone().unwrap_or_default(),
        );
        params.insert(
            "world_rules".to_string(),
            project_model.world_rules.clone().unwrap_or_default(),
        );
        params.insert("characters_info".to_string(), "[]".to_string());
        params.insert("foreshadow_reminders".to_string(), "[]".to_string());
        params.insert("relevant_memories".to_string(), "[]".to_string());
        params.insert(
            "previous_chapter_summary".to_string(),
            previous_chapter
                .and_then(|item| item.summary.clone())
                .unwrap_or_default(),
        );
        params.insert(
            "previous_chapter_content".to_string(),
            previous_chapter
                .and_then(|item| item.content.clone())
                .unwrap_or_default(),
        );
        params.insert(
            "continuation_point".to_string(),
            previous_chapter
                .and_then(|item| item.content.clone())
                .unwrap_or_default()
                .chars()
                .take(300)
                .collect(),
        );

        PromptTemplateService::format_prompt(&template.content, &params)
    }

    pub async fn generate_chapter_content(
        db: &DatabaseConnection,
        user_ai_service: &AIService,
        user_id: &str,
        chapter_id: &str,
    ) -> Result<Value, String> {
        let (chapter_model, project_model, previous_chapter) =
            Self::load_context(db, user_id, chapter_id).await?;
        let prompt =
            Self::build_prompt(&chapter_model, &project_model, previous_chapter.as_ref(), 3000)?;
        let response = user_ai_service
            .generate_text(&prompt, None, None)
            .await
            .map_err(|error| error.to_string())?;

        Ok(json!({
            "chapter_id": chapter_id,
            "chapter_number": chapter_model.chapter_number,
            "title": chapter_model.title,
            "content": response.content,
            "word_count": response.content.chars().count(),
        }))
    }

    pub async fn generate_and_persist_chapter_content(
        db: &DatabaseConnection,
        user_ai_service: &AIService,
        user_id: &str,
        chapter_id: &str,
        target_word_count: i32,
    ) -> Result<Value, String> {
        let (chapter_model, project_model, previous_chapter) =
            Self::load_context(db, user_id, chapter_id).await?;
        let prompt = Self::build_prompt(
            &chapter_model,
            &project_model,
            previous_chapter.as_ref(),
            target_word_count,
        )?;
        let response = user_ai_service
            .generate_text(&prompt, None, None)
            .await
            .map_err(|error| error.to_string())?;

        let cleaned_content = response.content.trim().to_string();
        let word_count = cleaned_content.chars().count() as i32;

        let mut active: chapter::ActiveModel = chapter_model.clone().into();
        active.content = Set(Some(cleaned_content.clone()));
        active.word_count = Set(word_count);
        active.status = Set("draft".to_string());
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active
            .update(db)
            .await
            .map_err(|error| error.to_string())?;

        Ok(json!({
            "chapter_id": chapter_id,
            "chapter_number": chapter_model.chapter_number,
            "title": chapter_model.title,
            "content": cleaned_content,
            "word_count": word_count,
        }))
    }
}
