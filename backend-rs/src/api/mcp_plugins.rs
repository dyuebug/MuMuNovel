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
use std::collections::HashMap;

mod service_owner;

use crate::mcp::McpClientManager;
use crate::services::auth::Claims;
use service_owner::{
    build_mcp_plugin_update_request, CallToolError, GetToolsError, McpPluginService,
    McpPluginUpdateRequest, TestPluginError,
};

const MCP_PLUGINS_LIST_CREATE_ROUTE: &str = "/mcp/plugins";
const MCP_PLUGINS_SIMPLE_CREATE_ROUTE: &str = "/mcp/plugins/simple";
const MCP_PLUGINS_CALL_ROUTE: &str = "/mcp/plugins/call";
const MCP_CALL_ALIAS_ROUTE: &str = "/mcp/call";
const MCP_PLUGINS_METRICS_ROUTE: &str = "/mcp/plugins/metrics";
const MCP_PLUGINS_CACHE_STATS_ROUTE: &str = "/mcp/plugins/cache/stats";
const MCP_PLUGINS_CACHE_CLEAR_ROUTE: &str = "/mcp/plugins/cache/clear";
const MCP_PLUGINS_SESSIONS_STATS_ROUTE: &str = "/mcp/plugins/sessions/stats";
const MCP_PLUGINS_DETAIL_ROUTE: &str = "/mcp/plugins/{plugin_id}";
const MCP_PLUGINS_TOGGLE_ROUTE: &str = "/mcp/plugins/{plugin_id}/toggle";
const MCP_PLUGINS_STATUS_ROUTE: &str = "/mcp/plugins/{plugin_id}/status";
const MCP_PLUGINS_TOOLS_ROUTE: &str = "/mcp/plugins/{plugin_id}/tools";
const MCP_PLUGINS_TEST_ROUTE: &str = "/mcp/plugins/{plugin_id}/test";

#[cfg(test)]
fn build_mcp_plugins_route_owner_contract() -> Value {
    json!({
        "owner": "mcp_plugins",
        "rust_owner": "backend-rs/src/api/mcp_plugins.rs",
        "routes": {
            "list": MCP_PLUGINS_LIST_CREATE_ROUTE,
            "create": MCP_PLUGINS_LIST_CREATE_ROUTE,
            "simple_create": MCP_PLUGINS_SIMPLE_CREATE_ROUTE,
            "call": MCP_PLUGINS_CALL_ROUTE,
            "call_alias": MCP_CALL_ALIAS_ROUTE,
            "metrics": MCP_PLUGINS_METRICS_ROUTE,
            "cache_stats": MCP_PLUGINS_CACHE_STATS_ROUTE,
            "cache_clear": MCP_PLUGINS_CACHE_CLEAR_ROUTE,
            "sessions_stats": MCP_PLUGINS_SESSIONS_STATS_ROUTE,
            "detail": MCP_PLUGINS_DETAIL_ROUTE,
            "update": MCP_PLUGINS_DETAIL_ROUTE,
            "delete": MCP_PLUGINS_DETAIL_ROUTE,
            "toggle": MCP_PLUGINS_TOGGLE_ROUTE,
            "status": MCP_PLUGINS_STATUS_ROUTE,
            "tools": MCP_PLUGINS_TOOLS_ROUTE,
            "test": MCP_PLUGINS_TEST_ROUTE
        },
        "methods": {
            "list": ["GET"],
            "create": ["POST"],
            "simple_create": ["POST"],
            "call": ["POST"],
            "call_alias": ["POST"],
            "metrics": ["GET"],
            "cache_stats": ["GET"],
            "cache_clear": ["POST"],
            "sessions_stats": ["GET"],
            "detail": ["GET", "PUT", "DELETE"],
            "toggle": ["POST"],
            "status": ["GET"],
            "tools": ["GET"],
            "test": ["POST"]
        },
        "service_owners": [
            "backend-rs/src/api/mcp_plugins/service_owner.rs",
            "backend-rs/src/mcp/mod.rs",
            "backend-rs/src/models/mcp_plugin.rs"
        ],
        "readiness_probes": [
            "mcp-plugins-list-auth-guard-rust",
            "mcp-plugins-simple-create-auth-guard-rust",
            "mcp-plugins-simple-create-business-rust",
            "mcp-plugins-list-business-rust",
            "mcp-plugins-detail-business-rust",
            "mcp-plugins-status-business-rust",
            "mcp-plugins-delete-business-rust",
            "mcp-plugins-missing-detail-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-mcp-plugins-business-owner",
            "business_probes": [
                "mcp-plugins-simple-create-business-rust",
                "mcp-plugins-list-business-rust",
                "mcp-plugins-detail-business-rust",
                "mcp-plugins-status-business-rust",
                "mcp-plugins-delete-business-rust",
                "mcp-plugins-missing-detail-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-mcp-plugins-business-owner",
            "readiness_probe_count": 8,
            "business_probe_count": 6,
            "auth_guard_probe_count": 2,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "source_map_files": [
            "backend/app/api/mcp_plugins.py",
            "backend/app/models/mcp_plugin.py",
            "backend/app/schemas/mcp_plugin.py",
            "backend/app/services/mcp_test_service.py",
            "backend/app/mcp/__init__.py"
        ],
        "next_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "MCP plugins route business smoke is covered by phase5-mcp-plugins-business-owner; final completion now requires explicit source-map freeze/delete/repoint approval with same-round rollback policy.",
        "rollback_boundary": {
            "source_map_policy": "keep_python_mcp_plugin_route_model_schema_test_service_facade_files_as_source_map_until_explicit_freeze_delete_round",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit source-map freeze/delete/repoint approval"
            ],
            "freeze_reason": "phase5-mcp-plugins-business-owner covers simple create, list, detail, status, delete, and missing-detail probes with zero Python fallback probes."
        }
    })
}

#[derive(Debug, Clone, PartialEq)]
enum RouteValueField {
    Missing,
    Present(Value),
}

impl Default for RouteValueField {
    fn default() -> Self {
        Self::Missing
    }
}

impl<'de> Deserialize<'de> for RouteValueField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self::Present)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct McpPluginUpdateRouteRequest {
    #[serde(default)]
    display_name: Option<Value>,
    #[serde(default)]
    description: RouteValueField,
    #[serde(default)]
    server_url: RouteValueField,
    #[serde(default)]
    command: RouteValueField,
    #[serde(default)]
    enabled: Option<Value>,
    #[serde(default)]
    category: Option<Value>,
    #[serde(default)]
    sort_order: Option<Value>,
    #[serde(default)]
    headers: Option<Value>,
    #[serde(default)]
    config: Option<Value>,
    #[serde(default)]
    args: Option<Value>,
    #[serde(default)]
    env: Option<Value>,
}

