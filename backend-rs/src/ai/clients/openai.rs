use futures::StreamExt;
use reqwest::{header::CONTENT_TYPE, Client, Url};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::types::{
    AIRequestError, AIResponse, AIStreamChunk, ChatMessage, ToolCall, ToolCallFunction, ToolChoice,
    ToolDef,
};

pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
    backup_urls: Vec<String>,
    compat_profile: String,
}

#[derive(Clone, Copy, Debug, Default)]
struct OpenAIRequestOptions {
    prefer_normalized_v1_candidate: bool,
    transport_max_retries: Option<u32>,
}

#[derive(Clone, Debug)]
struct EndpointTarget {
    base_url: String,
    endpoint_role: &'static str,
    endpoint_index: usize,
}

impl OpenAIClient {
    const NON_STREAM_MAX_TOKENS_LIMIT: u32 = 4096;

    fn serialize_tool_choice(tool_choice: Option<&ToolChoice>) -> Option<Value> {
        match tool_choice {
            Some(ToolChoice::Auto) => Some(serde_json::json!("auto")),
            Some(ToolChoice::None) => Some(serde_json::json!("none")),
            Some(ToolChoice::Required) => Some(serde_json::json!("required")),
            Some(ToolChoice::Function(function)) => Some(serde_json::json!({
                "type": "function",
                "function": {
                    "name": function.name.clone(),
                }
            })),
            None => None,
        }
    }

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

    fn uses_responses_compat_profile(compat_profile: &str) -> bool {
        matches!(
            compat_profile.trim().to_ascii_lowercase().as_str(),
            "sub2api" | "openai_responses"
        )
    }

    fn is_running_in_docker() -> bool {
        std::path::Path::new("/.dockerenv").exists()
    }

    fn is_local_gateway_host(hostname: Option<&str>) -> bool {
        matches!(
            hostname,
            Some("127.0.0.1" | "localhost" | "host.docker.internal")
        )
    }

    fn replace_base_url_host(base_url: &str, hostname: &str) -> Option<String> {
        let mut parsed = Url::parse(base_url).ok()?;
        parsed.set_host(Some(hostname)).ok()?;
        Some(parsed.to_string().trim_end_matches('/').to_string())
    }

    fn build_docker_host_candidate_for(base_url: &str, running_in_docker: bool) -> Option<String> {
        if !running_in_docker {
            return None;
        }

        let parsed = Url::parse(base_url).ok()?;
        if !matches!(parsed.host_str(), Some("127.0.0.1" | "localhost")) {
            return None;
        }

        Self::replace_base_url_host(base_url, "host.docker.internal")
    }

    fn build_http_fallback_candidate(base_url: &str) -> Option<String> {
        let mut parsed = Url::parse(base_url).ok()?;
        if parsed.scheme() != "https" || !Self::is_local_gateway_host(parsed.host_str()) {
            return None;
        }

        parsed.set_scheme("http").ok()?;
        Some(parsed.to_string().trim_end_matches('/').to_string())
    }

