use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, Utc};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::RoleClient;
use rmcp::ServiceExt;

type McpClient = RunningService<RoleClient, ClientInfo>;
const TOOL_CACHE_TTL_MINUTES: i64 = 5;

#[derive(Clone, Copy, PartialEq)]
enum TransportType {
    Sse,
    Stdio,
}

#[derive(Clone)]
struct ToolCacheEntry {
    tools: Vec<Value>,
    expire_at: chrono::DateTime<Utc>,
    hit_count: u64,
}

#[derive(Clone, Default)]
struct ToolMetric {
    total_calls: u64,
    success_calls: u64,
    failed_calls: u64,
    total_duration_ms: f64,
    last_call_time: Option<chrono::DateTime<Utc>>,
}

impl ToolMetric {
    fn record_success(&mut self, duration_ms: f64) {
        self.total_calls += 1;
        self.success_calls += 1;
        self.total_duration_ms += duration_ms;
        self.last_call_time = Some(Utc::now());
    }

    fn record_failure(&mut self, duration_ms: f64) {
        self.total_calls += 1;
        self.failed_calls += 1;
        self.total_duration_ms += duration_ms;
        self.last_call_time = Some(Utc::now());
    }

    fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.success_calls as f64 / self.total_calls as f64
        }
    }

    fn avg_duration_ms(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.total_duration_ms / self.total_calls as f64
        }
    }

    fn to_value(&self) -> Value {
        json!({
            "total_calls": self.total_calls,
            "success_calls": self.success_calls,
            "failed_calls": self.failed_calls,
            "success_rate": ((self.success_rate() * 1000.0).round() / 1000.0),
            "avg_duration_ms": ((self.avg_duration_ms() * 100.0).round() / 100.0),
            "last_call_time": self.last_call_time.map(|value| value.to_rfc3339()),
        })
    }
}

#[allow(dead_code)]
struct McpSession {
    client: McpClient,
    transport_type: TransportType,
    plugin_name: String,
    url: String,
    status: String,
    request_count: u64,
    error_count: u64,
    connected_at: chrono::DateTime<Utc>,
    last_access: chrono::DateTime<Utc>,
}

#[derive(Clone)]
pub struct McpClientManager {
    sessions: Arc<Mutex<HashMap<(String, String), McpSession>>>,
    tool_cache: Arc<Mutex<HashMap<String, ToolCacheEntry>>>,
    tool_metrics: Arc<Mutex<HashMap<String, ToolMetric>>>,
    #[cfg(test)]
    disconnect_calls: Arc<Mutex<Vec<(String, String)>>>,
}

