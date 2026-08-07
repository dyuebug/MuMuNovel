use chrono::Utc;
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::types::{
    retry_after_seconds_from_headers, AIRequestError, AIResponse, AIStreamChunk, ChatMessage,
    ToolCall, ToolCallFunction, ToolChoice, ToolDef,
};

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, routing::post, Json, Router};
    use serde_json::json;
    use tokio::net::TcpListener;

    #[test]
    fn parses_thinking_and_text_blocks_separately() {
        let response = AnthropicClient::parse_response(&json!({
            "content": [
                {"type": "thinking", "thinking": "显式思考"},
                {"type": "text", "text": "最终正文"},
                {"type": "redacted_thinking", "data": "do-not-show"}
            ],
            "stop_reason": "end_turn"
        }))
        .expect("parse response");

        assert_eq!(response.reasoning_content.as_deref(), Some("显式思考"));
        assert_eq!(response.content, "最终正文");
    }

    use super::AnthropicClient;
    use crate::ai::types::{ChatMessage, ToolChoice, ToolChoiceFunction};

    #[test]
    fn serializes_required_and_named_tool_choice() {
        assert_eq!(
            AnthropicClient::serialize_tool_choice(Some(&ToolChoice::Required)),
            Some(json!({ "type": "any" }))
        );
        assert_eq!(
            AnthropicClient::serialize_tool_choice(Some(&ToolChoice::Function(
                ToolChoiceFunction {
                    name: "get_weather".to_string(),
                }
            ))),
            Some(json!({
                "type": "tool",
                "name": "get_weather"
            }))
        );
    }

    #[tokio::test]
    async fn chat_completion_detailed_keeps_http_status_code() {
        let app = Router::new().route(
            "/v1/messages",
            post(|| async {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error": {"message": "bad gateway"}})),
                )
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let address = listener.local_addr().expect("listener address");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });

        let client = AnthropicClient::new("ak-test", &format!("http://{address}/v1"));
        let error = client
            .chat_completion_detailed(
                &[ChatMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                }],
                "claude-3-5-sonnet-latest",
                0.1,
                64,
                None,
                None,
                None,
            )
            .await
            .expect_err("gateway error should return structured failure");

        handle.abort();

        assert_eq!(error.status_code, Some(502));
        assert_eq!(error.transport_diagnostics, None);
        assert!(error.message.contains("Anthropic HTTP 502"));
    }
}

impl AnthropicClient {
    fn serialize_tool_choice(tool_choice: Option<&ToolChoice>) -> Option<Value> {
        match tool_choice {
            Some(ToolChoice::Auto) => Some(serde_json::json!({ "type": "auto" })),
            Some(ToolChoice::Required) => Some(serde_json::json!({ "type": "any" })),
            Some(ToolChoice::Function(function)) => Some(serde_json::json!({
                "type": "tool",
                "name": function.name.clone(),
            })),
            Some(ToolChoice::None) | None => None,
        }
    }

    pub fn new(api_key: &str, base_url: &str) -> Self {
        let normalized_base_url = base_url.trim().trim_end_matches('/').to_string();
        let mut client_builder = Client::builder().timeout(Duration::from_secs(300));
        if super::should_bypass_system_proxy(&normalized_base_url) {
            client_builder = client_builder.no_proxy();
        }
        let client = client_builder
            .build()
            .expect("failed to create HTTP client");
        Self {
            client,
            api_key: api_key.to_string(),
            base_url: normalized_base_url,
        }
    }

