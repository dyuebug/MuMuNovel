use axum::{extract::Extension, http::StatusCode, response::Json, routing::post, Router};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::polish_service::PolishService;

fn default_focus_mode() -> String {
    "balanced".into()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct PolishRequest {
    #[serde(alias = "text")]
    original_text: String,
    #[allow(dead_code)]
    project_id: Option<i64>,
    provider: Option<String>,
    model: Option<String>,
    temperature: Option<f64>,
    style: Option<String>,
    #[serde(default = "default_focus_mode")]
    focus_mode: String,
    #[serde(default = "default_true")]
    preserve_paragraphs: bool,
    #[serde(default = "default_true")]
    retain_hooks: bool,
}

#[derive(Deserialize)]
struct PolishBatchRequest {
    texts: Vec<String>,
    #[allow(dead_code)]
    project_id: Option<i64>,
    provider: Option<String>,
    model: Option<String>,
    temperature: Option<f64>,
    style: Option<String>,
    #[serde(default = "default_focus_mode")]
    focus_mode: String,
    #[serde(default = "default_true")]
    preserve_paragraphs: bool,
    #[serde(default = "default_true")]
    retain_hooks: bool,
}

async fn polish_text(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<PolishRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let original = body.original_text.trim().to_string();
    if original.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "original_text 或 text 不能为空"})),
        ));
    }

    match PolishService::polish_text(
        &db,
        &claims.sub,
        &original,
        body.style.as_deref(),
        &body.focus_mode,
        body.preserve_paragraphs,
        body.retain_hooks,
        body.provider.as_deref(),
        body.model.as_deref(),
        body.temperature,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("AI去味失败: {}", e)})),
        )),
    }
}

async fn polish_batch(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<PolishBatchRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let normalized: Vec<String> = body
        .texts
        .iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    if normalized.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "texts 不能为空"})),
        ));
    }

    match PolishService::polish_batch(
        &db,
        &claims.sub,
        &normalized,
        body.style.as_deref(),
        &body.focus_mode,
        body.preserve_paragraphs,
        body.retain_hooks,
        body.provider.as_deref(),
        body.model.as_deref(),
        body.temperature,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("批量AI去味失败: {}", e)})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/polish", post(polish_text))
        .route("/polish/batch", post(polish_batch))
}
