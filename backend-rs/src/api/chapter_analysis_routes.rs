use std::collections::HashMap;

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;

use crate::api::chapter_analysis_draft_error_mapper::{
    map_owned_auto_revision_draft_apply_error, map_owned_auto_revision_draft_load_error,
    map_owned_candidate_draft_apply_error, map_owned_candidate_draft_load_error,
};
use crate::api::chapter_analysis_query_error_mapper::{
    map_owned_chapter_analysis_view_error, map_owned_chapter_quality_metrics_query_error,
};
use crate::api::chapters_error_mapper::{
    internal_detail_error, map_load_analysis_task_status_error,
    map_prepare_chapter_analysis_trigger_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_analysis_draft_service::{
    apply_owned_auto_revision_draft_payload, apply_owned_candidate_draft_payload,
    load_owned_auto_revision_draft_payload, load_owned_candidate_draft_payload,
};
use crate::services::chapter_analysis_query_service::{
    load_analysis_task_status_payload, load_batch_analysis_task_status_payload,
    load_owned_chapter_analysis_view_payload, load_owned_chapter_quality_metrics_payload,
};
use crate::services::chapter_analysis_trigger_service::{
    dispatch_prepared_chapter_analysis_trigger, prepare_chapter_analysis_trigger,
};

#[derive(Deserialize)]
struct BatchAnalysisStatusRequest {
    chapter_ids: Vec<String>,
}

async fn get_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_owned_chapter_analysis_view_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_owned_chapter_analysis_view_error)?;
    Ok(Json(payload))
}

async fn get_chapter_quality_metrics(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_owned_chapter_quality_metrics_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_owned_chapter_quality_metrics_query_error)?;
    Ok(Json(payload))
}

async fn get_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_owned_auto_revision_draft_payload(&db, &chapter_id, &claims.sub, &query)
        .await
        .map_err(map_owned_auto_revision_draft_load_error)?;
    Ok(Json(payload))
}

async fn apply_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = apply_owned_auto_revision_draft_payload(&db, &chapter_id, &claims.sub, &body)
        .await
        .map_err(map_owned_auto_revision_draft_apply_error)?;
    Ok(Json(payload))
}

async fn get_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_owned_candidate_draft_payload(&db, &chapter_id, &claims.sub, &query)
        .await
        .map_err(map_owned_candidate_draft_load_error)?;
    Ok(Json(payload))
}

async fn apply_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = apply_owned_candidate_draft_payload(&db, &chapter_id, &claims.sub, &body)
        .await
        .map_err(map_owned_candidate_draft_apply_error)?;
    Ok(Json(payload))
}

async fn get_analysis_task_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_analysis_task_status_payload(&db, &claims.sub, &chapter_id)
        .await
        .map_err(map_load_analysis_task_status_error)?;
    Ok(Json(payload))
}

async fn get_batch_analysis_task_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<BatchAnalysisStatusRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_batch_analysis_task_status_payload(&db, &claims.sub, body.chapter_ids)
        .await
        .map_err(internal_detail_error)?;
    Ok(Json(payload))
}

async fn trigger_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let prepared = prepare_chapter_analysis_trigger(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_prepare_chapter_analysis_trigger_error)?;

    dispatch_prepared_chapter_analysis_trigger(db.clone(), claims.sub.clone(), &prepared);

    Ok(Json(prepared.payload))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(
            "/chapters/{chapter_id}/quality-metrics",
            get(get_chapter_quality_metrics),
        )
        .route("/chapters/{chapter_id}/analysis", get(get_chapter_analysis))
        .route(
            "/chapters/{chapter_id}/analysis/status",
            get(get_analysis_task_status),
        )
        .route(
            "/chapters/analysis/status/batch",
            post(get_batch_analysis_task_status),
        )
        .route(
            "/chapters/{chapter_id}/analyze",
            post(trigger_chapter_analysis),
        )
        .route(
            "/chapters/{chapter_id}/analysis/auto-revision-draft",
            get(get_auto_revision_draft),
        )
        .route(
            "/chapters/{chapter_id}/analysis/auto-revision-draft/apply",
            post(apply_auto_revision_draft),
        )
        .route(
            "/chapters/{chapter_id}/analysis/candidate-draft",
            get(get_candidate_draft),
        )
        .route(
            "/chapters/{chapter_id}/analysis/candidate-draft/apply",
            post(apply_candidate_draft),
        )
}
