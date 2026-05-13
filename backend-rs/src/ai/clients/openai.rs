use futures::StreamExt;
use reqwest::{header::CONTENT_TYPE, Client};
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::types::{
    AIResponse, AIStreamChunk, ChatMessage, ToolCall, ToolCallFunction, ToolDef,
};

pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAIClient {
    const NON_STREAM_MAX_TOKENS_LIMIT: u32 = 4096;

    fn preview_text(text: &str) -> String {
        let trimmed = text.trim();
        let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.len() > 240 {
            format!("{}...", &collapsed[..240])
        } else {
            collapsed
        }
    }

    fn invalid_content_error(prefix: &str, status: reqwest::StatusCode, text: &str) -> String {
        let preview = Self::preview_text(text);
        if text.trim_start().starts_with('<') {
            format!(
                "{} returned non-JSON content. The Base URL may be incorrect (for example, missing /v1). HTTP {}, response preview: {}",
                prefix, status, preview
            )
        } else {
            format!(
                "{} returned invalid JSON. HTTP {}, response preview: {}",
                prefix, status, preview
            )
        }
    }

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

    pub fn is_official_openai_base_url(base_url: &str) -> bool {
        reqwest::Url::parse(base_url)
            .ok()
            .and_then(|url| url.host_str().map(|host| host.eq_ignore_ascii_case("api.openai.com")))
            .unwrap_or(false)
    }

    pub async fn list_models(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/models", self.base_url);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| format!("OpenAI models request failed: {}", e))?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!("OpenAI models HTTP {}: {}", status, text));
        }

        let payload: Value = serde_json::from_str(&text)
            .map_err(|_| Self::invalid_content_error("OpenAI models", status, &text))?;
        let models = payload
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        if models.is_empty() {
            return Err("OpenAI models response did not contain any usable model ids".to_string());
        }

        Ok(models)
    }

    pub fn pick_fallback_model(current_model: &str, models: &[String]) -> Option<String> {
        let current = current_model.trim().to_lowercase();
        let current_tokens = current
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|token| token.len() >= 2)
            .collect::<Vec<_>>();

        let mut seen = HashSet::new();
        let candidates = models
            .iter()
            .map(|model| model.trim())
            .filter(|model| !model.is_empty())
            .filter(|model| seen.insert(model.to_lowercase()))
            .filter(|model| model.to_lowercase() != current)
            .map(|model| {
                let normalized = model.to_lowercase();
                let keyword_matches = current_tokens
                    .iter()
                    .filter(|token| normalized.contains(**token))
                    .count();
                let likely_chat = Self::looks_like_text_generation_model(&normalized);
                (model.to_string(), keyword_matches, likely_chat)
            })
            .collect::<Vec<_>>();

        candidates
            .iter()
            .filter(|(_, keyword_matches, likely_chat)| *likely_chat && *keyword_matches > 0)
            .max_by_key(|(_, keyword_matches, _)| *keyword_matches)
            .map(|(model, _, _)| model.clone())
            .or_else(|| {
                candidates
                    .iter()
                    .find(|(_, _, likely_chat)| *likely_chat)
                    .map(|(model, _, _)| model.clone())
            })
            .or_else(|| candidates.first().map(|(model, _, _)| model.clone()))
    }

    fn looks_like_text_generation_model(model: &str) -> bool {
        let excluded = [
            "embedding",
            "rerank",
            "tts",
            "whisper",
            "image",
            "vision-preview",
            "moderation",
            "transcription",
            "speech",
            "audio",
            "omni-moderation",
        ];
        if excluded.iter().any(|needle| model.contains(needle)) {
            return false;
        }

        let included = [
            "gpt",
            "chat",
            "claude",
            "gemini",
            "deepseek",
            "qwen",
            "llama",
            "mistral",
            "glm",
            "doubao",
            "hunyuan",
            "baichuan",
            "yi-",
            "command",
            "sonnet",
            "haiku",
            "opus",
            "reasoner",
            "instruct",
            "o1",
            "o3",
            "o4",
        ];

        included.iter().any(|needle| model.contains(needle))
    }

    pub async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        max_tokens: u32,
        tools: Option<&[ToolDef]>,
    ) -> Result<AIResponse, String> {
        if max_tokens > Self::NON_STREAM_MAX_TOKENS_LIMIT {
            return self
                .chat_completion_via_stream(messages, model, temperature, max_tokens, tools)
                .await;
        }

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

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!("OpenAI HTTP {}: {}", status, text));
        }

        let json: Value = serde_json::from_str(&text)
            .map_err(|_| Self::invalid_content_error("OpenAI", status, &text))?;
        Self::parse_response(&json)
    }

    async fn chat_completion_via_stream(
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
            "stream": true,
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
            .map_err(|e| format!("OpenAI stream fallback request failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI stream fallback HTTP {}: {}", status, text));
        }

        let content_type = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        let text = resp.text().await.unwrap_or_default();
        if !content_type.contains("text/event-stream") {
            let json: Value = serde_json::from_str(&text)
                .map_err(|_| Self::invalid_content_error("OpenAI stream fallback", status, &text))?;
            return Self::parse_response(&json);
        }

        Self::parse_stream_text(&text)
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
                client,
                api_key,
                base_url,
                messages,
                model,
                temperature,
                max_tokens,
                tools,
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

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI stream HTTP {}: {}", status, text));
        }

        let content_type = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        if !content_type.contains("text/event-stream") {
            let text = resp.text().await.unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                let parsed = Self::parse_response(&json)?;
                if !parsed.content.is_empty() {
                    let _ = tx
                        .send(Ok(AIStreamChunk {
                            content: Some(parsed.content),
                            tool_calls: None,
                            done: false,
                            finish_reason: None,
                        }))
                        .await;
                }
                let _ = tx
                    .send(Ok(AIStreamChunk {
                        content: None,
                        tool_calls: parsed.tool_calls,
                        done: true,
                        finish_reason: parsed.finish_reason.or(Some("stop".into())),
                    }))
                    .await;
                return Ok(());
            }

            return Err(Self::invalid_content_error("OpenAI stream", status, &text));
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
                                if let Some(content) = delta.get("content").and_then(|c| c.as_str())
                                {
                                    let _ = tx
                                        .send(Ok(AIStreamChunk {
                                            content: Some(content.to_string()),
                                            tool_calls: None,
                                            done: false,
                                            finish_reason: None,
                                        }))
                                        .await;
                                }
                                if let Some(tc_deltas) =
                                    delta.get("tool_calls").and_then(|t| t.as_array())
                                {
                                    for tc_delta in tc_deltas {
                                        let idx = tc_delta
                                            .get("index")
                                            .and_then(|i| i.as_i64())
                                            .unwrap_or(0)
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
                                        if let Some(id) =
                                            tc_delta.get("id").and_then(|i| i.as_str())
                                        {
                                            tool_calls_buffer[idx].id = id.to_string();
                                        }
                                        if let Some(func) = tc_delta.get("function") {
                                            if let Some(name) =
                                                func.get("name").and_then(|n| n.as_str())
                                            {
                                                tool_calls_buffer[idx].function.name =
                                                    name.to_string();
                                            }
                                            if let Some(args) =
                                                func.get("arguments").and_then(|a| a.as_str())
                                            {
                                                tool_calls_buffer[idx]
                                                    .function
                                                    .arguments
                                                    .push_str(args);
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(reason) =
                                choice.get("finish_reason").and_then(|r| r.as_str())
                            {
                                if reason == "stop" || reason == "tool_calls" {
                                    let _ = tx
                                        .send(Ok(AIStreamChunk {
                                            content: None,
                                            tool_calls: if tool_calls_buffer.is_empty() {
                                                None
                                            } else {
                                                Some(std::mem::take(&mut tool_calls_buffer))
                                            },
                                            done: true,
                                            finish_reason: Some(reason.to_string()),
                                        }))
                                        .await;
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

        let tool_calls = msg
            .get("tool_calls")
            .and_then(|tc| serde_json::from_value::<Vec<ToolCall>>(tc.clone()).ok());

        Ok(AIResponse {
            content,
            tool_calls,
            finish_reason,
        })
    }

    fn parse_stream_text(raw_sse_text: &str) -> Result<AIResponse, String> {
        let mut content = String::new();
        let mut tool_calls_buffer: Vec<ToolCall> = Vec::new();
        let mut finish_reason: Option<String> = None;

        for raw_line in raw_sse_text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data == "[DONE]" {
                if finish_reason.is_none() {
                    finish_reason = Some("stop".to_string());
                }
                continue;
            }

            let event: Value = match serde_json::from_str(data) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let Some(choices) = event.get("choices").and_then(|c| c.as_array()) else {
                continue;
            };

            for choice in choices {
                if let Some(delta) = choice.get("delta") {
                    if let Some(delta_content) = delta.get("content").and_then(|c| c.as_str()) {
                        content.push_str(delta_content);
                    }
                    if let Some(tc_deltas) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        for tc_delta in tc_deltas {
                            let idx = tc_delta
                                .get("index")
                                .and_then(|i| i.as_i64())
                                .unwrap_or(0) as usize;
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

                if let Some(message) = choice.get("message") {
                    if let Some(message_content) = message.get("content").and_then(|c| c.as_str()) {
                        content.push_str(message_content);
                    }
                    if tool_calls_buffer.is_empty() {
                        if let Some(tool_calls) = message.get("tool_calls")
                            .and_then(|tc| serde_json::from_value::<Vec<ToolCall>>(tc.clone()).ok())
                        {
                            tool_calls_buffer = tool_calls;
                        }
                    }
                }

                if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
                    finish_reason = Some(reason.to_string());
                }
            }
        }

        Ok(AIResponse {
            content,
            tool_calls: if tool_calls_buffer.is_empty() {
                None
            } else {
                Some(tool_calls_buffer)
            },
            finish_reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::OpenAIClient;

    #[test]
    fn prefers_similar_chat_model_for_fallback() {
        let models = vec![
            "text-embedding-3-large".to_string(),
            "deepseek-chat".to_string(),
            "deepseek-reasoner".to_string(),
        ];

        let fallback = OpenAIClient::pick_fallback_model("deepseek-v3", &models);

        assert_eq!(fallback.as_deref(), Some("deepseek-chat"));
    }

    #[test]
    fn skips_non_generation_models_when_possible() {
        let models = vec![
            "text-embedding-3-large".to_string(),
            "whisper-1".to_string(),
            "gpt-4.1-mini".to_string(),
        ];

        let fallback = OpenAIClient::pick_fallback_model("non-existent-model", &models);

        assert_eq!(fallback.as_deref(), Some("gpt-4.1-mini"));
    }
}
