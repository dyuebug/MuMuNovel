use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use reqwest::Method;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::config::AppConfig;
use crate::models::writing_style;
use crate::services::auth::Claims;
use crate::services::prompt_workshop_service::PromptWorkshopService;

fn normalize_tags_value(tags: Option<&Value>) -> Option<String> {
    let value = tags?;

    match value {
        Value::Null => None,
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }

            if trimmed.starts_with('[') {
                return Some(trimmed.to_string());
            }

            let items: Vec<String> = trimmed
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();

            if items.is_empty() {
                None
            } else {
                serde_json::to_string(&items).ok()
            }
        }
        Value::Array(items) => {
            let normalized: Vec<String> = items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();

            if normalized.is_empty() {
                None
            } else {
                serde_json::to_string(&normalized).ok()
            }
        }
        _ => None,
    }
}

fn workshop_cloud_url() -> String {
    std::env::var("WORKSHOP_CLOUD_URL")
        .unwrap_or_else(|_| "https://mumuverse.space:1566".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn workshop_instance_id() -> String {
    std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string())
}

fn workshop_user_identifier(user_id: &str) -> String {
    format!("{}:{}", workshop_instance_id(), user_id)
}

fn cloud_error(message: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"detail": message.into()})),
    )
}

async fn proxy_workshop_request(
    method: Method,
    path: &str,
    params: Vec<(&str, String)>,
    body: Option<Value>,
    user_identifier: Option<&str>,
) -> Result<Value, (StatusCode, Json<Value>)> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| cloud_error(format!("创建云端工坊客户端失败: {}", error)))?;

    let url = format!("{}/api/prompt-workshop{}", workshop_cloud_url(), path);
    let mut request = client
        .request(method, url)
        .query(&params)
        .header("X-Instance-ID", workshop_instance_id())
        .header("Content-Type", "application/json");

    if let Ok(secret) = std::env::var("WORKSHOP_PROXY_SHARED_SECRET") {
        if !secret.trim().is_empty() {
            request = request.header("X-Workshop-Secret", secret);
        }
    }
    if let Some(user_identifier) = user_identifier {
        request = request.header("X-User-ID", user_identifier);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }

    let response = request
        .send()
        .await
        .map_err(|error| cloud_error(format!("无法连接到云端工坊服务: {}", error)))?;
    let status = response.status();
    if !status.is_success() {
        let preview = response.text().await.unwrap_or_default();
        return Err(cloud_error(format!(
            "云端工坊服务错误: HTTP {}, {}",
            status.as_u16(),
            preview.chars().take(200).collect::<String>()
        )));
    }

    response
        .json::<Value>()
        .await
        .map_err(|error| cloud_error(format!("云端工坊返回非 JSON 内容: {}", error)))
}

fn workshop_response_data(response: &Value) -> &Value {
    response.get("data").unwrap_or(response)
}

fn required_workshop_text<'a>(item: &'a Value, field: &str) -> Result<&'a str, String> {
    item.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("云端提示词缺少必要字段: {}", field))
}

async fn create_writing_style_from_workshop_item(
    db: &DatabaseConnection,
    item: &Value,
    custom_name: Option<&str>,
    user_id: &str,
) -> Result<Value, String> {
    let name = required_workshop_text(item, "name")?;
    let prompt_content = required_workshop_text(item, "prompt_content")?;
    let description = item
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let count = writing_style::Entity::find()
        .filter(writing_style::Column::UserId.eq(user_id))
        .count(db)
        .await
        .map_err(|error| format!("{}", error))?;

    let inserted = writing_style::ActiveModel {
        user_id: Set(Some(user_id.to_string())),
        name: Set(custom_name.unwrap_or(name).to_string()),
        style_type: Set("custom".to_string()),
        description: Set(Some(format!("从提示词工坊导入: {}", description))),
        prompt_content: Set(prompt_content.to_string()),
        order_index: Set(count as i32 + 1),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|error| format!("{}", error))?;

    Ok(json!({
        "success": true,
        "message": "导入成功",
        "writing_style": {
            "id": inserted.id,
            "name": inserted.name,
            "style_type": inserted.style_type,
            "prompt_content": inserted.prompt_content,
        }
    }))
}

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
    tags: Option<Value>,
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
    tags: Option<Value>,
}