impl McpPluginUpdateRouteRequest {
    fn into_body(self) -> Value {
        let mut body = serde_json::Map::new();

        if let Some(value) = self.display_name {
            body.insert("display_name".to_string(), value);
        }
        if let RouteValueField::Present(value) = self.description {
            body.insert("description".to_string(), value);
        }
        if let RouteValueField::Present(value) = self.server_url {
            body.insert("server_url".to_string(), value);
        }
        if let RouteValueField::Present(value) = self.command {
            body.insert("command".to_string(), value);
        }
        if let Some(value) = self.enabled {
            body.insert("enabled".to_string(), value);
        }
        if let Some(value) = self.category {
            body.insert("category".to_string(), value);
        }
        if let Some(value) = self.sort_order {
            body.insert("sort_order".to_string(), value);
        }
        if let Some(value) = self.headers {
            body.insert("headers".to_string(), value);
        }
        if let Some(value) = self.config {
            body.insert("config".to_string(), value);
        }
        if let Some(value) = self.args {
            body.insert("args".to_string(), value);
        }
        if let Some(value) = self.env {
            body.insert("env".to_string(), value);
        }

        Value::Object(body)
    }
}

fn build_mcp_plugin_update_request_from_typed_route_payload(
    route_request: McpPluginUpdateRouteRequest,
) -> McpPluginUpdateRequest {
    build_mcp_plugin_update_request(&route_request.into_body())
}

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

#[derive(Debug, Deserialize)]
struct CreateRequest {
    plugin_name: String,
    #[serde(default)]
    display_name: Option<String>,
    description: Option<String>,
    #[serde(default = "default_type")]
    plugin_type: String,
    server_url: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    config: Option<serde_json::Map<String, Value>>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    category: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    sort_order: Option<i32>,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_type() -> String {
    "http".to_string()
}
fn default_true() -> bool {
    true
}

fn deserialize_optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
struct SimpleCreateRequest {
    config_json: String,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
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
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let args = serde_json::to_string(&body.args).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("无效的args字段: {}", error)})),
        )
    })?;
    let env = serde_json::to_string(&body.env).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("无效的env字段: {}", error)})),
        )
    })?;
    let headers = serde_json::to_string(&body.headers).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("无效的headers字段: {}", error)})),
        )
    })?;
    let config = serde_json::to_string(&body.config).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("无效的config字段: {}", error)})),
        )
    })?;
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
        if body.args.is_some() {
            Some(args.as_str())
        } else {
            None
        },
        if body.env.is_some() {
            Some(env.as_str())
        } else {
            None
        },
        if body.headers.is_some() {
            Some(headers.as_str())
        } else {
            None
        },
        if body.config.is_some() {
            Some(config.as_str())
        } else {
            None
        },
        body.category.as_deref(),
        body.sort_order,
        body.enabled,
    )
    .await
    {
        Ok(data) => {
            let finalized = if body.enabled {
                let plugin_id =
                    data.get("id")
                        .and_then(|value| value.as_str())
                        .ok_or_else(|| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"detail": "创建插件后缺少插件ID"})),
                            )
                        })?;

                McpPluginService::finalize_create_runtime_state_like_python(
                    &db,
                    &mcp,
                    plugin_id,
                    &claims.sub,
                )
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"detail": e})),
                    )
                })?
            } else {
                data
            };

            Ok(Json(finalized))
        }
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