    fn build_chat_completion_base_url_candidates_for(
        base_url: &str,
        compat_profile: &str,
        prefer_normalized_v1_candidate: bool,
        running_in_docker: bool,
    ) -> Vec<String> {
        let primary = base_url.trim_end_matches('/').to_string();
        if primary.is_empty() {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        if primary.ends_with("/v1") {
            candidates.push(primary.clone());
        } else if Self::uses_responses_compat_profile(compat_profile) {
            candidates.push(format!("{primary}/v1"));
        } else if prefer_normalized_v1_candidate {
            candidates.push(format!("{primary}/v1"));
            candidates.push(primary.clone());
        } else {
            candidates.push(primary.clone());
        }

        let mut expanded_candidates = Vec::new();
        for candidate in candidates {
            let mut variant_candidates = vec![candidate.clone()];
            if let Some(docker_candidate) =
                Self::build_docker_host_candidate_for(&candidate, running_in_docker)
            {
                variant_candidates.push(docker_candidate);
            }

            for variant_candidate in variant_candidates.clone() {
                if let Some(http_candidate) =
                    Self::build_http_fallback_candidate(&variant_candidate)
                {
                    variant_candidates.push(http_candidate);
                }
            }

            expanded_candidates.extend(variant_candidates);
        }

        let mut unique = Vec::new();
        for candidate in expanded_candidates {
            if !candidate.is_empty() && !unique.contains(&candidate) {
                unique.push(candidate);
            }
        }
        unique
    }

    fn build_chat_completion_base_url_candidates(
        base_url: &str,
        compat_profile: &str,
        prefer_normalized_v1_candidate: bool,
    ) -> Vec<String> {
        Self::build_chat_completion_base_url_candidates_for(
            base_url,
            compat_profile,
            prefer_normalized_v1_candidate,
            Self::is_running_in_docker(),
        )
    }

    fn build_request_endpoints(&self, primary_candidate_base_url: &str) -> Vec<EndpointTarget> {
        let mut targets = Vec::new();
        let normalized_primary = primary_candidate_base_url
            .trim()
            .trim_end_matches('/')
            .to_string();
        if !normalized_primary.is_empty() {
            targets.push(EndpointTarget {
                base_url: normalized_primary,
                endpoint_role: "primary",
                endpoint_index: 1,
            });
        }

        for (backup_offset, backup_base_url) in self.backup_urls.iter().enumerate() {
            let normalized_backup = backup_base_url.trim().trim_end_matches('/').to_string();
            if normalized_backup.is_empty() {
                continue;
            }
            targets.push(EndpointTarget {
                base_url: normalized_backup,
                endpoint_role: "backup",
                endpoint_index: backup_offset + 2,
            });
        }

        targets
    }

    fn summarize_transport_attempts(diagnostics: &Map<String, Value>) -> (bool, usize) {
        let attempts = diagnostics
            .get("attempts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let backup_endpoint_used = attempts.iter().any(|attempt| {
            attempt
                .get("endpoint_role")
                .and_then(Value::as_str)
                .map(|value| value == "backup")
                .unwrap_or(false)
        });
        let failover_count = attempts
            .iter()
            .filter(|attempt| {
                attempt
                    .get("will_failover")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .count();

        (backup_endpoint_used, failover_count)
    }

    fn default_transport_diagnostics() -> Map<String, Value> {
        let mut diagnostics = Map::new();
        diagnostics.insert("events".to_string(), Value::Array(Vec::new()));
        diagnostics.insert("attempts".to_string(), Value::Array(Vec::new()));
        diagnostics
    }

    fn record_transport_event(diagnostics: &mut Map<String, Value>, event: Value) {
        if let Some(events) = diagnostics.get_mut("events").and_then(Value::as_array_mut) {
            events.push(event);
        }
    }

    fn record_transport_attempt(diagnostics: &mut Map<String, Value>, attempt: Value) {
        if let Some(attempts) = diagnostics
            .get_mut("attempts")
            .and_then(Value::as_array_mut)
        {
            attempts.push(attempt);
        }
    }

    fn record_transport_summary(
        diagnostics: &mut Map<String, Value>,
        total_attempts: usize,
        effective_base_url: &str,
        endpoint_path: &str,
        backup_endpoint_used: bool,
        failover_count: usize,
    ) {
        diagnostics.insert(
            "summary".to_string(),
            serde_json::json!({
                "total_attempts": total_attempts,
                "effective_base_url": effective_base_url,
                "effective_endpoint": format!("{effective_base_url}{endpoint_path}"),
                "backup_endpoint_used": backup_endpoint_used,
                "failover_count": failover_count,
            }),
        );
    }

    fn finalize_transport_error(
        mut diagnostics: Map<String, Value>,
        total_attempts: usize,
        effective_base_url: &str,
        endpoint_path: &str,
        message: String,
        status_code: Option<u16>,
    ) -> AIRequestError {
        let (backup_endpoint_used, failover_count) =
            Self::summarize_transport_attempts(&diagnostics);
        Self::record_transport_summary(
            &mut diagnostics,
            total_attempts,
            effective_base_url,
            endpoint_path,
            backup_endpoint_used,
            failover_count,
        );

        AIRequestError::with_transport_status(message, Value::Object(diagnostics), status_code)
    }

    fn should_retry_chat_completions_candidate(
        error_message: &str,
        status: Option<reqwest::StatusCode>,
        is_network_error: bool,
    ) -> bool {
        if let Some(status) = status {
            return status.is_server_error() || matches!(status.as_u16(), 404 | 405 | 415 | 422);
        }

        if is_network_error {
            return true;
        }

        let lowered = error_message.to_ascii_lowercase();
        lowered.contains("non-json")
            || lowered.contains("non json")
            || lowered.contains("base url")
            || lowered.contains("/v1")
            || lowered.contains("doctype html")
            || lowered.contains("timeout")
            || lowered.contains("connection")
    }

    fn should_retry_chat_completions_endpoint_status(status: reqwest::StatusCode) -> bool {
        !matches!(status.as_u16(), 401 | 403 | 404)
    }

    fn should_failover_chat_completions_endpoint(
        status: Option<reqwest::StatusCode>,
        is_network_error: bool,
    ) -> bool {
        if is_network_error {
            return true;
        }

        status
            .map(|value| value.is_server_error() || value.as_u16() == 429)
            .unwrap_or(false)
    }

    fn build_chat_completion_body(
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        max_tokens: u32,
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
        stream: bool,
    ) -> Result<Value, String> {
        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        });
        if stream {
            body["stream"] = serde_json::json!(true);
        }

        if let Some(t) = tools {
            body["tools"] = serde_json::to_value(t).map_err(|e| format!("{}", e))?;
            match tool_choice {
                Some(tool_choice) => {
                    if let Some(tool_choice) = Self::serialize_tool_choice(Some(tool_choice)) {
                        body["tool_choice"] = tool_choice;
                    }
                }
                None => {
                    body["tool_choice"] = serde_json::json!("auto");
                }
            }
        }

        Ok(body)
    }

