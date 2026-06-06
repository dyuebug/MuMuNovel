use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

use crate::services::chapter_access_service::LoadAccessibleChapterError;
use crate::services::chapter_analysis_runtime_service::PrepareChapterAnalysisTriggerError;
use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
use crate::services::chapter_query_service::{
    ChapterQueryPayloadError, LoadAnnotationsPayloadError, LoadCanGeneratePayloadError,
    LoadNavigationPayloadError, LoadQualityTrendPayloadError, QualityTrendQueryRequestError,
    ReadQueryPayloadError,
};
use crate::services::chapter_regeneration_apply_service::ApplyPartialRegenerateError;
use crate::services::chapter_regeneration_prepare_service::{
    BuildRegenerationAiServiceError, PreparePartialRegenerationError,
    PreparePartialRegenerationStreamError,
};
use crate::services::chapter_regeneration_query_service::RegenerationTasksQueryRequestError;
use crate::services::chapter_regeneration_stream_workflow_service::{
    CreateChapterRegenerationStreamWorkflowError, CreatePartialRegenerationStreamWorkflowError,
    CreateRegenerationStreamWorkflowError,
};

pub type ChapterRouteError = (StatusCode, Json<Value>);

pub fn detail_error(status: StatusCode, detail: impl Into<String>) -> ChapterRouteError {
    (status, Json(json!({ "detail": detail.into() })))
}

pub fn internal_detail_error(detail: impl Into<String>) -> ChapterRouteError {
    detail_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
}

pub fn chapter_not_found_or_access_denied_error() -> ChapterRouteError {
    detail_error(StatusCode::NOT_FOUND, "Chapter not found or access denied")
}

pub fn project_not_found_or_access_denied_error() -> ChapterRouteError {
    detail_error(StatusCode::NOT_FOUND, "Project not found or access denied")
}

pub fn map_load_accessible_chapter_error(error: LoadAccessibleChapterError) -> ChapterRouteError {
    match error {
        LoadAccessibleChapterError::NotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        LoadAccessibleChapterError::Internal(error) => internal_detail_error(error),
    }
}

pub fn map_regeneration_tasks_query_request_error(
    error: RegenerationTasksQueryRequestError,
) -> ChapterRouteError {
    match error {
        RegenerationTasksQueryRequestError::LimitTooSmall => detail_error(
            StatusCode::BAD_REQUEST,
            "limit must be greater than or equal to 1",
        ),
        RegenerationTasksQueryRequestError::LimitTooLarge => detail_error(
            StatusCode::BAD_REQUEST,
            "limit must be less than or equal to 50",
        ),
    }
}

pub fn map_quality_trend_query_request_error(
    error: QualityTrendQueryRequestError,
) -> ChapterRouteError {
    match error {
        QualityTrendQueryRequestError::LimitTooSmall => detail_error(
            StatusCode::BAD_REQUEST,
            "limit must be greater than or equal to 1",
        ),
        QualityTrendQueryRequestError::LimitTooLarge => detail_error(
            StatusCode::BAD_REQUEST,
            "limit must be less than or equal to 50",
        ),
    }
}

pub fn map_prepare_chapter_analysis_trigger_error(
    error: PrepareChapterAnalysisTriggerError,
) -> ChapterRouteError {
    match error {
        PrepareChapterAnalysisTriggerError::Chapter(error) => {
            map_load_accessible_chapter_error(error)
        }
        PrepareChapterAnalysisTriggerError::Create(error) => {
            map_create_chapter_analysis_task_error(error)
        }
    }
}

pub fn map_create_chapter_analysis_task_error(
    error: CreateChapterAnalysisTaskError,
) -> ChapterRouteError {
    match error {
        CreateChapterAnalysisTaskError::ChapterEmpty => {
            detail_error(StatusCode::BAD_REQUEST, "章节不存在或内容为空")
        }
        CreateChapterAnalysisTaskError::ProjectMissing => {
            detail_error(StatusCode::NOT_FOUND, "项目不存在")
        }
        CreateChapterAnalysisTaskError::Internal(detail) => internal_detail_error(detail),
    }
}

fn map_chapter_query_payload_error(error: ChapterQueryPayloadError) -> ChapterRouteError {
    map_read_query_payload_error(error, |_| chapter_not_found_or_access_denied_error())
}