async fn create_plugin_simple(
    Extension(db): Extension<DatabaseConnection>,
    Extension(mcp): Extension<Arc<McpClientManager>>,
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
        Ok(data) => {
            if let Some(plugin_id) = data.get("id").and_then(|value| value.as_str()) {
                let db_clone = db.clone();
                let mcp_clone = mcp.clone();
                let plugin_id = plugin_id.to_string();
                let user_id = claims.sub.clone();
                tokio::spawn(async move {
                    let _ = McpPluginService::finalize_simple_create_runtime_state_like_python(
                        &db_clone, &mcp_clone, &plugin_id, &user_id,
                    )
                    .await;
                });
            }

            Ok(Json(data))
        }
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

async fn update_plugin(
    Extension(db): Extension<DatabaseConnection>,
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
    Json(body): Json<McpPluginUpdateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_mcp_plugin_update_request_from_typed_route_payload(body);

    match McpPluginService::update(&db, &plugin_id, &claims.sub, request).await {
        Ok(Some(data)) => {
            McpPluginService::refresh_updated_plugin_runtime_like_python(
                &db,
                &mcp,
                &plugin_id,
                &claims.sub,
            )
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": e})),
                )
            })?;
            Ok(Json(data))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn delete_plugin(
    Extension(db): Extension<DatabaseConnection>,
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::delete(&db, &mcp, &plugin_id, &claims.sub).await {
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
                let finalized = McpPluginService::finalize_toggle_runtime_state_like_python(
                    &db,
                    &mcp,
                    &plugin_id,
                    &claims.sub,
                )
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"detail": e})),
                    )
                })?;
                Ok(Json(finalized))
            } else {
                let plugin_name = data
                    .get("plugin_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !plugin_name.is_empty() {
                    let _ = mcp.disconnect(&claims.sub, plugin_name).await;
                }
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
            let plugin_name = data
                .get("plugin_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(name) = plugin_name {
                let session_key = format!("{}:{}", claims.sub, name);
                let sessions = mcp.session_stats_snapshot().await;
                let session_info = sessions.iter().find(|entry| {
                    entry.get("key").and_then(|value| value.as_str()) == Some(session_key.as_str())
                });
                Ok(Json(apply_python_session_status_to_plugin_status(
                    data,
                    session_info,
                )))
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

async fn get_metrics(
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Query(query): Query<MetricsQuery>,
) -> Json<Value> {
    Json(McpPluginService::get_metrics(
        mcp.metrics_snapshot(query.tool_name.as_deref()).await,
        query.tool_name.as_deref(),
    ))
}

async fn get_cache_stats(Extension(mcp): Extension<Arc<McpClientManager>>) -> Json<Value> {
    Json(McpPluginService::get_cache_stats(
        mcp.cache_stats_snapshot().await,
    ))
}

async fn get_session_stats(Extension(mcp): Extension<Arc<McpClientManager>>) -> Json<Value> {
    Json(McpPluginService::get_session_stats(
        mcp.session_stats_snapshot().await,
    ))
}

fn apply_python_session_status_to_plugin_status(
    mut data: Value,
    session_info: Option<&Value>,
) -> Value {
    let db_status = data
        .get("db_status")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let session_status =
        session_info.and_then(|entry| entry.get("status").and_then(|value| value.as_str()));

    data["session_status"] = session_info
        .and_then(|entry| entry.get("status").cloned())
        .unwrap_or(Value::Null);
    data["is_registered"] = json!(session_info.is_some());
    data["error_rate"] = session_info
        .and_then(|entry| entry.get("error_rate").cloned())
        .unwrap_or(json!(0));
    data["in_sync"] = json!(match session_status {
        Some(status) => db_status.as_deref() == Some(status),
        None => db_status.as_deref() == Some("inactive"),
    });

    data
}

fn resolve_cache_clear_target_user_id(
    requested_user_id: Option<&str>,
    current_user_id: &str,
) -> Result<String, (StatusCode, Json<Value>)> {
    if let Some(user_id) = requested_user_id {
        if user_id != current_user_id {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"detail": "无权清理其他用户的缓存"})),
            ));
        }
    }

    Ok(requested_user_id.unwrap_or(current_user_id).to_string())
}

