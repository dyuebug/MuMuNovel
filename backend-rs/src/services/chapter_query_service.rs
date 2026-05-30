use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::chapter;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_service::ChapterService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadQueryPayloadError<TNotFound> {
    NotFound(TNotFound),
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterReadNotFound {
    ChapterNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectReadNotFound {
    ProjectNotFound,
}

pub type ChapterQueryPayloadError = ReadQueryPayloadError<ChapterReadNotFound>;
pub type LoadQualityTrendPayloadError = ReadQueryPayloadError<ProjectReadNotFound>;
pub type LoadAnnotationsPayloadError = LoadAccessibleChapterError;
pub type LoadNavigationPayloadError = ChapterQueryPayloadError;
pub type LoadCanGeneratePayloadError = ChapterQueryPayloadError;

fn annotations_payload(chapter_id: &str) -> Value {
    json!({
        "chapter_id": chapter_id,
        "annotations": [],
        "memory_mapping": [],
    })
}

fn navigation_payload(
    previous: Option<chapter::Model>,
    current: Option<chapter::Model>,
    next: Option<chapter::Model>,
) -> Value {
    json!({
        "previous": previous,
        "current": current,
        "next": next,
    })
}

fn can_generate_payload(can_generate: bool) -> Value {
    json!({
        "can_generate": can_generate,
    })
}

fn quality_trend_payload(chapters: &[chapter::Model]) -> Value {
    json!(chapters
        .iter()
        .map(|chapter| json!({
            "chapter_id": chapter.id,
            "chapter_number": chapter.chapter_number,
            "title": chapter.title,
            "word_count": chapter.word_count,
            "status": chapter.status,
            "created_at": chapter.created_at.and_utc().to_rfc3339(),
        }))
        .collect::<Vec<Value>>())
}

pub async fn load_navigation_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadNavigationPayloadError> {
    match ChapterService::navigation(db, chapter_id, user_id).await {
        Ok(Some((previous, current, next))) => Ok(navigation_payload(previous, current, next)),
        Ok(None) => Err(ReadQueryPayloadError::NotFound(
            ChapterReadNotFound::ChapterNotFound,
        )),
        Err(error) => Err(ReadQueryPayloadError::Internal(error)),
    }
}

pub async fn load_can_generate_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadCanGeneratePayloadError> {
    match ChapterService::can_generate(db, chapter_id, user_id).await {
        Ok(Some(can_generate)) => Ok(can_generate_payload(can_generate)),
        Ok(None) => Err(ReadQueryPayloadError::NotFound(
            ChapterReadNotFound::ChapterNotFound,
        )),
        Err(error) => Err(ReadQueryPayloadError::Internal(error)),
    }
}

pub async fn load_annotations_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadAnnotationsPayloadError> {
    let _ = load_accessible_chapter(db, chapter_id, user_id).await?;

    Ok(annotations_payload(chapter_id))
}

pub async fn load_quality_trend_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, LoadQualityTrendPayloadError> {
    match ChapterService::list_by_project(db, project_id, user_id).await {
        Ok(Some(chapters)) => Ok(quality_trend_payload(&chapters)),
        Ok(None) => Err(ReadQueryPayloadError::NotFound(
            ProjectReadNotFound::ProjectNotFound,
        )),
        Err(error) => Err(ReadQueryPayloadError::Internal(error)),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::chapter;
    use crate::services::chapter_access_service::LoadAccessibleChapterError;

    use super::{
        annotations_payload, can_generate_payload, navigation_payload, quality_trend_payload,
        ChapterReadNotFound, LoadAnnotationsPayloadError, LoadCanGeneratePayloadError,
        LoadNavigationPayloadError, LoadQualityTrendPayloadError, ProjectReadNotFound,
        ReadQueryPayloadError,
    };

    fn chapter_model(id: &str, number: i32) -> chapter::Model {
        chapter::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_number: number,
            title: format!("第{}章", number),
            content: Some("正文".to_string()),
            summary: None,
            word_count: 2,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn should_build_navigation_payload() {
        let payload = navigation_payload(
            Some(chapter_model("chapter-1", 1)),
            Some(chapter_model("chapter-2", 2)),
            None,
        );

        assert_eq!(payload["previous"]["id"], "chapter-1");
        assert_eq!(payload["current"]["id"], "chapter-2");
        assert!(payload["next"].is_null());
    }

    #[test]
    fn should_build_can_generate_payload() {
        assert_eq!(can_generate_payload(true)["can_generate"], true);
        assert_eq!(can_generate_payload(false)["can_generate"], false);
    }

    #[test]
    fn should_build_empty_annotations_payload() {
        let payload = annotations_payload("chapter-1");

        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["annotations"].as_array().map(Vec::len), Some(0));
        assert_eq!(payload["memory_mapping"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn should_build_quality_trend_payload() {
        let payload =
            quality_trend_payload(&[chapter_model("chapter-1", 1), chapter_model("chapter-2", 2)]);

        assert_eq!(payload.as_array().map(Vec::len), Some(2));
        assert_eq!(payload[0]["chapter_id"], "chapter-1");
        assert_eq!(payload[1]["chapter_number"], 2);
    }

    #[test]
    fn should_alias_navigation_query_error_owner() {
        let error: LoadNavigationPayloadError =
            ReadQueryPayloadError::NotFound(ChapterReadNotFound::ChapterNotFound);

        assert!(matches!(
            error,
            ReadQueryPayloadError::NotFound(ChapterReadNotFound::ChapterNotFound)
        ));
    }

    #[test]
    fn should_alias_can_generate_query_error_owner() {
        let error: LoadCanGeneratePayloadError =
            ReadQueryPayloadError::Internal("boom".to_string());

        assert!(matches!(
            error,
            ReadQueryPayloadError::Internal(detail) if detail == "boom"
        ));
    }

    #[test]
    fn should_alias_access_not_found_error_for_annotations_query() {
        let error: LoadAnnotationsPayloadError = LoadAccessibleChapterError::NotFoundOrAccessDenied;

        assert_eq!(error, LoadAccessibleChapterError::NotFoundOrAccessDenied);
    }

    #[test]
    fn should_alias_access_internal_error_for_annotations_query() {
        let error: LoadAnnotationsPayloadError =
            LoadAccessibleChapterError::Internal("boom".to_string());

        assert_eq!(
            error,
            LoadAccessibleChapterError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_alias_quality_trend_query_error_owner() {
        let error: LoadQualityTrendPayloadError =
            ReadQueryPayloadError::NotFound(ProjectReadNotFound::ProjectNotFound);

        assert!(matches!(
            error,
            ReadQueryPayloadError::NotFound(ProjectReadNotFound::ProjectNotFound)
        ));
    }

    #[test]
    fn should_keep_quality_trend_internal_detail() {
        let error: LoadQualityTrendPayloadError =
            ReadQueryPayloadError::Internal("boom".to_string());

        assert!(matches!(
            error,
            ReadQueryPayloadError::Internal(detail) if detail == "boom"
        ));
    }
}
