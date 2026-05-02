use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::types::{AIResponse, AIStreamChunk, ChatMessage, ToolCall, ToolCallFunction, ToolDef};

pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAIClient {
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
        tools: Option<&[ToolDef]>,
    ) -> Result<AIResponse, String> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        });

        if let Some(t) = tools {
            body["tools"] = serde_json::to_value(t).map_err(|e| format!("{}", e))?;
            body["tool_choice"] = serde_json::json!("auto");
        }

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("OpenAI request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI HTTP {}: {}", status, text));
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
        tools: Option<Vec<ToolDef>>,
    ) -> ReceiverStream<Result<AIStreamChunk, String>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<AIStreamChunk, String>>(32);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let result = Self::stream_inner(
                client, api_key, base_url, messages, model, temperature, max_tokens, tools, tx.clone(),
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
        tools: Option<Vec<ToolDef>>,
        tx: tokio::sync::mpsc::Sender<Result<AIStreamChunk, String>>,
    ) -> Result<(), String> {
        let url = format!("{}/chat/completions", base_url);

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
            "stream": true,
        });

        if let Some(ref t) = tools {
            body["tools"] = serde_json::to_value(t).map_err(|e| format!("{}", e))?;
            body["tool_choice"] = serde_json::json!("auto");
        }

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("OpenAI stream request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI stream HTTP {}: {}", status, text));
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

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                if data == "[DONE]" {
                    let _ = tx
                        .send(Ok(AIStreamChunk {
                            content: None,
                            tool_calls: if tool_calls_buffer.is_empty() {
                                None
                            } else {
                                Some(std::mem::take(&mut tool_calls_buffer))
                            },
                            done: true,
                            finish_reason: Some("stop".into()),
                        }))
                        .await;
                    return Ok(());
                }

                if let Ok(event) = serde_json::from_str::<Value>(data) {
                    let choices = event.get("choices").and_then(|c| c.as_array());
                    if let Some(choices) = choices {
                        for choice in choices {
                            if let Some(delta) = choice.get("delta") {
                                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                    let _ = tx.send(Ok(AIStreamChunk {
                                        content: Some(content.to_string()),
                                        tool_calls: None,
                                        done: false,
                                        finish_reason: None,
                                    })).await;
                                }
                                if let Some(tc_deltas) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                    for tc_delta in tc_deltas {
                                        let idx = tc_delta.get("index").and_then(|i| i.as_i64()).unwrap_or(0) as usize;
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
                                        if let Some(id) = tc_delta.get("id").and_then(|i| i.as_str()) {
                                            tool_calls_buffer[idx].id = id.to_string();
                                        }
                                        if let Some(func) = tc_delta.get("function") {
                                            if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                                tool_calls_buffer[idx].function.name = name.to_string();
                                            }
                                            if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                                                tool_calls_buffer[idx].function.arguments.push_str(args);
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
                                if reason == "stop" || reason == "tool_calls" {
                                    let _ = tx.send(Ok(AIStreamChunk {
                                        content: None,
                                        tool_calls: if tool_calls_buffer.is_empty() {
                                            None
                                        } else {
                                            Some(std::mem::take(&mut tool_calls_buffer))
                                        },
                                        done: true,
                                        finish_reason: Some(reason.to_string()),
                                    })).await;
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_response(json: &Value) -> Result<AIResponse, String> {
        let choices = json
            .get("choices")
            .and_then(|c| c.as_array())
            .ok_or("no choices in response")?;
        let msg = choices
            .first()
            .and_then(|c| c.get("message"))
            .ok_or("no message in choice")?;
        let content = msg
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        let finish_reason = choices
            .first()
            .and_then(|c| c.get("finish_reason"))
            .and_then(|r| r.as_str())
            .map(|s| s.to_string());

        let tool_calls = msg.get("tool_calls").and_then(|tc| {
            serde_json::from_value::<Vec<ToolCall>>(tc.clone()).ok()
        });

        Ok(AIResponse {
            content,
            tool_calls,
            finish_reason,
        })
    }
}
