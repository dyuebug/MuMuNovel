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
    map_analysis_task_status_error, map_auto_revision_draft_apply_error,
    map_auto_revision_draft_load_error, map_candidate_draft_apply_error,
    map_candidate_draft_load_error,
};
use crate::api::chapter_analysis_query_error_mapper::{
    map_batch_analysis_task_status_query_error, map_chapter_analysis_view_error,
    map_chapter_quality_metrics_query_error,
};
use crate::api::chapters_error_mapper::map_prepare_chapter_analysis_trigger_error;
use crate::services::auth::Claims;
use crate::services::chapter_access_http_service::load_accessible_chapter_or_404;
use crate::services::chapter_analysis_draft_request_service::{
    parse_auto_revision_draft_apply_request, parse_auto_revision_draft_lookup_request,
    parse_candidate_draft_apply_request, parse_candidate_draft_lookup_request,
};
use crate::services::chapter_analysis_draft_service::{
    apply_auto_revision_draft_payload,
    apply_candidate_draft_payload, load_auto_revision_draft_payload,
    load_candidate_draft_payload,
};
use crate::services::chapter_analysis_quality_service::load_chapter_quality_metrics_payload;
use crate::services::chapter_analysis_query_service::{
    load_batch_analysis_task_status_payload,
    load_analysis_task_status_payload, load_chapter_analysis_view_payload,
};
use crate::services::chapter_analysis_runtime_service::execute_chapter_analysis_background;
use crate::services::chapter_analysis_trigger_service::prepare_chapter_analysis_trigger;

#[derive(Deserialize)]
struct BatchAnalysisStatusRequest {
    chapter_ids: Vec<String>,
}

async fn get_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let payload = load_chapter_analysis_view_payload(&db, &chapter)
        .await
        .map_err(map_chapter_analysis_view_error)?;
    Ok(Json(payload))
}

async fn get_chapter_quality_metrics(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let payload = load_chapter_quality_metrics_payload(&db, &chapter)
        .await
        .map_err(map_chapter_quality_metrics_query_error)?;
    Ok(Json(payload))
}

async fn get_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let request = parse_auto_revision_draft_lookup_request(&query);
    let history_id = request.history_id();
    let payload = load_auto_revision_draft_payload(&db, &chapter, history_id)
        .await
        .map_err(|error| map_auto_revision_draft_load_error(error, history_id.is_some()))?;
    Ok(Json(payload))
}

async fn apply_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let request = parse_auto_revision_draft_apply_request(&body);
    let history_id = request.history_id();
    let payload =
        apply_auto_revision_draft_payload(&db, &chapter, history_id, request.allow_stale)
            .await
            .map_err(|error| map_auto_revision_draft_apply_error(error, history_id.is_some()))?;
    Ok(Json(payload))
}

async fn get_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let request = parse_candidate_draft_lookup_request(&query);
    let attempt_id = request.attempt_id();
    let payload = load_candidate_draft_payload(&db, &chapter, attempt_id)
        .await
        .map_err(|error| map_candidate_draft_load_error(error, attempt_id.is_some()))?;
    Ok(Json(payload))
}

async fn apply_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let request = parse_candidate_draft_apply_request(&body);
    let attempt_id = request.attempt_id();
    let payload = apply_candidate_draft_payload(&db, &chapter, attempt_id, request.allow_stale)
        .await
        .map_err(|error| map_candidate_draft_apply_error(error, attempt_id.is_some()))?;
    Ok(Json(payload))
}

async fn get_analysis_task_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_analysis_task_status_payload(&db, &claims.sub, &chapter_id)
        .await
        .map_err(map_analysis_task_status_error)?;
    Ok(Json(payload))
}

async fn get_batch_analysis_task_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<BatchAnalysisStatusRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_batch_analysis_task_status_payload(&db, &claims.sub, body.chapter_ids)
        .await
        .map_err(map_batch_analysis_task_status_query_error)?;
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

    let db_for_task = db.clone();
    let user_id = claims.sub.clone();
    let chapter_id_for_task = prepared.chapter_id.clone();
    let task_id_for_task = prepared.task_id.clone();
    tokio::spawn(async move {
        execute_chapter_analysis_background(
            db_for_task,
            user_id,
            chapter_id_for_task,
            task_id_for_task,
        )
        .await;
    });

    Ok(Json(prepared.payload))
}

pub fn routes() -> Router {
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
        .route("/chapters/{chapter_id}/analyze", post(trigger_chapter_analysis))
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
