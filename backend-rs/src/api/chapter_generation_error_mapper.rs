use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    chapter_not_found_or_access_denied_error, detail_error, internal_detail_error,
};
use crate::services::chapter_generation_access_service::LoadAccessibleChapterForGenerationError;
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
        PrepareSingleChapterGenerationRequestError::PrerequisitesBlocked(error) => {
            detail_error(StatusCode::BAD_REQUEST, error)
        }
        PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooSmall => detail_error(
            StatusCode::BAD_REQUEST,
            "target_word_count must be greater than or equal to 500",
        ),
        PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooLarge => detail_error(
            StatusCode::BAD_REQUEST,
            "target_word_count must be less than or equal to 10000",
        ),
        PrepareSingleChapterGenerationRequestError::InvalidCreativeMode => {
            detail_error(StatusCode::BAD_REQUEST, "creative_mode is invalid")
        }
        PrepareSingleChapterGenerationRequestError::InvalidStoryFocus => {
            detail_error(StatusCode::BAD_REQUEST, "story_focus is invalid")
        }
        PrepareSingleChapterGenerationRequestError::InvalidPlotStage => {
            detail_error(StatusCode::BAD_REQUEST, "plot_stage is invalid")
        }
        PrepareSingleChapterGenerationRequestError::InvalidQualityPreset => {
            detail_error(StatusCode::BAD_REQUEST, "quality_preset is invalid")
        }
        PrepareSingleChapterGenerationRequestError::StoryCreationBriefTooLong => detail_error(
            StatusCode::BAD_REQUEST,
            "story_creation_brief must be at most 1200 characters",
        ),
        PrepareSingleChapterGenerationRequestError::QualityNotesTooLong => detail_error(
            StatusCode::BAD_REQUEST,
            "quality_notes must be at most 600 characters",
        ),
        PrepareSingleChapterGenerationRequestError::Internal(error) => internal_detail_error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::map_single_chapter_generation_request_error;
    use crate::services::chapter_generation_access_service::LoadAccessibleChapterForGenerationError;
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
    fn single_generation_background_prerequisites_blocked_maps_to_bad_request() {
        let response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::PrerequisitesBlocked(
                "前置章节尚未完成: 2 章".to_string(),
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "前置章节尚未完成: 2 章" }));
    }

    #[test]
    fn single_generation_target_word_count_bounds_match_python_contract() {
        let lower_response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooSmall,
        );
        let upper_response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooLarge,
        );

        assert_eq!(lower_response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            lower_response.1 .0,
            json!({ "detail": "target_word_count must be greater than or equal to 500" })
        );
        assert_eq!(upper_response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            upper_response.1 .0,
            json!({ "detail": "target_word_count must be less than or equal to 10000" })
        );
    }

    #[test]
    fn single_generation_generation_choice_errors_remain_bad_request() {
        let cases = [
            (
                PrepareSingleChapterGenerationRequestError::InvalidCreativeMode,
                "creative_mode is invalid",
            ),
            (
                PrepareSingleChapterGenerationRequestError::InvalidStoryFocus,
                "story_focus is invalid",
            ),
            (
                PrepareSingleChapterGenerationRequestError::InvalidPlotStage,
                "plot_stage is invalid",
            ),
            (
                PrepareSingleChapterGenerationRequestError::InvalidQualityPreset,
                "quality_preset is invalid",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_single_chapter_generation_request_error(error);

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn single_generation_generation_text_length_errors_remain_bad_request() {
        let cases = [
            (
                PrepareSingleChapterGenerationRequestError::StoryCreationBriefTooLong,
                "story_creation_brief must be at most 1200 characters",
            ),
            (
                PrepareSingleChapterGenerationRequestError::QualityNotesTooLong,
                "quality_notes must be at most 600 characters",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_single_chapter_generation_request_error(error);

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
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
