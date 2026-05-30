use axum::{http::StatusCode, Json};
use serde_json::Value;

use crate::api::chapters_error_mapper::{detail_error, map_load_accessible_chapter_error};
use crate::services::chapter_analysis_service::ChapterAnalysisQueryContextError;
use crate::services::chapter_analysis_view_query_service::LoadChapterAnalysisViewPayloadError;

pub type ChapterAnalysisQueryRouteError = (StatusCode, Json<Value>);

pub(crate) fn map_chapter_analysis_query_context_error(
    error: ChapterAnalysisQueryContextError,
) -> ChapterAnalysisQueryRouteError {
    match error {
        ChapterAnalysisQueryContextError::Chapter(error) => {
            map_load_accessible_chapter_error(error)
        }
        ChapterAnalysisQueryContextError::Internal(error) => {
            detail_error(StatusCode::INTERNAL_SERVER_ERROR, error)
        }
    }
}

pub fn map_owned_chapter_analysis_view_error(
    error: LoadChapterAnalysisViewPayloadError,
) -> ChapterAnalysisQueryRouteError {
    match error {
        LoadChapterAnalysisViewPayloadError::Context(error) => {
            map_chapter_analysis_query_context_error(error)
        }
        LoadChapterAnalysisViewPayloadError::AnalysisNotFound => {
            detail_error(StatusCode::NOT_FOUND, "Chapter analysis not found")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{map_chapter_analysis_query_context_error, map_owned_chapter_analysis_view_error};
    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_analysis_service::ChapterAnalysisQueryContextError;
    use crate::services::chapter_analysis_view_query_service::LoadChapterAnalysisViewPayloadError;
    use crate::services::chapter_quality_metrics_query_service::LoadChapterQualityMetricsPayloadError;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn owned_analysis_query_context_not_found_or_access_denied_remains_404() {
        let response =
            map_chapter_analysis_query_context_error(ChapterAnalysisQueryContextError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied,
            ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn owned_analysis_query_context_internal_error_remains_500() {
        let response = map_chapter_analysis_query_context_error(
            ChapterAnalysisQueryContextError::Internal("database exploded".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "database exploded" }));
    }

    #[test]
    fn owned_analysis_view_not_found_or_access_denied_remains_404() {
        let response =
            map_owned_chapter_analysis_view_error(LoadChapterAnalysisViewPayloadError::Context(
                ChapterAnalysisQueryContextError::Chapter(
                    LoadAccessibleChapterError::NotFoundOrAccessDenied,
                ),
            ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn owned_analysis_view_analysis_not_found_maps_to_404() {
        let response = map_owned_chapter_analysis_view_error(
            LoadChapterAnalysisViewPayloadError::AnalysisNotFound,
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter analysis not found" })
        );
    }

    #[test]
    fn owned_analysis_view_internal_error_remains_500() {
        let response =
            map_owned_chapter_analysis_view_error(LoadChapterAnalysisViewPayloadError::Context(
                ChapterAnalysisQueryContextError::Internal("database exploded".to_string()),
            ));

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "database exploded" }));
    }

    #[test]
    fn owned_quality_metrics_internal_error_uses_internal_detail() {
        let response = map_chapter_analysis_query_context_error(
            LoadChapterQualityMetricsPayloadError::Internal("database exploded".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "database exploded" }));
    }

    #[test]
    fn owned_quality_metrics_not_found_or_access_denied_reuses_shared_context_mapping() {
        let response = map_chapter_analysis_query_context_error(
            LoadChapterQualityMetricsPayloadError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }
}