    pub fn new(
        api_key: &str,
        base_url: &str,
        backup_urls: Vec<String>,
        read_timeout_secs: Option<f64>,
        compat_profile: Option<&str>,
    ) -> Self {
        let timeout = read_timeout_secs
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(Duration::from_secs_f64)
            .unwrap_or_else(|| Duration::from_secs(300));
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("failed to create HTTP client");
        Self {
            client,
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            backup_urls: backup_urls
                .into_iter()
                .map(|value| value.trim().trim_end_matches('/').to_string())
                .filter(|value| !value.is_empty())
                .collect(),
            compat_profile: compat_profile
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("openai")
                .to_ascii_lowercase(),
        }
    }

    pub fn is_official_openai_base_url(base_url: &str) -> bool {
        reqwest::Url::parse(base_url)
            .ok()
            .and_then(|url| {
                url.host_str()
                    .map(|host| host.eq_ignore_ascii_case("api.openai.com"))
            })
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
            .fold(
                None,
                |best: Option<&(String, usize, bool)>, candidate| match best {
                    Some((_, best_matches, _)) if *best_matches >= candidate.1 => best,
                    _ => Some(candidate),
                },
            )
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
            "gpt", "chat", "claude", "gemini", "deepseek", "qwen", "llama", "mistral", "glm",
            "doubao", "hunyuan", "baichuan", "yi-", "command", "sonnet", "haiku", "opus",
            "reasoner", "instruct", "o1", "o3", "o4",
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
        tool_choice: Option<&ToolChoice>,
        prefer_normalized_v1_candidate: bool,
        transport_max_retries: Option<u32>,
    ) -> Result<AIResponse, String> {
        if max_tokens > Self::NON_STREAM_MAX_TOKENS_LIMIT {
            return self
                .chat_completion_via_stream(
                    messages,
                    model,
                    temperature,
                    max_tokens,
                    tools,
                    tool_choice,
                    prefer_normalized_v1_candidate,
                    transport_max_retries,
                )
                .await;
        }

        let body = Self::build_chat_completion_body(
            messages,
            model,
            temperature,
            max_tokens,
            tools,
            tool_choice,
            false,
        )?;
        let request_options = OpenAIRequestOptions {
            prefer_normalized_v1_candidate,
            transport_max_retries,
        };
        self.chat_completion_with_candidates(&body, request_options)
            .await
            .map_err(|error| error.message)
    }

