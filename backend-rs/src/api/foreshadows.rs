use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete as route_delete, get, post, put},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::foreshadow_service::ForeshadowService;
use crate::services::foreshadow_service::{
    build_create_foreshadow_request_from_route_payload,
    build_plant_foreshadow_request_from_route_payload,
    build_resolve_foreshadow_request_from_route_payload,
    build_sync_foreshadow_from_analysis_request_from_route_payload,
    build_update_foreshadow_request_from_route_payload, CreateForeshadowRouteRequest,
    ForeshadowContextQueryRequest, ForeshadowContextRouteQuery, ForeshadowQueryRequestError,
    ForeshadowStatsQueryRequest, ForeshadowStatsRouteQuery, ListForeshadowsQueryRequest,
    ListForeshadowsRouteQuery, PendingResolveForeshadowsQueryRequest,
    PendingResolveForeshadowsRouteQuery, PlantForeshadowRouteRequest,
    ResolveForeshadowRouteRequest, SyncForeshadowFromAnalysisRouteRequest,
    UpdateForeshadowRouteRequest,
};

const FORESHADOWS_PROJECT_LIST_ROUTE: &str = "/foreshadows/projects/{projectId}";
const FORESHADOWS_PROJECT_STATS_ROUTE: &str = "/foreshadows/projects/{projectId}/stats";
const FORESHADOWS_CONTEXT_ROUTE: &str = "/foreshadows/projects/{projectId}/context/{chapterNumber}";
const FORESHADOWS_PENDING_RESOLVE_ROUTE: &str = "/foreshadows/projects/{projectId}/pending-resolve";
const FORESHADOWS_SYNC_FROM_ANALYSIS_ROUTE: &str =
    "/foreshadows/projects/{projectId}/sync-from-analysis";
const FORESHADOWS_CREATE_ROUTE: &str = "/foreshadows";
const FORESHADOWS_DETAIL_ROUTE: &str = "/foreshadows/{foreshadowId}";
const FORESHADOWS_PLANT_ROUTE: &str = "/foreshadows/{foreshadowId}/plant";
const FORESHADOWS_RESOLVE_ROUTE: &str = "/foreshadows/{foreshadowId}/resolve";
const FORESHADOWS_ABANDON_ROUTE: &str = "/foreshadows/{foreshadowId}/abandon";

