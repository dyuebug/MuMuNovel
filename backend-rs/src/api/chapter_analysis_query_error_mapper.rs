use axum::{http::StatusCode, Json};
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    chapter_not_found_or_access_denied_error, detail_error, internal_detail_error,
};
use crate::services::chapter_analysis_query_service::{
    LoadChapterAnalysisViewPayloadError, LoadChapterQualityMetricsPayloadError,
};

pub type ChapterAnalysisQueryRouteError = (StatusCode, Json<Value>);

pub fn map_owned_chapter_analysis_view_error(
    error: LoadChapterAnalysisViewPayloadError,
) -> ChapterAnalysisQueryRouteError {
    match error {
        LoadChapterAnalysisViewPayloadError::NotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        LoadChapterAnalysisViewPayloadError::Internal(error) => detail_error(
            if error == "Chapter analysis not found" {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            error,
        ),
    }
}

pub fn map_owned_chapter_quality_metrics_query_error(
    error: LoadChapterQualityMetricsPayloadError,
) -> ChapterAnalysisQueryRouteError {
    match error {
        LoadChapterQualityMetricsPayloadError::NotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        LoadChapterQualityMetricsPayloadError::Internal(error) => internal_detail_error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_owned_chapter_analysis_view_error, map_owned_chapter_quality_metrics_query_error,
    };
    use crate::services::chapter_analysis_query_service::{
        LoadChapterAnalysisViewPayloadError, LoadChapterQualityMetricsPayloadError,
    };
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn owned_analysis_view_not_found_or_access_denied_remains_404() {
        let response = map_owned_chapter_analysis_view_error(
            LoadChapterAnalysisViewPayloadError::NotFoundOrAccessDenied,
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn owned_analysis_view_internal_not_found_message_maps_to_404() {
        let response = map_owned_chapter_analysis_view_error(
            LoadChapterAnalysisViewPayloadError::Internal("Chapter analysis not found".to_string()),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter analysis not found" })
        );
    }

    #[test]
    fn owned_quality_metrics_internal_error_uses_internal_detail() {
        let response = map_owned_chapter_quality_metrics_query_error(
            LoadChapterQualityMetricsPayloadError::Internal("database exploded".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "database exploded" }));
    }
}
