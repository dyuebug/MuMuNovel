use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::writing_style_request_service::{
    build_create_writing_style_request_from_typed_route_payload,
    build_set_default_style_project_id,
    build_update_writing_style_request_from_typed_route_payload, BuildSetDefaultStyleRequestError,
    CreateWritingStyleRouteRequest, SetDefaultStyleRouteBody, SetDefaultStyleRouteQuery,
    UpdateWritingStyleRouteRequest,
};
use crate::services::writing_style_service::WritingStyleService;

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
    Json(body): Json<CreateWritingStyleRouteRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let request = build_create_writing_style_request_from_typed_route_payload(body);
    WritingStyleService::create_style(&db, &claims.sub, &request)
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
    Json(body): Json<UpdateWritingStyleRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_update_writing_style_request_from_typed_route_payload(body);
    WritingStyleService::update_style(&db, &claims.sub, style_id, &request)
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
    Query(params): Query<SetDefaultStyleRouteQuery>,
    body: Option<Json<SetDefaultStyleRouteBody>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let project_id = build_set_default_style_project_id(params, body.map(|Json(payload)| payload))
        .map_err(|error| match error {
            BuildSetDefaultStyleRequestError::MissingProjectId => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "project_id is required"})),
            ),
        })?;

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
