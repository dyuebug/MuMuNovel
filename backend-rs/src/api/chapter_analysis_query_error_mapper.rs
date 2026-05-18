use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

pub type ChapterAnalysisQueryRouteError = (StatusCode, Json<Value>);

fn detail_error(detail: impl Into<String>) -> ChapterAnalysisQueryRouteError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "detail": detail.into() })),
    )
}

pub fn map_chapter_analysis_view_error(
    error: String,
) -> ChapterAnalysisQueryRouteError {
    let status = if error == "Chapter analysis not found" {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, Json(json!({ "detail": error })))
}

pub fn map_chapter_quality_metrics_query_error(
    error: String,
) -> ChapterAnalysisQueryRouteError {
    detail_error(error)
}

pub fn map_batch_analysis_task_status_query_error(
    error: String,
) -> ChapterAnalysisQueryRouteError {
    detail_error(error)
}
