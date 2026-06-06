use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) enum RouteValueField {
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct McpPluginUpdateRouteRequest {
    #[serde(default)]
    pub display_name: Option<Value>,
    #[serde(default)]
    pub description: RouteValueField,
    #[serde(default)]
    pub server_url: RouteValueField,
    #[serde(default)]
    pub command: RouteValueField,
    #[serde(default)]
    pub enabled: Option<Value>,
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
        McpPluginUpdateRouteRequest, RouteValueField,
    };

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
}
