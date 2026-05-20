use serde_json::{json, Value};

use crate::models::chapter;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
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

pub struct ApplyPartialRegenerateRequest<'a> {
    pub new_text: Option<&'a str>,
    pub start_position: Option<usize>,
    pub end_position: Option<usize>,
}

struct PreparedPartialRegenerateApply {
    new_content: String,
    old_word_count: i32,
}

pub async fn apply_owned_partial_regenerate_payload(
    db: &sea_orm::DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: ApplyPartialRegenerateRequest<'_>,
) -> Result<Value, ApplyPartialRegenerateError> {
    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(|error| match error {
            LoadAccessibleChapterError::NotFoundOrAccessDenied => {
                ApplyPartialRegenerateError::NotFound
            }
            LoadAccessibleChapterError::Internal(detail) => {
                ApplyPartialRegenerateError::Internal(detail)
            }
        })?;

    apply_partial_regenerate_payload(
        db,
        chapter_id,
        user_id,
        &chapter,
        request.new_text.unwrap_or_default(),
        request.start_position.unwrap_or(0),
        request.end_position.unwrap_or(0),
    )
    .await
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
    let prepared =
        prepare_partial_regenerate_apply(chapter, new_text_raw, start_position, end_position)?;

    match ChapterService::update(
        db,
        chapter_id,
        user_id,
        None,
        Some(&prepared.new_content),
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
            "old_word_count": prepared.old_word_count,
            "message": "局部改写已应用",
        })),
        Ok(None) => Err(ApplyPartialRegenerateError::NotFound),
        Err(error) => Err(ApplyPartialRegenerateError::Internal(error)),
    }
}

fn prepare_partial_regenerate_apply(
    chapter: &chapter::Model,
    new_text_raw: &str,
    start_position: usize,
    end_position: usize,
) -> Result<PreparedPartialRegenerateApply, ApplyPartialRegenerateError> {
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

    Ok(PreparedPartialRegenerateApply {
        new_content,
        old_word_count: chapter.word_count,
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::chapter;

    use super::{
        prepare_partial_regenerate_apply, ApplyPartialRegenerateError,
        PreparedPartialRegenerateApply,
    };

    fn chapter_with_content(content: &str) -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            title: "测试章节".to_string(),
            chapter_number: 1,
            content: Some(content.to_string()),
            summary: None,
            word_count: content.chars().count() as i32,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    fn valid_prepared_apply(
        result: Result<PreparedPartialRegenerateApply, ApplyPartialRegenerateError>,
    ) -> PreparedPartialRegenerateApply {
        match result {
            Ok(prepared) => prepared,
            Err(_) => panic!("partial regenerate apply should be valid"),
        }
    }

    fn apply_error(
        result: Result<PreparedPartialRegenerateApply, ApplyPartialRegenerateError>,
    ) -> ApplyPartialRegenerateError {
        match result {
            Ok(_) => panic!("partial regenerate apply should be rejected"),
            Err(error) => error,
        }
    }

    #[test]
    fn should_prepare_partial_regenerate_apply_content() {
        let chapter = chapter_with_content("一二三四五");
        let prepared =
            valid_prepared_apply(prepare_partial_regenerate_apply(&chapter, "替换文本", 1, 4));

        assert_eq!(prepared.new_content, "一替换文本五");
        assert_eq!(prepared.old_word_count, 5);
    }

    #[test]
    fn should_reject_empty_partial_regenerate_apply_content() {
        let chapter = chapter_with_content("一二三");
        let error = apply_error(prepare_partial_regenerate_apply(&chapter, "   ", 0, 1));

        assert!(matches!(error, ApplyPartialRegenerateError::EmptyContent));
    }

    #[test]
    fn should_reject_meta_only_partial_regenerate_apply_content_as_empty() {
        let chapter = chapter_with_content("一二三");
        let error = apply_error(prepare_partial_regenerate_apply(
            &chapter,
            "```markdown\n作为AI：我将开始执行\n流程说明",
            0,
            1,
        ));

        assert!(matches!(error, ApplyPartialRegenerateError::EmptyContent));
    }

    #[test]
    fn should_reject_invalid_partial_regenerate_apply_range() {
        let chapter = chapter_with_content("一二三");
        let error = apply_error(prepare_partial_regenerate_apply(&chapter, "替换", 2, 2));

        assert!(matches!(error, ApplyPartialRegenerateError::InvalidRange));
    }
}