    fn build_request_body(
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        max_tokens: u32,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
        stream: bool,
    ) -> Result<Value, String> {
        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": messages,
        });

        if stream {
            body["stream"] = serde_json::json!(true);
        }

        if let Some(sp) = system_prompt {
            body["system"] = serde_json::json!(sp);
        }
        if let Some(t) = tools {
            body["tools"] = serde_json::to_value(t).map_err(|e| format!("{e}"))?;
            match tool_choice {
                Some(tool_choice) => {
                    if let Some(tool_choice) = Self::serialize_tool_choice(Some(tool_choice)) {
                        body["tool_choice"] = tool_choice;
                    }
                }
                None => {
                    body["tool_choice"] = serde_json::json!({ "type": "auto" });
                }
            }
        }

        Ok(body)
    }

    pub async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        max_tokens: u32,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<AIResponse, String> {
        self.chat_completion_detailed(
            messages,
            model,
            temperature,
            max_tokens,
            system_prompt,
            tools,
            tool_choice,
        )
        .await
        .map_err(|error| error.message)
    }

    pub async fn chat_completion_detailed(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        max_tokens: u32,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<AIResponse, AIRequestError> {
        let url = format!("{}/messages", self.base_url);
        let body = Self::build_request_body(
            messages,
            model,
            temperature,
            max_tokens,
            system_prompt,
            tools,
            tool_choice,
            false,
        )
        .map_err(AIRequestError::new)?;

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| AIRequestError::new(format!("Anthropic request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let retry_after_seconds = retry_after_seconds_from_headers(resp.headers(), Utc::now());
            let text = resp.text().await.unwrap_or_default();
            return Err(AIRequestError {
                message: format!("Anthropic HTTP {status}: {text}"),
                transport_diagnostics: None,
                status_code: Some(status.as_u16()),
                retry_after_seconds,
            });
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| AIRequestError::new(format!("{e}")))?;
        Self::parse_response(&json).map_err(AIRequestError::new)
    }

    pub fn chat_completion_stream(
        &self,
        messages: Vec<ChatMessage>,
        model: String,
        temperature: f64,
        max_tokens: u32,
        system_prompt: Option<String>,
        tools: Option<Vec<ToolDef>>,
        tool_choice: Option<ToolChoice>,
    ) -> ReceiverStream<Result<AIStreamChunk, AIRequestError>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<AIStreamChunk, AIRequestError>>(32);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let result = Self::stream_inner(
                client,
                api_key,
                base_url,
                messages,
                model,
                temperature,
                max_tokens,
                system_prompt,
                tools,
                tool_choice,
                tx.clone(),
            )
            .await;
            if let Err(e) = result {
                let _ = tx.send(Err(e)).await;
            }
        });

        ReceiverStream::new(rx)
    }

    async fn stream_inner(
        client: Client,
        api_key: String,
        base_url: String,
        messages: Vec<ChatMessage>,
        model: String,
        temperature: f64,
        max_tokens: u32,
        system_prompt: Option<String>,
        tools: Option<Vec<ToolDef>>,
        tool_choice: Option<ToolChoice>,
        tx: tokio::sync::mpsc::Sender<Result<AIStreamChunk, AIRequestError>>,
    ) -> Result<(), AIRequestError> {
        let url = format!("{}/messages", base_url);
        let body = Self::build_request_body(
            &messages,
            &model,
            temperature,
            max_tokens,
            system_prompt.as_deref(),
            tools.as_deref(),
            tool_choice.as_ref(),
            true,
        )
        .map_err(AIRequestError::new)?;

        let resp = client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| AIRequestError::new(format!("Anthropic stream request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let retry_after_seconds = retry_after_seconds_from_headers(resp.headers(), Utc::now());
            let text = resp.text().await.unwrap_or_default();
            return Err(AIRequestError {
                message: format!("Anthropic stream HTTP {status}: {text}"),
                transport_diagnostics: None,
                status_code: Some(status.as_u16()),
                retry_after_seconds,
            });
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut tool_calls_buffer: Vec<ToolCall> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AIRequestError::new(format!("stream error: {e}")))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                let Ok(event) = serde_json::from_str::<Value>(data) else {
                    continue;
                };

                let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match event_type {
                    "content_block_delta" => {
                        if let Some(delta) = event.get("delta") {
                            if let Some(delta_type) = delta.get("type").and_then(|t| t.as_str()) {
                                match delta_type {
                                    "thinking_delta" => {
                                        if let Some(thinking) =
                                            delta.get("thinking").and_then(Value::as_str)
                                        {
                                            let _ = tx
                                                .send(Ok(AIStreamChunk {
                                                    content: None,
                                                    reasoning_content: Some(thinking.to_string()),
                                                    tool_calls: None,
                                                    done: false,
                                                    finish_reason: None,
                                                }))
                                                .await;
                                        }
                                    }
                                    "text_delta" => {
                                        if let Some(text) =
                                            delta.get("text").and_then(|t| t.as_str())
                                        {
                                            let _ = tx
                                                .send(Ok(AIStreamChunk {
                                                    content: Some(text.to_string()),
                                                    reasoning_content: None,
                                                    tool_calls: None,
                                                    done: false,
                                                    finish_reason: None,
                                                }))
                                                .await;
                                        }
                                    }
                                    "input_json_delta" => {
                                        if let Some(partial) =
                                            delta.get("partial_json").and_then(|j| j.as_str())
                                        {
                                            let idx = event
                                                .get("index")
                                                .and_then(|i| i.as_i64())
                                                .unwrap_or(0)
                                                as usize;
                                            while tool_calls_buffer.len() <= idx {
                                                tool_calls_buffer.push(ToolCall {
                                                    id: format!("toolu_{}", idx),
                                                    call_type: "function".into(),
                                                    function: ToolCallFunction {
                                                        name: String::new(),
                                                        arguments: String::new(),
                                                    },
                                                });
                                            }
                                            tool_calls_buffer[idx]
                                                .function
                                                .arguments
                                                .push_str(partial);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    "content_block_start" => {
                        if let Some(content_block) = event.get("content_block") {
                            if content_block.get("type").and_then(|t| t.as_str())
                                == Some("tool_use")
                            {
                                let idx = event.get("index").and_then(|i| i.as_i64()).unwrap_or(0)
                                    as usize;
                                while tool_calls_buffer.len() <= idx {
                                    tool_calls_buffer.push(ToolCall {
                                        id: String::new(),
                                        call_type: "function".into(),
                                        function: ToolCallFunction {
                                            name: String::new(),
                                            arguments: String::new(),
                                        },
                                    });
                                }
                                if let Some(name) =
                                    content_block.get("name").and_then(|n| n.as_str())
                                {
                                    tool_calls_buffer[idx].function.name = name.to_string();
                                }
                                if let Some(id) = content_block.get("id").and_then(|i| i.as_str()) {
                                    tool_calls_buffer[idx].id = id.to_string();
                                }
                            }
                        }
                    }
                    "message_delta" => {
                        let finish_reason = event
                            .get("delta")
                            .and_then(|d| d.get("stop_reason"))
                            .and_then(|r| r.as_str())
                            .map(|s| s.to_string());
                        let _ = tx
                            .send(Ok(AIStreamChunk {
                                content: None,
                                reasoning_content: None,
                                tool_calls: if tool_calls_buffer.is_empty() {
                                    None
                                } else {
                                    Some(std::mem::take(&mut tool_calls_buffer))
                                },
                                done: true,
                                finish_reason,
                            }))
                            .await;
                        return Ok(());
                    }
                    "error" => {
                        let msg = event
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown error");
                        return Err(AIRequestError::new(msg.to_string()));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn parse_response(json: &Value) -> Result<AIResponse, String> {
        let mut content = String::new();
        let mut reasoning_content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let finish_reason = json
            .get("stop_reason")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string());

        if let Some(blocks) = json.get("content").and_then(|c| c.as_array()) {
            for block in blocks {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match block_type {
                    "thinking" => {
                        if let Some(thinking) = block.get("thinking").and_then(Value::as_str) {
                            reasoning_content.push_str(thinking);
                        }
                    }
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            content.push_str(text);
                        }
                    }
                    "tool_use" => {
                        let name = block
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string();
                        let id = block
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("")
                            .to_string();
                        let arguments = block
                            .get("input")
                            .map(|input| serde_json::to_string(input).unwrap_or_default())
                            .unwrap_or_default();
                        tool_calls.push(ToolCall {
                            id,
                            call_type: "function".into(),
                            function: ToolCallFunction { name, arguments },
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok(AIResponse {
            content,
            reasoning_content: (!reasoning_content.is_empty()).then_some(reasoning_content),
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            finish_reason,
            transport_diagnostics: None,
        })
    }
}
