use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
#[cfg(test)]
use serde_json::json;
use serde_json::Value;

use self::error_mapper::{
    map_chapter_analysis_query_context_error, map_owned_chapter_analysis_view_error,
};
use crate::api::chapters_error_mapper::{
    internal_detail_error, map_prepare_chapter_analysis_trigger_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_analysis_runtime_service::query_owner::{
    build_batch_analysis_status_request_from_route_payload, load_analysis_task_status_payload,
    load_batch_analysis_task_status_payload, load_owned_chapter_analysis_view_payload,
    BatchAnalysisStatusRouteRequest, ChapterAnalysisViewOptions,
};
use crate::services::chapter_analysis_runtime_service::trigger_chapter_analysis_write_workflow;
use crate::services::chapter_quality_metrics_query_service::load_owned_chapter_quality_metrics_payload;

const CHAPTER_QUALITY_METRICS_ROUTE: &str = "/chapters/{chapter_id}/quality-metrics";
const CHAPTER_ANALYSIS_VIEW_ROUTE: &str = "/chapters/{chapter_id}/analysis";
const CHAPTER_ANALYSIS_STATUS_ROUTE: &str = "/chapters/{chapter_id}/analysis/status";
const CHAPTER_BATCH_ANALYSIS_STATUS_ROUTE: &str = "/chapters/analysis/status/batch";
const CHAPTER_ANALYZE_ROUTE: &str = "/chapters/{chapter_id}/analyze";

#[cfg(test)]
fn build_chapter_analysis_route_owner_contract() -> Value {
    json!({
        "owner": "chapter_analysis_routes",
        "rust_owner": "backend-rs/src/api/chapter_analysis_routes.rs",
        "route_prefix": "/api",
        "routes": {
            "quality_metrics": CHAPTER_QUALITY_METRICS_ROUTE,
            "analysis_view": CHAPTER_ANALYSIS_VIEW_ROUTE,
            "analysis_status": CHAPTER_ANALYSIS_STATUS_ROUTE,
            "batch_analysis_status": CHAPTER_BATCH_ANALYSIS_STATUS_ROUTE,
            "trigger_analysis": CHAPTER_ANALYZE_ROUTE
        },
        "methods": {
            "quality_metrics": ["GET"],
            "analysis_view": ["GET"],
            "analysis_status": ["GET"],
            "batch_analysis_status": ["POST"],
            "trigger_analysis": ["POST"]
        },
        "service_handoffs": {
            "analysis_view_owner": "backend-rs/src/services/chapter_analysis_runtime_service/query_owner.rs",
            "analysis_query_owner": "backend-rs/src/services/chapter_analysis_runtime_service/query_owner.rs",
            "quality_metrics_owner": "backend-rs/src/services/chapter_quality_metrics_query_service.rs",
            "runtime_write_owner": "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "error_mapping": "private error_mapper module in backend-rs/src/api/chapter_analysis_routes.rs",
            "shared_trigger_error_mapping": "backend-rs/src/api/chapters_error_mapper.rs"
        },
        "request_contract": {
            "analysis_view": "include_full_draft defaults to false and controls draft payload expansion",
            "batch_analysis_status": "chapter_ids are trimmed, blank values are dropped, and duplicate ids are collapsed",
            "trigger_analysis": "chapter_id path plus authenticated user starts the analysis write workflow"
        },
        "readiness_evidence": [
            "chapters-analysis-auth-guard-rust",
            "chapters-batch-analysis-status-auth-guard-rust",
            "chapter-analysis-view-logged-in-not-found-rust",
            "chapter-analysis-quality-metrics-logged-in-not-found-rust",
            "chapter-analysis-status-logged-in-not-found-rust",
            "chapter-analysis-trigger-logged-in-not-found-rust",
            "chapter-analysis-view-business-rust",
            "chapter-analysis-quality-metrics-business-rust",
            "chapter-analysis-status-business-rust",
            "chapter-analysis-batch-status-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-chapter-analysis-owner",
            "business_probes": [
                "chapter-analysis-view-logged-in-not-found-rust",
                "chapter-analysis-quality-metrics-logged-in-not-found-rust",
                "chapter-analysis-status-logged-in-not-found-rust",
                "chapter-analysis-trigger-logged-in-not-found-rust",
                "chapter-analysis-view-business-rust",
                "chapter-analysis-quality-metrics-business-rust",
                "chapter-analysis-status-business-rust",
                "chapter-analysis-batch-status-business-rust"
            ],
            "fixture_probes": [
                "chapter-analysis-fixture-import-project-business-rust",
                "chapter-analysis-fixture-list-chapter-business-rust"
            ],
            "route_readiness_probes": [
                "chapters-analysis-auth-guard-rust",
                "chapters-batch-analysis-status-auth-guard-rust"
            ],
            "python_fallback_probe_count": 0,
            "manifest_profile": "phase5-chapter-analysis-owner",
            "profile_kind": "successful_result_business_readiness"
        },
        "source_map_files": [
            "backend/app/api/chapters.py",
            "backend/app/api/chapter_analysis_routes.py",
            "backend/app/api/chapter_analysis_task_routes.py",
            "backend/app/services/manual_chapter_analysis_service.py",
            "backend/app/services/manual_chapter_analysis_execution_service.py",
            "backend/app/services/chapter_analysis_support_service.py",
            "backend/app/services/chapter_analysis_response_service.py"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_chapter_analysis_route_service_files_as_source_map_until_explicit_freeze_delete_round",
            "python_route_files_status": "source_map_only_for_chapter_analysis_route_group",
            "source_map_freeze_status": "frozen_source_map_rollback_only",
            "source_map_physical_closeout_action": "repoint",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit delete approval for the repointed source-map shell"
            ],
            "freeze_reason": "Rust chapter_analysis route owner covers the analysis view/status/batch-status/quality-metrics/trigger route handlers, runtime query owner, private error mapper, auth-guard manifest probes, logged-in not-found probes, and successful analysis view / quality metrics / status / batch-status business probes; the Python route shells are repointed as rollback/source-map-only material.",
            "rollback_files": [
                "backend/app/api/chapters.py",
                "backend/app/api/chapter_analysis_routes.py",
                "backend/app/api/chapter_analysis_task_routes.py",
                "backend/app/services/manual_chapter_analysis_service.py",
                "backend/app/services/manual_chapter_analysis_execution_service.py",
                "backend/app/services/chapter_analysis_support_service.py",
                "backend/app/services/chapter_analysis_response_service.py"
            ]
        },
        "business_smoke_status": {
            "owner_profile": "phase5-chapter-analysis-owner",
            "owner_profile_probe_count": 10,
            "business_probe_count": 8,
            "fixture_probe_count": 2,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "source-map has been repointed to the Rust route owner; final physical deletion still requires a separate same-round approval and rollback policy",
        "migration_policy": "Chapter analysis route business smoke is covered by phase5-chapter-analysis-owner; the Python route shells have been repointed to rollback/source-map-only status, and final physical deletion still requires a separate same-round approval."
    })
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct ChapterAnalysisViewRouteQuery {
    #[serde(default)]
    include_full_draft: bool,
}

mod error_mapper {
    use axum::{http::StatusCode, Json};
    use serde_json::Value;

    use crate::api::chapters_error_mapper::{detail_error, map_load_accessible_chapter_error};
    use crate::services::chapter_analysis_runtime_service::query_owner::LoadChapterAnalysisViewPayloadError;
    use crate::services::chapter_analysis_service::ChapterAnalysisQueryContextError;

    type ChapterAnalysisQueryRouteError = (StatusCode, Json<Value>);

    pub(super) fn map_chapter_analysis_query_context_error(
        error: ChapterAnalysisQueryContextError,
    ) -> ChapterAnalysisQueryRouteError {
        match error {
            ChapterAnalysisQueryContextError::Chapter(error) => {
                map_load_accessible_chapter_error(error)
            }
            ChapterAnalysisQueryContextError::Internal(error) => {
                detail_error(StatusCode::INTERNAL_SERVER_ERROR, error)
            }
        }
    }

    pub(super) fn map_owned_chapter_analysis_view_error(
        error: LoadChapterAnalysisViewPayloadError,
    ) -> ChapterAnalysisQueryRouteError {
        match error {
            LoadChapterAnalysisViewPayloadError::Context(error) => {
                map_chapter_analysis_query_context_error(error)
            }
            LoadChapterAnalysisViewPayloadError::AnalysisNotFound => {
                detail_error(StatusCode::NOT_FOUND, "Chapter analysis not found")
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            map_chapter_analysis_query_context_error, map_owned_chapter_analysis_view_error,
        };
        use crate::services::chapter_access_service::LoadAccessibleChapterError;
        use crate::services::chapter_analysis_runtime_service::query_owner::LoadChapterAnalysisViewPayloadError;
        use crate::services::chapter_analysis_service::ChapterAnalysisQueryContextError;
        use crate::services::chapter_quality_metrics_query_service::LoadChapterQualityMetricsPayloadError;
        use axum::http::StatusCode;
        use serde_json::json;

        #[test]
        fn owned_analysis_query_context_not_found_or_access_denied_remains_404() {
            let response = map_chapter_analysis_query_context_error(
                ChapterAnalysisQueryContextError::Chapter(
                    LoadAccessibleChapterError::NotFoundOrAccessDenied,
                ),
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(
                response.1 .0,
                json!({ "detail": "Chapter not found or access denied" })
            );
        }

        #[test]
        fn owned_analysis_query_context_internal_error_remains_500() {
            let response = map_chapter_analysis_query_context_error(
                ChapterAnalysisQueryContextError::Internal("database exploded".to_string()),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "database exploded" }));
        }

        #[test]
        fn owned_analysis_view_not_found_or_access_denied_remains_404() {
            let response = map_owned_chapter_analysis_view_error(
                LoadChapterAnalysisViewPayloadError::Context(
                    ChapterAnalysisQueryContextError::Chapter(
                        LoadAccessibleChapterError::NotFoundOrAccessDenied,
                    ),
                ),
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(
                response.1 .0,
                json!({ "detail": "Chapter not found or access denied" })
            );
        }

        #[test]
        fn owned_analysis_view_analysis_not_found_maps_to_404() {
            let response = map_owned_chapter_analysis_view_error(
                LoadChapterAnalysisViewPayloadError::AnalysisNotFound,
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(
                response.1 .0,
                json!({ "detail": "Chapter analysis not found" })
            );
        }

        #[test]
        fn owned_analysis_view_internal_error_remains_500() {
            let response = map_owned_chapter_analysis_view_error(
                LoadChapterAnalysisViewPayloadError::Context(
                    ChapterAnalysisQueryContextError::Internal("database exploded".to_string()),
                ),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "database exploded" }));
        }

        #[test]
        fn owned_quality_metrics_internal_error_uses_internal_detail() {
            let response = map_chapter_analysis_query_context_error(
                LoadChapterQualityMetricsPayloadError::Internal("database exploded".to_string()),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "database exploded" }));
        }

        #[test]
        fn owned_quality_metrics_not_found_or_access_denied_reuses_shared_context_mapping() {
            let response = map_chapter_analysis_query_context_error(
                LoadChapterQualityMetricsPayloadError::Chapter(
                    LoadAccessibleChapterError::NotFoundOrAccessDenied,
                ),
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(
                response.1 .0,
                json!({ "detail": "Chapter not found or access denied" })
            );
        }
    }
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
            CHAPTER_QUALITY_METRICS_ROUTE,
            get(get_chapter_quality_metrics),
        )
        .route(CHAPTER_ANALYSIS_VIEW_ROUTE, get(get_chapter_analysis))
        .route(CHAPTER_ANALYSIS_STATUS_ROUTE, get(get_analysis_task_status))
        .route(
            CHAPTER_BATCH_ANALYSIS_STATUS_ROUTE,
            post(get_batch_analysis_task_status),
        )
        .route(CHAPTER_ANALYZE_ROUTE, post(trigger_chapter_analysis))
}