impl McpClientManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            tool_cache: Arc::new(Mutex::new(HashMap::new())),
            tool_metrics: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(test)]
            disconnect_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn cache_key(user_id: &str, plugin_name: &str) -> String {
        format!("{}:{}", user_id, plugin_name)
    }

    pub async fn is_registered(&self, user_id: &str, plugin_name: &str) -> bool {
        let sessions = self.sessions.lock().await;
        sessions.contains_key(&(user_id.to_string(), plugin_name.to_string()))
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
            url: server_url.to_string(),
            status: "active".to_string(),
            request_count: 0,
            error_count: 0,
            connected_at: Utc::now(),
            last_access: Utc::now(),
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
            url: command.to_string(),
            status: "active".to_string(),
            request_count: 0,
            error_count: 0,
            connected_at: Utc::now(),
            last_access: Utc::now(),
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
        drop(sessions);
        self.clear_cache(Some(user_id), Some(plugin_name)).await;
        #[cfg(test)]
        {
            self.disconnect_calls
                .lock()
                .await
                .push((user_id.to_string(), plugin_name.to_string()));
        }
        Ok(())
    }

    #[cfg(test)]
    pub async fn disconnect_calls_for_tests(&self) -> Vec<(String, String)> {
        self.disconnect_calls.lock().await.clone()
    }

    pub async fn list_tools(&self, user_id: &str, plugin_name: &str) -> Result<Vec<Value>, String> {
        let cache_key = Self::cache_key(user_id, plugin_name);
        let now = Utc::now();
        {
            let mut tool_cache = self.tool_cache.lock().await;
            if let Some(entry) = tool_cache.get_mut(&cache_key) {
                if now < entry.expire_at {
                    entry.hit_count += 1;
                    return Ok(entry.tools.clone());
                }
                tool_cache.remove(&cache_key);
            }
        }

        let mut sessions = self.sessions.lock().await;
        let key = (user_id.to_string(), plugin_name.to_string());
        let session = sessions
            .get_mut(&key)
            .ok_or("MCP会话不存在，请先启用插件")?;
        session.last_access = now;
        session.request_count += 1;
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
        drop(sessions);
        self.tool_cache.lock().await.insert(
            cache_key,
            ToolCacheEntry {
                tools: tools.clone(),
                expire_at: now + Duration::minutes(TOOL_CACHE_TTL_MINUTES),
                hit_count: 0,
            },
        );
        Ok(tools)
    }

    pub async fn call_tool(
        &self,
        user_id: &str,
        plugin_name: &str,
        tool_name: &str,
        arguments: Option<&Value>,
    ) -> Result<Value, String> {
        let mut sessions = self.sessions.lock().await;
        let key = (user_id.to_string(), plugin_name.to_string());
        let session = sessions.get_mut(&key).ok_or("MCP会话不存在")?;
        session.last_access = Utc::now();
        session.request_count += 1;
        let metric_key = format!("{}.{}", plugin_name, tool_name);
        let start = std::time::Instant::now();

        let mut params = CallToolRequestParams::new(tool_name.to_string());
        if let Some(args) = arguments {
            if let Some(obj) = args.as_object() {
                params = params.with_arguments(obj.clone());
            }
        }

        let result = session.client.call_tool(params).await.map_err(|e| {
            session.error_count += 1;
            format!("调用MCP工具失败: {}", e)
        });

        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        match result {
            Ok(result) => {
                drop(sessions);
                self.tool_metrics
                    .lock()
                    .await
                    .entry(metric_key)
                    .or_default()
                    .record_success(duration_ms);

                let content: Vec<Value> = result
                    .content
                    .iter()
                    .map(|c| match &**c {
                        rmcp::model::RawContent::Text(tc) => {
                            json!({"type": "text", "text": tc.text})
                        }
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
            Err(error) => {
                drop(sessions);
                self.tool_metrics
                    .lock()
                    .await
                    .entry(metric_key)
                    .or_default()
                    .record_failure(duration_ms);
                Err(error)
            }
        }
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

    pub async fn session_stats_snapshot(&self) -> Vec<Value> {
        let sessions = self.sessions.lock().await;
        sessions
            .iter()
            .map(|((user_id, plugin_name), session)| {
                let total_requests = session.request_count;
                let error_rate = if total_requests == 0 {
                    0.0
                } else {
                    session.error_count as f64 / total_requests as f64
                };

                json!({
                    "key": format!("{}:{}", user_id, plugin_name),
                    "url": session.url,
                    "status": session.status,
                    "request_count": session.request_count,
                    "error_count": session.error_count,
                    "error_rate": ((error_rate * 1000.0).round() / 1000.0),
                    "created_at": session.connected_at.to_rfc3339(),
                    "last_access": session.last_access.to_rfc3339(),
                })
            })
            .collect()
    }

    pub async fn metrics_snapshot(&self, tool_name: Option<&str>) -> Value {
        let tool_metrics = self.tool_metrics.lock().await;
        if let Some(tool_name) = tool_name {
            return tool_metrics
                .get(tool_name)
                .map(|metric| json!({ tool_name: metric.to_value() }))
                .unwrap_or_else(|| json!({}));
        }

        let payload = tool_metrics
            .iter()
            .map(|(key, metric)| (key.clone(), metric.to_value()))
            .collect::<serde_json::Map<String, Value>>();
        Value::Object(payload)
    }

    pub async fn cache_stats_snapshot(&self) -> Value {
        let tool_cache = self.tool_cache.lock().await;
        let entries = tool_cache
            .iter()
            .map(|(key, entry)| {
                json!({
                    "key": key,
                    "tools_count": entry.tools.len(),
                    "hit_count": entry.hit_count,
                    "expire_time": entry.expire_at.to_rfc3339(),
                })
            })
            .collect::<Vec<_>>();
        let total_hits: u64 = tool_cache.values().map(|entry| entry.hit_count).sum();

        json!({
            "total_entries": tool_cache.len(),
            "total_hits": total_hits,
            "cache_ttl_minutes": TOOL_CACHE_TTL_MINUTES,
            "entries": entries,
        })
    }

    pub async fn clear_cache(&self, user_id: Option<&str>, plugin_name: Option<&str>) {
        let mut tool_cache = self.tool_cache.lock().await;
        match (user_id, plugin_name) {
            (Some(user_id), Some(plugin_name)) => {
                tool_cache.remove(&Self::cache_key(user_id, plugin_name));
            }
            (Some(user_id), None) => {
                let prefix = format!("{}:", user_id);
                tool_cache.retain(|key, _| !key.starts_with(prefix.as_str()));
            }
            (None, _) => {
                tool_cache.clear();
            }
        }
    }

    #[cfg(test)]
    pub async fn seed_metric_for_tests(
        &self,
        tool_key: &str,
        total_calls: u64,
        success_calls: u64,
        failed_calls: u64,
        total_duration_ms: f64,
        last_call_time: Option<chrono::DateTime<Utc>>,
    ) {
        self.tool_metrics.lock().await.insert(
            tool_key.to_string(),
            ToolMetric {
                total_calls,
                success_calls,
                failed_calls,
                total_duration_ms,
                last_call_time,
            },
        );
    }

    #[cfg(test)]
    pub async fn seed_cache_entry_for_tests(
        &self,
        cache_key: &str,
        tools: Vec<Value>,
        hit_count: u64,
        expire_at: chrono::DateTime<Utc>,
    ) {
        self.tool_cache.lock().await.insert(
            cache_key.to_string(),
            ToolCacheEntry {
                tools,
                expire_at,
                hit_count,
            },
        );
    }
}
