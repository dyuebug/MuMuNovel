use axum::{extract::Extension, http::StatusCode, response::Json, routing::post, Router};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::inspiration_service::InspirationService;

fn inspiration_error_status(detail: &str) -> StatusCode {
    let lower = detail.to_lowercase();
    let is_bad_request = lower.contains("api key")
        || lower.contains("base url")
        || lower.contains("invalid token")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
        || detail.contains("用户设置不存在")
        || detail.contains("请先在设置")
        || detail.contains("缺少有效")
        || detail.contains("配置")
        || detail.contains("密钥");

    if is_bad_request {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[derive(Deserialize)]
struct GenerateOptionsRequest {
    step: String,
    context: Value,
}

#[derive(Deserialize)]
struct RefineOptionsRequest {
    step: String,
    context: Value,
    feedback: String,
    #[serde(default)]
    previous_options: Vec<String>,
}

#[derive(Deserialize)]
struct QuickGenerateRequest {
    title: Option<String>,
    description: Option<String>,
    theme: Option<String>,
    genre: Option<Vec<String>>,
    narrative_perspective: Option<String>,
}

async fn generate_options(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<GenerateOptionsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match InspirationService::generate_options(&db, &claims.sub, &body.step, &body.context).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let detail = format!("生成选项失败: {}", e);
            Err((
                inspiration_error_status(&detail),
                Json(json!({ "detail": detail })),
            ))
        }
    }
}

async fn refine_options(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RefineOptionsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match InspirationService::refine_options(
        &db,
        &claims.sub,
        &body.step,
        &body.context,
        &body.feedback,
        &body.previous_options,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let detail = format!("生成选项失败: {}", e);
            Err((
                inspiration_error_status(&detail),
                Json(json!({ "detail": detail })),
            ))
        }
    }
}

async fn quick_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<QuickGenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let genre_ref: Option<&[String]> = body.genre.as_deref();
    match InspirationService::quick_generate(
        &db,
        &claims.sub,
        body.title.as_deref(),
        body.description.as_deref(),
        body.theme.as_deref(),
        genre_ref,
        body.narrative_perspective.as_deref(),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let detail = format!("智能补全失败: {}", e);
            Err((
                inspiration_error_status(&detail),
                Json(json!({ "detail": detail })),
            ))
        }
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/inspiration/generate-options", post(generate_options))
        .route("/inspiration/refine-options", post(refine_options))
        .route("/inspiration/quick-generate", post(quick_generate))
}
