use axum::{http::StatusCode, response::Json};
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    chapter_not_found_or_access_denied_error, internal_detail_error,
};
use crate::services::chapter_regeneration_query_service::LoadRegenerationTasksPayloadError;

pub fn map_regeneration_tasks_query_error(
    error: LoadRegenerationTasksPayloadError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadRegenerationTasksPayloadError::NotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        LoadRegenerationTasksPayloadError::Internal(detail) => internal_detail_error(detail),
    }
}