#[cfg(test)]
fn build_foreshadows_route_owner_contract() -> Value {
    json!({
        "owner": "foreshadows",
        "rust_owner": "backend-rs/src/api/foreshadows.rs",
        "route_prefix": "/api",
        "routes": {
            "project_list": FORESHADOWS_PROJECT_LIST_ROUTE,
            "stats": FORESHADOWS_PROJECT_STATS_ROUTE,
            "context": FORESHADOWS_CONTEXT_ROUTE,
            "pending_resolve": FORESHADOWS_PENDING_RESOLVE_ROUTE,
            "sync_from_analysis": FORESHADOWS_SYNC_FROM_ANALYSIS_ROUTE,
            "create": FORESHADOWS_CREATE_ROUTE,
            "detail": FORESHADOWS_DETAIL_ROUTE,
            "update": FORESHADOWS_DETAIL_ROUTE,
            "delete": FORESHADOWS_DETAIL_ROUTE,
            "plant": FORESHADOWS_PLANT_ROUTE,
            "resolve": FORESHADOWS_RESOLVE_ROUTE,
            "abandon": FORESHADOWS_ABANDON_ROUTE
        },
        "method_contract": {
            "project_list": ["GET"],
            "stats": ["GET"],
            "context": ["GET"],
            "pending_resolve": ["GET"],
            "sync_from_analysis": ["POST"],
            "create": ["POST"],
            "detail": ["GET", "PUT", "DELETE"],
            "plant": ["POST"],
            "resolve": ["POST"],
            "abandon": ["POST"]
        },
        "service_handoffs": {
            "request_owner": "backend-rs/src/services/foreshadow_service.rs",
            "workflow_owner": "backend-rs/src/services/foreshadow_service.rs"
        },
        "readiness_probes": [
            "foreshadows-project-list-auth-guard-rust",
            "foreshadows-stats-auth-guard-rust",
            "foreshadows-setup-project-business-rust",
            "foreshadows-create-business-rust",
            "foreshadows-list-business-rust",
            "foreshadows-stats-business-rust",
            "foreshadows-detail-business-rust",
            "foreshadows-update-business-rust",
            "foreshadows-setup-plant-chapter-business-rust",
            "foreshadows-setup-resolve-chapter-business-rust",
            "foreshadows-plant-business-rust",
            "foreshadows-pending-resolve-business-rust",
            "foreshadows-context-business-rust",
            "foreshadows-resolve-business-rust",
            "foreshadows-stats-after-resolve-business-rust",
            "foreshadows-create-abandon-business-rust",
            "foreshadows-abandon-business-rust",
            "foreshadows-sync-from-analysis-business-rust",
            "foreshadows-synced-detail-business-rust",
            "foreshadows-delete-business-rust",
            "foreshadows-missing-detail-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-foreshadows-business-owner",
            "business_probes": [
                "foreshadows-setup-project-business-rust",
                "foreshadows-create-business-rust",
                "foreshadows-list-business-rust",
                "foreshadows-stats-business-rust",
                "foreshadows-detail-business-rust",
                "foreshadows-update-business-rust",
                "foreshadows-setup-plant-chapter-business-rust",
                "foreshadows-setup-resolve-chapter-business-rust",
                "foreshadows-plant-business-rust",
                "foreshadows-pending-resolve-business-rust",
                "foreshadows-context-business-rust",
                "foreshadows-resolve-business-rust",
                "foreshadows-stats-after-resolve-business-rust",
                "foreshadows-create-abandon-business-rust",
                "foreshadows-abandon-business-rust",
                "foreshadows-sync-from-analysis-business-rust",
                "foreshadows-synced-detail-business-rust",
                "foreshadows-delete-business-rust",
                "foreshadows-missing-detail-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "source_map_files": [],
        "rollback_boundary": {
            "source_map_policy": "foreshadows_route_model_schema_source_map_deleted_no_python_foreshadow_shell_remains",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "source_map_freeze_status": "physical_closeout_completed",
            "source_map_physical_closeout_action": "delete_completed",
            "remaining_blockers": [
                "optional schema/migration owner follow-up if the Python Alembic metadata surface later needs explicit Rust migration-owner replacement"
            ],
            "freeze_reason": "phase5-foreshadows-business-owner covers setup, CRUD, stats, context, pending resolve, plant, resolve, abandon, sync-from-analysis, synced detail, delete, and missing-detail probes with zero Python fallback probes, while the detached Python foreshadow model/schema files no longer have any production consumers and are physically deleted."
        },
        "business_smoke_status": {
            "owner_profile": "phase5-foreshadows-business-owner",
            "business_probe_count": 19,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "foreshadows Python model/schema source-map deleted; remaining maturity work is limited to optional schema/migration owner follow-up outside the active route-group boundary",
        "migration_policy": "Foreshadows route business smoke is covered by phase5-foreshadows-business-owner; the Python route shell is no longer registered in app bootstrap, the legacy model/schema files are physically deleted, and the remaining maturity work is limited to optional schema/migration owner follow-up outside the active route-group boundary."
    })
}

#[derive(Deserialize, Default)]
struct AbandonQuery {
    reason: Option<String>,
}

async fn list_project(
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Query(params): Query<ListForeshadowsRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = ListForeshadowsQueryRequest::from_route_query(params)
        .map_err(map_foreshadow_query_request_error)?;

    ForeshadowService::list_project(
        &db,
        &project_id,
        request.status(),
        request.category(),
        request.source_type(),
        request.is_long_term(),
        Some(request.page()),
        Some(request.limit()),
    )
    .await
    .map(Json)
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })
}

async fn get_stats(
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Query(params): Query<ForeshadowStatsRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = ForeshadowStatsQueryRequest::from_route_query(params)
        .map_err(map_foreshadow_query_request_error)?;

    ForeshadowService::get_stats(&db, &project_id, request.current_chapter())
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn get_context(
    Extension(db): Extension<DatabaseConnection>,
    Path((project_id, chapter_number)): Path<(String, i32)>,
    Query(params): Query<ForeshadowContextRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = ForeshadowContextQueryRequest::from_route_query(params)
        .map_err(map_foreshadow_query_request_error)?;

    ForeshadowService::get_context(
        &db,
        &project_id,
        chapter_number,
        request.include_pending(),
        request.include_overdue(),
        Some(request.lookahead()),
    )
    .await
    .map(Json)
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })
}

async fn list_pending_resolve(
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Query(params): Query<PendingResolveForeshadowsRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = PendingResolveForeshadowsQueryRequest::from_route_query(params)
        .map_err(map_foreshadow_query_request_error)?;

    ForeshadowService::list_pending_resolve(
        &db,
        &project_id,
        request.current_chapter(),
        Some(request.lookahead()),
    )
    .await
    .map(Json)
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })
}