    pub async fn chat_completion_detailed(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        max_tokens: u32,
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
        prefer_normalized_v1_candidate: bool,
        transport_max_retries: Option<u32>,
    ) -> Result<AIResponse, AIRequestError> {
        if max_tokens > Self::NON_STREAM_MAX_TOKENS_LIMIT {
            return self
                .chat_completion_via_stream(
                    messages,
                    model,
                    temperature,
                    max_tokens,
                    tools,
                    tool_choice,
                    prefer_normalized_v1_candidate,
                    transport_max_retries,
                )
                .await
                .map_err(AIRequestError::new);
        }

        let body = Self::build_chat_completion_body(
            messages,
            model,
            temperature,
            max_tokens,
            tools,
            tool_choice,
            false,
        )
        .map_err(AIRequestError::new)?;
        let request_options = OpenAIRequestOptions {
            prefer_normalized_v1_candidate,
            transport_max_retries,
        };
        self.chat_completion_with_candidates(&body, request_options)
            .await
    }

    async fn chat_completion_via_stream(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        max_tokens: u32,
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
        prefer_normalized_v1_candidate: bool,
        transport_max_retries: Option<u32>,
    ) -> Result<AIResponse, String> {
        let body = Self::build_chat_completion_body(
            messages,
            model,
            temperature,
            max_tokens,
            tools,
            tool_choice,
            true,
        )?;
        let request_options = OpenAIRequestOptions {
            prefer_normalized_v1_candidate,
            transport_max_retries,
        };
        self.chat_completion_with_candidates(&body, request_options)
            .await
            .map_err(|error| error.message)
    }

