use futures::StreamExt;
use reqwest::Client;
use serde_json::{Map, Value};
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::types::{
    AIRequestError, AIResponse, AIStreamChunk, ChatMessage, ToolCall, ToolCallFunction, ToolChoice,
    ToolDef,
};

pub struct GeminiClient {
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
    fn keeps_reasoning_empty_when_provider_has_no_explicit_reasoning_channel() {
        let response = GeminiClient::parse_response(&json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "最终正文"}]
                },
                "finishReason": "STOP"
            }]
        }))
        .expect("parse response");

        assert_eq!(response.content, "最终正文");
        assert_eq!(response.reasoning_content, None);
    }

    use super::GeminiClient;
    use crate::ai::types::{ChatMessage, ToolDef, ToolFunction};

    #[test]
    fn converts_tools_to_gemini_and_strips_schema_fields() {
        let tools = vec![ToolDef {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "get_weather".to_string(),
                description: "Get weather".to_string(),
                parameters: json!({
                    "properties": {
                        "city": { "type": "string" }
                    },
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "additionalProperties": false
                }),
            },
        }];

        let converted = GeminiClient::convert_tools_to_gemini(&tools);
        assert_eq!(
            converted,
            json!([{
                "functionDeclarations": [{
                    "name": "get_weather",
                    "description": "Get weather",
                    "parameters": {
                        "properties": {
                            "city": { "type": "string" }
                        },
                        "type": "object"
                    }
                }]
            }])
        );
    }

    #[tokio::test]
    async fn gemini_chat_completion_detailed_keeps_http_status_code() {
        let app = Router::new().route(
            "/models/gemini-2.5-flash:generateContent",
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

        let client = GeminiClient::new("gk-test", &format!("http://{address}"));
        let error = client
            .chat_completion_detailed(
                &[ChatMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                }],
                "gemini-2.5-flash",
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
        assert!(error.message.contains("Gemini HTTP 502"));
    }
}

impl GeminiClient {
    const DEFAULT_BASE_URL: &'static str = "https://generativelanguage.googleapis.com/v1beta";