async fn build_clear_cache_payload(
    mcp: &McpClientManager,
    query: &CacheClearQuery,
    current_user_id: &str,
) -> Result<Value, (StatusCode, Json<Value>)> {
    let target_user_id =
        resolve_cache_clear_target_user_id(query.user_id.as_deref(), current_user_id)?;
    mcp.clear_cache(Some(target_user_id.as_str()), query.plugin_name.as_deref())
        .await;
    Ok(McpPluginService::clear_cache(
        Some(target_user_id.as_str()),
        query.plugin_name.as_deref(),
    ))
}

async fn clear_cache(
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<CacheClearQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = build_clear_cache_payload(&mcp, &query, &claims.sub).await?;
    Ok(Json(payload))
}

fn map_get_tools_error(error: GetToolsError) -> (StatusCode, Json<Value>) {
    match error {
        GetToolsError::PluginNotFound => {
            (StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))
        }
        GetToolsError::PluginDisabled => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "插件未启用"})),
        ),
        GetToolsError::RegisterFailed(detail) | GetToolsError::FetchFailed(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

async fn get_plugin_tools(
    Extension(db): Extension<DatabaseConnection>,
    Extension(mcp): Extension<Arc<McpClientManager>>,
    Extension(claims): Extension<Claims>,
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match McpPluginService::get_tools(&db, &mcp, &plugin_id, &claims.sub).await {
        Ok(data) => Ok(Json(data)),
        Err(error) => Err(map_get_tools_error(error)),
    }
}

fn map_call_tool_error(error: CallToolError) -> (StatusCode, Json<Value>) {
    match error {
        CallToolError::PluginNotFound => {
            (StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))
        }
        CallToolError::PluginDisabled => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "插件未启用"})),
        ),
        CallToolError::RegisterFailed(detail) | CallToolError::CallFailed(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

fn map_test_plugin_error(error: TestPluginError) -> (StatusCode, Json<Value>) {
    match error {
        TestPluginError::PluginNotFound => {
            (StatusCode::NOT_FOUND, Json(json!({"detail": "插件不存在"})))
        }
        TestPluginError::TestFailed(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
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
        Err(error) => Err(map_call_tool_error(error)),
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
        Err(error) => Err(map_test_plugin_error(error)),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(
            MCP_PLUGINS_LIST_CREATE_ROUTE,
            get(list_plugins).post(create_plugin),
        )
        .route(MCP_PLUGINS_SIMPLE_CREATE_ROUTE, post(create_plugin_simple))
        .route(MCP_PLUGINS_CALL_ROUTE, post(call_mcp_tool))
        .route(MCP_CALL_ALIAS_ROUTE, post(call_mcp_tool))
        .route(MCP_PLUGINS_METRICS_ROUTE, get(get_metrics))
        .route(MCP_PLUGINS_CACHE_STATS_ROUTE, get(get_cache_stats))
        .route(MCP_PLUGINS_CACHE_CLEAR_ROUTE, post(clear_cache))
        .route(MCP_PLUGINS_SESSIONS_STATS_ROUTE, get(get_session_stats))
        .route(
            MCP_PLUGINS_DETAIL_ROUTE,
            get(get_plugin).put(update_plugin).delete(delete_plugin),
        )
        .route(MCP_PLUGINS_TOGGLE_ROUTE, post(toggle_plugin))
        .route(MCP_PLUGINS_STATUS_ROUTE, get(get_plugin_status))
        .route(MCP_PLUGINS_TOOLS_ROUTE, get(get_plugin_tools))
        .route(MCP_PLUGINS_TEST_ROUTE, post(test_plugin))
}

#[cfg(test)]
mod tests {
    use super::{
        apply_python_session_status_to_plugin_status, build_clear_cache_payload,
        build_mcp_plugin_update_request_from_typed_route_payload,
        build_mcp_plugins_route_owner_contract, get_cache_stats, get_metrics, get_session_stats,
        map_call_tool_error, map_get_tools_error, map_test_plugin_error,
        resolve_cache_clear_target_user_id, CacheClearQuery, CreateRequest,
        McpPluginUpdateRouteRequest, MetricsQuery, RouteValueField, SimpleCreateRequest,
        MCP_CALL_ALIAS_ROUTE, MCP_PLUGINS_CACHE_CLEAR_ROUTE, MCP_PLUGINS_CACHE_STATS_ROUTE,
        MCP_PLUGINS_CALL_ROUTE, MCP_PLUGINS_DETAIL_ROUTE, MCP_PLUGINS_LIST_CREATE_ROUTE,
        MCP_PLUGINS_METRICS_ROUTE, MCP_PLUGINS_SESSIONS_STATS_ROUTE,
        MCP_PLUGINS_SIMPLE_CREATE_ROUTE, MCP_PLUGINS_STATUS_ROUTE, MCP_PLUGINS_TEST_ROUTE,
        MCP_PLUGINS_TOGGLE_ROUTE, MCP_PLUGINS_TOOLS_ROUTE,
    };
    use crate::mcp::McpClientManager;
    use axum::extract::Query;
    use axum::http::StatusCode;
    use axum::Extension;
    use chrono::{Duration, Utc};
    use serde_json::{json, Value};
    use std::sync::Arc;

    use super::service_owner::{CallToolError, GetToolsError, TestPluginError};

    #[test]
    fn build_mcp_plugin_update_request_from_typed_route_payload_keeps_existing_contract() {
        let request =
            build_mcp_plugin_update_request_from_typed_route_payload(McpPluginUpdateRouteRequest {
                display_name: Some(json!("Weather Plugin")),
                description: RouteValueField::Present(json!("Fetch weather")),
                server_url: RouteValueField::Present(json!("https://example.com/mcp")),
                command: RouteValueField::Present(json!("node server.js")),
                enabled: Some(json!(true)),
                category: Some(json!("tools")),
                sort_order: Some(json!(12)),
                headers: Some(json!({"Authorization": "Bearer token"})),
                config: Some(json!({"timeout": 30})),
                args: Some(json!(["--stdio"])),
                env: Some(json!({"NODE_ENV": "production"})),
            });

        assert_eq!(request.display_name.as_deref(), Some("Weather Plugin"));
        assert_eq!(request.description, Some(json!("Fetch weather")));
        assert_eq!(request.server_url, Some(json!("https://example.com/mcp")));
        assert_eq!(request.command, Some(json!("node server.js")));
        assert_eq!(request.enabled, Some(true));
        assert_eq!(request.category.as_deref(), Some("tools"));
        assert_eq!(request.sort_order, Some(12));
        assert_eq!(
            request.headers,
            Some(json!({"Authorization": "Bearer token"}))
        );
        assert_eq!(request.config, Some(json!({"timeout": 30})));
        assert_eq!(request.args, Some(json!(["--stdio"])));
        assert_eq!(request.env, Some(json!({"NODE_ENV": "production"})));
    }

    #[test]
    fn build_mcp_plugin_update_request_preserves_explicit_null_for_nullable_string_fields() {
        let request =
            build_mcp_plugin_update_request_from_typed_route_payload(McpPluginUpdateRouteRequest {
                description: RouteValueField::Present(Value::Null),
                server_url: RouteValueField::Present(Value::Null),
                command: RouteValueField::Present(Value::Null),
                ..McpPluginUpdateRouteRequest::default()
            });

        assert_eq!(request.description, Some(Value::Null));
        assert_eq!(request.server_url, Some(Value::Null));
        assert_eq!(request.command, Some(Value::Null));
    }

    #[test]
    fn should_publish_mcp_plugins_route_owner_contract() {
        let contract = build_mcp_plugins_route_owner_contract();

        assert_eq!(contract["owner"], "mcp_plugins");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/mcp_plugins.rs");
        assert_eq!(contract["routes"]["list"], MCP_PLUGINS_LIST_CREATE_ROUTE);
        assert_eq!(contract["routes"]["create"], MCP_PLUGINS_LIST_CREATE_ROUTE);
        assert_eq!(
            contract["routes"]["simple_create"],
            MCP_PLUGINS_SIMPLE_CREATE_ROUTE
        );
        assert_eq!(contract["routes"]["call"], MCP_PLUGINS_CALL_ROUTE);
        assert_eq!(contract["routes"]["call_alias"], MCP_CALL_ALIAS_ROUTE);
        assert_eq!(contract["routes"]["metrics"], MCP_PLUGINS_METRICS_ROUTE);
        assert_eq!(
            contract["routes"]["cache_stats"],
            MCP_PLUGINS_CACHE_STATS_ROUTE
        );
        assert_eq!(
            contract["routes"]["cache_clear"],
            MCP_PLUGINS_CACHE_CLEAR_ROUTE
        );
        assert_eq!(
            contract["routes"]["sessions_stats"],
            MCP_PLUGINS_SESSIONS_STATS_ROUTE
        );
        assert_eq!(contract["routes"]["detail"], MCP_PLUGINS_DETAIL_ROUTE);
        assert_eq!(contract["routes"]["update"], MCP_PLUGINS_DETAIL_ROUTE);
        assert_eq!(contract["routes"]["delete"], MCP_PLUGINS_DETAIL_ROUTE);
        assert_eq!(contract["routes"]["toggle"], MCP_PLUGINS_TOGGLE_ROUTE);
        assert_eq!(contract["routes"]["status"], MCP_PLUGINS_STATUS_ROUTE);
        assert_eq!(contract["routes"]["tools"], MCP_PLUGINS_TOOLS_ROUTE);
        assert_eq!(contract["routes"]["test"], MCP_PLUGINS_TEST_ROUTE);

        assert_eq!(
            contract["methods"]["detail"],
            json!(["GET", "PUT", "DELETE"])
        );
        assert_eq!(contract["methods"]["call_alias"], json!(["POST"]));
        assert_eq!(
            contract["service_owners"]
                .as_array()
                .expect("service owner list should be present")
                .len(),
            3
        );
        assert_eq!(
            contract["readiness_probes"]
                .as_array()
                .expect("readiness probes should be present")
                .len(),
            8
        );
        assert_eq!(
            contract["readiness_probes"]
                .as_array()
                .expect("readiness probes should be present")
                .last()
                .expect("readiness probes should not be empty"),
            "mcp-plugins-missing-detail-business-rust"
        );
        assert_eq!(
            contract["source_map_files"]
                .as_array()
                .expect("source map files should be present")
                .len(),
            5
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-mcp-plugins-business-owner"
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be present");
        assert_eq!(business_probes.len(), 6);
        assert!(business_probes
            .iter()
            .any(|probe| probe == "mcp-plugins-status-business-rust"));
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(8)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            json!(2)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("phase5-mcp-plugins-business-owner"));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
    }

    #[test]
    fn should_keep_mcp_plugins_route_group_paths_stable() {
        assert_eq!(MCP_PLUGINS_LIST_CREATE_ROUTE, "/mcp/plugins");
        assert_eq!(MCP_PLUGINS_SIMPLE_CREATE_ROUTE, "/mcp/plugins/simple");
        assert_eq!(MCP_PLUGINS_CALL_ROUTE, "/mcp/plugins/call");
        assert_eq!(MCP_CALL_ALIAS_ROUTE, "/mcp/call");
        assert_eq!(MCP_PLUGINS_METRICS_ROUTE, "/mcp/plugins/metrics");
        assert_eq!(MCP_PLUGINS_CACHE_STATS_ROUTE, "/mcp/plugins/cache/stats");
        assert_eq!(MCP_PLUGINS_CACHE_CLEAR_ROUTE, "/mcp/plugins/cache/clear");
        assert_eq!(
            MCP_PLUGINS_SESSIONS_STATS_ROUTE,
            "/mcp/plugins/sessions/stats"
        );
        assert_eq!(MCP_PLUGINS_DETAIL_ROUTE, "/mcp/plugins/{plugin_id}");
        assert_eq!(MCP_PLUGINS_TOGGLE_ROUTE, "/mcp/plugins/{plugin_id}/toggle");
        assert_eq!(MCP_PLUGINS_STATUS_ROUTE, "/mcp/plugins/{plugin_id}/status");
        assert_eq!(MCP_PLUGINS_TOOLS_ROUTE, "/mcp/plugins/{plugin_id}/tools");
        assert_eq!(MCP_PLUGINS_TEST_ROUTE, "/mcp/plugins/{plugin_id}/test");
    }

    #[test]
    fn mcp_plugin_update_route_request_into_body_keeps_missing_vs_present_contract() {
        let body = McpPluginUpdateRouteRequest {
            display_name: Some(json!("Weather Plugin")),
            description: RouteValueField::Present(json!(null)),
            server_url: RouteValueField::Missing,
            command: RouteValueField::Present(json!("node server.js")),
            enabled: Some(json!(true)),
            ..McpPluginUpdateRouteRequest::default()
        }
        .into_body();

        assert_eq!(
            body,
            json!({
                "display_name": "Weather Plugin",
                "description": null,
                "command": "node server.js",
                "enabled": true
            })
        );
        assert!(body.get("server_url").is_none());
    }

    #[tokio::test]
    async fn clear_cache_defaults_to_current_user_like_python_route() {
        let mcp = Arc::new(McpClientManager::new());
        let payload = build_clear_cache_payload(
            &mcp,
            &CacheClearQuery {
                user_id: None,
                plugin_name: None,
            },
            "user-1",
        )
        .await
        .expect("missing user_id should default to current user like Python");

        assert_eq!(payload["success"], true);
        assert_eq!(payload["message"], "已清理用户 user-1 的所有缓存");
    }

    #[tokio::test]
    async fn clear_cache_keeps_plugin_message_priority_when_defaulting_user_like_python_route() {
        let mcp = Arc::new(McpClientManager::new());
        let payload = build_clear_cache_payload(
            &mcp,
            &CacheClearQuery {
                user_id: None,
                plugin_name: Some("exa".to_string()),
            },
            "user-1",
        )
        .await
        .expect("plugin cache clear should still use current user target");

        assert_eq!(payload["success"], true);
        assert_eq!(payload["message"], "已清理插件 exa 的缓存");
    }

    #[test]
    fn clear_cache_rejects_other_user_like_python_route() {
        let error = resolve_cache_clear_target_user_id(Some("user-2"), "user-1")
            .expect_err("cross-user cache clear should be forbidden");

        assert_eq!(error.0, StatusCode::FORBIDDEN);
        assert_eq!(error.1 .0, json!({"detail": "无权清理其他用户的缓存"}));
    }

    #[test]
    fn create_plugin_route_request_accepts_python_structured_mcp_fields() {
        let request: CreateRequest = serde_json::from_value(json!({
            "plugin_name": "exa",
            "display_name": "Exa Search",
            "plugin_type": "http",
            "server_url": "https://example.com/mcp",
            "args": ["--stdio", "--debug"],
            "env": {"NODE_ENV": "production"},
            "headers": {"Authorization": "Bearer token"},
            "config": {"timeout": 30, "retries": 2},
            "category": "search",
            "enabled": true
        }))
        .expect("Python-compatible MCPPluginCreate payload should deserialize");

        assert_eq!(request.plugin_name, "exa");
        assert_eq!(request.display_name.as_deref(), Some("Exa Search"));
        assert_eq!(request.plugin_type, "http");
        assert_eq!(
            request.server_url.as_deref(),
            Some("https://example.com/mcp")
        );
        assert_eq!(
            request.args.as_ref(),
            Some(&vec!["--stdio".to_string(), "--debug".to_string()])
        );
        assert_eq!(
            request.env.as_ref().and_then(|env| env.get("NODE_ENV")),
            Some(&"production".to_string())
        );
        assert_eq!(
            request
                .headers
                .as_ref()
                .and_then(|headers| headers.get("Authorization")),
            Some(&"Bearer token".to_string())
        );
        assert_eq!(
            request
                .config
                .as_ref()
                .and_then(|config| config.get("timeout")),
            Some(&json!(30))
        );
        assert_eq!(request.category.as_deref(), Some("search"));
        assert!(request.enabled);
    }

    #[tokio::test]
    async fn get_session_stats_uses_python_total_sessions_contract() {
        let mcp = Arc::new(McpClientManager::new());

        let payload = get_session_stats(Extension(mcp)).await.0;

        assert_eq!(payload["session_stats"]["total_sessions"], json!(0));
        assert_eq!(payload["session_stats"]["sessions"], json!([]));
        assert!(payload.get("timestamp").is_some());
    }

    #[tokio::test]
    async fn get_metrics_uses_python_runtime_metrics_contract() {
        let mcp = Arc::new(McpClientManager::new());
        mcp.seed_metric_for_tests("exa.search", 3, 2, 1, 150.0, Some(Utc::now()))
            .await;

        let payload = get_metrics(
            Extension(mcp),
            Query(MetricsQuery {
                tool_name: Some("exa.search".to_string()),
            }),
        )
        .await
        .0;

        assert_eq!(payload["tool_name"], json!("exa.search"));
        assert_eq!(payload["metrics"]["exa.search"]["total_calls"], json!(3));
        assert_eq!(payload["metrics"]["exa.search"]["success_calls"], json!(2));
        assert_eq!(payload["metrics"]["exa.search"]["failed_calls"], json!(1));
        assert_eq!(
            payload["metrics"]["exa.search"]["success_rate"],
            json!(0.667)
        );
        assert_eq!(
            payload["metrics"]["exa.search"]["avg_duration_ms"],
            json!(50.0)
        );
    }

    #[tokio::test]
    async fn get_cache_stats_uses_python_runtime_cache_contract() {
        let mcp = Arc::new(McpClientManager::new());
        mcp.seed_cache_entry_for_tests(
            "user-1:exa",
            vec![json!({"name": "search"}), json!({"name": "read"})],
            4,
            Utc::now() + Duration::minutes(5),
        )
        .await;

        let payload = get_cache_stats(Extension(mcp)).await.0;

        assert_eq!(payload["cache_stats"]["total_entries"], json!(1));
        assert_eq!(payload["cache_stats"]["total_hits"], json!(4));
        assert_eq!(payload["cache_stats"]["cache_ttl_minutes"], json!(5));
        assert_eq!(
            payload["cache_stats"]["entries"][0]["key"],
            json!("user-1:exa")
        );
        assert_eq!(
            payload["cache_stats"]["entries"][0]["tools_count"],
            json!(2)
        );
        assert_eq!(payload["cache_stats"]["entries"][0]["hit_count"], json!(4));
    }

    #[tokio::test]
    async fn clear_cache_removes_runtime_cache_entries_like_python() {
        let mcp = Arc::new(McpClientManager::new());
        mcp.seed_cache_entry_for_tests(
            "user-1:exa",
            vec![json!({"name": "search"})],
            1,
            Utc::now() + Duration::minutes(5),
        )
        .await;

        let _payload = build_clear_cache_payload(
            &mcp,
            &CacheClearQuery {
                user_id: None,
                plugin_name: Some("exa".to_string()),
            },
            "user-1",
        )
        .await
        .expect("cache clear should succeed");

        let payload = get_cache_stats(Extension(mcp)).await.0;
        assert_eq!(payload["cache_stats"]["total_entries"], json!(0));
        assert_eq!(payload["cache_stats"]["entries"], json!([]));
    }

    #[test]
    fn create_plugin_route_request_applies_python_default_category_and_sort_order_when_missing() {
        let request: CreateRequest = serde_json::from_value(json!({
            "plugin_name": "exa"
        }))
        .expect("missing defaulted Python fields should still deserialize");

        assert_eq!(request.category, None);
        assert_eq!(request.sort_order, None);
    }

    #[test]
    fn create_plugin_route_request_rejects_null_category_like_python_defaulted_string() {
        let error = serde_json::from_value::<CreateRequest>(json!({
            "plugin_name": "exa",
            "category": null
        }))
        .expect_err("explicit null category should fail like Python defaulted string");

        assert!(error.to_string().contains("invalid type: null"));
    }

    #[test]
    fn create_plugin_route_request_rejects_null_sort_order_like_python_defaulted_int() {
        let error = serde_json::from_value::<CreateRequest>(json!({
            "plugin_name": "exa",
            "sort_order": null
        }))
        .expect_err("explicit null sort_order should fail like Python defaulted int");

        assert!(error.to_string().contains("invalid type: null"));
    }

    #[test]
    fn create_plugin_route_request_accepts_python_sort_order_when_present() {
        let request: CreateRequest = serde_json::from_value(json!({
            "plugin_name": "exa",
            "sort_order": 12
        }))
        .expect("Python-valid sort_order should deserialize");

        assert_eq!(request.sort_order, Some(12));
    }

    #[test]
    fn simple_create_request_rejects_null_category_like_python_defaulted_string() {
        let error = serde_json::from_value::<SimpleCreateRequest>(json!({
            "config_json": "{\"mcpServers\":{\"exa\":{\"url\":\"https://example.com/mcp\"}}}",
            "category": null
        }))
        .expect_err("simple create category null should fail like Python defaulted string");

        assert!(error.to_string().contains("invalid type: null"));
    }

    #[test]
    fn get_tools_error_mapping_keeps_python_transport_contract() {
        let (status, body) = map_get_tools_error(GetToolsError::PluginDisabled);
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.0["detail"], "插件未启用");

        let (status, body) = map_get_tools_error(GetToolsError::RegisterFailed(
            "插件注册失败: exa".to_string(),
        ));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.0["detail"], "插件注册失败: exa");
    }

    #[test]
    fn call_tool_error_mapping_keeps_python_transport_contract() {
        let (status, body) = map_call_tool_error(CallToolError::PluginNotFound);
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.0["detail"], "插件不存在");

        let (status, body) =
            map_call_tool_error(CallToolError::CallFailed("工具调用失败: boom".to_string()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.0["detail"], "工具调用失败: boom");
    }

    #[test]
    fn test_plugin_error_mapping_keeps_python_transport_contract() {
        let (status, body) = map_test_plugin_error(TestPluginError::PluginNotFound);
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.0["detail"], "插件不存在");

        let (status, body) =
            map_test_plugin_error(TestPluginError::TestFailed("测试失败: boom".to_string()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.0["detail"], "测试失败: boom");
    }

    #[test]
    fn plugin_status_payload_uses_python_session_contract_when_registered() {
        let payload = apply_python_session_status_to_plugin_status(
            json!({
                "plugin_id": "plugin-1",
                "plugin_name": "exa",
                "db_status": "active",
                "session_status": null,
                "is_registered": false,
                "error_rate": 0,
                "in_sync": false,
                "timestamp": "2026-06-02T00:00:00+00:00"
            }),
            Some(&json!({
                "key": "user-1:exa",
                "status": "active",
                "error_rate": 0.125
            })),
        );

        assert_eq!(payload["session_status"], json!("active"));
        assert_eq!(payload["is_registered"], json!(true));
        assert_eq!(payload["error_rate"], json!(0.125));
        assert_eq!(payload["in_sync"], json!(true));
    }

    #[test]
    fn plugin_status_payload_uses_python_inactive_sync_contract_when_missing_session() {
        let payload = apply_python_session_status_to_plugin_status(
            json!({
                "plugin_id": "plugin-1",
                "plugin_name": "exa",
                "db_status": "inactive",
                "session_status": null,
                "is_registered": false,
                "error_rate": 0,
                "in_sync": false,
                "timestamp": "2026-06-02T00:00:00+00:00"
            }),
            None,
        );

        assert_eq!(payload["session_status"], Value::Null);
        assert_eq!(payload["is_registered"], json!(false));
        assert_eq!(payload["error_rate"], json!(0));
        assert_eq!(payload["in_sync"], json!(true));
    }
}