    async fn chat_completion_with_candidates(
        &self,
        body: &Value,
        request_options: OpenAIRequestOptions,
    ) -> Result<AIResponse, AIRequestError> {
        let primary_candidates = Self::build_chat_completion_base_url_candidates(
            &self.base_url,
            &self.compat_profile,
            request_options.prefer_normalized_v1_candidate,
        );
        let endpoint_path = "/chat/completions";
        let max_attempts_per_endpoint =
            request_options.transport_max_retries.unwrap_or(1).max(1) as usize;
        let mut diagnostics = Self::default_transport_diagnostics();
        diagnostics.insert(
            "transport_max_retries".to_string(),
            serde_json::json!(max_attempts_per_endpoint),
        );
        Self::record_transport_event(
            &mut diagnostics,
            serde_json::json!({
                "type": "api_mode_selected",
                "api_mode": "chat_completions",
            }),
        );

        let mut last_error = None;
        let primary_candidate_count = primary_candidates.len();

        'candidate_loop: for (candidate_index, primary_candidate_base_url) in
            primary_candidates.iter().enumerate()
        {
            let request_endpoints = self.build_request_endpoints(primary_candidate_base_url);
            let endpoint_count = request_endpoints.len();

            for (endpoint_position, target) in request_endpoints.into_iter().enumerate() {
                let url = format!("{}{endpoint_path}", target.base_url);
                Self::record_transport_event(
                    &mut diagnostics,
                    serde_json::json!({
                        "type": "chat_completions_candidate_selected",
                        "candidate_base_url": target.base_url,
                        "candidate_index": candidate_index + 1,
                        "candidate_count": primary_candidate_count,
                        "original_base_url": self.base_url,
                        "endpoint_role": target.endpoint_role,
                        "endpoint_index": target.endpoint_index,
                    }),
                );

                for attempt in 0..max_attempts_per_endpoint {
                    let response = self
                        .client
                        .post(&url)
                        .header("Authorization", format!("Bearer {}", self.api_key))
                        .json(body)
                        .send()
                        .await;

                    match response {
                        Ok(resp) => {
                            let status = resp.status();
                            let content_type = resp
                                .headers()
                                .get(CONTENT_TYPE)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or("")
                                .to_ascii_lowercase();
                            let text = resp.text().await.unwrap_or_default();

                            if !status.is_success() {
                                let error = format!("OpenAI HTTP {}: {}", status, text);
                                let can_failover = endpoint_position + 1 < endpoint_count;
                                let can_retry_candidate =
                                    Self::should_retry_chat_completions_candidate(
                                        &error,
                                        Some(status),
                                        false,
                                    );
                                let will_retry_same_endpoint =
                                    Self::should_retry_chat_completions_endpoint_status(status)
                                        && attempt + 1 < max_attempts_per_endpoint;
                                let will_failover = !will_retry_same_endpoint
                                    && can_failover
                                    && Self::should_failover_chat_completions_endpoint(
                                        Some(status),
                                        false,
                                    );
                                Self::record_transport_attempt(
                                    &mut diagnostics,
                                    serde_json::json!({
                                        "api_mode": "chat_completions",
                                        "base_url": target.base_url,
                                        "endpoint": url,
                                        "endpoint_role": target.endpoint_role,
                                        "endpoint_index": target.endpoint_index,
                                        "attempt_number": attempt + 1,
                                        "max_attempts": max_attempts_per_endpoint,
                                        "result": "http_error",
                                        "status_code": status.as_u16(),
                                        "will_retry_same_endpoint": will_retry_same_endpoint,
                                        "will_failover": will_failover,
                                    }),
                                );
                                Self::record_transport_event(
                                    &mut diagnostics,
                                    serde_json::json!({
                                        "type": "chat_completions_candidate_failed",
                                        "candidate_base_url": target.base_url,
                                        "candidate_index": candidate_index + 1,
                                        "status_code": status.as_u16(),
                                        "endpoint_role": target.endpoint_role,
                                        "endpoint_index": target.endpoint_index,
                                    }),
                                );
                                last_error = Some(error.clone());
                                if will_retry_same_endpoint {
                                    continue;
                                }
                                if will_failover {
                                    break;
                                }
                                if candidate_index + 1 < primary_candidate_count
                                    && can_retry_candidate
                                {
                                    continue 'candidate_loop;
                                }
                                let total_attempts = diagnostics
                                    .get("attempts")
                                    .and_then(Value::as_array)
                                    .map(|attempts| attempts.len())
                                    .unwrap_or(0);
                                return Err(Self::finalize_transport_error(
                                    diagnostics,
                                    total_attempts,
                                    &target.base_url,
                                    endpoint_path,
                                    error,
                                    Some(status.as_u16()),
                                ));
                            }

                            let parsed = if content_type.contains("text/event-stream") {
                                Self::parse_stream_text(&text)
                            } else {
                                match serde_json::from_str::<Value>(&text) {
                                    Ok(json) => Self::parse_response(&json),
                                    Err(_) => {
                                        Err(Self::invalid_content_error("OpenAI", status, &text))
                                    }
                                }
                            };

                            match parsed {
                                Ok(mut response) => {
                                    Self::record_transport_attempt(
                                        &mut diagnostics,
                                        serde_json::json!({
                                            "api_mode": "chat_completions",
                                            "base_url": target.base_url,
                                            "endpoint": url,
                                            "endpoint_role": target.endpoint_role,
                                            "endpoint_index": target.endpoint_index,
                                            "attempt_number": attempt + 1,
                                            "max_attempts": max_attempts_per_endpoint,
                                            "result": "success",
                                        }),
                                    );
                                    Self::record_transport_event(
                                        &mut diagnostics,
                                        serde_json::json!({
                                            "type": "chat_completions_candidate_succeeded",
                                            "candidate_base_url": target.base_url,
                                            "candidate_index": candidate_index + 1,
                                            "endpoint_role": target.endpoint_role,
                                            "endpoint_index": target.endpoint_index,
                                        }),
                                    );
                                    let total_attempts = diagnostics
                                        .get("attempts")
                                        .and_then(Value::as_array)
                                        .map(|attempts| attempts.len())
                                        .unwrap_or(0);
                                    let (backup_endpoint_used, failover_count) =
                                        Self::summarize_transport_attempts(&diagnostics);
                                    Self::record_transport_summary(
                                        &mut diagnostics,
                                        total_attempts,
                                        &target.base_url,
                                        endpoint_path,
                                        backup_endpoint_used,
                                        failover_count,
                                    );
                                    response.transport_diagnostics =
                                        Some(Value::Object(diagnostics));
                                    return Ok(response);
                                }
                                Err(error) => {
                                    let can_retry_candidate =
                                        Self::should_retry_chat_completions_candidate(
                                            &error, None, false,
                                        );
                                    Self::record_transport_attempt(
                                        &mut diagnostics,
                                        serde_json::json!({
                                            "api_mode": "chat_completions",
                                            "base_url": target.base_url,
                                            "endpoint": url,
                                            "endpoint_role": target.endpoint_role,
                                            "endpoint_index": target.endpoint_index,
                                            "attempt_number": attempt + 1,
                                            "max_attempts": max_attempts_per_endpoint,
                                            "result": "parse_error",
                                            "will_retry_same_endpoint": false,
                                            "will_failover": false,
                                        }),
                                    );
                                    Self::record_transport_event(
                                        &mut diagnostics,
                                        serde_json::json!({
                                            "type": "chat_completions_candidate_failed",
                                            "candidate_base_url": target.base_url,
                                            "candidate_index": candidate_index + 1,
                                            "error_message": error,
                                            "endpoint_role": target.endpoint_role,
                                            "endpoint_index": target.endpoint_index,
                                        }),
                                    );
                                    last_error = Some(error.clone());
                                    if candidate_index + 1 < primary_candidate_count
                                        && can_retry_candidate
                                    {
                                        continue 'candidate_loop;
                                    }
                                    let total_attempts = diagnostics
                                        .get("attempts")
                                        .and_then(Value::as_array)
                                        .map(|attempts| attempts.len())
                                        .unwrap_or(0);
                                    return Err(Self::finalize_transport_error(
                                        diagnostics,
                                        total_attempts,
                                        &target.base_url,
                                        endpoint_path,
                                        error,
                                        None,
                                    ));
                                }
                            }
                        }
                        Err(error) => {
                            let is_network_error = error.is_connect() || error.is_timeout();
                            let error = format!("OpenAI request failed: {}", error);
                            let can_failover = endpoint_position + 1 < endpoint_count;
                            let can_retry_candidate = Self::should_retry_chat_completions_candidate(
                                &error,
                                None,
                                is_network_error,
                            );
                            let will_retry_same_endpoint =
                                is_network_error && attempt + 1 < max_attempts_per_endpoint;
                            let will_failover = !will_retry_same_endpoint
                                && can_failover
                                && Self::should_failover_chat_completions_endpoint(
                                    None,
                                    is_network_error,
                                );
                            Self::record_transport_attempt(
                                &mut diagnostics,
                                serde_json::json!({
                                    "api_mode": "chat_completions",
                                    "base_url": target.base_url,
                                    "endpoint": url,
                                    "endpoint_role": target.endpoint_role,
                                    "endpoint_index": target.endpoint_index,
                                    "attempt_number": attempt + 1,
                                    "max_attempts": max_attempts_per_endpoint,
                                    "result": "network_error",
                                    "will_retry_same_endpoint": will_retry_same_endpoint,
                                    "will_failover": will_failover,
                                }),
                            );
                            Self::record_transport_event(
                                &mut diagnostics,
                                serde_json::json!({
                                    "type": "chat_completions_candidate_failed",
                                    "candidate_base_url": target.base_url,
                                    "candidate_index": candidate_index + 1,
                                    "error_message": error,
                                    "endpoint_role": target.endpoint_role,
                                    "endpoint_index": target.endpoint_index,
                                }),
                            );
                            last_error = Some(error.clone());
                            if will_retry_same_endpoint {
                                continue;
                            }
                            if will_failover {
                                break;
                            }
                            if candidate_index + 1 < primary_candidate_count && can_retry_candidate
                            {
                                continue 'candidate_loop;
                            }
                            let total_attempts = diagnostics
                                .get("attempts")
                                .and_then(Value::as_array)
                                .map(|attempts| attempts.len())
                                .unwrap_or(0);
                            return Err(Self::finalize_transport_error(
                                diagnostics,
                                total_attempts,
                                &target.base_url,
                                endpoint_path,
                                error,
                                None,
                            ));
                        }
                    }
                }
            }
        }

        let final_error = last_error.unwrap_or_else(|| "OpenAI request failed".to_string());
        let effective_base_url = primary_candidates
            .last()
            .map(String::as_str)
            .unwrap_or(self.base_url.as_str());
        let total_attempts = diagnostics
            .get("attempts")
            .and_then(Value::as_array)
            .map(|attempts| attempts.len())
            .unwrap_or(0);
        Err(Self::finalize_transport_error(
            diagnostics,
            total_attempts,
            effective_base_url,
            endpoint_path,
            final_error,
            None,
        ))
    }

