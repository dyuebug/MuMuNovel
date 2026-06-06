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
    build_auto_revision_draft_payload_request_from_route_payload,
    build_auto_revision_draft_payload_request_from_route_query,
    build_candidate_draft_payload_request_from_route_payload,
    build_candidate_draft_payload_request_from_route_query, load_owned_auto_revision_draft_payload,
    load_owned_candidate_draft_payload, AutoRevisionDraftApplyRouteRequest,
    AutoRevisionDraftLookupRouteQuery, CandidateDraftApplyRouteRequest,
    CandidateDraftLookupRouteQuery,
};
use crate::services::chapter_analysis_query_service::{
    build_batch_analysis_status_request_from_route_payload, load_analysis_task_status_payload,
    load_batch_analysis_task_status_payload, BatchAnalysisStatusRouteRequest,
};
use crate::services::chapter_analysis_runtime_service::trigger_chapter_analysis_write_workflow;
use crate::services::chapter_analysis_view_query_service::{
    load_owned_chapter_analysis_view_payload, ChapterAnalysisViewOptions,
};
use crate::services::chapter_quality_metrics_query_service::load_owned_chapter_quality_metrics_payload;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct ChapterAnalysisViewRouteQuery {
    #[serde(default)]
    include_full_draft: bool,
}
async fn get_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<ChapterAnalysisViewRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_owned_chapter_analysis_view_payload(
        &db,
        &chapter_id,
        &claims.sub,
        ChapterAnalysisViewOptions::new(query.include_full_draft),
    )
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
    let request = build_auto_revision_draft_payload_request_from_route_query(query);
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
    let request = build_auto_revision_draft_payload_request_from_route_payload(body);
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
    let request = build_candidate_draft_payload_request_from_route_query(query);
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
    let request = build_candidate_draft_payload_request_from_route_payload(body);
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
    let request = build_batch_analysis_status_request_from_route_payload(body);
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
        CandidateDraftLookupRouteQuery, ChapterAnalysisViewRouteQuery,
    };
    use crate::services::chapter_analysis_draft_service::{
        build_auto_revision_draft_payload_request_from_route_payload,
        build_auto_revision_draft_payload_request_from_route_query,
        build_candidate_draft_payload_request_from_route_payload,
        build_candidate_draft_payload_request_from_route_query, OwnedDraftPayloadRequest,
    };
    use crate::services::chapter_analysis_query_service::build_batch_analysis_status_request_from_route_payload;

    #[test]
    fn should_build_batch_analysis_status_request_from_route_payload() {
        let request = build_batch_analysis_status_request_from_route_payload(
            BatchAnalysisStatusRouteRequest {
                chapter_ids: vec![
                    " chapter-1 ".to_string(),
                    "".to_string(),
                    "chapter-2".to_string(),
                    "chapter-1".to_string(),
                    "   ".to_string(),
                ],
            },
        );

        assert_eq!(
            request,
            build_batch_analysis_status_request_from_route_payload(
                BatchAnalysisStatusRouteRequest {
                    chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
                }
            )
        );
    }

    #[test]
    fn should_parse_chapter_analysis_view_route_query_include_full_draft() {
        let query = ChapterAnalysisViewRouteQuery {
            include_full_draft: true,
        };
        let default_query = ChapterAnalysisViewRouteQuery::default();

        assert!(query.include_full_draft);
        assert!(!default_query.include_full_draft);
    }

    #[test]
    fn should_build_auto_revision_draft_payload_request_from_route_query() {
        let route_query = AutoRevisionDraftLookupRouteQuery {
            history_id: Some(" history-1 ".to_string()),
        };
        let request = build_auto_revision_draft_payload_request_from_route_query(route_query);

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
        let request = build_auto_revision_draft_payload_request_from_route_payload(route_request);

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
        let request = build_candidate_draft_payload_request_from_route_query(route_query);

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
        let request = build_candidate_draft_payload_request_from_route_payload(route_request);

        assert_eq!(request, OwnedDraftPayloadRequest::new(None, false));
    }
}
