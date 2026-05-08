use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::RoleClient;
use rmcp::ServiceExt;

type McpClient = RunningService<RoleClient, ClientInfo>;

#[derive(Clone, Copy, PartialEq)]
enum TransportType {
    Sse,
    Stdio,
}

#[allow(dead_code)]
struct McpSession {
    client: McpClient,
    transport_type: TransportType,
    plugin_name: String,
    connected_at: chrono::DateTime<Utc>,
}

pub struct McpClientManager {
    sessions: Arc<Mutex<HashMap<(String, String), McpSession>>>,
}

impl McpClientManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn is_registered(&self, user_id: &str, plugin_name: &str) -> bool {
        let sessions = self.sessions.lock().await;
        sessions.contains_key(&(user_id.to_string(), plugin_name.to_string()))
    }

    pub async fn get_session_status(&self, user_id: &str, plugin_name: &str) -> &'static str {
        let sessions = self.sessions.lock().await;
        if sessions.contains_key(&(user_id.to_string(), plugin_name.to_string())) {
            "connected"
        } else {
            "disconnected"
        }
    }

    pub async fn connect_sse(
        &self,
        user_id: &str,
        plugin_name: &str,
        server_url: &str,
    ) -> Result<(), String> {
        let transport = StreamableHttpClientTransport::from_uri(server_url);
        let client_info = ClientInfo::new(
            ClientCapabilities::default(),
            Implementation::new("mumu-novel-backend", env!("CARGO_PKG_VERSION")),
        );
        let client = client_info
            .serve(transport)
            .await
            .map_err(|e| format!("MCP SSE连接失败: {}", e))?;

        let session = McpSession {
            client,
            transport_type: TransportType::Sse,
            plugin_name: plugin_name.to_string(),
            connected_at: Utc::now(),
        };
        let mut sessions = self.sessions.lock().await;
        sessions.insert((user_id.to_string(), plugin_name.to_string()), session);
        Ok(())
    }

    pub async fn connect_stdio(
        &self,
        user_id: &str,
        plugin_name: &str,
        command: &str,
        args: &[String],
        env: Option<&Value>,
    ) -> Result<(), String> {
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(env_obj) = env.and_then(|v| v.as_object()) {
            for (k, v) in env_obj {
                if let Some(s) = v.as_str() {
                    cmd.env(k, s);
                }
            }
        }

        let transport = rmcp::transport::TokioChildProcess::new(cmd)
            .map_err(|e| format!("MCP stdio进程启动失败: {}", e))?;
        let client_info = ClientInfo::new(
            ClientCapabilities::default(),
            Implementation::new("mumu-novel-backend", env!("CARGO_PKG_VERSION")),
        );
        let client = client_info
            .serve(transport)
            .await
            .map_err(|e| format!("MCP stdio连接失败: {}", e))?;

        let session = McpSession {
            client,
            transport_type: TransportType::Stdio,
            plugin_name: plugin_name.to_string(),
            connected_at: Utc::now(),
        };
        let mut sessions = self.sessions.lock().await;
        sessions.insert((user_id.to_string(), plugin_name.to_string()), session);
        Ok(())
    }

    pub async fn disconnect(&self, user_id: &str, plugin_name: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.remove(&(user_id.to_string(), plugin_name.to_string())) {
            let _ = session.client.cancel().await;
        }
        Ok(())
    }

    pub async fn list_tools(&self, user_id: &str, plugin_name: &str) -> Result<Vec<Value>, String> {
        let sessions = self.sessions.lock().await;
        let key = (user_id.to_string(), plugin_name.to_string());
        let session = sessions.get(&key).ok_or("MCP会话不存在，请先启用插件")?;
        let tools_result = session
            .client
            .list_tools(Default::default())
            .await
            .map_err(|e| format!("获取工具列表失败: {}", e))?;

        let tools: Vec<Value> = tools_result
            .tools
            .iter()
            .map(|t| {
                let desc: String = t
                    .description
                    .clone()
                    .map(|d| d.to_string())
                    .unwrap_or_default();
                json!({
                    "name": t.name.to_string(),
                    "description": desc,
                    "inputSchema": t.input_schema,
                })
            })
            .collect();
        Ok(tools)
    }

    pub async fn call_tool(
        &self,
        user_id: &str,
        plugin_name: &str,
        tool_name: &str,
        arguments: Option<&Value>,
    ) -> Result<Value, String> {
        let sessions = self.sessions.lock().await;
        let key = (user_id.to_string(), plugin_name.to_string());
        let session = sessions.get(&key).ok_or("MCP会话不存在")?;

        let mut params = CallToolRequestParams::new(tool_name.to_string());
        if let Some(args) = arguments {
            if let Some(obj) = args.as_object() {
                params = params.with_arguments(obj.clone());
            }
        }

        let result = session
            .client
            .call_tool(params)
            .await
            .map_err(|e| format!("调用MCP工具失败: {}", e))?;

        let content: Vec<Value> = result
            .content
            .iter()
            .map(|c| match &**c {
                rmcp::model::RawContent::Text(tc) => json!({"type": "text", "text": tc.text}),
                rmcp::model::RawContent::Image(img) => {
                    json!({"type": "image", "data": img.data, "mimeType": img.mime_type})
                }
                rmcp::model::RawContent::Resource(res) => {
                    json!({"type": "resource", "resource": res.resource})
                }
                _ => json!({"type": "unknown"}),
            })
            .collect();

        Ok(json!({
            "content": content,
            "isError": result.is_error.unwrap_or(false),
        }))
    }

    pub async fn test_connection(&self, user_id: &str, plugin_name: &str) -> Result<Value, String> {
        if !self.is_registered(user_id, plugin_name).await {
            return Ok(json!({"success": false, "message": "MCP会话不存在"}));
        }
        let start = std::time::Instant::now();
        match self.list_tools(user_id, plugin_name).await {
            Ok(tools) => {
                let elapsed = start.elapsed().as_millis();
                Ok(
                    json!({"success": true, "message": "连接测试成功", "response_time_ms": elapsed, "tools_count": tools.len()}),
                )
            }
            Err(e) => {
                let elapsed = start.elapsed().as_millis();
                Ok(
                    json!({"success": false, "message": "连接测试失败", "response_time_ms": elapsed, "error": e}),
                )
            }
        }
    }

    pub async fn format_tools_for_openai(
        &self,
        user_id: &str,
        plugin_name: &str,
    ) -> Result<Vec<Value>, String> {
        let tools = self.list_tools(user_id, plugin_name).await?;
        let openai_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                let name = t["name"].as_str().unwrap_or("");
                let description = t["description"].as_str().unwrap_or("");
                let input_schema = t
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or(json!({"type": "object", "properties": {}}));
                json!({
                    "type": "function",
                    "function": {
                        "name": format!("{}__{}", plugin_name, name),
                        "description": description,
                        "parameters": input_schema,
                    }
                })
            })
            .collect();
        Ok(openai_tools)
    }

    pub fn parse_function_name(function_name: &str) -> Result<(String, String), String> {
        let parts: Vec<&str> = function_name.splitn(2, "__").collect();
        if parts.len() != 2 {
            return Err(format!("无效的函数名格式: {}", function_name));
        }
        Ok((parts[0].to_string(), parts[1].to_string()))
    }

    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.lock().await;
        sessions.len()
    }
}
