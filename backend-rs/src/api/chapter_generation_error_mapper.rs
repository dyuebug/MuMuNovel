use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    chapter_not_found_or_access_denied_error, detail_error, internal_detail_error,
};
use crate::services::chapter_batch_generation_access_service::LoadAccessibleChapterForGenerationError;
use crate::services::chapter_single_generation_prepare_service::PrepareSingleChapterGenerationRequestError;

fn map_single_chapter_generation_error(
    error: LoadAccessibleChapterForGenerationError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadAccessibleChapterForGenerationError::ChapterNotFound => {
            detail_error(StatusCode::NOT_FOUND, "Chapter not found")
        }
        LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        LoadAccessibleChapterForGenerationError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_single_chapter_generation_request_error(
    error: PrepareSingleChapterGenerationRequestError,
) -> (StatusCode, Json<Value>) {
    match error {
        PrepareSingleChapterGenerationRequestError::Chapter(error) => {
            map_single_chapter_generation_error(error)
        }
        PrepareSingleChapterGenerationRequestError::Config(error) => {
            detail_error(StatusCode::BAD_REQUEST, error)
        }
        PrepareSingleChapterGenerationRequestError::Internal(error) => internal_detail_error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::map_single_chapter_generation_request_error;
    use crate::services::chapter_batch_generation_access_service::LoadAccessibleChapterForGenerationError;
    use crate::services::chapter_single_generation_prepare_service::PrepareSingleChapterGenerationRequestError;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn single_generation_background_not_found_or_access_denied_remains_404() {
        let response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn single_generation_background_config_error_maps_to_bad_request() {
        let response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::Config("model missing".to_string()),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "model missing" }));
    }

    #[test]
    fn single_generation_background_not_found_remains_404() {
        let response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "Chapter not found" }));
    }
}
