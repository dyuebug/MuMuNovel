use serde_json::{json, Value};

use crate::models::chapter;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};
use crate::services::chapter_service::ChapterService;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyPartialRegenerateRequest {
    new_text: Option<String>,
    start_position: Option<usize>,
    end_position: Option<usize>,
}

impl ApplyPartialRegenerateRequest {
    pub fn new(
        new_text: Option<String>,
        start_position: Option<usize>,
        end_position: Option<usize>,
    ) -> Self {
        Self {
            new_text,
            start_position,
            end_position,
        }
    }

    pub fn from_route_payload(
        new_text: Option<String>,
        start_position: Option<usize>,
        end_position: Option<usize>,
    ) -> Self {
        Self::new(new_text, start_position, end_position)
    }

    pub fn new_text(&self) -> Option<&str> {
        self.new_text.as_deref()
    }

    pub fn start_position(&self) -> usize {
        self.start_position.unwrap_or(0)
    }

    pub fn end_position(&self) -> usize {
        self.end_position.unwrap_or(0)
    }
}

pub enum ApplyPartialRegenerateError {
    EmptyContent,
    WorkflowMetaText,
    InvalidRange,
    Chapter(LoadAccessibleChapterError),
    Internal(String),
}

pub async fn apply_owned_partial_regenerate_payload(
    db: &sea_orm::DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: ApplyPartialRegenerateRequest,
) -> Result<Value, ApplyPartialRegenerateError> {
    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(ApplyPartialRegenerateError::Chapter)?;

    apply_partial_regenerate_payload(
        db,
        chapter_id,
        user_id,
        &chapter,
        request.new_text().unwrap_or_default(),
        request.start_position(),
        request.end_position(),
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
    let new_content =
        prepare_partial_regenerate_apply(chapter, new_text_raw, start_position, end_position)?;

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
            "old_word_count": chapter.word_count,
            "message": "局部改写已应用",
        })),
        Ok(None) => Err(ApplyPartialRegenerateError::Chapter(
            LoadAccessibleChapterError::NotFoundOrAccessDenied,
        )),
        Err(error) => Err(ApplyPartialRegenerateError::Internal(error)),
    }
}

fn prepare_partial_regenerate_apply(
    chapter: &chapter::Model,
    new_text_raw: &str,
    start_position: usize,
    end_position: usize,
) -> Result<String, ApplyPartialRegenerateError> {
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

    Ok(new_content)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::chapter;
    use crate::services::chapter_access_service::LoadAccessibleChapterError;

    use super::{
        prepare_partial_regenerate_apply, ApplyPartialRegenerateError,
        ApplyPartialRegenerateRequest,
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

    fn valid_prepared_apply(result: Result<String, ApplyPartialRegenerateError>) -> String {
        match result {
            Ok(prepared) => prepared,
            Err(_) => panic!("partial regenerate apply should be valid"),
        }
    }

    fn apply_error(
        result: Result<String, ApplyPartialRegenerateError>,
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

        assert_eq!(prepared, "一替换文本五");
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

    #[test]
    fn should_build_apply_partial_regenerate_request_from_route_payload() {
        let request = ApplyPartialRegenerateRequest::from_route_payload(
            Some("新文本".to_string()),
            Some(12),
            Some(24),
        );

        assert_eq!(request.new_text(), Some("新文本"));
        assert_eq!(request.start_position(), 12);
        assert_eq!(request.end_position(), 24);
    }

    #[test]
    fn should_alias_chapter_access_not_found_error_for_partial_apply() {
        let error = ApplyPartialRegenerateError::Chapter(
            LoadAccessibleChapterError::NotFoundOrAccessDenied,
        );

        assert!(matches!(
            error,
            ApplyPartialRegenerateError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied
            )
        ));
    }

    #[test]
    fn should_alias_chapter_access_internal_error_for_partial_apply() {
        let error = ApplyPartialRegenerateError::Chapter(LoadAccessibleChapterError::Internal(
            "boom".to_string(),
        ));

        assert!(matches!(
            error,
            ApplyPartialRegenerateError::Chapter(LoadAccessibleChapterError::Internal(detail))
            if detail == "boom"
        ));
    }

    #[test]
    fn should_keep_apply_partial_regenerate_request_defaults_contract() {
        let request = ApplyPartialRegenerateRequest::default();

        assert_eq!(request.new_text(), None);
        assert_eq!(request.start_position(), 0);
        assert_eq!(request.end_position(), 0);
    }
}
