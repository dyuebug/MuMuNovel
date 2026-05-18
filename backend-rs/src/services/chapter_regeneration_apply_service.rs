use serde_json::{json, Value};

use crate::models::chapter;
use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};
use crate::services::chapter_service::ChapterService;

pub enum ApplyPartialRegenerateError {
    EmptyContent,
    WorkflowMetaText,
    InvalidRange,
    NotFound,
    Internal(String),
}

pub async fn apply_partial_regenerate_payload(
    db: &sea_orm::DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    chapter: &chapter::Model,
    new_text_raw: &str,
    start_position: usize,
    end_position: usize,
) -> Result<Value, ApplyPartialRegenerateError> {
    let (new_text, _) = sanitize_generated_narrative_text(new_text_raw);
    if new_text.trim().is_empty() {
        return Err(ApplyPartialRegenerateError::EmptyContent);
    }
    if contains_chapter_workflow_meta_text(&new_text) {
        return Err(ApplyPartialRegenerateError::WorkflowMetaText);
    }

    let current_content = chapter.content.clone().unwrap_or_default();
    let content_chars: Vec<char> = current_content.chars().collect();
    let content_length = content_chars.len();
    if start_position >= end_position || end_position > content_length {
        return Err(ApplyPartialRegenerateError::InvalidRange);
    }

    let prefix: String = content_chars[..start_position].iter().collect();
    let suffix: String = content_chars[end_position..].iter().collect();
    let new_content = format!("{prefix}{new_text}{suffix}");
    let old_word_count = chapter.word_count;

    match ChapterService::update(
        db,
        chapter_id,
        user_id,
        None,
        Some(&new_content),
        None,
        None,
        None,
        None,
    )
    .await
    {
        Ok(Some(updated)) => Ok(json!({
            "success": true,
            "chapter_id": chapter_id,
            "word_count": updated.word_count,
            "old_word_count": old_word_count,
            "message": "局部改写已应用",
        })),
        Ok(None) => Err(ApplyPartialRegenerateError::NotFound),
        Err(error) => Err(ApplyPartialRegenerateError::Internal(error)),
    }
}