    pub fn new(api_key: &str, base_url: &str) -> Self {
        let normalized_base_url = if base_url.trim().is_empty() {
            Self::DEFAULT_BASE_URL.to_string()
        } else {
            base_url.trim().trim_end_matches('/').to_string()
        };
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

    fn normalize_model(model: &str) -> String {
        model.trim().trim_start_matches("models/").to_string()
    }

    fn convert_tools_to_gemini(tools: &[ToolDef]) -> Value {
        let declarations = tools
            .iter()
            .filter(|tool| tool.tool_type == "function")
            .map(|tool| {
                let mut declaration = Map::new();
                declaration.insert(
                    "name".to_string(),
                    Value::String(tool.function.name.clone()),
                );
                declaration.insert(
                    "description".to_string(),
                    Value::String(if tool.function.description.is_empty() {
                        tool.function.name.clone()
                    } else {
                        tool.function.description.clone()
                    }),
                );

                if let Value::Object(mut parameters) = tool.function.parameters.clone() {
                    parameters.remove("$schema");
                    parameters.remove("additionalProperties");
                    if !parameters.contains_key("type") {
                        parameters.insert("type".to_string(), Value::String("object".to_string()));
                    }
                    declaration.insert("parameters".to_string(), Value::Object(parameters));
                }

                Value::Object(declaration)
            })
            .collect::<Vec<_>>();

        Value::Array(vec![serde_json::json!({
            "functionDeclarations": declarations,
        })])
    }

    fn resolve_system_instruction(
        messages: &[ChatMessage],
        explicit_system_prompt: Option<&str>,
    ) -> Option<String> {
        let message_system_prompt = messages
            .iter()
            .filter(|message| message.role == "system")
            .map(|message| message.content.trim())
            .filter(|content| !content.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        match (
            explicit_system_prompt
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            (!message_system_prompt.is_empty()).then_some(message_system_prompt),
        ) {
            (Some(explicit), Some(from_messages)) => Some(format!("{explicit}\n\n{from_messages}")),
            (Some(explicit), None) => Some(explicit.to_string()),
            (None, Some(from_messages)) => Some(from_messages),
            (None, None) => None,
        }
    }

    fn build_contents(messages: &[ChatMessage]) -> Vec<Value> {
        messages
            .iter()
            .filter(|message| message.role != "system")
            .map(|message| {
                serde_json::json!({
                    "role": if message.role == "user" { "user" } else { "model" },
                    "parts": [{ "text": message.content }],
                })
            })
            .collect()
    }

    fn build_request_body(
        messages: &[ChatMessage],
        temperature: f64,
        max_tokens: u32,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
    ) -> Value {
        let mut body = serde_json::json!({
            "contents": Self::build_contents(messages),
            "generationConfig": {
                "temperature": temperature,
                "maxOutputTokens": max_tokens,
            }
        });

        if let Some(system_prompt) = Self::resolve_system_instruction(messages, system_prompt) {
            body["systemInstruction"] = serde_json::json!({
                "parts": [{ "text": system_prompt }]
            });
        }

        if let Some(tools) = tools {
            body["tools"] = Self::convert_tools_to_gemini(tools);
        }

        body
    }

    fn parse_response(json: &Value) -> Result<AIResponse, String> {
        let candidates = json
            .get("candidates")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if candidates.is_empty() {
            return Ok(AIResponse {
                content: String::new(),
                reasoning_content: None,
                tool_calls: None,
                finish_reason: Some("stop".to_string()),
                transport_diagnostics: None,
            });
        }

        let first_candidate = &candidates[0];
        let parts = first_candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut content = String::new();
        let mut tool_calls = Vec::new();
        for part in parts {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                content.push_str(text);
            } else if let Some(function_call) = part.get("functionCall") {
                let name = function_call
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let arguments = function_call
                    .get("args")
                    .map(|args| serde_json::to_string(args).unwrap_or_default())
                    .unwrap_or_else(|| "{}".to_string());
                tool_calls.push(ToolCall {
                    id: format!("call_{name}"),
                    call_type: "function".to_string(),
                    function: ToolCallFunction { name, arguments },
                });
            }
        }

        let finish_reason = if tool_calls.is_empty() {
            first_candidate
                .get("finishReason")
                .and_then(Value::as_str)
                .map(|value| value.to_ascii_lowercase())
                .or(Some("stop".to_string()))
        } else {
            Some("tool_calls".to_string())
        };

        Ok(AIResponse {
            content,
            reasoning_content: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            finish_reason,
            transport_diagnostics: None,
        })
    }

    pub async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        max_tokens: u32,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
        _tool_choice: Option<&ToolChoice>,
    ) -> Result<AIResponse, String> {
        self.chat_completion_detailed(
            messages,
            model,
            temperature,
            max_tokens,
            system_prompt,
            tools,
            None,
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
        _tool_choice: Option<&ToolChoice>,
    ) -> Result<AIResponse, AIRequestError> {
        let normalized_model = Self::normalize_model(model);
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, normalized_model, self.api_key
        );
        let body =
            Self::build_request_body(messages, temperature, max_tokens, system_prompt, tools);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AIRequestError::new(format!("Gemini request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AIRequestError {
                message: format!("Gemini HTTP {status}: {text}"),
                transport_diagnostics: None,
                status_code: Some(status.as_u16()),
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
        _tool_choice: Option<ToolChoice>,
    ) -> ReceiverStream<Result<AIStreamChunk, String>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<AIStreamChunk, String>>(32);
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
                tx.clone(),
            )
            .await;
            if let Err(error) = result {
                let _ = tx.send(Err(error)).await;
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
        tx: tokio::sync::mpsc::Sender<Result<AIStreamChunk, String>>,
    ) -> Result<(), String> {
        let normalized_model = Self::normalize_model(&model);
        let url = format!(
            "{}/models/{}:streamGenerateContent?key={}&alt=sse",
            base_url, normalized_model, api_key
        );
        let body = Self::build_request_body(
            &messages,
            temperature,
            max_tokens,
            system_prompt.as_deref(),
            tools.as_deref(),
        );

        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Gemini stream request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Gemini stream HTTP {status}: {text}"));
        }

        let mut buffer = String::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Gemini stream error: {e}"))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                let Ok(payload) = serde_json::from_str::<Value>(data) else {
                    continue;
                };

                let response = Self::parse_response(&payload)?;
                if !response.content.is_empty() {
                    let _ = tx
                        .send(Ok(AIStreamChunk {
                            content: Some(response.content),
                            reasoning_content: None,
                            tool_calls: None,
                            done: false,
                            finish_reason: None,
                        }))
                        .await;
                }
                if let Some(tool_calls) = response.tool_calls {
                    let _ = tx
                        .send(Ok(AIStreamChunk {
                            content: None,
                            reasoning_content: None,
                            tool_calls: Some(tool_calls),
                            done: false,
                            finish_reason: None,
                        }))
                        .await;
                }
            }
        }

        let _ = tx
            .send(Ok(AIStreamChunk {
                content: None,
                reasoning_content: None,
                tool_calls: None,
                done: true,
                finish_reason: None,
            }))
            .await;

        Ok(())
    }
}
