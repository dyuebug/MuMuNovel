use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::mcp_plugin;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct McpPluginUpdateRequest {
    pub display_name: Option<String>,
    pub description: Option<Value>,
    pub server_url: Option<Value>,
    pub command: Option<Value>,
    pub enabled: Option<bool>,
    pub category: Option<String>,
    pub sort_order: Option<i64>,
    pub headers: Option<Value>,
    pub config: Option<Value>,
    pub args: Option<Value>,
    pub env: Option<Value>,
}

pub(crate) fn build_mcp_plugin_update_request(body: &Value) -> McpPluginUpdateRequest {
    McpPluginUpdateRequest {
        display_name: body
            .get("display_name")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        description: body.get("description").cloned(),
        server_url: body.get("server_url").cloned(),
        command: body.get("command").cloned(),
        enabled: body.get("enabled").and_then(Value::as_bool),
        category: body
            .get("category")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        sort_order: body.get("sort_order").and_then(Value::as_i64),
        headers: body.get("headers").cloned(),
        config: body.get("config").cloned(),
        args: body.get("args").cloned(),
        env: body.get("env").cloned(),
    }
}

fn optional_string_update(value: Option<&Value>) -> Option<Option<String>> {
    match value {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(text)) => Some(Some(text.clone())),
        Some(_) => None,
    }
}

fn optional_json_string_update(value: Option<&Value>) -> Option<Option<String>> {
    match value {
        None => None,
        Some(Value::Null) => Some(None),
        Some(other) => Some(Some(other.to_string())),
    }
}

fn plugin_to_dict(p: &mcp_plugin::Model) -> Value {
    json!({
        "id": p.id,
        "user_id": p.user_id,
        "plugin_name": p.plugin_name,
        "display_name": p.display_name,
        "description": p.description,
        "plugin_type": p.plugin_type,
        "server_url": p.server_url,
        "command": p.command,
        "args": p.args.as_ref().and_then(|a| serde_json::from_str::<Value>(a).ok()),
        "env": p.env.as_ref().and_then(|a| serde_json::from_str::<Value>(a).ok()),
        "headers": p.headers.as_ref().and_then(|a| serde_json::from_str::<Value>(a).ok()),
        "config": p.config.as_ref().and_then(|a| serde_json::from_str::<Value>(a).ok()),
        "tools": p.tools.as_ref().and_then(|a| serde_json::from_str::<Value>(a).ok()),
        "enabled": p.enabled,
        "status": p.status,
        "last_error": p.last_error,
        "last_test_at": p.last_test_at.map(|t| t.and_utc().to_rfc3339()),
        "category": p.category,
        "sort_order": p.sort_order,
        "created_at": p.created_at.and_utc().to_rfc3339(),
        "updated_at": p.updated_at.map(|t| t.and_utc().to_rfc3339()),
    })
}

fn supports_python_route_registration(plugin: &mcp_plugin::Model) -> bool {
    matches!(
        plugin.plugin_type.as_str(),
        "http" | "streamable_http" | "sse"
    ) && plugin.server_url.is_some()
}

async fn ensure_python_route_registered(
    plugin: &mcp_plugin::Model,
    user_id: &str,
    mcp_manager: &crate::mcp::McpClientManager,
) -> Result<bool, String> {
    if mcp_manager
        .is_registered(user_id, &plugin.plugin_name)
        .await
    {
        return Ok(true);
    }

    if !supports_python_route_registration(plugin) {
        return Ok(false);
    }

    let url = plugin
        .server_url
        .as_deref()
        .ok_or_else(|| "插件注册失败".to_string())?;

    mcp_manager
        .connect_sse(user_id, &plugin.plugin_name, url)
        .await
        .map_err(|_| format!("插件注册失败: {}", plugin.plugin_name))?;

    Ok(true)
}