#[cfg(test)]
mod tests {
    use super::{
        build_chapter_analysis_route_owner_contract, BatchAnalysisStatusRouteRequest,
        ChapterAnalysisViewRouteQuery, CHAPTER_ANALYSIS_STATUS_ROUTE, CHAPTER_ANALYSIS_VIEW_ROUTE,
        CHAPTER_ANALYZE_ROUTE, CHAPTER_BATCH_ANALYSIS_STATUS_ROUTE, CHAPTER_QUALITY_METRICS_ROUTE,
    };
    use crate::services::chapter_analysis_runtime_service::query_owner::build_batch_analysis_status_request_from_route_payload;
    use serde_json::json;

    #[test]
    fn should_publish_chapter_analysis_route_owner_contract() {
        let contract = build_chapter_analysis_route_owner_contract();

        assert_eq!(contract["owner"], "chapter_analysis_routes");
        assert_eq!(
            contract["rust_owner"],
            "backend-rs/src/api/chapter_analysis_routes.rs"
        );
        assert_eq!(
            contract["routes"]["quality_metrics"],
            CHAPTER_QUALITY_METRICS_ROUTE
        );
        assert_eq!(
            contract["routes"]["analysis_view"],
            CHAPTER_ANALYSIS_VIEW_ROUTE
        );
        assert_eq!(
            contract["routes"]["batch_analysis_status"],
            CHAPTER_BATCH_ANALYSIS_STATUS_ROUTE
        );
        assert_eq!(
            contract["service_handoffs"]["error_mapping"],
            "private error_mapper module in backend-rs/src/api/chapter_analysis_routes.rs"
        );
        assert_eq!(
            contract["readiness_evidence"][1],
            "chapters-batch-analysis-status-auth-guard-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-chapter-analysis-owner"
        );
        assert_eq!(
            contract["readiness_evidence"][2],
            "chapter-analysis-view-logged-in-not-found-rust"
        );
        assert_eq!(
            contract["owner_profile"]["route_readiness_probes"][1],
            "chapters-batch-analysis-status-auth-guard-rust"
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be an array")
            .contains(&json!("chapter-analysis-trigger-logged-in-not-found-rust")));
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be an array")
            .contains(&json!("chapter-analysis-view-business-rust")));
        assert_eq!(
            contract["owner_profile"]["fixture_probes"][0],
            "chapter-analysis-fixture-import-project-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["manifest_profile"],
            "phase5-chapter-analysis-owner"
        );
        assert_eq!(
            contract["owner_profile"]["profile_kind"],
            "successful_result_business_readiness"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_status"],
            "frozen_source_map_rollback_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_physical_closeout_action"],
            "repoint"
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"][0],
            "explicit delete approval for the repointed source-map shell"
        );
        assert_eq!(
            contract["source_map_files"][0],
            "backend/app/api/chapters.py"
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["owner_profile_probe_count"],
            json!(10)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(8)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(2)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "source-map has been repointed to the Rust route owner; final physical deletion still requires a separate same-round approval and rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("business smoke is covered"));
        assert!(!contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("Do not count python-fallback = 0 as completion"));
        assert!(!contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("business smoke boundary"));
    }

    #[test]
    fn should_keep_chapter_analysis_route_paths_stable() {
        assert_eq!(
            json!({
                "quality_metrics": CHAPTER_QUALITY_METRICS_ROUTE,
                "analysis_view": CHAPTER_ANALYSIS_VIEW_ROUTE,
                "analysis_status": CHAPTER_ANALYSIS_STATUS_ROUTE,
                "batch_analysis_status": CHAPTER_BATCH_ANALYSIS_STATUS_ROUTE,
                "trigger_analysis": CHAPTER_ANALYZE_ROUTE
            }),
            json!({
                "quality_metrics": "/chapters/{chapter_id}/quality-metrics",
                "analysis_view": "/chapters/{chapter_id}/analysis",
                "analysis_status": "/chapters/{chapter_id}/analysis/status",
                "batch_analysis_status": "/chapters/analysis/status/batch",
                "trigger_analysis": "/chapters/{chapter_id}/analyze"
            })
        );
    }

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
}
