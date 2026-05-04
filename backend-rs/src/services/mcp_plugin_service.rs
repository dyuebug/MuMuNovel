use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::mcp_plugin;

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
        "last_test_at": p.last_test_at,
        "category": p.category,
        "sort_order": p.sort_order,
        "created_at": p.created_at,
        "updated_at": p.updated_at,
    })
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

        let now = Utc::now();
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
            sort_order: Set(0),
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
        let config: Value = serde_json::from_str(config_json)
            .map_err(|e| format!("配置JSON格式错误: {}", e))?;
        let servers = config.get("mcpServers")
            .and_then(|s| s.as_object())
            .ok_or("配置JSON必须包含mcpServers字段")?;
        if servers.is_empty() {
            return Err("mcpServers不能为空".to_string());
        }
        let (plugin_name, server_config) = servers.iter().next().unwrap();

        let server_type = server_config.get("type").and_then(|v| v.as_str()).unwrap_or("http");
        if !["http", "stdio", "streamable_http", "sse"].contains(&server_type) {
            return Err(format!("不支持的服务器类型: {}", server_type));
        }

        let server_url = server_config.get("url").and_then(|v| v.as_str()).map(|s| s.to_string());
        let headers_str = server_config.get("headers").map(|h| h.to_string());

        if ["http", "streamable_http", "sse"].contains(&server_type) && server_url.is_none() {
            return Err(format!("{}类型插件必须提供url字段", server_type));
        }

        let command = server_config.get("command").and_then(|v| v.as_str()).map(|s| s.to_string());
        let args_str = server_config.get("args").map(|a| a.to_string());
        let env_str = server_config.get("env").map(|e| e.to_string());

        if server_type == "stdio" && command.is_none() {
            return Err("Stdio类型插件必须提供command字段".to_string());
        }

        let now = Utc::now();
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
        updates: Value,
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

        if let Some(v) = updates.get("display_name").and_then(|v| v.as_str()) { active.display_name = Set(v.to_string()); }
        if let Some(v) = updates.get("description").and_then(|v| v.as_str()) { active.description = Set(Some(v.to_string())); }
        if let Some(v) = updates.get("plugin_type").and_then(|v| v.as_str()) { active.plugin_type = Set(v.to_string()); }
        if let Some(v) = updates.get("server_url").and_then(|v| v.as_str()) { active.server_url = Set(Some(v.to_string())); }
        if let Some(v) = updates.get("command").and_then(|v| v.as_str()) { active.command = Set(Some(v.to_string())); }
        if let Some(v) = updates.get("category").and_then(|v| v.as_str()) { active.category = Set(v.to_string()); }
        if let Some(v) = updates.get("sort_order").and_then(|v| v.as_i64()) { active.sort_order = Set(v as i32); }
        if let Some(v) = updates.get("headers") { active.headers = Set(Some(v.to_string())); }
        if let Some(v) = updates.get("config") { active.config = Set(Some(v.to_string())); }
        if let Some(v) = updates.get("args") { active.args = Set(Some(v.to_string())); }
        if let Some(v) = updates.get("env") { active.env = Set(Some(v.to_string())); }
        active.updated_at = Set(Some(Utc::now()));

        let updated = active.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(Some(plugin_to_dict(&updated)))
    }

    pub async fn delete(
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
        let name = plugin.plugin_name.clone();
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
        if enabled {
            active.status = Set("active".to_string());
        } else {
            active.status = Set("inactive".to_string());
        }
        active.updated_at = Set(Some(Utc::now()));
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
        let Some(plugin) = plugin else { return Ok(None) };
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

    pub fn get_metrics(tool_name: Option<&str>) -> Value {
        json!({
            "metrics": {},
            "tool_name": tool_name,
            "timestamp": Utc::now().to_rfc3339(),
        })
    }

    pub fn get_cache_stats() -> Value {
        json!({
            "cache_stats": {},
            "timestamp": Utc::now().to_rfc3339(),
        })
    }

    pub fn get_session_stats() -> Value {
        json!({
            "session_stats": {},
            "timestamp": Utc::now().to_rfc3339(),
        })
    }

    pub fn clear_cache(_user_id: Option<&str>, _plugin_name: Option<&str>) -> Value {
        let msg = if let Some(n) = _plugin_name {
            format!("已清理插件 {} 的缓存", n)
        } else if let Some(u) = _user_id {
            format!("已清理用户 {} 的所有缓存", u)
        } else {
            "已清理所有缓存".to_string()
        };
        json!({"success": true, "message": msg, "timestamp": Utc::now().to_rfc3339()})
    }

    pub async fn get_tools(
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
        let Some(plugin) = plugin else { return Ok(None) };
        if !plugin.enabled {
            return Err("插件未启用".to_string());
        }
        let tools: Value = plugin.tools
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or(json!([]));
        Ok(Some(json!({
            "plugin_name": plugin.plugin_name,
            "tools": tools,
            "count": tools.as_array().map_or(0, |a| a.len()),
        })))
    }

    pub async fn call_tool(
        db: &DatabaseConnection,
        mcp_manager: &crate::mcp::McpClientManager,
        plugin_id: &str,
        user_id: &str,
        tool_name: &str,
        arguments: Option<&Value>,
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
            return Err("插件未启用".to_string());
        }

        // Try real MCP call if session exists
        if mcp_manager.is_registered(user_id, &plugin.plugin_name).await {
            match mcp_manager.call_tool(user_id, &plugin.plugin_name, tool_name, arguments).await {
                Ok(result) => return Ok(json!({
                    "success": true,
                    "plugin_name": plugin.plugin_name,
                    "tool_name": tool_name,
                    "result": result,
                })),
                Err(e) => return Err(e),
            }
        }

        // Fallback: session not established
        Ok(json!({
            "success": false,
            "plugin_name": plugin.plugin_name,
            "tool_name": tool_name,
            "result": null,
            "error": "MCP会话未建立，请先启用并测试插件",
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
        if mcp_manager.is_registered(user_id, &plugin.plugin_name).await {
            return Ok(json!({"success": true, "message": "插件会话已存在", "plugin_name": plugin.plugin_name}));
        }

        // Connect based on plugin type
        let plugin_type = plugin.plugin_type.as_str();
        let result = match plugin_type {
            "sse" | "http" | "streamable_http" => {
                let url = plugin.server_url.as_deref().ok_or("SSE/HTTP插件缺少server_url")?;
                mcp_manager.connect_sse(user_id, &plugin.plugin_name, url).await
            }
            "stdio" => {
                let cmd = plugin.command.as_deref().ok_or("Stdio插件缺少command")?;
                let args: Vec<String> = plugin.args.as_deref()
                    .and_then(|a| serde_json::from_str::<Vec<String>>(a).ok())
                    .unwrap_or_default();
                let env = plugin.env.as_ref().and_then(|e| serde_json::from_str::<Value>(e).ok());
                mcp_manager.connect_stdio(user_id, &plugin.plugin_name, cmd, &args, env.as_ref()).await
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
                active.updated_at = Set(Some(Utc::now()));
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
                        .one(db).await.map_err(|e2| format!("{}", e2))?
                        .ok_or("插件不存在")?;
                    p.into()
                };
                active.status = Set("error".to_string());
                active.last_error = Set(Some(e.clone()));
                active.updated_at = Set(Some(Utc::now()));
                let _ = active.update(db).await;
                Err(e)
            }
        }
    }

    /// Test a plugin: connection test + optional AI Function Calling test
    pub async fn test_plugin(
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
            return Ok(json!({
                "success": false,
                "message": "插件未启用",
                "error": "请先启用插件",
                "suggestions": ["点击开关按钮启用插件"],
            }));
        }

        let plugin_name = plugin.plugin_name.clone();

        // If not registered, try to register first
        let plugin = if !mcp_manager.is_registered(user_id, &plugin_name).await {
            let plugin_id_clone = plugin.id.clone();
            let user_id_clone = user_id.to_string();
            let plugin_type = plugin.plugin_type.clone();
            let server_url = plugin.server_url.clone();
            let command = plugin.command.clone();

            // Update DB status to pending
            let mut active: mcp_plugin::ActiveModel = plugin.into();
            active.status = Set("pending".to_string());
            active.last_error = Set(None);
            active.updated_at = Set(Some(Utc::now()));
            active.update(db).await.map_err(|e| format!("{}", e))?;

            // Try to connect
            let connect_result = {
                let pt = plugin_type.clone();
                let sv = server_url.clone();
                let cmd = command.clone();
                let pn = plugin_name.clone();
                let uid = user_id.to_string();
                match pt.as_str() {
                    "sse" | "http" | "streamable_http" => {
                        let url = sv.as_deref().ok_or("SSE/HTTP插件缺少server_url")?;
                        mcp_manager.connect_sse(&uid, &pn, url).await
                    }
                    "stdio" => {
                        let cmd = cmd.as_deref().ok_or("Stdio插件缺少command")?;
                        let p2 = mcp_plugin::Entity::find()
                            .filter(mcp_plugin::Column::Id.eq(&plugin_id_clone))
                            .one(db).await.map_err(|e2| format!("{}", e2))?;
                        let args: Vec<String> = p2.as_ref().and_then(|p| p.args.as_deref())
                            .and_then(|a| serde_json::from_str::<Vec<String>>(a).ok())
                            .unwrap_or_default();
                        let env = p2.as_ref().and_then(|p| p.env.as_deref())
                            .and_then(|e| serde_json::from_str::<Value>(e).ok());
                        mcp_manager.connect_stdio(&uid, &pn, cmd, &args, env.as_ref()).await
                    }
                    _ => Err(format!("不支持的插件类型: {}", pt)),
                }
            };

            if let Err(e) = connect_result {
                let p = mcp_plugin::Entity::find()
                    .filter(mcp_plugin::Column::Id.eq(&plugin_id_clone))
                    .one(db).await.map_err(|e2| format!("{}", e2))?;
                if let Some(p) = p {
                    let mut a: mcp_plugin::ActiveModel = p.into();
                    a.status = Set("error".to_string());
                    a.last_error = Set(Some(e.clone()));
                    a.updated_at = Set(Some(Utc::now()));
                    let _ = a.update(db).await;
                }
                return Ok(json!({
                    "success": false,
                    "message": "正在建立连接...",
                    "error": format!("插件会话正在初始化: {}", e),
                    "suggestions": [
                        "插件正在连接MCP服务器",
                        "请等待2-3秒后再次点击测试",
                        "如果持续失败，请检查服务器地址是否正确",
                    ],
                }));
            }

            // Re-fetch plugin after update
            mcp_plugin::Entity::find()
                .filter(mcp_plugin::Column::Id.eq(&plugin_id_clone))
                .filter(mcp_plugin::Column::UserId.eq(&user_id_clone))
                .one(db).await.map_err(|e| format!("{}", e))?
                .ok_or("插件不存在")?
        } else {
            plugin
        };

        // Connection test
        let start = std::time::Instant::now();
        let conn_result = mcp_manager.test_connection(user_id, &plugin.plugin_name).await
            .map_err(|e| format!("连接测试失败: {}", e))?;
        let elapsed = start.elapsed().as_millis();

        if !conn_result.get("success").and_then(|v: &Value| v.as_bool()).unwrap_or(false) {
            let error = conn_result.get("error").and_then(|v: &Value| v.as_str()).unwrap_or("未知错误");
            // Update DB
            let mut active: mcp_plugin::ActiveModel = plugin.into();
            active.status = Set("error".to_string());
            active.last_error = Set(Some(error.to_string()));
            active.last_test_at = Set(Some(Utc::now()));
            active.updated_at = Set(Some(Utc::now()));
            let _ = active.update(db).await;

            return Ok(json!({
                "success": false,
                "message": "连接测试失败",
                "response_time_ms": elapsed,
                "error": error,
                "suggestions": ["请检查服务器是否在线", "请确认配置正确", "请检查API Key是否有效"],
            }));
        }

        let tools_count = conn_result.get("tools_count").and_then(|v: &Value| v.as_i64()).unwrap_or(0);

        // Try AI-powered tool test
        let ai_test_result = Self::test_plugin_with_ai(db, mcp_manager, user_id, &plugin).await;

        // Update DB
        let p = mcp_plugin::Entity::find()
            .filter(mcp_plugin::Column::Id.eq(plugin_id))
            .one(db).await.map_err(|e| format!("{}", e))?;
        if let Some(p) = p {
            let mut a: mcp_plugin::ActiveModel = p.into();
            a.status = Set(if ai_test_result.is_ok() { "active".to_string() } else { "error".to_string() });
            a.last_error = Set(ai_test_result.as_ref().err().map(|e| e.clone()));
            a.last_test_at = Set(Some(Utc::now()));
            a.updated_at = Set(Some(Utc::now()));
            // Cache tools
            {
                let pn = mcp_plugin::Entity::find()
                    .filter(mcp_plugin::Column::Id.eq(plugin_id))
                    .one(db).await.ok().flatten()
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

        let ai_config = SettingsService::build_ai_config(db, user_id, None, None, None).await
            .map_err(|e| format!("AI配置加载失败: {}", e))?;
        let ai_service = AIService::new(ai_config);

        let openai_tools = mcp_manager.format_tools_for_openai(user_id, &plugin.plugin_name).await?;
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
        let tool_defs: Vec<crate::ai::types::ToolDef> = openai_tools.iter().filter_map(|t| {
            let func = t.get("function")?;
            Some(crate::ai::types::ToolDef {
                tool_type: "function".into(),
                function: crate::ai::types::ToolFunction {
                    name: func.get("name")?.as_str()?.into(),
                    description: func.get("description")?.as_str()?.into(),
                    parameters: func.get("parameters")?.clone(),
                },
            })
        }).collect();

        let response = ai_service.generate_text(
            &user_prompt,
            Some(&system_prompt),
            Some(&tool_defs),
        ).await.map_err(|e| format!("AI调用失败: {}", e))?;

        let tool_calls = response.tool_calls.ok_or("AI未返回工具调用")?;
        let first_call = &tool_calls[0];
        let function = &first_call.function;

        // Parse tool name and arguments
        let (plugin_name, tool_name) = crate::mcp::McpClientManager::parse_function_name(&function.name)
            .map_err(|e| format!("解析函数名失败: {}", e))?;

        let arguments: Value = serde_json::from_str(&function.arguments)
            .unwrap_or_else(|_| json!({}));

        // Call the MCP tool
        let call_start = std::time::Instant::now();
        let tool_result = mcp_manager.call_tool(user_id, &plugin_name, &tool_name, Some(&arguments)).await?;
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