fn map_read_query_payload_error<TNotFound>(
    error: ReadQueryPayloadError<TNotFound>,
    not_found_error: impl FnOnce(TNotFound) -> ChapterRouteError,
) -> ChapterRouteError {
    match error {
        ReadQueryPayloadError::NotFound(not_found) => not_found_error(not_found),
        ReadQueryPayloadError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_load_navigation_payload_error(error: LoadNavigationPayloadError) -> ChapterRouteError {
    map_chapter_query_payload_error(error)
}

pub fn map_load_annotations_payload_error(error: LoadAnnotationsPayloadError) -> ChapterRouteError {
    map_load_accessible_chapter_error(error)
}

pub fn map_load_quality_trend_payload_error(
    error: LoadQualityTrendPayloadError,
) -> ChapterRouteError {
    map_read_query_payload_error(error, |_| project_not_found_or_access_denied_error())
}

pub fn map_load_can_generate_payload_error(
    error: LoadCanGeneratePayloadError,
) -> ChapterRouteError {
    map_chapter_query_payload_error(error)
}

pub fn map_apply_partial_regenerate_error(error: ApplyPartialRegenerateError) -> ChapterRouteError {
    match error {
        ApplyPartialRegenerateError::EmptyContent => {
            detail_error(StatusCode::BAD_REQUEST, "改写内容为空")
        }
        ApplyPartialRegenerateError::WorkflowMetaText => {
            detail_error(StatusCode::BAD_REQUEST, "改写内容仍包含工作流提示文本")
        }
        ApplyPartialRegenerateError::InvalidRange => {
            detail_error(StatusCode::BAD_REQUEST, "改写位置非法")
        }
        ApplyPartialRegenerateError::Chapter(error) => map_load_accessible_chapter_error(error),
        ApplyPartialRegenerateError::Internal(detail) => internal_detail_error(detail),
    }
}

fn map_build_regeneration_ai_service_error(
    error: BuildRegenerationAiServiceError,
) -> ChapterRouteError {
    match error {
        BuildRegenerationAiServiceError::InvalidConfig(detail) => {
            detail_error(StatusCode::BAD_REQUEST, detail)
        }
        BuildRegenerationAiServiceError::InvalidTargetWordCountTooSmall => detail_error(
            StatusCode::BAD_REQUEST,
            "target_word_count must be greater than or equal to 500",
        ),
        BuildRegenerationAiServiceError::InvalidTargetWordCountTooLarge => detail_error(
            StatusCode::BAD_REQUEST,
            "target_word_count must be less than or equal to 10000",
        ),
        BuildRegenerationAiServiceError::InvalidCreativeMode => {
            detail_error(StatusCode::BAD_REQUEST, "creative_mode is invalid")
        }
        BuildRegenerationAiServiceError::InvalidStoryFocus => {
            detail_error(StatusCode::BAD_REQUEST, "story_focus is invalid")
        }
        BuildRegenerationAiServiceError::InvalidPlotStage => {
            detail_error(StatusCode::BAD_REQUEST, "plot_stage is invalid")
        }
        BuildRegenerationAiServiceError::InvalidQualityPreset => {
            detail_error(StatusCode::BAD_REQUEST, "quality_preset is invalid")
        }
        BuildRegenerationAiServiceError::StoryCreationBriefTooLong => detail_error(
            StatusCode::BAD_REQUEST,
            "story_creation_brief must be at most 1200 characters",
        ),
        BuildRegenerationAiServiceError::QualityNotesTooLong => detail_error(
            StatusCode::BAD_REQUEST,
            "quality_notes must be at most 600 characters",
        ),
        BuildRegenerationAiServiceError::WebResearchQueryTooLong => detail_error(
            StatusCode::BAD_REQUEST,
            "web_research_query must be at most 500 characters",
        ),
    }
}

pub fn map_prepare_chapter_regeneration_stream_error(
    error: BuildRegenerationAiServiceError,
) -> ChapterRouteError {
    map_build_regeneration_ai_service_error(error)
}

pub fn map_prepare_partial_regeneration_stream_error(
    error: PreparePartialRegenerationStreamError,
) -> ChapterRouteError {
    match error {
        PreparePartialRegenerationStreamError::Input(
            PreparePartialRegenerationError::InvalidRange,
        ) => detail_error(StatusCode::BAD_REQUEST, "改写位置非法"),
        PreparePartialRegenerationStreamError::Input(
            PreparePartialRegenerationError::EmptySelectedText,
        ) => detail_error(StatusCode::BAD_REQUEST, "选中内容为空"),
        PreparePartialRegenerationStreamError::Input(
            PreparePartialRegenerationError::EmptyUserInstructions,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "user_instructions must be at least 1 character",
        ),
        PreparePartialRegenerationStreamError::Input(
            PreparePartialRegenerationError::UserInstructionsTooLong,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "user_instructions must be at most 1000 characters",
        ),
        PreparePartialRegenerationStreamError::Input(
            PreparePartialRegenerationError::ContextCharsTooSmall,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "context_chars must be greater than or equal to 100",
        ),
        PreparePartialRegenerationStreamError::Input(
            PreparePartialRegenerationError::ContextCharsTooLarge,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "context_chars must be less than or equal to 2000",
        ),
        PreparePartialRegenerationStreamError::Input(
            PreparePartialRegenerationError::TargetWordCountTooSmall,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "target_word_count must be greater than or equal to 10",
        ),
        PreparePartialRegenerationStreamError::Input(
            PreparePartialRegenerationError::TargetWordCountTooLarge,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "target_word_count must be less than or equal to 5000",
        ),
        PreparePartialRegenerationStreamError::Input(
            PreparePartialRegenerationError::WebResearchQueryTooLong,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "web_research_query must be at most 500 characters",
        ),
        PreparePartialRegenerationStreamError::Style(detail) => {
            detail_error(StatusCode::BAD_REQUEST, detail)
        }
        PreparePartialRegenerationStreamError::Config(error) => {
            map_build_regeneration_ai_service_error(error)
        }
    }
}

pub fn map_create_chapter_regeneration_stream_workflow_error(
    error: CreateChapterRegenerationStreamWorkflowError,
) -> ChapterRouteError {
    map_create_regeneration_stream_workflow_error(
        error,
        map_prepare_chapter_regeneration_stream_error,
    )
}

pub fn map_create_partial_regeneration_stream_workflow_error(
    error: CreatePartialRegenerationStreamWorkflowError,
) -> ChapterRouteError {
    map_create_regeneration_stream_workflow_error(
        error,
        map_prepare_partial_regeneration_stream_error,
    )
}

fn map_create_regeneration_stream_workflow_error<TPrepareError>(
    error: CreateRegenerationStreamWorkflowError<TPrepareError>,
    prepare_error_mapper: impl FnOnce(TPrepareError) -> ChapterRouteError,
) -> ChapterRouteError {
    match error {
        CreateRegenerationStreamWorkflowError::Chapter(error) => {
            map_load_accessible_chapter_error(error)
        }
        CreateRegenerationStreamWorkflowError::Prepare(error) => prepare_error_mapper(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_build_regeneration_ai_service_error, map_chapter_query_payload_error,
        map_create_chapter_analysis_task_error, map_load_can_generate_payload_error,
        map_load_navigation_payload_error, map_prepare_chapter_analysis_trigger_error,
        map_prepare_chapter_regeneration_stream_error,
        map_prepare_partial_regeneration_stream_error, map_quality_trend_query_request_error,
        map_regeneration_tasks_query_request_error,
    };
    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_analysis_runtime_service::PrepareChapterAnalysisTriggerError;
    use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
    use crate::services::chapter_query_service::{
        ChapterQueryPayloadError, ChapterReadNotFound, QualityTrendQueryRequestError,
        ReadQueryPayloadError,
    };
    use crate::services::chapter_regeneration_prepare_service::{
        BuildRegenerationAiServiceError, PreparePartialRegenerationError,
        PreparePartialRegenerationStreamError,
    };
    use crate::services::chapter_regeneration_query_service::RegenerationTasksQueryRequestError;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn regeneration_ai_service_invalid_config_remains_bad_request() {
        let response = map_build_regeneration_ai_service_error(
            BuildRegenerationAiServiceError::InvalidConfig("missing provider".to_string()),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "missing provider" }));
    }

    #[test]
    fn chapter_query_payload_not_found_remains_404() {
        let response = map_chapter_query_payload_error(ReadQueryPayloadError::NotFound(
            ChapterReadNotFound::ChapterNotFound,
        ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn chapter_query_payload_internal_error_remains_500() {
        let response =
            map_chapter_query_payload_error(ChapterQueryPayloadError::Internal("boom".to_string()));

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "boom" }));
    }

    #[test]
    fn chapter_regeneration_stream_invalid_config_remains_bad_request() {
        let response = map_prepare_chapter_regeneration_stream_error(
            BuildRegenerationAiServiceError::InvalidConfig("missing provider".to_string()),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "missing provider" }));
    }

    #[test]
    fn chapter_regeneration_target_word_count_bounds_match_python_contract() {
        let lower_response = map_prepare_chapter_regeneration_stream_error(
            BuildRegenerationAiServiceError::InvalidTargetWordCountTooSmall,
        );
        let upper_response = map_prepare_chapter_regeneration_stream_error(
            BuildRegenerationAiServiceError::InvalidTargetWordCountTooLarge,
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
    fn chapter_regeneration_generation_choice_errors_remain_bad_request() {
        let cases = [
            (
                BuildRegenerationAiServiceError::InvalidCreativeMode,
                "creative_mode is invalid",
            ),
            (
                BuildRegenerationAiServiceError::InvalidStoryFocus,
                "story_focus is invalid",
            ),
            (
                BuildRegenerationAiServiceError::InvalidPlotStage,
                "plot_stage is invalid",
            ),
            (
                BuildRegenerationAiServiceError::InvalidQualityPreset,
                "quality_preset is invalid",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_prepare_chapter_regeneration_stream_error(error);

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn chapter_regeneration_generation_text_length_errors_remain_bad_request() {
        let cases = [
            (
                BuildRegenerationAiServiceError::StoryCreationBriefTooLong,
                "story_creation_brief must be at most 1200 characters",
            ),
            (
                BuildRegenerationAiServiceError::QualityNotesTooLong,
                "quality_notes must be at most 600 characters",
            ),
            (
                BuildRegenerationAiServiceError::WebResearchQueryTooLong,
                "web_research_query must be at most 500 characters",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_prepare_chapter_regeneration_stream_error(error);

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn regeneration_tasks_query_limit_errors_match_python_query_bounds() {
        let cases = [
            (
                RegenerationTasksQueryRequestError::LimitTooSmall,
                "limit must be greater than or equal to 1",
            ),
            (
                RegenerationTasksQueryRequestError::LimitTooLarge,
                "limit must be less than or equal to 50",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_regeneration_tasks_query_request_error(error);

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn quality_trend_query_limit_errors_match_python_query_bounds() {
        let cases = [
            (
                QualityTrendQueryRequestError::LimitTooSmall,
                "limit must be greater than or equal to 1",
            ),
            (
                QualityTrendQueryRequestError::LimitTooLarge,
                "limit must be less than or equal to 50",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_quality_trend_query_request_error(error);

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn chapter_analysis_trigger_chapter_access_denied_remains_404() {
        let response = map_prepare_chapter_analysis_trigger_error(
            PrepareChapterAnalysisTriggerError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn create_chapter_analysis_task_project_missing_remains_not_found() {
        let response =
            map_create_chapter_analysis_task_error(CreateChapterAnalysisTaskError::ProjectMissing);

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "项目不存在" }));
    }

    #[test]
    fn chapter_analysis_trigger_project_missing_remains_not_found() {
        let response =
            map_prepare_chapter_analysis_trigger_error(PrepareChapterAnalysisTriggerError::Create(
                CreateChapterAnalysisTaskError::ProjectMissing,
            ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "项目不存在" }));
    }

    #[test]
    fn navigation_query_not_found_remains_404() {
        let response = map_load_navigation_payload_error(ReadQueryPayloadError::NotFound(
            ChapterReadNotFound::ChapterNotFound,
        ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn can_generate_query_internal_error_remains_500() {
        let response = map_load_can_generate_payload_error(ChapterQueryPayloadError::Internal(
            "boom".to_string(),
        ));

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "boom" }));
    }

    #[test]
    fn partial_regeneration_input_range_error_remains_bad_request() {
        let response = map_prepare_partial_regeneration_stream_error(
            PreparePartialRegenerationStreamError::Input(
                PreparePartialRegenerationError::InvalidRange,
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "改写位置非法" }));
    }

    #[test]
    fn partial_regeneration_python_request_bound_errors_remain_bad_request() {
        let cases = [
            (
                PreparePartialRegenerationError::EmptyUserInstructions,
                "user_instructions must be at least 1 character",
            ),
            (
                PreparePartialRegenerationError::UserInstructionsTooLong,
                "user_instructions must be at most 1000 characters",
            ),
            (
                PreparePartialRegenerationError::ContextCharsTooSmall,
                "context_chars must be greater than or equal to 100",
            ),
            (
                PreparePartialRegenerationError::ContextCharsTooLarge,
                "context_chars must be less than or equal to 2000",
            ),
            (
                PreparePartialRegenerationError::TargetWordCountTooSmall,
                "target_word_count must be greater than or equal to 10",
            ),
            (
                PreparePartialRegenerationError::TargetWordCountTooLarge,
                "target_word_count must be less than or equal to 5000",
            ),
            (
                PreparePartialRegenerationError::WebResearchQueryTooLong,
                "web_research_query must be at most 500 characters",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_prepare_partial_regeneration_stream_error(
                PreparePartialRegenerationStreamError::Input(error),
            );

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn partial_regeneration_style_error_remains_bad_request() {
        let response = map_prepare_partial_regeneration_stream_error(
            PreparePartialRegenerationStreamError::Style("style missing".to_string()),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "style missing" }));
    }

    #[test]
    fn partial_regeneration_config_error_remains_bad_request() {
        let response = map_prepare_partial_regeneration_stream_error(
            PreparePartialRegenerationStreamError::Config(
                BuildRegenerationAiServiceError::InvalidConfig("config missing".to_string()),
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "config missing" }));
    }
}
