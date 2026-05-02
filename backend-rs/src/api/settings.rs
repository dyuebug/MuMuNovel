use axum::{
    extract::Extension,
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::settings_service::SettingsService;

async fn get_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::get_or_create(&db, &claims.sub).await {
        Ok(settings) => Ok(Json(settings)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn create_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::update(&db, &claims.sub, &body).await {
        Ok(settings) => Ok(Json(settings)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn update_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::update(&db, &claims.sub, &body).await {
        Ok(settings) => Ok(Json(settings)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn delete_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::delete(&db, &claims.sub).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/settings", get(get_settings))
        .route("/settings", post(create_settings))
        .route("/settings", put(update_settings))
        .route("/settings", delete(delete_settings))
}
