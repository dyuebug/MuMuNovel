use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::ai::clients::anthropic::AnthropicClient;
use crate::ai::clients::gemini::GeminiClient;
use crate::ai::clients::openai::OpenAIClient;
use crate::ai::config::AIConfig;
use crate::ai::execution_trace::{
    build_ai_execution_trace, AIExecutionOutcome, AIExecutionTraceV1, TrackedAIRequestError,
    TrackedAIResponse, TrackedAIStream,
};
use crate::ai::types::{
    AIRequestError, AIResponse, AIStreamChunk, ChatMessage, ToolChoice, ToolDef,
};
use crate::services::settings_service::default_model_for_provider;

pub struct AIService {
    config: AIConfig,
}

impl AIService {
    fn model_fallback_reason(error: &str) -> Option<&'static str> {
        let normalized = error.to_lowercase();
        if normalized.contains("base url") {
            return None;
        }
        if normalized.contains("model not found")
            || normalized.contains("\"code\":\"not_found\"")
            || normalized.contains("\"code\": \"not_found\"")
            || normalized.contains("模型不存在")
        {
            return Some("model_not_found");
        }
        if normalized.contains("inaccessible") || normalized.contains("not deployed") {
            return Some("model_unavailable");
        }
        None
    }

    fn should_retry_with_fallback_model(error: &str) -> bool {
        Self::model_fallback_reason(error).is_some()
    }

    fn complete_stream_trace(
        completion: &mut Option<oneshot::Sender<AIExecutionTraceV1>>,
        trace: AIExecutionTraceV1,
    ) {
        if let Some(sender) = completion.take() {
            let _ = sender.send(trace);
        }
    }

    fn static_fallback_model(provider: &str, current_model: &str) -> Option<String> {
        let fallback = default_model_for_provider(provider);
        (fallback.trim() != current_model.trim()).then_some(fallback)
    }

    async fn resolve_fallback_model(
        provider: &str,
        api_key: &str,
        base_url: &str,
        current_model: &str,
    ) -> Option<String> {
        match provider {
            "anthropic" | "gemini" => Self::static_fallback_model(provider, current_model),
            _ => {
                let client = OpenAIClient::new(api_key, base_url, Vec::new(), None, Some(provider));
                if let Ok(models) = client.list_models().await {
                    if let Some(model) = OpenAIClient::pick_fallback_model(current_model, &models) {
                        return Some(model);
                    }
                }

                if OpenAIClient::is_official_openai_base_url(base_url) {
                    return Self::static_fallback_model(provider, current_model);
                }

                None
            }
        }
    }

    async fn call_client_with_model(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
        model: &str,
    ) -> Result<AIResponse, String> {
        match self.config.provider.as_str() {
            "anthropic" => {
                let client = AnthropicClient::new(&self.config.api_key, &self.config.base_url);
                client
                    .chat_completion(
                        messages,
                        model,
                        self.config.temperature,
                        self.config.max_tokens,
                        self.config.system_prompt.as_deref(),
                        tools,
                        tool_choice,
                    )
                    .await
            }
            "gemini" => {
                let client = GeminiClient::new(&self.config.api_key, &self.config.base_url);
                client
                    .chat_completion(
                        messages,
                        model,
                        self.config.temperature,
                        self.config.max_tokens,
                        self.config.system_prompt.as_deref(),
                        tools,
                        tool_choice,
                    )
                    .await
            }
            _ => {
                let client = OpenAIClient::new(
                    &self.config.api_key,
                    &self.config.base_url,
                    self.config.backup_urls.clone(),
                    self.config.read_timeout_secs,
                    Some(&self.config.provider),
                );
                client
                    .chat_completion(
                        messages,
                        model,
                        self.config.temperature,
                        self.config.max_tokens,
                        tools,
                        tool_choice,
                        self.config.prefer_normalized_v1_candidate,
                        self.config.transport_max_retries,
                    )
                    .await
            }
        }
    }

    async fn call_client_with_model_detailed(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
        model: &str,
    ) -> Result<AIResponse, AIRequestError> {
        match self.config.provider.as_str() {
            "anthropic" => {
                let client = AnthropicClient::new(&self.config.api_key, &self.config.base_url);
                client
                    .chat_completion_detailed(
                        messages,
                        model,
                        self.config.temperature,
                        self.config.max_tokens,
                        self.config.system_prompt.as_deref(),
                        tools,
                        tool_choice,
                    )
                    .await
            }
            "gemini" => {
                let client = GeminiClient::new(&self.config.api_key, &self.config.base_url);
                client
                    .chat_completion_detailed(
                        messages,
                        model,
                        self.config.temperature,
                        self.config.max_tokens,
                        self.config.system_prompt.as_deref(),
                        tools,
                        tool_choice,
                    )
                    .await
            }
            _ => {
                let client = OpenAIClient::new(
                    &self.config.api_key,
                    &self.config.base_url,
                    self.config.backup_urls.clone(),
                    self.config.read_timeout_secs,
                    Some(&self.config.provider),
                );
                client
                    .chat_completion_detailed(
                        messages,
                        model,
                        self.config.temperature,
                        self.config.max_tokens,
                        tools,
                        tool_choice,
                        self.config.prefer_normalized_v1_candidate,
                        self.config.transport_max_retries,
                    )
                    .await
            }
        }
    }

    pub fn new(config: AIConfig) -> Self {
        Self { config }
    }

    pub async fn generate_text(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
    ) -> Result<AIResponse, String> {
        let messages = Self::build_messages(system_prompt, prompt);
        self.call_client(&messages, tools, None).await
    }

    pub async fn generate_text_detailed(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
    ) -> Result<AIResponse, AIRequestError> {
        let messages = Self::build_messages(system_prompt, prompt);
        self.call_client_detailed(&messages, tools, None).await
    }

    pub async fn generate_text_tracked(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
        allow_model_fallback: bool,
    ) -> Result<TrackedAIResponse, TrackedAIRequestError> {
        let messages = Self::build_messages(system_prompt, prompt);
        self.call_client_tracked_detailed(&messages, tools, None, allow_model_fallback)
            .await
    }

    pub async fn generate_text_with_tool_choice_tracked_detailed(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
        allow_model_fallback: bool,
    ) -> Result<TrackedAIResponse, TrackedAIRequestError> {
        let messages = Self::build_messages(system_prompt, prompt);
        self.call_client_tracked_detailed(&messages, tools, tool_choice, allow_model_fallback)
            .await
    }

    pub async fn generate_text_with_tool_choice_detailed(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<AIResponse, AIRequestError> {
        let messages = Self::build_messages(system_prompt, prompt);
        self.call_client_detailed(&messages, tools, tool_choice)
            .await
    }

    pub fn generate_text_stream(
        &self,
        prompt: String,
        system_prompt: Option<String>,
        tools: Option<Vec<ToolDef>>,
    ) -> ReceiverStream<Result<AIStreamChunk, String>> {
        let messages = Self::build_messages_vec(system_prompt, prompt);
        self.call_client_stream(messages, tools)
    }

    pub fn generate_text_stream_tracked(
        &self,
        prompt: String,
        system_prompt: Option<String>,
        tools: Option<Vec<ToolDef>>,
        allow_model_fallback: bool,
    ) -> TrackedAIStream {
        let messages = Self::build_messages_vec(system_prompt, prompt);
        self.call_client_stream_tracked(messages, tools, allow_model_fallback)
    }

    async fn call_client(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<AIResponse, String> {
        match self
            .call_client_with_model(messages, tools, tool_choice, &self.config.model)
            .await
        {
            Ok(response) => Ok(response),
            Err(error) => {
                if Self::should_retry_with_fallback_model(&error) {
                    if let Some(fallback_model) = Self::resolve_fallback_model(
                        &self.config.provider,
                        &self.config.api_key,
                        &self.config.base_url,
                        &self.config.model,
                    )
                    .await
                    {
                        return self
                            .call_client_with_model(messages, tools, tool_choice, &fallback_model)
                            .await
                            .map_err(|fallback_error| {
                                format!(
                                    "{}; fallback model {} also failed: {}",
                                    error, fallback_model, fallback_error
                                )
                            });
                    }
                }
                Err(error)
            }
        }
    }

    async fn call_client_detailed(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<AIResponse, AIRequestError> {
        self.call_client_tracked_detailed(messages, tools, tool_choice, true)
            .await
            .map(|tracked| tracked.response)
            .map_err(|tracked| tracked.error)
    }

    async fn call_client_tracked_detailed(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
        allow_model_fallback: bool,
    ) -> Result<TrackedAIResponse, TrackedAIRequestError> {
        let requested_model = self.config.model.clone();
        match self
            .call_client_with_model_detailed(messages, tools, tool_choice, &requested_model)
            .await
        {
            Ok(response) => {
                let execution = build_ai_execution_trace(
                    &self.config.provider,
                    &requested_model,
                    &requested_model,
                    AIExecutionOutcome::Succeeded,
                    None,
                    response.transport_diagnostics.as_ref(),
                );
                Ok(TrackedAIResponse {
                    response,
                    execution,
                })
            }
            Err(primary_error) => {
                let fallback_reason = Self::model_fallback_reason(&primary_error.message);
                if allow_model_fallback {
                    if let Some(reason) = fallback_reason {
                        if let Some(fallback_model) = Self::resolve_fallback_model(
                            &self.config.provider,
                            &self.config.api_key,
                            &self.config.base_url,
                            &requested_model,
                        )
                        .await
                        {
                            return match self
                                .call_client_with_model_detailed(
                                    messages,
                                    tools,
                                    tool_choice,
                                    &fallback_model,
                                )
                                .await
                            {
                                Ok(response) => {
                                    let execution = build_ai_execution_trace(
                                        &self.config.provider,
                                        &requested_model,
                                        &fallback_model,
                                        AIExecutionOutcome::Succeeded,
                                        Some(reason),
                                        response.transport_diagnostics.as_ref(),
                                    );
                                    Ok(TrackedAIResponse {
                                        response,
                                        execution,
                                    })
                                }
                                Err(fallback_error) => {
                                    let error = AIRequestError {
                                        message: format!(
                                            "{}; fallback model {} also failed: {}",
                                            primary_error.message,
                                            fallback_model,
                                            fallback_error.message
                                        ),
                                        transport_diagnostics: fallback_error
                                            .transport_diagnostics
                                            .or(primary_error.transport_diagnostics),
                                        status_code: fallback_error
                                            .status_code
                                            .or(primary_error.status_code),
                                        retry_after_seconds: fallback_error
                                            .retry_after_seconds
                                            .or(primary_error.retry_after_seconds),
                                    };
                                    let execution = build_ai_execution_trace(
                                        &self.config.provider,
                                        &requested_model,
                                        &fallback_model,
                                        AIExecutionOutcome::Failed,
                                        Some(reason),
                                        error.transport_diagnostics.as_ref(),
                                    );
                                    Err(TrackedAIRequestError { error, execution })
                                }
                            };
                        }
                    }
                }

                let execution = build_ai_execution_trace(
                    &self.config.provider,
                    &requested_model,
                    &requested_model,
                    AIExecutionOutcome::Failed,
                    None,
                    primary_error.transport_diagnostics.as_ref(),
                );
                Err(TrackedAIRequestError {
                    error: primary_error,
                    execution,
                })
            }
        }
    }

    fn call_client_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDef>>,
    ) -> ReceiverStream<Result<AIStreamChunk, String>> {
        let mut typed_stream = self
            .call_client_stream_tracked(messages, tools, true)
            .stream;
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<AIStreamChunk, String>>(32);
        tokio::spawn(async move {
            while let Some(item) = typed_stream.next().await {
                if tx
                    .send(item.map_err(|error| error.to_string()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        ReceiverStream::new(rx)
    }

    fn call_client_stream_tracked(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDef>>,
        allow_model_fallback: bool,
    ) -> TrackedAIStream {
        let primary_stream = match self.config.provider.as_str() {
            "anthropic" => {
                let client = AnthropicClient::new(&self.config.api_key, &self.config.base_url);
                client.chat_completion_stream(
                    messages.clone(),
                    self.config.model.clone(),
                    self.config.temperature,
                    self.config.max_tokens,
                    self.config.system_prompt.clone(),
                    tools.clone(),
                    None,
                )
            }
            "gemini" => {
                let client = GeminiClient::new(&self.config.api_key, &self.config.base_url);
                client.chat_completion_stream(
                    messages.clone(),
                    self.config.model.clone(),
                    self.config.temperature,
                    self.config.max_tokens,
                    self.config.system_prompt.clone(),
                    tools.clone(),
                    None,
                )
            }
            _ => {
                let client = OpenAIClient::new(
                    &self.config.api_key,
                    &self.config.base_url,
                    self.config.backup_urls.clone(),
                    self.config.read_timeout_secs,
                    Some(&self.config.provider),
                );
                client.chat_completion_stream(
                    messages.clone(),
                    self.config.model.clone(),
                    self.config.temperature,
                    self.config.max_tokens,
                    tools.clone(),
                    None,
                )
            }
        };

        let provider = self.config.provider.clone();
        let api_key = self.config.api_key.clone();
        let base_url = self.config.base_url.clone();
        let requested_model = self.config.model.clone();
        let max_tokens = self.config.max_tokens;
        let temperature = self.config.temperature;
        let system_prompt = self.config.system_prompt.clone();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<AIStreamChunk, AIRequestError>>(32);
        let (completion_tx, completion_rx) = oneshot::channel::<AIExecutionTraceV1>();

        tokio::spawn(async move {
            let mut completion = Some(completion_tx);
            let mut primary_stream = primary_stream;
            let Some(first_item) = primary_stream.next().await else {
                Self::complete_stream_trace(
                    &mut completion,
                    build_ai_execution_trace(
                        &provider,
                        &requested_model,
                        &requested_model,
                        AIExecutionOutcome::Succeeded,
                        None,
                        None,
                    ),
                );
                return;
            };

            match first_item {
                Ok(chunk) => {
                    let mut outcome = AIExecutionOutcome::Succeeded;
                    if tx.send(Ok(chunk)).await.is_err() {
                        outcome = AIExecutionOutcome::Failed;
                    } else {
                        while let Some(item) = primary_stream.next().await {
                            if item.is_err() {
                                outcome = AIExecutionOutcome::Failed;
                            }
                            if tx.send(item).await.is_err() {
                                outcome = AIExecutionOutcome::Failed;
                                break;
                            }
                        }
                    }
                    Self::complete_stream_trace(
                        &mut completion,
                        build_ai_execution_trace(
                            &provider,
                            &requested_model,
                            &requested_model,
                            outcome,
                            None,
                            None,
                        ),
                    );
                }
                Err(error) => {
                    let fallback_reason = Self::model_fallback_reason(&error.message);
                    if !allow_model_fallback || fallback_reason.is_none() {
                        let _ = tx.send(Err(error)).await;
                        Self::complete_stream_trace(
                            &mut completion,
                            build_ai_execution_trace(
                                &provider,
                                &requested_model,
                                &requested_model,
                                AIExecutionOutcome::Failed,
                                None,
                                None,
                            ),
                        );
                        return;
                    }

                    let Some(fallback_model) = Self::resolve_fallback_model(
                        &provider,
                        &api_key,
                        &base_url,
                        &requested_model,
                    )
                    .await
                    else {
                        let _ = tx.send(Err(error)).await;
                        Self::complete_stream_trace(
                            &mut completion,
                            build_ai_execution_trace(
                                &provider,
                                &requested_model,
                                &requested_model,
                                AIExecutionOutcome::Failed,
                                None,
                                None,
                            ),
                        );
                        return;
                    };

                    let fallback_stream = match provider.as_str() {
                        "anthropic" => {
                            let client = AnthropicClient::new(&api_key, &base_url);
                            client.chat_completion_stream(
                                messages.clone(),
                                fallback_model.clone(),
                                temperature,
                                max_tokens,
                                system_prompt.clone(),
                                tools.clone(),
                                None,
                            )
                        }
                        "gemini" => {
                            let client = GeminiClient::new(&api_key, &base_url);
                            client.chat_completion_stream(
                                messages.clone(),
                                fallback_model.clone(),
                                temperature,
                                max_tokens,
                                system_prompt.clone(),
                                tools.clone(),
                                None,
                            )
                        }
                        _ => {
                            let client = OpenAIClient::new(
                                &api_key,
                                &base_url,
                                Vec::new(),
                                None,
                                Some(&provider),
                            );
                            client.chat_completion_stream(
                                messages.clone(),
                                fallback_model.clone(),
                                temperature,
                                max_tokens,
                                tools.clone(),
                                None,
                            )
                        }
                    };

                    let mut outcome = AIExecutionOutcome::Succeeded;
                    tokio::pin!(fallback_stream);
                    while let Some(fallback_item) = fallback_stream.next().await {
                        match fallback_item {
                            Ok(chunk) => {
                                if tx.send(Ok(chunk)).await.is_err() {
                                    outcome = AIExecutionOutcome::Failed;
                                    break;
                                }
                            }
                            Err(fallback_error) => {
                                outcome = AIExecutionOutcome::Failed;
                                let _ = tx
                                    .send(Err(AIRequestError {
                                        message: format!(
                                            "{}; fallback model {} also failed: {}",
                                            error, fallback_model, fallback_error
                                        ),
                                        transport_diagnostics: fallback_error
                                            .transport_diagnostics
                                            .or(error.transport_diagnostics),
                                        status_code: fallback_error
                                            .status_code
                                            .or(error.status_code),
                                        retry_after_seconds: fallback_error
                                            .retry_after_seconds
                                            .or(error.retry_after_seconds),
                                    }))
                                    .await;
                                break;
                            }
                        }
                    }
                    Self::complete_stream_trace(
                        &mut completion,
                        build_ai_execution_trace(
                            &provider,
                            &requested_model,
                            &fallback_model,
                            outcome,
                            fallback_reason,
                            None,
                        ),
                    );
                }
            }
        });

        TrackedAIStream {
            stream: ReceiverStream::new(rx),
            completion: completion_rx,
        }
    }

    fn build_messages(system_prompt: Option<&str>, prompt: &str) -> Vec<ChatMessage> {
        let mut msgs = Vec::new();
        if let Some(sp) = system_prompt {
            msgs.push(ChatMessage {
                role: "system".into(),
                content: sp.to_string(),
            });
        }
        msgs.push(ChatMessage {
            role: "user".into(),
            content: prompt.to_string(),
        });
        msgs
    }

    fn build_messages_vec(system_prompt: Option<String>, prompt: String) -> Vec<ChatMessage> {
        let mut msgs = Vec::new();
        if let Some(sp) = system_prompt {
            msgs.push(ChatMessage {
                role: "system".into(),
                content: sp,
            });
        }
        msgs.push(ChatMessage {
            role: "user".into(),
            content: prompt,
        });
        msgs
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use axum::extract::State;
    use axum::http::{header::RETRY_AFTER, HeaderValue, StatusCode};
    use axum::response::{IntoResponse, Response};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use serde_json::{json, Value};
    use tokio::net::TcpListener;
    use tokio_stream::StreamExt;

    use super::AIService;
    use crate::ai::config::AIConfig;
    use crate::ai::execution_trace::{AIExecutionFallbackKind, AIExecutionOutcome};

    #[derive(Clone, Copy)]
    enum TestMode {
        PrimarySuccess,
        FallbackSuccess,
        FallbackFailure,
        FallbackFailureWithBothRetryHints,
        FallbackFailureWithPrimaryRetryHint,
    }

    #[derive(Clone)]
    struct TestServerState {
        mode: TestMode,
        model_list_calls: Arc<AtomicUsize>,
    }

    struct TestServerHandle {
        handle: tokio::task::JoinHandle<()>,
    }

    impl Drop for TestServerHandle {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    async fn models_handler(State(state): State<TestServerState>) -> Json<Value> {
        state.model_list_calls.fetch_add(1, Ordering::SeqCst);
        Json(json!({"data": [{"id": "fallback-model"}]}))
    }

    async fn chat_handler(
        State(state): State<TestServerState>,
        Json(body): Json<Value>,
    ) -> Response {
        let model = body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if model == "primary-model" && matches!(state.mode, TestMode::PrimarySuccess) {
            return successful_chat_response("primary response");
        }
        if model == "primary-model" {
            let retry_after = matches!(
                state.mode,
                TestMode::FallbackFailureWithBothRetryHints
                    | TestMode::FallbackFailureWithPrimaryRetryHint
            )
            .then_some("120");
            return json_response(
                StatusCode::NOT_FOUND,
                json!({
                    "error": {"message": "model not found", "code": "not_found"}
                }),
                retry_after,
            );
        }
        if model == "fallback-model" && matches!(state.mode, TestMode::FallbackSuccess) {
            return successful_chat_response("fallback response");
        }

        let retry_after =
            matches!(state.mode, TestMode::FallbackFailureWithBothRetryHints).then_some("240");
        json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({
                "error": {"message": "fallback upstream unavailable"}
            }),
            retry_after,
        )
    }

    fn successful_chat_response(content: &str) -> Response {
        json_response(
            StatusCode::OK,
            json!({
                "choices": [{
                    "message": {"content": content},
                    "finish_reason": "stop"
                }]
            }),
            None,
        )
    }

    fn json_response(status: StatusCode, payload: Value, retry_after: Option<&str>) -> Response {
        let mut response = (status, Json(payload)).into_response();
        if let Some(retry_after) = retry_after {
            response.headers_mut().insert(
                RETRY_AFTER,
                HeaderValue::from_str(retry_after).expect("valid retry-after test header"),
            );
        }
        response
    }

    async fn spawn_openai_server(mode: TestMode) -> (String, Arc<AtomicUsize>, TestServerHandle) {
        let model_list_calls = Arc::new(AtomicUsize::new(0));
        let state = TestServerState {
            mode,
            model_list_calls: Arc::clone(&model_list_calls),
        };
        let app = Router::new()
            .route("/v1/models", get(models_handler))
            .route("/v1/chat/completions", post(chat_handler))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });

        (
            format!("http://{address}/v1"),
            model_list_calls,
            TestServerHandle { handle },
        )
    }

    fn test_ai_config(base_url: String) -> AIConfig {
        AIConfig {
            provider: "openai".to_string(),
            api_key: "test-secret-key".to_string(),
            base_url,
            model: "primary-model".to_string(),
            temperature: 0.2,
            max_tokens: 128,
            ..AIConfig::default()
        }
    }

    #[tokio::test]
    async fn tracked_non_stream_primary_success_records_actual_execution() {
        let (base_url, model_list_calls, _server) =
            spawn_openai_server(TestMode::PrimarySuccess).await;
        let service = AIService::new(test_ai_config(base_url));

        let tracked = service
            .generate_text_tracked("secret prompt", None, None, true)
            .await
            .expect("primary request should succeed");

        assert_eq!(tracked.response.content, "primary response");
        assert_eq!(tracked.execution.requested_model, "primary-model");
        assert_eq!(tracked.execution.actual_model, "primary-model");
        assert_eq!(tracked.execution.outcome, AIExecutionOutcome::Succeeded);
        assert!(tracked.execution.fallbacks.is_empty());
        assert_eq!(
            tracked
                .execution
                .endpoint_summary
                .as_ref()
                .expect("endpoint summary")
                .endpoint_role,
            "primary"
        );
        assert_eq!(model_list_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn tracked_non_stream_disables_model_fallback_when_policy_closes_it() {
        let (base_url, model_list_calls, _server) =
            spawn_openai_server(TestMode::FallbackSuccess).await;
        let service = AIService::new(test_ai_config(base_url));

        let tracked_error = service
            .generate_text_tracked("secret prompt", None, None, false)
            .await
            .expect_err("primary model should fail without fallback");

        assert_eq!(tracked_error.execution.actual_model, "primary-model");
        assert_eq!(tracked_error.execution.outcome, AIExecutionOutcome::Failed);
        assert!(tracked_error.execution.fallbacks.is_empty());
        assert_eq!(model_list_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn tracked_non_stream_records_model_fallback_success() {
        let (base_url, model_list_calls, _server) =
            spawn_openai_server(TestMode::FallbackSuccess).await;
        let service = AIService::new(test_ai_config(base_url));

        let tracked = service
            .generate_text_tracked("secret prompt", None, None, true)
            .await
            .expect("fallback request should succeed");

        assert_eq!(tracked.response.content, "fallback response");
        assert_eq!(tracked.execution.requested_model, "primary-model");
        assert_eq!(tracked.execution.actual_model, "fallback-model");
        assert_eq!(tracked.execution.outcome, AIExecutionOutcome::Succeeded);
        assert_eq!(tracked.execution.fallbacks.len(), 1);
        assert_eq!(
            tracked.execution.fallbacks[0].kind,
            AIExecutionFallbackKind::ModelFallback
        );
        assert_eq!(tracked.execution.fallbacks[0].reason, "model_not_found");
        assert_eq!(model_list_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn tracked_non_stream_records_model_fallback_failure_without_raw_error_in_trace() {
        let (base_url, model_list_calls, _server) =
            spawn_openai_server(TestMode::FallbackFailure).await;
        let service = AIService::new(test_ai_config(base_url));

        let tracked_error = service
            .generate_text_tracked("secret prompt", None, None, true)
            .await
            .expect_err("fallback request should fail");

        assert_eq!(tracked_error.execution.actual_model, "fallback-model");
        assert_eq!(tracked_error.execution.outcome, AIExecutionOutcome::Failed);
        assert_eq!(
            tracked_error.execution.fallbacks[0].kind,
            AIExecutionFallbackKind::ModelFallback
        );
        let serialized = serde_json::to_string(&tracked_error.execution).expect("serialize trace");
        assert!(!serialized.contains("fallback upstream unavailable"));
        assert!(!serialized.contains("secret prompt"));
        assert!(!serialized.contains("test-secret-key"));
        assert_eq!(model_list_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn tracked_non_stream_prefers_fallback_retry_after_hint() {
        let (base_url, _model_list_calls, _server) =
            spawn_openai_server(TestMode::FallbackFailureWithBothRetryHints).await;
        let service = AIService::new(test_ai_config(base_url));

        let tracked_error = service
            .generate_text_tracked("secret prompt", None, None, true)
            .await
            .expect_err("fallback request should fail");

        assert_eq!(tracked_error.error.status_code, Some(503));
        assert_eq!(tracked_error.error.retry_after_seconds, Some(240));
    }

    #[tokio::test]
    async fn tracked_non_stream_uses_primary_retry_after_when_fallback_has_none() {
        let (base_url, _model_list_calls, _server) =
            spawn_openai_server(TestMode::FallbackFailureWithPrimaryRetryHint).await;
        let service = AIService::new(test_ai_config(base_url));

        let tracked_error = service
            .generate_text_tracked("secret prompt", None, None, true)
            .await
            .expect_err("fallback request should fail");

        assert_eq!(tracked_error.error.status_code, Some(503));
        assert_eq!(tracked_error.error.retry_after_seconds, Some(120));
    }

    #[tokio::test]
    async fn tracked_stream_completes_trace_after_model_fallback_success() {
        let (base_url, model_list_calls, _server) =
            spawn_openai_server(TestMode::FallbackSuccess).await;
        let service = AIService::new(test_ai_config(base_url));
        let tracked = service.generate_text_stream_tracked(
            "secret stream prompt".to_string(),
            None,
            None,
            true,
        );
        let mut stream = tracked.stream;
        let completion = tracked.completion;
        let mut content = String::new();
        while let Some(item) = stream.next().await {
            let chunk = item.expect("fallback stream chunk");
            if let Some(value) = chunk.content {
                content.push_str(&value);
            }
        }
        let execution = completion.await.expect("completion trace");

        assert_eq!(content, "fallback response");
        assert_eq!(execution.actual_model, "fallback-model");
        assert_eq!(execution.outcome, AIExecutionOutcome::Succeeded);
        assert_eq!(
            execution.fallbacks[0].kind,
            AIExecutionFallbackKind::ModelFallback
        );
        assert_eq!(model_list_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn legacy_non_stream_and_stream_methods_keep_existing_return_shapes() {
        let (base_url, _model_list_calls, _server) =
            spawn_openai_server(TestMode::PrimarySuccess).await;
        let service = AIService::new(test_ai_config(base_url));

        let response = service
            .generate_text("legacy prompt", None, None)
            .await
            .expect("legacy response");
        assert_eq!(response.content, "primary response");

        let mut stream = service.generate_text_stream("legacy prompt".to_string(), None, None);
        let mut stream_content = String::new();
        while let Some(item) = stream.next().await {
            let chunk = item.expect("legacy stream chunk");
            if let Some(value) = chunk.content {
                stream_content.push_str(&value);
            }
        }
        assert_eq!(stream_content, "primary response");
    }
}
