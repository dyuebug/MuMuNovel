use serde_json::Value;

use crate::models::chapter;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_regeneration_full_stream_service::{
    build_full_chapter_regeneration_stream, FullChapterRegenerationStream,
    FullChapterRegenerationStreamInput,
};
use crate::services::chapter_regeneration_partial_stream_service::{
    build_partial_chapter_regeneration_stream, PartialChapterRegenerationStream,
    PartialChapterRegenerationStreamInput,
};
use crate::services::chapter_regeneration_prepare_service::{
    prepare_chapter_regeneration_stream, prepare_partial_regeneration_stream,
    PrepareChapterRegenerationStreamError, PreparePartialRegenerationStreamError,
};

pub enum CreateChapterRegenerationStreamWorkflowError {
    Chapter(LoadAccessibleChapterError),
    Prepare(PrepareChapterRegenerationStreamError),
}

pub enum CreatePartialRegenerationStreamWorkflowError {
    Chapter(LoadAccessibleChapterError),
    Prepare(PreparePartialRegenerationStreamError),
}

pub struct PartialRegenerationStreamWorkflowRequest<'a> {
    pub selected_text: &'a str,
    pub start_position: usize,
    pub end_position: usize,
    pub context_chars: Option<usize>,
    pub user_instructions: &'a str,
    pub length_mode: Option<&'a str>,
    pub target_word_count: Option<usize>,
    pub style_id: Option<i32>,
    pub enable_web_research: Option<bool>,
    pub web_research_query: Option<&'a str>,
}

pub fn normalize_partial_regeneration_context_chars(context_chars: Option<usize>) -> usize {
    context_chars.unwrap_or(500)
}

pub fn normalize_partial_regeneration_web_research_enabled(enabled: Option<bool>) -> bool {
    enabled.unwrap_or(false)
}

async fn load_chapter_for_regeneration_stream(
    db: &sea_orm::DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, LoadAccessibleChapterError> {
    load_accessible_chapter(db, chapter_id, user_id).await
}

pub async fn create_chapter_regeneration_stream_workflow(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    body: &Value,
) -> Result<FullChapterRegenerationStream, CreateChapterRegenerationStreamWorkflowError> {
    let chapter = load_chapter_for_regeneration_stream(db, chapter_id, user_id)
        .await
        .map_err(CreateChapterRegenerationStreamWorkflowError::Chapter)?;
    let prepared = prepare_chapter_regeneration_stream(db, user_id, &chapter, body)
        .await
        .map_err(CreateChapterRegenerationStreamWorkflowError::Prepare)?;

    Ok(build_full_chapter_regeneration_stream(
        FullChapterRegenerationStreamInput {
            task_label: "Chapter Rewrite".to_string(),
            chapter_id: chapter_id.to_string(),
            chapter_word_count: chapter.word_count as usize,
            prompt: prepared.prompt,
            ai_service: prepared.ai_service,
        },
    ))
}

pub async fn create_partial_regeneration_stream_workflow(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    request: PartialRegenerationStreamWorkflowRequest<'_>,
) -> Result<PartialChapterRegenerationStream, CreatePartialRegenerationStreamWorkflowError> {
    let chapter = load_chapter_for_regeneration_stream(db, chapter_id, user_id)
        .await
        .map_err(CreatePartialRegenerationStreamWorkflowError::Chapter)?;
    let stream_prepared = prepare_partial_regeneration_stream(
        db,
        user_id,
        &chapter,
        request.selected_text,
        request.start_position,
        request.end_position,
        normalize_partial_regeneration_context_chars(request.context_chars),
        request.user_instructions,
        request.length_mode,
        request.target_word_count,
        request.style_id,
        normalize_partial_regeneration_web_research_enabled(request.enable_web_research),
        request.web_research_query,
    )
    .await
    .map_err(CreatePartialRegenerationStreamWorkflowError::Prepare)?;
    let prepared = stream_prepared.prepared;

    Ok(build_partial_chapter_regeneration_stream(
        PartialChapterRegenerationStreamInput {
            target_words: prepared.target_words,
            original_word_count: prepared.original_word_count,
            start_position: request.start_position,
            end_position: request.end_position,
            prompt: prepared.prompt,
            ai_service: stream_prepared.ai_service,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_partial_regeneration_context_chars,
        normalize_partial_regeneration_web_research_enabled,
    };

    #[test]
    fn should_normalize_partial_regeneration_context_chars() {
        assert_eq!(normalize_partial_regeneration_context_chars(None), 500);
        assert_eq!(normalize_partial_regeneration_context_chars(Some(0)), 0);
        assert_eq!(
            normalize_partial_regeneration_context_chars(Some(1200)),
            1200
        );
    }

    #[test]
    fn should_normalize_partial_regeneration_web_research_enabled() {
        assert!(!normalize_partial_regeneration_web_research_enabled(None));
        assert!(!normalize_partial_regeneration_web_research_enabled(Some(
            false
        )));
        assert!(normalize_partial_regeneration_web_research_enabled(Some(
            true
        )));
    }
}
