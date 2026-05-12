use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::writing_style_service::WritingStyleService;

#[derive(Deserialize, Default)]
struct SetDefaultQuery {
    project_id: Option<String>,
}

#[derive(Deserialize, Default)]
struct SetDefaultBody {
    project_id: Option<String>,
}

async fn list_presets(
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::list_presets(&db)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn list_user_styles(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::list_user_styles(&db, &claims.sub)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn list_project_styles(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::list_project_styles(&db, &claims.sub, &project_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn get_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(style_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::get_style(&db, &claims.sub, style_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn create_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    WritingStyleService::create_style(&db, &claims.sub, &body)
        .await
        .map(|v| (StatusCode::CREATED, Json(v)))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn update_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(style_id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::update_style(&db, &claims.sub, style_id, &body)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn delete_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(style_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::delete_style(&db, &claims.sub, style_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn set_default_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(style_id): Path<i32>,
    Query(params): Query<SetDefaultQuery>,
    body: Option<Json<SetDefaultBody>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let project_id = body
        .and_then(|Json(payload)| payload.project_id)
        .or(params.project_id)
        .unwrap_or_default();

    if project_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "project_id is required"})),
        ));
    }

    WritingStyleService::set_default_style(&db, &claims.sub, style_id, &project_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn initialize_defaults(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    WritingStyleService::initialize_defaults(&db, &claims.sub, &project_id)
        .await
        .map(|v| (StatusCode::CREATED, Json(v)))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

pub fn routes() -> Router {
    Router::new()
        .route("/writing-styles/presets/list", get(list_presets))
        .route("/writing-styles/user", get(list_user_styles))
        .route(
            "/writing-styles/project/{project_id}",
            get(list_project_styles),
        )
        .route(
            "/writing-styles/project/{project_id}/initialize",
            post(initialize_defaults),
        )
        .route(
            "/writing-styles/project/{project_id}/init-defaults",
            post(initialize_defaults),
        )
        .route("/writing-styles", post(create_style))
        .route(
            "/writing-styles/{style_id}",
            get(get_style).put(update_style).delete(delete_style),
        )
        .route(
            "/writing-styles/{style_id}/set-default",
            post(set_default_style),
        )
}