    pub fn chat_completion_stream(
        &self,
        messages: Vec<ChatMessage>,
        model: String,
        temperature: f64,
        max_tokens: u32,
        tools: Option<Vec<ToolDef>>,
        tool_choice: Option<ToolChoice>,
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
        tools: Option<Vec<ToolDef>>,
        tool_choice: Option<ToolChoice>,
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
            match tool_choice.as_ref() {
                Some(tool_choice) => {
                    if let Some(tool_choice) = Self::serialize_tool_choice(Some(tool_choice)) {
                        body["tool_choice"] = tool_choice;
                    }
                }
                None => {
                    body["tool_choice"] = serde_json::json!("auto");
                }
            }
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
            transport_diagnostics: None,
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
                            let idx = tc_delta.get("index").and_then(|i| i.as_i64()).unwrap_or(0)
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
                        if let Some(tool_calls) = message
                            .get("tool_calls")
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
            transport_diagnostics: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::OpenAIClient;
    use crate::ai::types::{ToolChoice, ToolChoiceFunction};

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

    #[test]
    fn serializes_required_and_named_tool_choice() {
        assert_eq!(
            OpenAIClient::serialize_tool_choice(Some(&ToolChoice::Required)),
            Some(json!("required"))
        );
        assert_eq!(
            OpenAIClient::serialize_tool_choice(Some(&ToolChoice::Function(ToolChoiceFunction {
                name: "get_weather".to_string(),
            }))),
            Some(json!({
                "type": "function",
                "function": {
                    "name": "get_weather"
                }
            }))
        );
    }

    #[test]
    fn build_chat_completion_candidates_expand_local_https_and_docker_variants() {
        let candidates = OpenAIClient::build_chat_completion_base_url_candidates_for(
            "https://127.0.0.1:8317",
            "openai",
            true,
            true,
        );

        assert_eq!(
            candidates,
            vec![
                "https://127.0.0.1:8317/v1".to_string(),
                "https://host.docker.internal:8317/v1".to_string(),
                "http://127.0.0.1:8317/v1".to_string(),
                "http://host.docker.internal:8317/v1".to_string(),
                "https://127.0.0.1:8317".to_string(),
                "https://host.docker.internal:8317".to_string(),
                "http://127.0.0.1:8317".to_string(),
                "http://host.docker.internal:8317".to_string(),
            ]
        );
    }

    #[test]
    fn network_errors_are_retryable_chat_completion_candidates() {
        assert!(OpenAIClient::should_retry_chat_completions_candidate(
            "OpenAI request failed: error sending request for url",
            None,
            true,
        ));
    }
}