async fn get_one(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::get_one(&db, &foreshadow_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

fn map_foreshadow_query_request_error(
    error: ForeshadowQueryRequestError,
) -> (StatusCode, Json<Value>) {
    let detail = match error {
        ForeshadowQueryRequestError::PageTooSmall => "page must be greater than or equal to 1",
        ForeshadowQueryRequestError::LimitTooSmall => "limit must be greater than or equal to 1",
        ForeshadowQueryRequestError::LimitTooLarge => "limit must be less than or equal to 100",
        ForeshadowQueryRequestError::CurrentChapterMissing => "current_chapter is required",
        ForeshadowQueryRequestError::CurrentChapterTooSmall => {
            "current_chapter must be greater than or equal to 1"
        }
        ForeshadowQueryRequestError::LookaheadTooSmall => {
            "lookahead must be greater than or equal to 1"
        }
        ForeshadowQueryRequestError::LookaheadTooLarge => {
            "lookahead must be less than or equal to 20"
        }
    };

    (StatusCode::BAD_REQUEST, Json(json!({ "detail": detail })))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::services::foreshadow_service::ForeshadowQueryRequestError;

    use super::{
        build_foreshadows_route_owner_contract, map_foreshadow_query_request_error,
        FORESHADOWS_ABANDON_ROUTE, FORESHADOWS_CONTEXT_ROUTE, FORESHADOWS_CREATE_ROUTE,
        FORESHADOWS_DETAIL_ROUTE, FORESHADOWS_PENDING_RESOLVE_ROUTE, FORESHADOWS_PLANT_ROUTE,
        FORESHADOWS_PROJECT_LIST_ROUTE, FORESHADOWS_PROJECT_STATS_ROUTE, FORESHADOWS_RESOLVE_ROUTE,
        FORESHADOWS_SYNC_FROM_ANALYSIS_ROUTE,
    };

    #[test]
    fn foreshadow_query_errors_match_python_query_bounds() {
        let cases = [
            (
                ForeshadowQueryRequestError::PageTooSmall,
                "page must be greater than or equal to 1",
            ),
            (
                ForeshadowQueryRequestError::LimitTooSmall,
                "limit must be greater than or equal to 1",
            ),
            (
                ForeshadowQueryRequestError::LimitTooLarge,
                "limit must be less than or equal to 100",
            ),
            (
                ForeshadowQueryRequestError::CurrentChapterMissing,
                "current_chapter is required",
            ),
            (
                ForeshadowQueryRequestError::CurrentChapterTooSmall,
                "current_chapter must be greater than or equal to 1",
            ),
            (
                ForeshadowQueryRequestError::LookaheadTooSmall,
                "lookahead must be greater than or equal to 1",
            ),
            (
                ForeshadowQueryRequestError::LookaheadTooLarge,
                "lookahead must be less than or equal to 20",
            ),
        ];

        for (error, expected_detail) in cases {
            let (status, body) = map_foreshadow_query_request_error(error);

            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(body.0["detail"], expected_detail);
        }
    }

    #[test]
    fn should_publish_foreshadows_route_owner_contract() {
        let contract = build_foreshadows_route_owner_contract();

        assert_eq!(contract["owner"], "foreshadows");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/foreshadows.rs");
        assert_eq!(
            contract["routes"]["project_list"],
            FORESHADOWS_PROJECT_LIST_ROUTE
        );
        assert_eq!(contract["routes"]["stats"], FORESHADOWS_PROJECT_STATS_ROUTE);
        assert_eq!(
            contract["routes"]["sync_from_analysis"],
            FORESHADOWS_SYNC_FROM_ANALYSIS_ROUTE
        );
        assert_eq!(contract["routes"]["detail"], FORESHADOWS_DETAIL_ROUTE);
        assert_eq!(contract["routes"]["plant"], FORESHADOWS_PLANT_ROUTE);
        assert_eq!(contract["routes"]["resolve"], FORESHADOWS_RESOLVE_ROUTE);
        assert_eq!(contract["routes"]["abandon"], FORESHADOWS_ABANDON_ROUTE);
        assert_eq!(contract["readiness_probes"].as_array().unwrap().len(), 21);
        assert_eq!(
            contract["readiness_probes"][20],
            "foreshadows-missing-detail-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-foreshadows-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"]
                .as_array()
                .expect("business probes should be present")
                .len(),
            19
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][15],
            "foreshadows-sync-from-analysis-business-rust"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 0);
        assert!(contract["source_map_files"].get(0).is_none());
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            19
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "foreshadows Python model/schema source-map deleted; remaining maturity work is limited to optional schema/migration owner follow-up outside the active route-group boundary"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("business smoke is covered"));
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("legacy model/schema files are physically deleted"));
        assert!(!contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("logged-in business probes are accepted"));
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"][0],
            "optional schema/migration owner follow-up if the Python Alembic metadata surface later needs explicit Rust migration-owner replacement"
        );
    }

    #[test]
    fn should_keep_foreshadows_route_group_paths_stable() {
        assert_eq!(
            FORESHADOWS_PROJECT_LIST_ROUTE,
            "/foreshadows/projects/{projectId}"
        );
        assert_eq!(
            FORESHADOWS_PROJECT_STATS_ROUTE,
            "/foreshadows/projects/{projectId}/stats"
        );
        assert_eq!(
            FORESHADOWS_CONTEXT_ROUTE,
            "/foreshadows/projects/{projectId}/context/{chapterNumber}"
        );
        assert_eq!(
            FORESHADOWS_PENDING_RESOLVE_ROUTE,
            "/foreshadows/projects/{projectId}/pending-resolve"
        );
        assert_eq!(
            FORESHADOWS_SYNC_FROM_ANALYSIS_ROUTE,
            "/foreshadows/projects/{projectId}/sync-from-analysis"
        );
        assert_eq!(FORESHADOWS_CREATE_ROUTE, "/foreshadows");
        assert_eq!(FORESHADOWS_DETAIL_ROUTE, "/foreshadows/{foreshadowId}");
        assert_eq!(FORESHADOWS_PLANT_ROUTE, "/foreshadows/{foreshadowId}/plant");
        assert_eq!(
            FORESHADOWS_RESOLVE_ROUTE,
            "/foreshadows/{foreshadowId}/resolve"
        );
        assert_eq!(
            FORESHADOWS_ABANDON_ROUTE,
            "/foreshadows/{foreshadowId}/abandon"
        );
    }
}

