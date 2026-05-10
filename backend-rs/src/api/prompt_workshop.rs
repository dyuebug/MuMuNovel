use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::AppConfig;
use crate::services::auth::Claims;
use crate::services::prompt_workshop_service::PromptWorkshopService;

#[derive(Deserialize)]
struct ImportRequest {
    custom_name: Option<String>,
}

#[derive(Deserialize)]
struct SubmitRequest {
    name: String,
    description: Option<String>,
    prompt_content: String,
    #[serde(default = "default_category")]
    category: String,
    tags: Option<String>,
    author_display_name: Option<String>,
    #[serde(default)]
    is_anonymous: bool,
}

fn default_category() -> String {
    "general".to_string()
}

#[derive(Deserialize)]
struct ReviewRequest {
    action: String,
    review_note: Option<String>,
    category: Option<String>,
    tags: Option<String>,
}

#[derive(Deserialize)]
struct AdminItemCreate {
    name: String,
    description: Option<String>,
    prompt_content: String,
    #[serde(default = "default_category")]
    category: String,
    tags: Option<String>,
}

#[derive(Deserialize)]
struct UpdateQuery {
    force: Option<bool>,
}

async fn get_status(Extension(cfg): Extension<AppConfig>) -> Json<Value> {
    let mut status = PromptWorkshopService::get_status(&cfg).await;
    if let Some(map) = status.as_object_mut() {
        let mode = map
            .get("mode")
            .and_then(|value| value.as_str())
            .unwrap_or("client")
            .to_string();
        let cloud_url = map
            .get("cloud_url")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| std::env::var("WORKSHOP_CLOUD_URL").unwrap_or_else(|_| "https://mumuverse.space:1566".to_string()));
        let cloud_connected = map
            .get("cloud_connected")
            .and_then(|value| value.as_bool())
            .unwrap_or(mode == "server");

        map.insert("mode".to_string(), json!(mode));
        map.insert(
            "instance_id".to_string(),
            json!(std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string())),
        );
        map.insert("cloud_url".to_string(), json!(cloud_url));
        map.insert("cloud_connected".to_string(), json!(cloud_connected));
    }
    Json(status)
}

#[derive(Deserialize)]
struct ListQuery {
    category: Option<String>,
    search: Option<String>,
    tags: Option<String>,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_limit")]
    limit: u64,
}

fn default_sort() -> String {
    "newest".to_string()
}
fn default_page() -> u64 {
    1
}
fn default_limit() -> u64 {
    20
}

async fn get_items(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string());
    let user_identifier = format!("{}:{}", instance_id, claims.sub);
    match PromptWorkshopService::get_items(
        &db,
        query.category.as_deref(),
        query.search.as_deref(),
        query.tags.as_deref(),
        &query.sort,
        query.page,
        query.limit,
        Some(&user_identifier),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn get_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string());
    let user_identifier = format!("{}:{}", instance_id, claims.sub);
    match PromptWorkshopService::get_item(&db, &item_id, Some(&user_identifier)).await {
        Ok(Some(data)) => Ok(Json(data)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "提示词项目不存在"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn import_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(item_id): Path<String>,
    Json(body): Json<ImportRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match PromptWorkshopService::import_item(
        &db,
        &item_id,
        body.custom_name.as_deref(),
        &claims.sub,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn toggle_like(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string());
    let user_identifier = format!("{}:{}", instance_id, claims.sub);
    match PromptWorkshopService::toggle_like(&db, &item_id, &user_identifier).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn record_download(
    Extension(db): Extension<DatabaseConnection>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match PromptWorkshopService::record_download(&db, &item_id).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn submit_prompt(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<SubmitRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string());
    let user_identifier = format!("{}:{}", instance_id, claims.sub);
    let submitter_name = body
        .author_display_name
        .clone()
        .unwrap_or_else(|| claims.sub.clone());
    match PromptWorkshopService::submit_prompt(
        &db,
        &user_identifier,
        &submitter_name,
        &body.name,
        body.description.as_deref(),
        &body.prompt_content,
        &body.category,
        body.tags.as_deref(),
        body.author_display_name.as_deref(),
        body.is_anonymous,
        &instance_id,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

#[derive(Deserialize)]
struct MySubmissionsQuery {
    status: Option<String>,
}

async fn get_my_submissions(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<MySubmissionsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string());
    let user_identifier = format!("{}:{}", instance_id, claims.sub);
    match PromptWorkshopService::get_my_submissions(&db, &user_identifier, query.status.as_deref())
        .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn withdraw_submission(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(submission_id): Path<String>,
    Query(query): Query<UpdateQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string());
    let user_identifier = format!("{}:{}", instance_id, claims.sub);
    match PromptWorkshopService::withdraw_submission(
        &db,
        &submission_id,
        &user_identifier,
        query.force.unwrap_or(false),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

// ==================== Admin helpers ====================

fn check_admin(cfg: &AppConfig, claims: &Claims) -> Result<(), (StatusCode, Json<Value>)> {
    if !PromptWorkshopService::check_workshop_server(cfg) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"detail": "该功能仅在云端服务可用"})),
        ));
    }
    if !claims.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"detail": "需要管理员权限"})),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct AdminSubmissionsQuery {
    status: Option<String>,
    source: Option<String>,
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_limit")]
    limit: u64,
}

async fn admin_get_submissions(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Query(query): Query<AdminSubmissionsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    match PromptWorkshopService::admin_get_submissions(
        &db,
        query.status.as_deref(),
        query.source.as_deref(),
        query.page,
        query.limit,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn admin_review_submission(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(submission_id): Path<String>,
    Json(body): Json<ReviewRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    match PromptWorkshopService::admin_review_submission(
        &db,
        &submission_id,
        &body.action,
        body.review_note.as_deref(),
        body.category.as_deref(),
        body.tags.as_deref(),
        &claims.sub,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

async fn admin_create_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<AdminItemCreate>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    match PromptWorkshopService::admin_create_item(
        &db,
        &body.name,
        body.description.as_deref(),
        &body.prompt_content,
        &body.category,
        body.tags.as_deref(),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn admin_update_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    match PromptWorkshopService::admin_update_item(&db, &item_id, body).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn admin_delete_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    match PromptWorkshopService::admin_delete_item(&db, &item_id).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn admin_get_stats(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    match PromptWorkshopService::admin_get_stats(&db).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/prompt-workshop/status", get(get_status))
        .route("/prompt-workshop/items", get(get_items))
        .route("/prompt-workshop/items/{item_id}", get(get_item))
        .route("/prompt-workshop/items/{item_id}/import", post(import_item))
        .route("/prompt-workshop/items/{item_id}/like", post(toggle_like))
        .route(
            "/prompt-workshop/items/{item_id}/download",
            post(record_download),
        )
        .route("/prompt-workshop/submit", post(submit_prompt))
        .route("/prompt-workshop/my-submissions", get(get_my_submissions))
        .route(
            "/prompt-workshop/submissions/{submission_id}",
            delete(withdraw_submission),
        )
        .route(
            "/prompt-workshop/admin/submissions",
            get(admin_get_submissions),
        )
        .route(
            "/prompt-workshop/admin/submissions/{submission_id}/review",
            post(admin_review_submission),
        )
        .route("/prompt-workshop/admin/items", post(admin_create_item))
        .route(
            "/prompt-workshop/admin/items/{item_id}",
            axum::routing::put(admin_update_item).delete(admin_delete_item),
        )
        .route("/prompt-workshop/admin/stats", get(admin_get_stats))
}
