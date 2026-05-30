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
    map_auto_revision_draft_apply_error, map_auto_revision_draft_load_error,
    map_candidate_draft_apply_error, map_candidate_draft_load_error,
    map_owned_auto_revision_draft_error, map_owned_candidate_draft_error,
};
use crate::api::chapter_analysis_query_error_mapper::{
    map_chapter_analysis_query_context_error, map_owned_chapter_analysis_view_error,
};
use crate::api::chapters_error_mapper::{
    internal_detail_error, map_prepare_chapter_analysis_trigger_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_analysis_draft_service::{
    apply_owned_auto_revision_draft_payload, apply_owned_candidate_draft_payload,
    load_owned_auto_revision_draft_payload, load_owned_candidate_draft_payload,
    OwnedDraftPayloadRequest,
};
use crate::services::chapter_analysis_query_service::{
    load_analysis_task_status_payload, load_batch_analysis_task_status_payload,
    BatchAnalysisStatusRequest,
};
use crate::services::chapter_analysis_runtime_service::trigger_chapter_analysis_write_workflow;
use crate::services::chapter_analysis_view_query_service::load_owned_chapter_analysis_view_payload;
use crate::services::chapter_quality_metrics_query_service::load_owned_chapter_quality_metrics_payload;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct BatchAnalysisStatusRouteRequest {
    pub(crate) chapter_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct AutoRevisionDraftLookupRouteQuery {
    history_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct CandidateDraftLookupRouteQuery {
    attempt_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct AutoRevisionDraftApplyRouteRequest {
    history_id: Option<String>,
    #[serde(default)]
    allow_stale: bool,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct CandidateDraftApplyRouteRequest {
    attempt_id: Option<String>,
    #[serde(default)]
    allow_stale: bool,
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
        .map_err(map_chapter_analysis_query_context_error)?;
    Ok(Json(payload))
}

async fn get_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<AutoRevisionDraftLookupRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = OwnedDraftPayloadRequest::from_route_selector(query.history_id, false);
    let payload = load_owned_auto_revision_draft_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(|error| {
            map_owned_auto_revision_draft_error(error, map_auto_revision_draft_load_error)
        })?;
    Ok(Json(payload))
}

async fn apply_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<AutoRevisionDraftApplyRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = OwnedDraftPayloadRequest::from_route_selector(body.history_id, body.allow_stale);
    let payload = apply_owned_auto_revision_draft_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(|error| {
            map_owned_auto_revision_draft_error(error, map_auto_revision_draft_apply_error)
        })?;
    Ok(Json(payload))
}

async fn get_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<CandidateDraftLookupRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = OwnedDraftPayloadRequest::from_route_selector(query.attempt_id, false);
    let payload = load_owned_candidate_draft_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(|error| map_owned_candidate_draft_error(error, map_candidate_draft_load_error))?;
    Ok(Json(payload))
}

async fn apply_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<CandidateDraftApplyRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = OwnedDraftPayloadRequest::from_route_selector(body.attempt_id, body.allow_stale);
    let payload = apply_owned_candidate_draft_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(|error| map_owned_candidate_draft_error(error, map_candidate_draft_apply_error))?;
    Ok(Json(payload))
}

async fn get_analysis_task_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_analysis_task_status_payload(&db, &claims.sub, &chapter_id)
        .await
        .map_err(map_chapter_analysis_query_context_error)?;
    Ok(Json(payload))
}

async fn get_batch_analysis_task_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<BatchAnalysisStatusRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = BatchAnalysisStatusRequest::from_route_chapter_ids(body.chapter_ids);
    let payload = load_batch_analysis_task_status_payload(&db, &claims.sub, request)
        .await
        .map_err(internal_detail_error)?;
    Ok(Json(payload))
}

async fn trigger_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = trigger_chapter_analysis_write_workflow(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_prepare_chapter_analysis_trigger_error)?;
    Ok(Json(payload))
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

#[cfg(test)]
mod tests {
    use super::{
        AutoRevisionDraftApplyRouteRequest, AutoRevisionDraftLookupRouteQuery,
        BatchAnalysisStatusRouteRequest, CandidateDraftApplyRouteRequest,
        CandidateDraftLookupRouteQuery,
    };
    use crate::services::chapter_analysis_query_service::BatchAnalysisStatusRequest;
    use crate::services::chapter_analysis_draft_service::OwnedDraftPayloadRequest;

    #[test]
    fn should_build_batch_analysis_status_request_from_route_payload() {
        let request = BatchAnalysisStatusRequest::from_route_chapter_ids(
            BatchAnalysisStatusRouteRequest {
            chapter_ids: vec![
                " chapter-1 ".to_string(),
                "".to_string(),
                "chapter-2".to_string(),
                "chapter-1".to_string(),
                "   ".to_string(),
            ],
        }
        .chapter_ids,
        );

        assert_eq!(
            request,
            BatchAnalysisStatusRequest::from_route_chapter_ids(vec![
                "chapter-1".to_string(),
                "chapter-2".to_string(),
            ])
        );
    }

    #[test]
    fn should_build_auto_revision_draft_payload_request_from_route_query() {
        let route_query = AutoRevisionDraftLookupRouteQuery {
            history_id: Some(" history-1 ".to_string()),
        };
        let request = OwnedDraftPayloadRequest::from_route_selector(route_query.history_id, false);

        assert_eq!(
            request,
            OwnedDraftPayloadRequest::new(Some("history-1"), false)
        );
    }

    #[test]
    fn should_build_auto_revision_draft_payload_request_from_route_body() {
        let route_request = AutoRevisionDraftApplyRouteRequest {
            history_id: Some(" history-1 ".to_string()),
            allow_stale: true,
        };
        let request = OwnedDraftPayloadRequest::from_route_selector(
            route_request.history_id,
            route_request.allow_stale,
        );

        assert_eq!(
            request,
            OwnedDraftPayloadRequest::new(Some("history-1"), true)
        );
    }

    #[test]
    fn should_build_candidate_draft_payload_request_from_route_query() {
        let route_query = CandidateDraftLookupRouteQuery {
            attempt_id: Some(" attempt-1 ".to_string()),
        };
        let request = OwnedDraftPayloadRequest::from_route_selector(route_query.attempt_id, false);

        assert_eq!(
            request,
            OwnedDraftPayloadRequest::new(Some("attempt-1"), false)
        );
    }

    #[test]
    fn should_build_candidate_draft_payload_request_from_route_body() {
        let route_request = CandidateDraftApplyRouteRequest {
            attempt_id: Some("   ".to_string()),
            allow_stale: false,
        };
        let request = OwnedDraftPayloadRequest::from_route_selector(
            route_request.attempt_id,
            route_request.allow_stale,
        );

        assert_eq!(request, OwnedDraftPayloadRequest::new(None, false));
    }
}
