use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct McpPluginUpdateRequest {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub plugin_type: Option<String>,
    pub server_url: Option<String>,
    pub command: Option<String>,
    pub category: Option<String>,
    pub sort_order: Option<i64>,
    pub headers: Option<Value>,
    pub config: Option<Value>,
    pub args: Option<Value>,
    pub env: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct McpPluginUpdateRouteRequest {
    #[serde(default)]
    pub display_name: Option<Value>,
    #[serde(default)]
    pub description: Option<Value>,
    #[serde(default)]
    pub plugin_type: Option<Value>,
    #[serde(default)]
    pub server_url: Option<Value>,
    #[serde(default)]
    pub command: Option<Value>,
    #[serde(default)]
    pub category: Option<Value>,
    #[serde(default)]
    pub sort_order: Option<Value>,
    #[serde(default)]
    pub headers: Option<Value>,
    #[serde(default)]
    pub config: Option<Value>,
    #[serde(default)]
    pub args: Option<Value>,
    #[serde(default)]
    pub env: Option<Value>,
}

impl McpPluginUpdateRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "display_name": self.display_name,
            "description": self.description,
            "plugin_type": self.plugin_type,
            "server_url": self.server_url,
            "command": self.command,
            "category": self.category,
            "sort_order": self.sort_order,
            "headers": self.headers,
            "config": self.config,
            "args": self.args,
            "env": self.env,
        })
    }
}

pub(crate) fn build_mcp_plugin_update_request(body: &Value) -> McpPluginUpdateRequest {
    McpPluginUpdateRequest {
        display_name: body
            .get("display_name")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        description: body
            .get("description")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        plugin_type: body
            .get("plugin_type")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        server_url: body
            .get("server_url")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        command: body
            .get("command")
            .and_then(Value::as_str)
            .map(ToString::to_string),
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

pub(crate) fn build_mcp_plugin_update_request_from_typed_route_payload(
    route_request: McpPluginUpdateRouteRequest,
) -> McpPluginUpdateRequest {
    build_mcp_plugin_update_request(&route_request.into_body())
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_mcp_plugin_update_request, build_mcp_plugin_update_request_from_typed_route_payload,
        McpPluginUpdateRouteRequest,
    };

    #[test]
    fn build_mcp_plugin_update_request_keeps_existing_contract() {
        let request = build_mcp_plugin_update_request(&json!({
            "display_name": "Weather Plugin",
            "description": "Fetch weather",
            "plugin_type": "http",
            "server_url": "https://example.com/mcp",
            "command": "node server.js",
            "category": "tools",
            "sort_order": 12,
            "headers": {"Authorization": "Bearer token"},
            "config": {"timeout": 30},
            "args": ["--stdio"],
            "env": {"NODE_ENV": "production"}
        }));

        assert_eq!(request.display_name.as_deref(), Some("Weather Plugin"));
        assert_eq!(request.description.as_deref(), Some("Fetch weather"));
        assert_eq!(request.plugin_type.as_deref(), Some("http"));
        assert_eq!(
            request.server_url.as_deref(),
            Some("https://example.com/mcp")
        );
        assert_eq!(request.command.as_deref(), Some("node server.js"));
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
            "plugin_type": ["http"],
            "server_url": {"url": "https://example.com"},
            "command": 9,
            "category": null,
            "sort_order": "12"
        }));

        assert_eq!(request.display_name, None);
        assert_eq!(request.description, None);
        assert_eq!(request.plugin_type, None);
        assert_eq!(request.server_url, None);
        assert_eq!(request.command, None);
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
    fn build_mcp_plugin_update_request_from_typed_route_payload_keeps_existing_contract() {
        let request =
            build_mcp_plugin_update_request_from_typed_route_payload(McpPluginUpdateRouteRequest {
                display_name: Some(json!("Weather Plugin")),
                description: Some(json!("Fetch weather")),
                plugin_type: Some(json!("http")),
                server_url: Some(json!("https://example.com/mcp")),
                command: Some(json!("node server.js")),
                category: Some(json!("tools")),
                sort_order: Some(json!(12)),
                headers: Some(json!({"Authorization": "Bearer token"})),
                config: Some(json!({"timeout": 30})),
                args: Some(json!(["--stdio"])),
                env: Some(json!({"NODE_ENV": "production"})),
            });

        assert_eq!(request.display_name.as_deref(), Some("Weather Plugin"));
        assert_eq!(request.description.as_deref(), Some("Fetch weather"));
        assert_eq!(request.plugin_type.as_deref(), Some("http"));
        assert_eq!(
            request.server_url.as_deref(),
            Some("https://example.com/mcp")
        );
        assert_eq!(request.command.as_deref(), Some("node server.js"));
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
}
