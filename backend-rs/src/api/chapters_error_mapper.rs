use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

use crate::services::chapter_access_service::LoadAccessibleChapterError;
use crate::services::chapter_analysis_service::LoadAnalysisTaskStatusError;
use crate::services::chapter_analysis_trigger_service::PrepareChapterAnalysisTriggerError;
use crate::services::chapter_annotation_query_service::LoadAnnotationsPayloadError;
use crate::services::chapter_quality_query_service::LoadQualityTrendPayloadError;
use crate::services::chapter_query_service::{
    LoadCanGeneratePayloadError, LoadNavigationPayloadError,
};
use crate::services::chapter_regeneration_apply_service::ApplyPartialRegenerateError;
use crate::services::chapter_regeneration_prepare_service::{
    PrepareChapterRegenerationStreamError, PreparePartialRegenerationStreamError,
};
use crate::services::chapter_regeneration_stream_workflow_service::{
    CreateChapterRegenerationStreamWorkflowError, CreatePartialRegenerationStreamWorkflowError,
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

pub fn map_prepare_chapter_analysis_trigger_error(
    error: PrepareChapterAnalysisTriggerError,
) -> ChapterRouteError {
    match error {
        PrepareChapterAnalysisTriggerError::ChapterNotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        PrepareChapterAnalysisTriggerError::ChapterEmpty => {
            detail_error(StatusCode::BAD_REQUEST, "章节不存在或内容为空")
        }
        PrepareChapterAnalysisTriggerError::ProjectMissing => {
            detail_error(StatusCode::NOT_FOUND, "项目不存在")
        }
        PrepareChapterAnalysisTriggerError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_load_analysis_task_status_error(
    error: LoadAnalysisTaskStatusError,
) -> ChapterRouteError {
    match error {
        LoadAnalysisTaskStatusError::ChapterNotFound => chapter_not_found_or_access_denied_error(),
        LoadAnalysisTaskStatusError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_load_navigation_payload_error(error: LoadNavigationPayloadError) -> ChapterRouteError {
    match error {
        LoadNavigationPayloadError::NotFound => chapter_not_found_or_access_denied_error(),
        LoadNavigationPayloadError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_load_annotations_payload_error(error: LoadAnnotationsPayloadError) -> ChapterRouteError {
    match error {
        LoadAnnotationsPayloadError::NotFound => chapter_not_found_or_access_denied_error(),
        LoadAnnotationsPayloadError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_load_quality_trend_payload_error(
    error: LoadQualityTrendPayloadError,
) -> ChapterRouteError {
    match error {
        LoadQualityTrendPayloadError::NotFound => project_not_found_or_access_denied_error(),
        LoadQualityTrendPayloadError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_load_can_generate_payload_error(
    error: LoadCanGeneratePayloadError,
) -> ChapterRouteError {
    match error {
        LoadCanGeneratePayloadError::NotFound => chapter_not_found_or_access_denied_error(),
        LoadCanGeneratePayloadError::Internal(detail) => internal_detail_error(detail),
    }
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
        ApplyPartialRegenerateError::NotFound => chapter_not_found_or_access_denied_error(),
        ApplyPartialRegenerateError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_prepare_chapter_regeneration_stream_error(
    error: PrepareChapterRegenerationStreamError,
) -> ChapterRouteError {
    match error {
        PrepareChapterRegenerationStreamError::InvalidConfig(detail) => {
            detail_error(StatusCode::BAD_REQUEST, detail)
        }
    }
}

pub fn map_prepare_partial_regeneration_stream_error(
    error: PreparePartialRegenerationStreamError,
) -> ChapterRouteError {
    match error {
        PreparePartialRegenerationStreamError::InvalidRange => {
            detail_error(StatusCode::BAD_REQUEST, "改写位置非法")
        }
        PreparePartialRegenerationStreamError::EmptySelectedText => {
            detail_error(StatusCode::BAD_REQUEST, "选中内容为空")
        }
        PreparePartialRegenerationStreamError::InvalidStyle(detail) => {
            detail_error(StatusCode::BAD_REQUEST, detail)
        }
        PreparePartialRegenerationStreamError::InvalidConfig(detail) => {
            detail_error(StatusCode::BAD_REQUEST, detail)
        }
    }
}

pub fn map_create_chapter_regeneration_stream_workflow_error(
    error: CreateChapterRegenerationStreamWorkflowError,
) -> ChapterRouteError {
    match error {
        CreateChapterRegenerationStreamWorkflowError::Chapter(error) => {
            map_load_accessible_chapter_error(error)
        }
        CreateChapterRegenerationStreamWorkflowError::Prepare(error) => {
            map_prepare_chapter_regeneration_stream_error(error)
        }
    }
}

pub fn map_create_partial_regeneration_stream_workflow_error(
    error: CreatePartialRegenerationStreamWorkflowError,
) -> ChapterRouteError {
    match error {
        CreatePartialRegenerationStreamWorkflowError::Chapter(error) => {
            map_load_accessible_chapter_error(error)
        }
        CreatePartialRegenerationStreamWorkflowError::Prepare(error) => {
            map_prepare_partial_regeneration_stream_error(error)
        }
    }
}
