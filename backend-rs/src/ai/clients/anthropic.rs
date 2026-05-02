use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::types::{AIResponse, AIStreamChunk, ChatMessage, ToolCall, ToolCallFunction, ToolDef};

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl AnthropicClient {
    pub fn new(api_key: &str, base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .expect("failed to create HTTP client");
        Self {
            client,
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        max_tokens: u32,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
    ) -> Result<AIResponse, String> {
        let url = format!("{}/messages", self.base_url);

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": messages,
        });

        if let Some(sp) = system_prompt {
            body["system"] = serde_json::json!(sp);
        }
        if let Some(t) = tools {
            body["tools"] = serde_json::to_value(t).map_err(|e| format!("{}", e))?;
        }

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Anthropic request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Anthropic HTTP {}: {}", status, text));
        }

        let json: Value = resp.json().await.map_err(|e| format!("{}", e))?;
        Self::parse_response(&json)
    }

    pub fn chat_completion_stream(
        &self,
        messages: Vec<ChatMessage>,
        model: String,
        temperature: f64,
        max_tokens: u32,
        system_prompt: Option<String>,
        tools: Option<Vec<ToolDef>>,
    ) -> ReceiverStream<Result<AIStreamChunk, String>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<AIStreamChunk, String>>(32);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let result = Self::stream_inner(
                client, api_key, base_url, messages, model, temperature, max_tokens,
                system_prompt, tools, tx.clone(),
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
        tx: tokio::sync::mpsc::Sender<Result<AIStreamChunk, String>>,
    ) -> Result<(), String> {
        let url = format!("{}/messages", base_url);

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": messages,
            "stream": true,
        });

        if let Some(ref sp) = system_prompt {
            body["system"] = serde_json::json!(sp);
        }
        if let Some(ref t) = tools {
            body["tools"] = serde_json::to_value(t).map_err(|e| format!("{}", e))?;
        }

        let resp = client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Anthropic stream request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Anthropic stream HTTP {}: {}", status, text));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut tool_calls_buffer: Vec<ToolCall> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("stream error: {}", e))?;
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
                                    "text_delta" => {
                                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                            let _ = tx.send(Ok(AIStreamChunk {
                                                content: Some(text.to_string()),
                                                tool_calls: None,
                                                done: false,
                                                finish_reason: None,
                                            })).await;
                                        }
                                    }
                                    "input_json_delta" => {
                                        if let Some(partial) = delta.get("partial_json").and_then(|j| j.as_str()) {
                                            let idx = event.get("index").and_then(|i| i.as_i64()).unwrap_or(0) as usize;
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
                                            tool_calls_buffer[idx].function.arguments.push_str(partial);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    "content_block_start" => {
                        if let Some(content_block) = event.get("content_block") {
                            if content_block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                let idx = event.get("index").and_then(|i| i.as_i64()).unwrap_or(0) as usize;
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
                                if let Some(name) = content_block.get("name").and_then(|n| n.as_str()) {
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
                        let _ = tx.send(Ok(AIStreamChunk {
                            content: None,
                            tool_calls: if tool_calls_buffer.is_empty() {
                                None
                            } else {
                                Some(std::mem::take(&mut tool_calls_buffer))
                            },
                            done: true,
                            finish_reason,
                        })).await;
                        return Ok(());
                    }
                    "error" => {
                        let msg = event.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).unwrap_or("unknown error");
                        return Err(msg.to_string());
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn parse_response(json: &Value) -> Result<AIResponse, String> {
        let mut content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let finish_reason = json
            .get("stop_reason")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string());

        if let Some(blocks) = json.get("content").and_then(|c| c.as_array()) {
            for block in blocks {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            content.push_str(text);
                        }
                    }
                    "tool_use" => {
                        let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                        let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
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
            tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            finish_reason,
        })
    }
}
