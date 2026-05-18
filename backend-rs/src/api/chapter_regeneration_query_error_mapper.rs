use axum::{
    http::StatusCode,
    response::Json,
};
use serde_json::{json, Value};

pub fn map_regeneration_tasks_query_error(
    detail: String,
) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "detail": detail })),
    )
}
