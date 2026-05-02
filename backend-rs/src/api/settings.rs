use axum::{
    extract::{Extension, Path},
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

// ========== Presets ==========

async fn get_presets(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::list_presets(&db, &claims.sub).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))),
    }
}

async fn create_preset(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match SettingsService::create_preset(&db, &claims.sub, &body).await {
        Ok(result) => Ok((StatusCode::CREATED, Json(result))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))),
    }
}

async fn update_preset(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(preset_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::update_preset(&db, &claims.sub, &preset_id, &body).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            let detail = format!("{}", e);
            if detail.contains("not found") {
                Err((StatusCode::NOT_FOUND, Json(json!({"detail": detail}))))
            } else {
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": detail}))))
            }
        }
    }
}

async fn delete_preset(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(preset_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::delete_preset(&db, &claims.sub, &preset_id).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))),
    }
}

async fn activate_preset(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(preset_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::activate_preset(&db, &claims.sub, &preset_id).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            let detail = format!("{}", e);
            if detail.contains("not found") {
                Err((StatusCode::NOT_FOUND, Json(json!({"detail": detail}))))
            } else {
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": detail}))))
            }
        }
    }
}

async fn create_preset_from_current(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("My Preset");
    let description = body.get("description").and_then(|v| v.as_str());
    match SettingsService::create_preset_from_current(&db, &claims.sub, name, description).await {
        Ok(result) => Ok((StatusCode::CREATED, Json(result))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/settings", get(get_settings))
        .route("/settings", post(create_settings))
        .route("/settings", put(update_settings))
        .route("/settings", delete(delete_settings))
        .route("/settings/presets", get(get_presets))
        .route("/settings/presets", post(create_preset))
        .route("/settings/presets/from-current", post(create_preset_from_current))
        .route("/settings/presets/{preset_id}", put(update_preset))
        .route("/settings/presets/{preset_id}", delete(delete_preset))
        .route("/settings/presets/{preset_id}/activate", post(activate_preset))
}