#[derive(Deserialize)]
struct AdminItemCreate {
    name: String,
    description: Option<String>,
    prompt_content: String,
    #[serde(default = "default_category")]
    category: String,
    tags: Option<Value>,
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
            .unwrap_or_else(|| {
                std::env::var("WORKSHOP_CLOUD_URL")
                    .unwrap_or_else(|_| "https://mumuverse.space:1566".to_string())
            });
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
    Extension(cfg): Extension<AppConfig>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let mut params = vec![
            ("sort", query.sort.clone()),
            ("page", query.page.to_string()),
            ("limit", query.limit.to_string()),
        ];
        if let Some(category) = &query.category {
            params.push(("category", category.clone()));
        }
        if let Some(search) = &query.search {
            params.push(("search", search.clone()));
        }
        if let Some(tags) = &query.tags {
            params.push(("tags", tags.clone()));
        }
        return proxy_workshop_request(Method::GET, "/items", params, None, Some(&user_identifier))
            .await
            .map(Json);
    }

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
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        return proxy_workshop_request(
            Method::GET,
            &format!("/items/{}", item_id),
            Vec::new(),
            None,
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

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
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
    Json(body): Json<ImportRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let user_identifier = workshop_user_identifier(&claims.sub);
        let item_response = proxy_workshop_request(
            Method::GET,
            &format!("/items/{}", item_id),
            Vec::new(),
            None,
            Some(&user_identifier),
        )
        .await?;
        let item = workshop_response_data(&item_response);

        let _ = proxy_workshop_request(
            Method::POST,
            &format!("/items/{}/download", item_id),
            Vec::new(),
            Some(json!({
                "instance_id": workshop_instance_id(),
                "user_identifier": user_identifier,
            })),
            Some(&user_identifier),
        )
        .await;

        return create_writing_style_from_workshop_item(
            &db,
            item,
            body.custom_name.as_deref(),
            &claims.sub,
        )
        .await
        .map(Json)
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        });
    }

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
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        return proxy_workshop_request(
            Method::POST,
            &format!("/items/{}/like", item_id),
            Vec::new(),
            None,
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

    match PromptWorkshopService::toggle_like(&db, &item_id, &user_identifier).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn record_download(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let user_identifier = workshop_user_identifier(&claims.sub);
        return proxy_workshop_request(
            Method::POST,
            &format!("/items/{}/download", item_id),
            Vec::new(),
            Some(json!({
                "instance_id": workshop_instance_id(),
                "user_identifier": user_identifier,
            })),
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

    match PromptWorkshopService::record_download(&db, &item_id).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn submit_prompt(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<SubmitRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let instance_id = workshop_instance_id();
    let user_identifier = workshop_user_identifier(&claims.sub);
    let submitter_name = body
        .author_display_name
        .clone()
        .unwrap_or_else(|| claims.sub.clone());
    let tags = normalize_tags_value(body.tags.as_ref());

    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let header_user_identifier = user_identifier.clone();
        let mut payload = Map::new();
        payload.insert("instance_id".to_string(), json!(instance_id));
        payload.insert("submitter_id".to_string(), json!(user_identifier));
        payload.insert("submitter_name".to_string(), json!(submitter_name));
        payload.insert("name".to_string(), json!(body.name));
        payload.insert("description".to_string(), json!(body.description));
        payload.insert("prompt_content".to_string(), json!(body.prompt_content));
        payload.insert("category".to_string(), json!(body.category));
        payload.insert(
            "author_display_name".to_string(),
            json!(body.author_display_name),
        );
        payload.insert("is_anonymous".to_string(), json!(body.is_anonymous));
        payload.insert(
            "tags".to_string(),
            tags.as_deref()
                .and_then(|value| serde_json::from_str::<Value>(value).ok())
                .unwrap_or(Value::Null),
        );
        return proxy_workshop_request(
            Method::POST,
            "/submit",
            Vec::new(),
            Some(Value::Object(payload)),
            Some(&header_user_identifier),
        )
        .await
        .map(Json);
    }

    match PromptWorkshopService::submit_prompt(
        &db,
        &user_identifier,
        &submitter_name,
        &body.name,
        body.description.as_deref(),
        &body.prompt_content,
        &body.category,
        tags.as_deref(),
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
    Extension(cfg): Extension<AppConfig>,
    Query(query): Query<MySubmissionsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let mut params = Vec::new();
        if let Some(status) = &query.status {
            params.push(("status", status.clone()));
        }
        return proxy_workshop_request(
            Method::GET,
            "/my-submissions",
            params,
            None,
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

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
    Extension(cfg): Extension<AppConfig>,
    Path(submission_id): Path<String>,
    Query(query): Query<UpdateQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let mut params = Vec::new();
        if query.force.unwrap_or(false) {
            params.push(("force", "true".to_string()));
        }
        return proxy_workshop_request(
            Method::DELETE,
            &format!("/submissions/{}", submission_id),
            params,
            None,
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

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
    let tags = normalize_tags_value(body.tags.as_ref());
    match PromptWorkshopService::admin_review_submission(
        &db,
        &submission_id,
        &body.action,
        body.review_note.as_deref(),
        body.category.as_deref(),
        tags.as_deref(),
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
    let tags = normalize_tags_value(body.tags.as_ref());
    match PromptWorkshopService::admin_create_item(
        &db,
        &body.name,
        body.description.as_deref(),
        &body.prompt_content,
        &body.category,
        tags.as_deref(),
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