fn build_test_plugin_pending_response_like_python() -> Value {
    json!({
        "success": false,
        "message": "正在建立连接...",
        "error": "插件会话正在初始化，请稍后重试",
        "suggestions": [
            "插件正在连接MCP服务器",
            "请等待2-3秒后再次点击测试",
            "如果持续失败，请检查服务器地址是否正确",
        ],
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum GetToolsError {
    PluginNotFound,
    PluginDisabled,
    RegisterFailed(String),
    FetchFailed(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum CallToolError {
    PluginNotFound,
    PluginDisabled,
    RegisterFailed(String),
    CallFailed(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum TestPluginError {
    PluginNotFound,
    TestFailed(String),
}

pub struct McpPluginService;

impl McpPluginService {
    pub async fn list(
        db: &DatabaseConnection,
        user_id: &str,
        enabled_only: bool,
        category: Option<&str>,
    ) -> Result<Vec<Value>, String> {
        use mcp_plugin::{Column as C, Entity};
        let mut query = Entity::find().filter(C::UserId.eq(user_id));
        if enabled_only {
            query = query.filter(C::Enabled.eq(true));
        }
        if let Some(c) = category {
            query = query.filter(C::Category.eq(c));
        }
        query = query.order_by_asc(C::SortOrder).order_by_asc(C::CreatedAt);
        let plugins = query.all(db).await.map_err(|e| format!("{}", e))?;
        Ok(plugins.iter().map(|p| plugin_to_dict(p)).collect())
    }

    pub async fn get(
        db: &DatabaseConnection,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<Option<Value>, String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(plugin.as_ref().map(|p| plugin_to_dict(p)))
    }

    pub async fn create(
        db: &DatabaseConnection,
        user_id: &str,
        plugin_name: &str,
        display_name: &str,
        description: Option<&str>,
        plugin_type: &str,
        server_url: Option<&str>,
        command: Option<&str>,
        args: Option<&str>,
        env: Option<&str>,
        headers: Option<&str>,
        config: Option<&str>,
        category: Option<&str>,
        sort_order: Option<i32>,
        enabled: bool,
    ) -> Result<Value, String> {
        use mcp_plugin::Column as C;

        // Check duplicate
        let existing = mcp_plugin::Entity::find()
            .filter(C::UserId.eq(user_id))
            .filter(C::PluginName.eq(plugin_name))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        if existing.is_some() {
            return Err(format!("插件名已存在: {}", plugin_name));
        }

        let now = Utc::now().naive_utc();
        let status = if enabled { "pending" } else { "inactive" };
        let model = mcp_plugin::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_string()),
            plugin_name: Set(plugin_name.to_string()),
            display_name: Set(display_name.to_string()),
            description: Set(description.map(|s| s.to_string())),
            plugin_type: Set(plugin_type.to_string()),
            server_url: Set(server_url.map(|s| s.to_string())),
            command: Set(command.map(|s| s.to_string())),
            args: Set(args.map(|s| s.to_string())),
            env: Set(env.map(|s| s.to_string())),
            headers: Set(headers.map(|s| s.to_string())),
            config: Set(config.map(|s| s.to_string())),
            tools: Set(None),
            enabled: Set(enabled),
            status: Set(status.to_string()),
            last_error: Set(None),
            last_test_at: Set(None),
            category: Set(category.unwrap_or("general").to_string()),
            sort_order: Set(sort_order.unwrap_or(0)),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        let inserted = model.insert(db).await.map_err(|e| format!("{}", e))?;
        Ok(plugin_to_dict(&inserted))
    }

    pub async fn create_or_update_simple(
        db: &DatabaseConnection,
        user_id: &str,
        config_json: &str,
        category: Option<&str>,
        enabled: bool,
    ) -> Result<Value, String> {
        let config: Value =
            serde_json::from_str(config_json).map_err(|e| format!("配置JSON格式错误: {}", e))?;
        let servers = config
            .get("mcpServers")
            .and_then(|s| s.as_object())
            .ok_or("配置JSON必须包含mcpServers字段")?;
        if servers.is_empty() {
            return Err("mcpServers不能为空".to_string());
        }
        let (plugin_name, server_config) = servers.iter().next().unwrap();

        let server_type = server_config
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("http");
        if !["http", "stdio", "streamable_http", "sse"].contains(&server_type) {
            return Err(format!("不支持的服务器类型: {}", server_type));
        }

        let server_url = server_config
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let headers_str = server_config.get("headers").map(|h| h.to_string());

        if ["http", "streamable_http", "sse"].contains(&server_type) && server_url.is_none() {
            return Err(format!("{}类型插件必须提供url字段", server_type));
        }

        let command = server_config
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let args_str = server_config.get("args").map(|a| a.to_string());
        let env_str = server_config.get("env").map(|e| e.to_string());

        if server_type == "stdio" && command.is_none() {
            return Err("Stdio类型插件必须提供command字段".to_string());
        }

        let now = Utc::now().naive_utc();
        let status = if enabled { "pending" } else { "inactive" };

        // Check if exists
        let existing = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .filter(mcp_plugin::Column::PluginName.eq(plugin_name))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;

        if let Some(existing) = existing {
            // Update existing
            let mut active: mcp_plugin::ActiveModel = existing.into();
            active.display_name = Set(plugin_name.to_string());
            active.plugin_type = Set(server_type.to_string());
            active.server_url = Set(server_url);
            active.command = Set(command);
            active.args = Set(args_str);
            active.env = Set(env_str);
            active.headers = Set(headers_str);
            active.category = Set(category.unwrap_or("general").to_string());
            active.enabled = Set(enabled);
            if enabled {
                active.status = Set("pending".to_string());
            }
            active.updated_at = Set(Some(now));
            let updated = active.update(db).await.map_err(|e| format!("{}", e))?;
            Ok(plugin_to_dict(&updated))
        } else {
            // Create new
            let model = mcp_plugin::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                user_id: Set(user_id.to_string()),
                plugin_name: Set(plugin_name.to_string()),
                display_name: Set(plugin_name.to_string()),
                description: Set(None),
                plugin_type: Set(server_type.to_string()),
                server_url: Set(server_url),
                command: Set(command),
                args: Set(args_str),
                env: Set(env_str),
                headers: Set(headers_str),
                config: Set(None),
                tools: Set(None),
                enabled: Set(enabled),
                status: Set(status.to_string()),
                last_error: Set(None),
                last_test_at: Set(None),
                category: Set(category.unwrap_or("general").to_string()),
                sort_order: Set(0),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            };
            let inserted = model.insert(db).await.map_err(|e| format!("{}", e))?;
            Ok(plugin_to_dict(&inserted))
        }
    }

    pub async fn update(
        db: &DatabaseConnection,
        plugin_id: &str,
        user_id: &str,
        updates: McpPluginUpdateRequest,
    ) -> Result<Option<Value>, String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(plugin) = plugin else {
            return Ok(None);
        };
        let mut active: mcp_plugin::ActiveModel = plugin.into();

        if let Some(v) = updates.display_name.as_ref() {
            active.display_name = Set(v.clone());
        }
        if let Some(value) = optional_string_update(updates.description.as_ref()) {
            active.description = Set(value);
        }
        if let Some(value) = optional_string_update(updates.server_url.as_ref()) {
            active.server_url = Set(value);
        }
        if let Some(value) = optional_string_update(updates.command.as_ref()) {
            active.command = Set(value);
        }
        if let Some(v) = updates.enabled {
            active.enabled = Set(v);
        }
        if let Some(v) = updates.category.as_ref() {
            active.category = Set(v.clone());
        }
        if let Some(v) = updates.sort_order {
            active.sort_order = Set(v as i32);
        }
        if let Some(value) = optional_json_string_update(updates.headers.as_ref()) {
            active.headers = Set(value);
        }
        if let Some(value) = optional_json_string_update(updates.config.as_ref()) {
            active.config = Set(value);
        }
        if let Some(value) = optional_json_string_update(updates.args.as_ref()) {
            active.args = Set(value);
        }
        if let Some(value) = optional_json_string_update(updates.env.as_ref()) {
            active.env = Set(value);
        }
        active.updated_at = Set(Some(Utc::now().naive_utc()));

        let updated = active.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(Some(plugin_to_dict(&updated)))
    }

    async fn register_updated_plugin_runtime_like_python(
        plugin: &mcp_plugin::Model,
        user_id: &str,
        mcp_manager: &crate::mcp::McpClientManager,
    ) -> Result<bool, String> {
        if !supports_python_route_registration(plugin) {
            return Ok(false);
        }

        let Some(url) = plugin.server_url.as_deref() else {
            return Ok(false);
        };

        match mcp_manager
            .connect_sse(user_id, &plugin.plugin_name, url)
            .await
        {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub async fn refresh_updated_plugin_runtime_like_python(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<(), String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(plugin) = plugin else {
            return Err("插件不存在".to_string());
        };

        if !plugin.enabled {
            return Ok(());
        }

        mcp_manager
            .disconnect(user_id, &plugin.plugin_name)
            .await
            .map_err(|e| format!("插件会话注销失败: {}", e))?;

        let _ = Self::register_updated_plugin_runtime_like_python(&plugin, user_id, mcp_manager)
            .await?;

        Ok(())
    }

    pub async fn delete(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<Option<Value>, String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(plugin) = plugin else {
            return Ok(None);
        };
        let name = plugin.plugin_name.clone();
        let _ = mcp_manager.disconnect(user_id, &name).await;
        mcp_plugin::Entity::delete_by_id(&plugin.id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(Some(json!({"message": "插件已删除", "plugin_name": name})))
    }

    pub async fn toggle(
        db: &DatabaseConnection,
        plugin_id: &str,
        user_id: &str,
        enabled: bool,
    ) -> Result<Option<Value>, String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(plugin) = plugin else {
            return Ok(None);
        };
        let plugin_name = plugin.plugin_name.clone();
        let mut active: mcp_plugin::ActiveModel = plugin.into();
        active.enabled = Set(enabled);
        if !enabled {
            active.status = Set("inactive".to_string());
        }
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        let updated = active.update(db).await.map_err(|e| format!("{}", e))?;
        let mut result = plugin_to_dict(&updated);
        result["plugin_name"] = json!(plugin_name);
        Ok(Some(result))
    }

    pub async fn get_status(
        db: &DatabaseConnection,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<Option<Value>, String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(plugin) = plugin else {
            return Ok(None);
        };
        Ok(Some(json!({
            "plugin_id": plugin.id,
            "plugin_name": plugin.plugin_name,
            "db_status": plugin.status,
            "session_status": null,
            "is_registered": false,
            "error_rate": 0,
            "in_sync": plugin.status == "inactive",
            "timestamp": Utc::now().to_rfc3339(),
        })))
    }

    pub fn get_metrics(metrics: Value, tool_name: Option<&str>) -> Value {
        json!({
            "metrics": metrics,
            "tool_name": tool_name,
            "timestamp": Utc::now().to_rfc3339(),
        })
    }

    pub fn get_cache_stats(cache_stats: Value) -> Value {
        json!({
            "cache_stats": cache_stats,
            "timestamp": Utc::now().to_rfc3339(),
        })
    }

    pub fn get_session_stats(sessions: Vec<Value>) -> Value {
        json!({
            "session_stats": {
                "total_sessions": sessions.len(),
                "sessions": sessions,
            },
            "timestamp": Utc::now().to_rfc3339(),
        })
    }

    pub fn clear_cache(target_user_id: Option<&str>, plugin_name: Option<&str>) -> Value {
        let msg = if let Some(n) = plugin_name {
            format!("已清理插件 {} 的缓存", n)
        } else if let Some(u) = target_user_id {
            format!("已清理用户 {} 的所有缓存", u)
        } else {
            "已清理所有缓存".to_string()
        };
        json!({"success": true, "message": msg, "timestamp": Utc::now().to_rfc3339()})
    }

    pub async fn get_tools(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<Value, GetToolsError> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| GetToolsError::FetchFailed(format!("获取工具列表失败: {}", e)))?;
        let Some(plugin) = plugin else {
            return Err(GetToolsError::PluginNotFound);
        };
        if !plugin.enabled {
            return Err(GetToolsError::PluginDisabled);
        }
        if !ensure_python_route_registered(&plugin, user_id, mcp_manager)
            .await
            .map_err(GetToolsError::RegisterFailed)?
        {
            return Err(GetToolsError::RegisterFailed(format!(
                "插件注册失败: {}",
                plugin.plugin_name
            )));
        }

        let tools = mcp_manager
            .list_tools(user_id, &plugin.plugin_name)
            .await
            .map_err(|e| GetToolsError::FetchFailed(format!("获取工具列表失败: {}", e)))?;

        let tools_json = json!(tools);
        let plugin_name = plugin.plugin_name.clone();
        let mut active: mcp_plugin::ActiveModel = plugin.into();
        active.tools = Set(Some(tools_json.to_string()));
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        let _ = active.update(db).await;

        Ok(json!({
            "plugin_name": plugin_name,
            "tools": tools_json,
            "count": tools_json.as_array().map_or(0, |a| a.len()),
        }))
    }

    pub async fn call_tool(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
        tool_name: &str,
        arguments: Option<&Value>,
    ) -> Result<Value, CallToolError> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| CallToolError::CallFailed(format!("工具调用失败: {}", e)))?;
        let Some(plugin) = plugin else {
            return Err(CallToolError::PluginNotFound);
        };
        if !plugin.enabled {
            return Err(CallToolError::PluginDisabled);
        }

        if !ensure_python_route_registered(&plugin, user_id, mcp_manager)
            .await
            .map_err(CallToolError::RegisterFailed)?
        {
            return Err(CallToolError::RegisterFailed(format!(
                "插件注册失败: {}",
                plugin.plugin_name
            )));
        }

        let result = mcp_manager
            .call_tool(user_id, &plugin.plugin_name, tool_name, arguments)
            .await
            .map_err(|e| CallToolError::CallFailed(format!("工具调用失败: {}", e)))?;

        Ok(json!({
            "success": true,
            "plugin_name": plugin.plugin_name,
            "tool_name": tool_name,
            "result": result,
        }))
    }

    /// Register (connect) a plugin's MCP server and cache the session
    pub async fn register_plugin(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<Value, String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(plugin) = plugin else {
            return Err("插件不存在".to_string());
        };

        // Already registered?
        if mcp_manager
            .is_registered(user_id, &plugin.plugin_name)
            .await
        {
            return Ok(
                json!({"success": true, "message": "插件会话已存在", "plugin_name": plugin.plugin_name}),
            );
        }

        // Connect based on plugin type
        let plugin_type = plugin.plugin_type.as_str();
        let result = match plugin_type {
            "sse" | "http" | "streamable_http" => {
                let url = plugin
                    .server_url
                    .as_deref()
                    .ok_or("SSE/HTTP插件缺少server_url")?;
                mcp_manager
                    .connect_sse(user_id, &plugin.plugin_name, url)
                    .await
            }
            "stdio" => {
                let cmd = plugin.command.as_deref().ok_or("Stdio插件缺少command")?;
                let args: Vec<String> = plugin
                    .args
                    .as_deref()
                    .and_then(|a| serde_json::from_str::<Vec<String>>(a).ok())
                    .unwrap_or_default();
                let env = plugin
                    .env
                    .as_ref()
                    .and_then(|e| serde_json::from_str::<Value>(e).ok());
                mcp_manager
                    .connect_stdio(user_id, &plugin.plugin_name, cmd, &args, env.as_ref())
                    .await
            }
            _ => Err(format!("不支持的插件类型: {}", plugin_type)),
        };

        match result {
            Ok(()) => {
                // Update DB status to active
                let p_name = plugin.plugin_name.clone();
                let mut active: mcp_plugin::ActiveModel = plugin.into();
                active.status = Set("active".to_string());
                active.last_error = Set(None);
                active.updated_at = Set(Some(Utc::now().naive_utc()));
                let _ = active.clone().update(db).await;

                // Cache tools to DB
                if let Ok(tools) = mcp_manager.list_tools(user_id, &p_name).await {
                    active.tools = Set(Some(serde_json::to_string(&tools).unwrap_or_default()));
                    let _ = active.update(db).await;
                }

                Ok(json!({"success": true, "message": "插件已连接", "plugin_name": p_name}))
            }
            Err(e) => {
                // Update DB status to error
                let mut active: mcp_plugin::ActiveModel = {
                    let p = mcp_plugin::Entity::find()
                        .filter(mcp_plugin::Column::Id.eq(plugin_id))
                        .one(db)
                        .await
                        .map_err(|e2| format!("{}", e2))?
                        .ok_or("插件不存在")?;
                    p.into()
                };
                active.status = Set("error".to_string());
                active.last_error = Set(Some(e.clone()));
                active.updated_at = Set(Some(Utc::now().naive_utc()));
                let _ = active.update(db).await;
                Err(e)
            }
        }
    }

    async fn queue_test_plugin_registration_like_python(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin: &mcp_plugin::Model,
        user_id: &str,
    ) -> Result<(), String> {
        let mut active: mcp_plugin::ActiveModel = plugin.clone().into();
        active.status = Set("pending".to_string());
        active.last_error = Set(None);
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await.map_err(|e| format!("{}", e))?;

        let db_clone = db.clone();
        let mcp_clone = mcp_manager.clone();
        let plugin_id = plugin.id.clone();
        let user_id = user_id.to_string();

        tokio::spawn(async move {
            let _ = Self::finalize_simple_create_runtime_state_like_python(
                &db_clone, &mcp_clone, &plugin_id, &user_id,
            )
            .await;
        });

        Ok(())
    }

    /// Test a plugin: connection test + optional AI Function Calling test
    pub async fn test_plugin(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<Value, TestPluginError> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| TestPluginError::TestFailed(format!("{}", e)))?;
        let Some(plugin) = plugin else {
            return Err(TestPluginError::PluginNotFound);
        };
        if !plugin.enabled {
            return Ok(json!({
                "success": false,
                "message": "插件未启用",
                "error": "请先启用插件",
                "suggestions": ["点击开关按钮启用插件"],
            }));
        }

        let plugin_name = plugin.plugin_name.clone();

        // Match Python's route contract: pending + background registration,
        // then ask the caller to retry instead of connecting synchronously.
        let plugin = if !mcp_manager.is_registered(user_id, &plugin_name).await {
            Self::queue_test_plugin_registration_like_python(db, mcp_manager, &plugin, user_id)
                .await
                .map_err(TestPluginError::TestFailed)?;
            return Ok(build_test_plugin_pending_response_like_python());
        } else {
            plugin
        };

        // Connection test
        let start = std::time::Instant::now();
        let conn_result = mcp_manager
            .test_connection(user_id, &plugin.plugin_name)
            .await
            .map_err(|e| TestPluginError::TestFailed(format!("连接测试失败: {}", e)))?;
        let elapsed = start.elapsed().as_millis();

        if !conn_result
            .get("success")
            .and_then(|v: &Value| v.as_bool())
            .unwrap_or(false)
        {
            let error = conn_result
                .get("error")
                .and_then(|v: &Value| v.as_str())
                .unwrap_or("未知错误");
            // Update DB
            let mut active: mcp_plugin::ActiveModel = plugin.into();
            active.status = Set("error".to_string());
            active.last_error = Set(Some(error.to_string()));
            active.last_test_at = Set(Some(Utc::now().naive_utc()));
            active.updated_at = Set(Some(Utc::now().naive_utc()));
            let _ = active.update(db).await;

            return Ok(json!({
                "success": false,
                "message": "连接测试失败",
                "response_time_ms": elapsed,
                "error": error,
                "suggestions": ["请检查服务器是否在线", "请确认配置正确", "请检查API Key是否有效"],
            }));
        }

        let tools_count = conn_result
            .get("tools_count")
            .and_then(|v: &Value| v.as_i64())
            .unwrap_or(0);

        // Try AI-powered tool test
        let ai_test_result = Self::test_plugin_with_ai(db, mcp_manager, user_id, &plugin).await;

        // Update DB
        let p = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .one(db)
            .await
            .map_err(|e| TestPluginError::TestFailed(format!("{}", e)))?;
        if let Some(p) = p {
            let mut a: mcp_plugin::ActiveModel = p.into();
            a.status = Set(if ai_test_result.is_ok() {
                "active".to_string()
            } else {
                "error".to_string()
            });
            a.last_error = Set(ai_test_result.as_ref().err().map(|e| e.clone()));
            a.last_test_at = Set(Some(Utc::now().naive_utc()));
            a.updated_at = Set(Some(Utc::now().naive_utc()));
            // Cache tools
            {
                let pn = mcp_plugin::Entity::find()
                    .filter(mcp_plugin::Column::Id.eq(plugin_id))
                    .one(db)
                    .await
                    .ok()
                    .flatten()
                    .map(|p| p.plugin_name)
                    .unwrap_or_default();
                if let Ok(tools) = mcp_manager.list_tools(user_id, &pn).await {
                    a.tools = Set(Some(serde_json::to_string(&tools).unwrap_or_default()));
                }
            }
            let _ = a.update(db).await;
        }

        match ai_test_result {
            Ok(ai_result) => Ok(json!({
                "success": true,
                "message": "Function Calling测试成功",
                "response_time_ms": elapsed,
                "tools_count": tools_count,
                "suggestions": ai_result["suggestions"].as_array().cloned().unwrap_or_default(),
            })),
            Err(ai_err) => Ok(json!({
                "success": true,  // connection succeeded even if AI test failed
                "message": "连接测试成功",
                "response_time_ms": elapsed,
                "tools_count": tools_count,
                "error": ai_err,
                "suggestions": [
                    format!("连接测试: 成功"),
                    format!("可用工具数: {}", tools_count),
                    "提示: 配置AI服务后可进行智能工具调用测试",
                ],
            })),
        }
    }

    async fn update_runtime_status(
        db: &DatabaseConnection,
        plugin: mcp_plugin::Model,
        status: &str,
        last_error: Option<String>,
    ) -> Result<Value, String> {
        let mut active: mcp_plugin::ActiveModel = plugin.into();
        active.status = Set(status.to_string());
        active.last_error = Set(last_error);
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        let updated = active.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(plugin_to_dict(&updated))
    }

    pub async fn finalize_create_runtime_state_like_python(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<Value, String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(plugin) = plugin else {
            return Err("插件不存在".to_string());
        };

        if !plugin.enabled {
            return Ok(plugin_to_dict(&plugin));
        }

        if supports_python_route_registration(&plugin)
            && Self::register_plugin(db, mcp_manager, plugin_id, user_id)
                .await
                .is_ok()
        {
            return Self::get(db, plugin_id, user_id)
                .await?
                .ok_or_else(|| "插件不存在".to_string());
        }

        Self::update_runtime_status(db, plugin, "error", Some("加载失败".to_string())).await
    }

    pub async fn finalize_simple_create_runtime_state_like_python(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<(), String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(plugin) = plugin else {
            return Ok(());
        };

        let _ = mcp_manager.disconnect(user_id, &plugin.plugin_name).await;

        if !plugin.enabled {
            return Ok(());
        }

        if supports_python_route_registration(&plugin)
            && Self::register_plugin(db, mcp_manager, plugin_id, user_id)
                .await
                .is_ok()
        {
            return Ok(());
        }

        Self::update_runtime_status(db, plugin, "error", Some("连接失败".to_string())).await?;
        Ok(())
    }

    pub async fn finalize_toggle_runtime_state_like_python(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
    ) -> Result<Value, String> {
        let plugin = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .filter(mcp_plugin::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(plugin) = plugin else {
            return Err("插件不存在".to_string());
        };

        if !plugin.enabled {
            return Ok(plugin_to_dict(&plugin));
        }

        if supports_python_route_registration(&plugin)
            && Self::register_plugin(db, mcp_manager, plugin_id, user_id)
                .await
                .is_ok()
        {
            let refreshed = mcp_plugin::Entity::find()
                .filter(mcp_plugin::Column::Id.eq(plugin_id))
                .filter(mcp_plugin::Column::UserId.eq(user_id))
                .one(db)
                .await
                .map_err(|e| format!("{}", e))?
                .ok_or_else(|| "插件不存在".to_string())?;

            if refreshed.status == "active" && refreshed.last_error.is_none() {
                return Ok(plugin_to_dict(&refreshed));
            }

            return Self::update_runtime_status(db, refreshed, "active", None).await;
        }

        Self::update_runtime_status(db, plugin, "error", Some("加载失败".to_string())).await
    }

    /// Use AI to select a tool and generate test parameters, then call it
    async fn test_plugin_with_ai(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        user_id: &str,
        plugin: &mcp_plugin::Model,
    ) -> Result<Value, String> {
        use crate::ai::service::AIService;
        use crate::services::prompt_template_service::PromptTemplateService;
        use crate::services::settings_service::SettingsService;

        let ai_config = SettingsService::build_ai_config(db, user_id, None, None, None)
            .await
            .map_err(|e| format!("AI配置加载失败: {}", e))?;
        let ai_service = AIService::new(ai_config);

        let openai_tools = mcp_manager
            .format_tools_for_openai(user_id, &plugin.plugin_name)
            .await?;
        if openai_tools.is_empty() {
            return Err("插件没有提供任何工具".to_string());
        }

        // Get MCP test prompt template
        let system_prompt = match PromptTemplateService::system_template_info("MCP_TOOL_TEST") {
            Some(t) => t.content.clone(),
            None => "你是一个MCP工具测试助手。请从提供的工具列表中选择一个适合测试的工具，并生成合理的测试参数。".to_string(),
        };

        let user_prompt = format!(
            "请从以下工具中选择一个进行测试，并生成合理的测试参数：\n\n{}",
            serde_json::to_string_pretty(&openai_tools).unwrap_or_default(),
        );

        // Call AI with tool calling — convert Value tools to ToolDef
        let tool_defs: Vec<crate::ai::types::ToolDef> = openai_tools
            .iter()
            .filter_map(|t| {
                let func = t.get("function")?;
                Some(crate::ai::types::ToolDef {
                    tool_type: "function".into(),
                    function: crate::ai::types::ToolFunction {
                        name: func.get("name")?.as_str()?.into(),
                        description: func.get("description")?.as_str()?.into(),
                        parameters: func.get("parameters")?.clone(),
                    },
                })
            })
            .collect();

        let response = ai_service
            .generate_text(&user_prompt, Some(&system_prompt), Some(&tool_defs))
            .await
            .map_err(|e| format!("AI调用失败: {}", e))?;

        let tool_calls = response.tool_calls.ok_or("AI未返回工具调用")?;
        let first_call = &tool_calls[0];
        let function = &first_call.function;

        // Parse tool name and arguments
        let (plugin_name, tool_name) =
            crate::mcp::McpClientManager::parse_function_name(&function.name)
                .map_err(|e| format!("解析函数名失败: {}", e))?;

        let arguments: Value =
            serde_json::from_str(&function.arguments).unwrap_or_else(|_| json!({}));

        // Call the MCP tool
        let call_start = std::time::Instant::now();
        let tool_result = mcp_manager
            .call_tool(user_id, &plugin_name, &tool_name, Some(&arguments))
            .await?;
        let call_time = call_start.elapsed().as_millis();

        let result_str = serde_json::to_string(&tool_result).unwrap_or_default();
        let result_preview = if result_str.len() > 800 {
            format!("{}...(结果已截断)", &result_str[..800])
        } else {
            result_str
        };

        Ok(json!({
            "success": true,
            "message": format!("Function Calling测试成功！工具 '{}' 调用正常", tool_name),
            "suggestions": [
                format!("AI选择: {}", tool_name),
                format!("参数: {}", serde_json::to_string(&arguments).unwrap_or_default()),
                format!("耗时: {}ms", call_time),
                format!("结果: {}", result_preview),
            ],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_mcp_plugin_update_request, build_test_plugin_pending_response_like_python,
        ensure_python_route_registered, mcp_plugin, optional_json_string_update,
        optional_string_update, supports_python_route_registration,
    };
    use crate::mcp::McpClientManager;
    use chrono::Utc;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DbBackend, Schema, Set};
    use serde_json::{json, Value};
    use uuid::Uuid;

    #[test]
    fn optional_string_update_preserves_missing_null_and_string_semantics() {
        assert_eq!(optional_string_update(None), None);
        assert_eq!(optional_string_update(Some(&Value::Null)), Some(None));
        assert_eq!(
            optional_string_update(Some(&json!("https://example.com"))),
            Some(Some("https://example.com".to_string()))
        );
        assert_eq!(optional_string_update(Some(&json!({"url": "bad"}))), None);
    }

    #[test]
    fn optional_json_string_update_preserves_missing_null_and_json_semantics() {
        assert_eq!(optional_json_string_update(None), None);
        assert_eq!(optional_json_string_update(Some(&Value::Null)), Some(None));
        assert_eq!(
            optional_json_string_update(Some(&json!({"Authorization": "Bearer token"}))),
            Some(Some("{\"Authorization\":\"Bearer token\"}".to_string()))
        );
        assert_eq!(
            optional_json_string_update(Some(&json!(["--stdio"]))),
            Some(Some("[\"--stdio\"]".to_string()))
        );
    }

    #[test]
    fn build_mcp_plugin_update_request_keeps_existing_contract() {
        let request = build_mcp_plugin_update_request(&json!({
            "display_name": "Weather Plugin",
            "description": "Fetch weather",
            "server_url": "https://example.com/mcp",
            "command": "node server.js",
            "enabled": true,
            "category": "tools",
            "sort_order": 12,
            "headers": {"Authorization": "Bearer token"},
            "config": {"timeout": 30},
            "args": ["--stdio"],
            "env": {"NODE_ENV": "production"}
        }));

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
    fn build_mcp_plugin_update_request_ignores_type_mismatches_for_string_and_sort_order_fields() {
        let request = build_mcp_plugin_update_request(&json!({
            "display_name": 123,
            "description": false,
            "server_url": {"url": "https://example.com"},
            "command": 9,
            "enabled": "true",
            "category": null,
            "sort_order": "12"
        }));

        assert_eq!(request.display_name, None);
        assert_eq!(request.description, Some(json!(false)));
        assert_eq!(
            request.server_url,
            Some(json!({"url": "https://example.com"}))
        );
        assert_eq!(request.command, Some(json!(9)));
        assert_eq!(request.enabled, None);
        assert_eq!(request.category, None);
        assert_eq!(request.sort_order, None);
    }

    #[test]
    fn build_mcp_plugin_update_request_keeps_null_json_fields_present() {
        let request = build_mcp_plugin_update_request(&json!({
            "headers": null,
            "config": null,
            "args": null,
            "env": null
        }));

        assert_eq!(request.headers, Some(Value::Null));
        assert_eq!(request.config, Some(Value::Null));
        assert_eq!(request.args, Some(Value::Null));
        assert_eq!(request.env, Some(Value::Null));
    }

    #[test]
    fn build_mcp_plugin_update_request_ignores_python_unsupported_plugin_type_field() {
        let request = build_mcp_plugin_update_request(&json!({
            "plugin_type": "http"
        }));

        assert_eq!(request.display_name, None);
        assert_eq!(request.description, None);
        assert_eq!(request.server_url, None);
        assert_eq!(request.command, None);
        assert_eq!(request.enabled, None);
        assert_eq!(request.category, None);
        assert_eq!(request.sort_order, None);
        assert_eq!(request.headers, None);
        assert_eq!(request.config, None);
        assert_eq!(request.args, None);
        assert_eq!(request.env, None);
    }

    fn plugin_model(plugin_type: &str, server_url: Option<&str>) -> mcp_plugin::Model {
        let now = Utc::now().naive_utc();
        mcp_plugin::Model {
            id: Uuid::new_v4().to_string(),
            user_id: "user-1".to_string(),
            plugin_name: "exa".to_string(),
            display_name: "Exa".to_string(),
            description: None,
            plugin_type: plugin_type.to_string(),
            server_url: server_url.map(|value| value.to_string()),
            command: None,
            args: None,
            env: None,
            headers: None,
            config: None,
            tools: None,
            enabled: true,
            status: "pending".to_string(),
            last_error: None,
            last_test_at: None,
            category: "general".to_string(),
            sort_order: 0,
            created_at: now,
            updated_at: Some(now),
        }
    }

    async fn setup_mcp_plugin_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(mcp_plugin::Entity)))
            .await
            .expect("create mcp_plugins table");
        db
    }

    async fn insert_plugin(
        db: &sea_orm::DatabaseConnection,
        plugin: &mcp_plugin::Model,
    ) -> mcp_plugin::Model {
        mcp_plugin::ActiveModel {
            id: Set(plugin.id.clone()),
            user_id: Set(plugin.user_id.clone()),
            plugin_name: Set(plugin.plugin_name.clone()),
            display_name: Set(plugin.display_name.clone()),
            description: Set(plugin.description.clone()),
            plugin_type: Set(plugin.plugin_type.clone()),
            server_url: Set(plugin.server_url.clone()),
            command: Set(plugin.command.clone()),
            args: Set(plugin.args.clone()),
            env: Set(plugin.env.clone()),
            headers: Set(plugin.headers.clone()),
            config: Set(plugin.config.clone()),
            tools: Set(plugin.tools.clone()),
            enabled: Set(plugin.enabled),
            status: Set(plugin.status.clone()),
            last_error: Set(plugin.last_error.clone()),
            last_test_at: Set(plugin.last_test_at),
            category: Set(plugin.category.clone()),
            sort_order: Set(plugin.sort_order),
            created_at: Set(plugin.created_at),
            updated_at: Set(plugin.updated_at),
        }
        .insert(db)
        .await
        .expect("insert plugin")
    }

    #[test]
    fn supports_python_route_registration_only_for_http_style_plugins_with_url() {
        assert!(supports_python_route_registration(&plugin_model(
            "http",
            Some("https://example.com/mcp")
        )));
        assert!(supports_python_route_registration(&plugin_model(
            "streamable_http",
            Some("https://example.com/mcp")
        )));
        assert!(supports_python_route_registration(&plugin_model(
            "sse",
            Some("https://example.com/mcp")
        )));
        assert!(!supports_python_route_registration(&plugin_model(
            "stdio", None
        )));
        assert!(!supports_python_route_registration(&plugin_model(
            "http", None
        )));
    }

    #[tokio::test]
    async fn ensure_python_route_registered_skips_stdio_like_python_runtime_owner() {
        let plugin = plugin_model("stdio", None);
        let manager = McpClientManager::new();

        let registered = ensure_python_route_registered(&plugin, "user-1", &manager)
            .await
            .expect("stdio plugins should not error in Python parity helper");

        assert!(!registered);
        assert!(!manager.is_registered("user-1", "exa").await);
    }

    #[tokio::test]
    async fn refresh_updated_plugin_runtime_like_python_skips_stdio_without_error() {
        let db = setup_mcp_plugin_db().await;
        let plugin = plugin_model("stdio", None);
        let inserted = insert_plugin(&db, &plugin).await;
        let manager = McpClientManager::new();

        super::McpPluginService::refresh_updated_plugin_runtime_like_python(
            &db,
            &manager,
            &inserted.id,
            "user-1",
        )
        .await
        .expect("stdio update refresh should be best-effort and non-fatal");

        assert!(!manager.is_registered("user-1", "exa").await);
    }

    #[tokio::test]
    async fn delete_unregisters_runtime_like_python_before_removing_db_row() {
        let db = setup_mcp_plugin_db().await;
        let plugin = plugin_model("stdio", None);
        let inserted = insert_plugin(&db, &plugin).await;
        let manager = McpClientManager::new();

        super::McpPluginService::delete(&db, &manager, &inserted.id, "user-1")
            .await
            .expect("delete should succeed");

        assert_eq!(
            manager.disconnect_calls_for_tests().await,
            vec![("user-1".to_string(), "exa".to_string())]
        );

        let payload = super::McpPluginService::get(&db, &inserted.id, "user-1")
            .await
            .expect("get should not error after delete");
        assert!(payload.is_none());
    }

    #[test]
    fn test_plugin_pending_response_keeps_python_retry_contract() {
        let payload = build_test_plugin_pending_response_like_python();

        assert_eq!(payload["success"], json!(false));
        assert_eq!(payload["message"], json!("正在建立连接..."));
        assert_eq!(payload["error"], json!("插件会话正在初始化，请稍后重试"));
        assert_eq!(
            payload["suggestions"],
            json!([
                "插件正在连接MCP服务器",
                "请等待2-3秒后再次点击测试",
                "如果持续失败，请检查服务器地址是否正确",
            ])
        );
    }

    #[test]
    fn get_session_stats_keeps_python_session_list_contract() {
        let payload = super::McpPluginService::get_session_stats(vec![json!({
            "key": "user-1:exa",
            "url": "https://example.com/mcp",
            "status": "active",
            "request_count": 0,
            "error_count": 0,
            "error_rate": 0.0,
            "created_at": "2026-06-02T00:00:00+00:00",
            "last_access": "2026-06-02T00:00:00+00:00",
        })]);

        assert_eq!(payload["session_stats"]["total_sessions"], json!(1));
        assert_eq!(
            payload["session_stats"]["sessions"][0]["key"],
            json!("user-1:exa")
        );
        assert_eq!(
            payload["session_stats"]["sessions"][0]["status"],
            json!("active")
        );
    }
}