async fn create(
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<CreateForeshadowRouteRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let request = build_create_foreshadow_request_from_route_payload(body);

    ForeshadowService::create(&db, &request)
        .await
        .map(|v| (StatusCode::CREATED, Json(v)))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn update(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
    Json(body): Json<UpdateForeshadowRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_update_foreshadow_request_from_route_payload(body);

    ForeshadowService::update(&db, &foreshadow_id, &request)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn delete_foreshadow(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::delete(&db, &foreshadow_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn plant(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
    Json(body): Json<PlantForeshadowRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_plant_foreshadow_request_from_route_payload(body);

    ForeshadowService::plant(&db, &foreshadow_id, &request)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn resolve(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
    Json(body): Json<ResolveForeshadowRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_resolve_foreshadow_request_from_route_payload(body);

    ForeshadowService::resolve(&db, &foreshadow_id, &request)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn abandon(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
    Query(params): Query<AbandonQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::abandon(&db, &foreshadow_id, params.reason.as_deref())
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn sync_from_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Json(body): Json<SyncForeshadowFromAnalysisRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_sync_foreshadow_from_analysis_request_from_route_payload(body);

    ForeshadowService::sync_from_analysis(&db, &project_id, &request)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

pub fn routes() -> Router {
    Router::new()
        .route(FORESHADOWS_PROJECT_LIST_ROUTE, get(list_project))
        .route(FORESHADOWS_PROJECT_STATS_ROUTE, get(get_stats))
        .route(FORESHADOWS_CONTEXT_ROUTE, get(get_context))
        .route(FORESHADOWS_PENDING_RESOLVE_ROUTE, get(list_pending_resolve))
        .route(
            FORESHADOWS_SYNC_FROM_ANALYSIS_ROUTE,
            post(sync_from_analysis),
        )
        .route(FORESHADOWS_CREATE_ROUTE, post(create))
        .route(FORESHADOWS_DETAIL_ROUTE, get(get_one))
        .route(FORESHADOWS_DETAIL_ROUTE, put(update))
        .route(FORESHADOWS_DETAIL_ROUTE, route_delete(delete_foreshadow))
        .route(FORESHADOWS_PLANT_ROUTE, post(plant))
        .route(FORESHADOWS_RESOLVE_ROUTE, post(resolve))
        .route(FORESHADOWS_ABANDON_ROUTE, post(abandon))
}
