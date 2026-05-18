use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

use crate::services::chapter_crud_service::{
    CreateChapterPayloadError, DeleteChapterPayloadError, GetChapterPayloadError,
    ListChaptersByProjectPathPayloadError, ListChaptersPayloadError,
    UpdateChapterPayloadError, UpdateExpansionPlanPayloadError,
};

pub type ChapterCrudRouteError = (StatusCode, Json<Value>);

fn success_message_error(
    status: StatusCode,
    message: impl Into<String>,
) -> ChapterCrudRouteError {
    (
        status,
        Json(json!({
            "success": false,
            "message": message.into(),
        })),
    )
}

fn detail_error(status: StatusCode, detail: impl Into<String>) -> ChapterCrudRouteError {
    (status, Json(json!({ "detail": detail.into() })))
}

pub fn map_create_chapter_payload_error(
    error: CreateChapterPayloadError,
) -> ChapterCrudRouteError {
    match error {
        CreateChapterPayloadError::ProjectNotFound => success_message_error(
            StatusCode::NOT_FOUND,
            "Project not found or access denied",
        ),
        CreateChapterPayloadError::Internal(detail) => {
            success_message_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
        }
    }
}

pub fn map_list_chapters_payload_error(
    error: ListChaptersPayloadError,
) -> ChapterCrudRouteError {
    match error {
        ListChaptersPayloadError::ProjectNotFound => success_message_error(
            StatusCode::NOT_FOUND,
            "Project not found or access denied",
        ),
        ListChaptersPayloadError::Internal(detail) => {
            success_message_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
        }
    }
}

pub fn map_list_chapters_by_project_path_payload_error(
    error: ListChaptersByProjectPathPayloadError,
) -> ChapterCrudRouteError {
    match error {
        ListChaptersByProjectPathPayloadError::ProjectNotFound => {
            detail_error(StatusCode::NOT_FOUND, "Project not found")
        }
        ListChaptersByProjectPathPayloadError::Internal(detail) => {
            detail_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
        }
    }
}

pub fn map_get_chapter_payload_error(error: GetChapterPayloadError) -> ChapterCrudRouteError {
    match error {
        GetChapterPayloadError::ChapterNotFound => success_message_error(
            StatusCode::NOT_FOUND,
            "Chapter not found or access denied",
        ),
        GetChapterPayloadError::Internal(detail) => {
            success_message_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
        }
    }
}

pub fn map_update_chapter_payload_error(
    error: UpdateChapterPayloadError,
) -> ChapterCrudRouteError {
    match error {
        UpdateChapterPayloadError::ChapterNotFound => success_message_error(
            StatusCode::NOT_FOUND,
            "Chapter not found or access denied",
        ),
        UpdateChapterPayloadError::Internal(detail) => {
            success_message_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
        }
    }
}

pub fn map_delete_chapter_payload_error(
    error: DeleteChapterPayloadError,
) -> ChapterCrudRouteError {
    match error {
        DeleteChapterPayloadError::ChapterNotFound => success_message_error(
            StatusCode::NOT_FOUND,
            "Chapter not found or access denied",
        ),
        DeleteChapterPayloadError::Internal(detail) => {
            success_message_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
        }
    }
}

pub fn map_update_expansion_plan_payload_error(
    error: UpdateExpansionPlanPayloadError,
) -> ChapterCrudRouteError {
    match error {
        UpdateExpansionPlanPayloadError::ChapterNotFound => success_message_error(
            StatusCode::NOT_FOUND,
            "Chapter not found or access denied",
        ),
        UpdateExpansionPlanPayloadError::Internal(detail) => {
            success_message_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
        }
    }
}
