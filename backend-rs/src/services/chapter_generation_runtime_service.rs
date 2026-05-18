use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde_json::{json, Value};

use crate::ai::service::AIService;
use crate::ai::types::AIResponse;
use crate::models::chapter;
use crate::services::chapter_generation_context_service::load_generation_context;
use crate::services::chapter_generation_prompt_context_provider_service::{
    resolve_default_prompt_context_provider_payload, PromptContextProviderPayload,
};
use crate::services::chapter_generation_prompt_service::{
    build_prompt_with_provider_payload,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedChapterPersistencePlan {
    cleaned_content: String,
    word_count: i32,
    next_status: String,
    response_payload: Value,
}

fn prepare_generated_chapter_persistence_plan(
    chapter_model: &chapter::Model,
    response: AIResponse,
) -> GeneratedChapterPersistencePlan {
    let cleaned_content = response.content.trim().to_string();
    let word_count = cleaned_content.chars().count() as i32;
    let next_status = "draft".to_string();
    let response_payload = json!({
        "chapter_id": chapter_model.id,
        "chapter_number": chapter_model.chapter_number,
        "title": chapter_model.title,
        "content": cleaned_content,
        "word_count": word_count,
    });

    GeneratedChapterPersistencePlan {
        cleaned_content,
        word_count,
        next_status,
        response_payload,
    }
}

async fn persist_generated_chapter_content(
    db: &DatabaseConnection,
    chapter_model: chapter::Model,
    response: AIResponse,
) -> Result<Value, String> {
    let plan = prepare_generated_chapter_persistence_plan(&chapter_model, response);

    let mut active: chapter::ActiveModel = chapter_model.clone().into();
    active.content = Set(Some(plan.cleaned_content.clone()));
    active.word_count = Set(plan.word_count);
    active.status = Set(plan.next_status);
    active.updated_at = Set(Some(Utc::now().naive_utc()));
    active
        .update(db)
        .await
        .map_err(|error| error.to_string())?;

    Ok(plan.response_payload)
}

pub async fn generate_and_persist_chapter_content_with_provider_payload(
    db: &DatabaseConnection,
    user_ai_service: &AIService,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
) -> Result<Value, String> {
    let (chapter_model, project_model, previous_chapter) =
        load_generation_context(db, user_id, chapter_id).await?;
    let prompt = build_prompt_with_provider_payload(
        &chapter_model,
        &project_model,
        previous_chapter.as_ref(),
        target_word_count,
        provider_payload,
    )?;
    let response = user_ai_service
        .generate_text(&prompt, None, None)
        .await
        .map_err(|error| error.to_string())?;

    persist_generated_chapter_content(db, chapter_model, response).await
}

pub async fn generate_and_persist_chapter_content(
    db: &DatabaseConnection,
    user_ai_service: &AIService,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
) -> Result<Value, String> {
    generate_and_persist_chapter_content_with_provider_payload(
        db,
        user_ai_service,
        user_id,
        chapter_id,
        target_word_count,
        resolve_default_prompt_context_provider_payload(),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        prepare_generated_chapter_persistence_plan,
    };
    use crate::ai::types::AIResponse;
    use crate::models::chapter;
    use crate::services::chapter_generation_prompt_context_provider_service::resolve_default_prompt_context_provider_payload;
    use chrono::Utc;

    fn build_chapter() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            title: "第一章".to_string(),
            chapter_number: 1,
            content: None,
            summary: None,
            expansion_plan: None,
            status: "pending".to_string(),
            word_count: 0,
            outline_id: None,
            sub_index: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_resolve_default_prompt_context_provider_payload() {
        let payload = resolve_default_prompt_context_provider_payload();

        assert_eq!(payload.characters_info, "[]");
        assert_eq!(payload.foreshadow_reminders, "[]");
        assert_eq!(payload.relevant_memories, "[]");
    }

    #[test]
    fn should_prepare_generated_chapter_persistence_plan() {
        let chapter_model = build_chapter();
        let response = AIResponse {
            content: "  你好\n世界  ".to_string(),
            tool_calls: None,
            finish_reason: Some("stop".to_string()),
        };

        let plan = prepare_generated_chapter_persistence_plan(&chapter_model, response);

        assert_eq!(plan.cleaned_content, "你好\n世界");
        assert_eq!(plan.word_count, 5);
        assert_eq!(plan.next_status, "draft");
        assert_eq!(plan.response_payload["chapter_id"], "chapter-1");
        assert_eq!(plan.response_payload["content"], "你好\n世界");
        assert_eq!(plan.response_payload["word_count"], 5);
    }
}
