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

#[derive(Deserialize, Default)]
struct ListQuery {
    status: Option<String>,
    category: Option<String>,
    source_type: Option<String>,
    is_long_term: Option<bool>,
    page: Option<u64>,
    limit: Option<u64>,
}

#[derive(Deserialize, Default)]
struct StatsQuery {
    current_chapter: Option<i32>,
}

#[derive(Deserialize, Default)]
struct ContextQuery {
    include_pending: Option<bool>,
    include_overdue: Option<bool>,
    lookahead: Option<i32>,
}

#[derive(Deserialize, Default)]
struct PendingResolveQuery {
    current_chapter: Option<i32>,
    lookahead: Option<i32>,
}

#[derive(Deserialize, Default)]
struct AbandonQuery {
    reason: Option<String>,
}

async fn list_project(
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Query(params): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::list_project(
        &db, &project_id,
        params.status.as_deref(), params.category.as_deref(),
        params.source_type.as_deref(), params.is_long_term,
        params.page, params.limit,
    ).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn get_stats(
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Query(params): Query<StatsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::get_stats(&db, &project_id, params.current_chapter).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn get_context(
    Extension(db): Extension<DatabaseConnection>,
    Path((project_id, chapter_number)): Path<(String, i32)>,
    Query(params): Query<ContextQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::get_context(
        &db, &project_id, chapter_number,
        params.include_pending, params.include_overdue, params.lookahead,
    ).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn list_pending_resolve(
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Query(params): Query<PendingResolveQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::list_pending_resolve(
        &db, &project_id,
        params.current_chapter.unwrap_or(1),
        params.lookahead,
    ).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn get_one(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::get_one(&db, &foreshadow_id).await.map(Json).map_err(|e| {
        (StatusCode::NOT_FOUND, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn create(
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    ForeshadowService::create(&db, &body).await
        .map(|v| (StatusCode::CREATED, Json(v)))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)}))))
}

async fn update(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::update(&db, &foreshadow_id, &body).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn delete_foreshadow(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::delete(&db, &foreshadow_id).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn plant(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::plant(&db, &foreshadow_id, &body).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn resolve(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::resolve(&db, &foreshadow_id, &body).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn abandon(
    Extension(db): Extension<DatabaseConnection>,
    Path(foreshadow_id): Path<String>,
    Query(params): Query<AbandonQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::abandon(&db, &foreshadow_id, params.reason.as_deref()).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

async fn sync_from_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ForeshadowService::sync_from_analysis(&db, &project_id, &body).await.map(Json).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })
}

pub fn routes() -> Router {
    Router::new()
        .route("/foreshadows/projects/{projectId}", get(list_project))
        .route("/foreshadows/projects/{projectId}/stats", get(get_stats))
        .route("/foreshadows/projects/{projectId}/context/{chapterNumber}", get(get_context))
        .route("/foreshadows/projects/{projectId}/pending-resolve", get(list_pending_resolve))
        .route("/foreshadows/projects/{projectId}/sync-from-analysis", post(sync_from_analysis))
        .route("/foreshadows", post(create))
        .route("/foreshadows/{foreshadowId}", get(get_one))
        .route("/foreshadows/{foreshadowId}", put(update))
        .route("/foreshadows/{foreshadowId}", route_delete(delete_foreshadow))
        .route("/foreshadows/{foreshadowId}/plant", post(plant))
        .route("/foreshadows/{foreshadowId}/resolve", post(resolve))
        .route("/foreshadows/{foreshadowId}/abandon", post(abandon))
}
