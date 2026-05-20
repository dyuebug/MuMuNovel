use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

use crate::services::chapter_crud_workflow_service::{
    CreateChapterPayloadError, DeleteChapterPayloadError, GetChapterPayloadError,
    ListChaptersByProjectPathPayloadError, ListChaptersPayloadError, UpdateChapterPayloadError,
    UpdateExpansionPlanPayloadError,
};

pub type ChapterCrudRouteError = (StatusCode, Json<Value>);

fn success_message_error(status: StatusCode, message: impl Into<String>) -> ChapterCrudRouteError {
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

fn internal_success_message_error(detail: impl Into<String>) -> ChapterCrudRouteError {
    success_message_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
}

fn project_not_found_or_access_denied_message_error() -> ChapterCrudRouteError {
    success_message_error(StatusCode::NOT_FOUND, "Project not found or access denied")
}

fn chapter_not_found_or_access_denied_message_error() -> ChapterCrudRouteError {
    success_message_error(StatusCode::NOT_FOUND, "Chapter not found or access denied")
}

pub fn map_create_chapter_payload_error(error: CreateChapterPayloadError) -> ChapterCrudRouteError {
    match error {
        CreateChapterPayloadError::ProjectNotFound => {
            project_not_found_or_access_denied_message_error()
        }
        CreateChapterPayloadError::Internal(detail) => internal_success_message_error(detail),
    }
}

pub fn map_list_chapters_payload_error(error: ListChaptersPayloadError) -> ChapterCrudRouteError {
    match error {
        ListChaptersPayloadError::ProjectNotFound => {
            project_not_found_or_access_denied_message_error()
        }
        ListChaptersPayloadError::Internal(detail) => internal_success_message_error(detail),
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
        GetChapterPayloadError::ChapterNotFound => {
            chapter_not_found_or_access_denied_message_error()
        }
        GetChapterPayloadError::Internal(detail) => internal_success_message_error(detail),
    }
}

pub fn map_update_chapter_payload_error(error: UpdateChapterPayloadError) -> ChapterCrudRouteError {
    match error {
        UpdateChapterPayloadError::ChapterNotFound => {
            chapter_not_found_or_access_denied_message_error()
        }
        UpdateChapterPayloadError::Internal(detail) => internal_success_message_error(detail),
    }
}

pub fn map_delete_chapter_payload_error(error: DeleteChapterPayloadError) -> ChapterCrudRouteError {
    match error {
        DeleteChapterPayloadError::ChapterNotFound => {
            chapter_not_found_or_access_denied_message_error()
        }
        DeleteChapterPayloadError::Internal(detail) => internal_success_message_error(detail),
    }
}

pub fn map_update_expansion_plan_payload_error(
    error: UpdateExpansionPlanPayloadError,
) -> ChapterCrudRouteError {
    match error {
        UpdateExpansionPlanPayloadError::ChapterNotFound => {
            chapter_not_found_or_access_denied_message_error()
        }
        UpdateExpansionPlanPayloadError::Internal(detail) => internal_success_message_error(detail),
    }
}
