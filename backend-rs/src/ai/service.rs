use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::ai::clients::anthropic::AnthropicClient;
use crate::ai::clients::openai::OpenAIClient;
use crate::ai::config::AIConfig;
use crate::ai::types::{AIResponse, AIStreamChunk, ChatMessage, ToolDef};
use crate::services::settings_service::default_model_for_provider;

pub struct AIService {
    config: AIConfig,
}

impl AIService {
    fn should_retry_with_fallback_model(error: &str) -> bool {
        let normalized = error.to_lowercase();
        (normalized.contains("model not found")
            || normalized.contains("\"code\":\"not_found\"")
            || normalized.contains("\"code\": \"not_found\"")
            || normalized.contains("模型不存在")
            || normalized.contains("inaccessible")
            || normalized.contains("not deployed"))
            && !normalized.contains("base url")
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
                let client = OpenAIClient::new(api_key, base_url);
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
                    )
                    .await
            }
            _ => {
                let client = OpenAIClient::new(&self.config.api_key, &self.config.base_url);
                client
                    .chat_completion(
                        messages,
                        model,
                        self.config.temperature,
                        self.config.max_tokens,
                        tools,
                    )
                    .await
            }
        }
    }

    pub fn new(config: AIConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &AIConfig {
        &self.config
    }

    pub async fn generate_text(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        tools: Option<&[ToolDef]>,
    ) -> Result<AIResponse, String> {
        let messages = Self::build_messages(system_prompt, prompt);
        self.call_client(&messages, tools).await
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

    async fn call_client(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDef]>,
    ) -> Result<AIResponse, String> {
        match self
            .call_client_with_model(messages, tools, &self.config.model)
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
                            .call_client_with_model(messages, tools, &fallback_model)
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

    fn call_client_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDef>>,
    ) -> ReceiverStream<Result<AIStreamChunk, String>> {
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
                )
            }
            _ => {
                let client = OpenAIClient::new(&self.config.api_key, &self.config.base_url);
                client.chat_completion_stream(
                    messages.clone(),
                    self.config.model.clone(),
                    self.config.temperature,
                    self.config.max_tokens,
                    tools.clone(),
                )
            }
        };

        let provider = self.config.provider.clone();
        let api_key = self.config.api_key.clone();
        let base_url = self.config.base_url.clone();
        let current_model = self.config.model.clone();
        let max_tokens = self.config.max_tokens;
        let temperature = self.config.temperature;
        let system_prompt = self.config.system_prompt.clone();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<AIStreamChunk, String>>(32);

        tokio::spawn(async move {
            let mut primary_stream = primary_stream;
            let Some(first_item) = primary_stream.next().await else {
                return;
            };

            match first_item {
                Ok(chunk) => {
                    if tx.send(Ok(chunk)).await.is_err() {
                        return;
                    }
                    while let Some(item) = primary_stream.next().await {
                        if tx.send(item).await.is_err() {
                            return;
                        }
                    }
                }
                Err(error) => {
                    if !AIService::should_retry_with_fallback_model(&error) {
                        let _ = tx.send(Err(error)).await;
                        return;
                    }

                    let Some(fallback_model) = AIService::resolve_fallback_model(
                        &provider,
                        &api_key,
                        &base_url,
                        &current_model,
                    )
                    .await
                    else {
                        let _ = tx.send(Err(error)).await;
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
                            )
                        }
                        _ => {
                            let client = OpenAIClient::new(&api_key, &base_url);
                            client.chat_completion_stream(
                                messages.clone(),
                                fallback_model.clone(),
                                temperature,
                                max_tokens,
                                tools.clone(),
                            )
                        }
                    };

                    tokio::pin!(fallback_stream);
                    while let Some(fallback_item) = fallback_stream.next().await {
                        match fallback_item {
                            Ok(chunk) => {
                                if tx.send(Ok(chunk)).await.is_err() {
                                    return;
                                }
                            }
                            Err(fallback_error) => {
                                let _ = tx
                                    .send(Err(format!(
                                        "{}; fallback model {} also failed: {}",
                                        error, fallback_model, fallback_error
                                    )))
                                    .await;
                                return;
                            }
                        }
                    }
                }
            }
        });

        ReceiverStream::new(rx)
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
