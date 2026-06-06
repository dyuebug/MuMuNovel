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

use crate::services::foreshadow_request_service::{
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
use crate::services::foreshadow_service::ForeshadowService;

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

    use crate::services::foreshadow_request_service::ForeshadowQueryRequestError;

    use super::map_foreshadow_query_request_error;

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
        .route("/foreshadows/projects/{projectId}", get(list_project))
        .route("/foreshadows/projects/{projectId}/stats", get(get_stats))
        .route(
            "/foreshadows/projects/{projectId}/context/{chapterNumber}",
            get(get_context),
        )
        .route(
            "/foreshadows/projects/{projectId}/pending-resolve",
            get(list_pending_resolve),
        )
        .route(
            "/foreshadows/projects/{projectId}/sync-from-analysis",
            post(sync_from_analysis),
        )
        .route("/foreshadows", post(create))
        .route("/foreshadows/{foreshadowId}", get(get_one))
        .route("/foreshadows/{foreshadowId}", put(update))
        .route(
            "/foreshadows/{foreshadowId}",
            route_delete(delete_foreshadow),
        )
        .route("/foreshadows/{foreshadowId}/plant", post(plant))
        .route("/foreshadows/{foreshadowId}/resolve", post(resolve))
        .route("/foreshadows/{foreshadowId}/abandon", post(abandon))
}
