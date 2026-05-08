use std::sync::Arc;

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

use crate::mcp::McpClientManager;
use crate::services::auth::Claims;
use crate::services::mcp_plugin_service::McpPluginService;

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    enabled_only: bool,
    category: Option<String>,
}

#[derive(Deserialize)]
struct ToggleQuery {
    enabled: bool,
}

#[derive(Deserialize)]
struct CreateRequest {
    plugin_name: String,
    #[serde(default)]
    display_name: Option<String>,
    description: Option<String>,
    #[serde(default = "default_type")]
    plugin_type: String,
    server_url: Option<String>,
    command: Option<String>,
    args: Option<String>,
    env: Option<String>,
    headers: Option<String>,
    config: Option<String>,
    category: Option<String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_type() -> String {
    "http".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct SimpleCreateRequest {
    config_json: String,
    category: Option<String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Deserialize)]
struct ToolCallRequest {
    plugin_id: String,
    tool_name: String,
    arguments: Option<Value>,
}

#[derive(Deserialize)]
struct CacheClearQuery {
    user_id: Option<String>,
    plugin_name: Option<String>,
}

#[derive(Deserialize)]
struct MetricsQuery {
    tool_name: Option<String>,
}

async fn list_plugins(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::list(
        &db,
        &claims.sub,
        query.enabled_only,
        query.category.as_deref(),
    )
    .await
    {
        Ok(plugins) => Ok(Json(json!(plugins))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn create_plugin(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let display_name = body
        .display_name
        .unwrap_or_else(|| body.plugin_name.clone());
    match McpPluginService::create(
        &db,
        &claims.sub,
        &body.plugin_name,
        &display_name,
        body.description.as_deref(),
        &body.plugin_type,
        body.server_url.as_deref(),
        body.command.as_deref(),
        body.args.as_deref(),
        body.env.as_deref(),
        body.headers.as_deref(),
        body.config.as_deref(),
        body.category.as_deref(),
        body.enabled,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

async fn create_plugin_simple(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<SimpleCreateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::create_or_update_simple(
        &db,
        &claims.sub,
        &body.config_json,
        body.category.as_deref(),
        body.enabled,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

async fn update_plugin(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::update(&db, &plugin_id, &claims.sub, body).await {
        Ok(Some(data)) => Ok(Json(data)),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn delete_plugin(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::delete(&db, &plugin_id, &claims.sub).await {
        Ok(Some(data)) => Ok(Json(data)),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn toggle_plugin(
    Extension(db): Extension<DatabaseConnection>,
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
    Query(query): Query<ToggleQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::toggle(&db, &plugin_id, &claims.sub, query.enabled).await {
        Ok(Some(data)) => {
            if query.enabled {
                let db_clone = db.clone();
                let mcp_clone = mcp.clone();
                let pid = plugin_id.clone();
                let uid = claims.sub.clone();
                tokio::spawn(async move {
                    let _ =
                        McpPluginService::register_plugin(&db_clone, &mcp_clone, &pid, &uid).await;
                });
            } else {
                let plugin_name = data
                    .get("plugin_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !plugin_name.is_empty() {
                    let _ = mcp.disconnect(&claims.sub, plugin_name).await;
                }
            }
            Ok(Json(data))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn get_plugin(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::get(&db, &plugin_id, &claims.sub).await {
        Ok(Some(data)) => Ok(Json(data)),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn get_plugin_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::get_status(&db, &plugin_id, &claims.sub).await {
        Ok(Some(data)) => {
            // Enrich with real session status from MCP manager
            let plugin_name = data
                .get("plugin_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(name) = plugin_name {
                let is_reg = mcp.is_registered(&claims.sub, &name).await;
                let sess = mcp.get_session_status(&claims.sub, &name).await;
                let mut enriched = data;
                enriched["session_status"] = json!(sess);
                enriched["is_registered"] = json!(is_reg);
                Ok(Json(enriched))
            } else {
                Ok(Json(data))
            }
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn get_metrics(Query(query): Query<MetricsQuery>) -> Json<Value> {
    Json(McpPluginService::get_metrics(query.tool_name.as_deref()))
}

async fn get_cache_stats() -> Json<Value> {
    Json(McpPluginService::get_cache_stats())
}

async fn get_session_stats(Extension(mcp): Extension<Arc<McpClientManager>>) -> Json<Value> {
    Json(json!({
        "session_stats": {"active_sessions": mcp.session_count().await},
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

async fn clear_cache(Query(query): Query<CacheClearQuery>) -> Json<Value> {
    Json(McpPluginService::clear_cache(
        query.user_id.as_deref(),
        query.plugin_name.as_deref(),
    ))
}

async fn get_plugin_tools(
    Extension(db): Extension<DatabaseConnection>,
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Try real MCP tools first via service
    match McpPluginService::get_tools(&db, &plugin_id, &claims.sub).await {
        Ok(Some(data)) => {
            // If we have a live session, try to get fresh tools
            if let Some(name) = data.get("plugin_name").and_then(|v| v.as_str()) {
                if mcp.is_registered(&claims.sub, name).await {
                    if let Ok(tools) = mcp.list_tools(&claims.sub, name).await {
                        return Ok(Json(json!({
                            "plugin_name": name,
                            "tools": tools,
                            "count": tools.len(),
                        })));
                    }
                }
            }
            Ok(Json(data))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

async fn call_mcp_tool(
    Extension(db): Extension<DatabaseConnection>,
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ToolCallRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::call_tool(
        &db,
        &mcp,
        &body.plugin_id,
        &claims.sub,
        &body.tool_name,
        body.arguments.as_ref(),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

async fn test_plugin(
    Extension(db): Extension<DatabaseConnection>,
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::test_plugin(&db, &mcp, &plugin_id, &claims.sub).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/mcp/plugins", get(list_plugins).post(create_plugin))
        .route("/mcp/plugins/simple", post(create_plugin_simple))
        .route("/mcp/plugins/call", post(call_mcp_tool))
        .route("/mcp/plugins/metrics", get(get_metrics))
        .route("/mcp/plugins/cache/stats", get(get_cache_stats))
        .route("/mcp/plugins/cache/clear", post(clear_cache))
        .route("/mcp/plugins/sessions/stats", get(get_session_stats))
        .route(
            "/mcp/plugins/{plugin_id}",
            get(get_plugin).put(update_plugin).delete(delete_plugin),
        )
        .route("/mcp/plugins/{plugin_id}/toggle", post(toggle_plugin))
        .route("/mcp/plugins/{plugin_id}/status", get(get_plugin_status))
        .route("/mcp/plugins/{plugin_id}/tools", get(get_plugin_tools))
        .route("/mcp/plugins/{plugin_id}/test", post(test_plugin))
}
