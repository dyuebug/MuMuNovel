use tokio_stream::wrappers::ReceiverStream;

use crate::ai::clients::anthropic::AnthropicClient;
use crate::ai::clients::openai::OpenAIClient;
use crate::ai::config::AIConfig;
use crate::ai::types::{AIResponse, AIStreamChunk, ChatMessage, ToolDef};

pub struct AIService {
    config: AIConfig,
}

impl AIService {
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
        match self.config.provider.as_str() {
            "anthropic" => {
                let client = AnthropicClient::new(&self.config.api_key, &self.config.base_url);
                client
                    .chat_completion(
                        messages,
                        &self.config.model,
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
                        &self.config.model,
                        self.config.temperature,
                        self.config.max_tokens,
                        tools,
                    )
                    .await
            }
        }
    }

    fn call_client_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDef>>,
    ) -> ReceiverStream<Result<AIStreamChunk, String>> {
        match self.config.provider.as_str() {
            "anthropic" => {
                let client = AnthropicClient::new(&self.config.api_key, &self.config.base_url);
                client.chat_completion_stream(
                    messages,
                    self.config.model.clone(),
                    self.config.temperature,
                    self.config.max_tokens,
                    self.config.system_prompt.clone(),
                    tools,
                )
            }
            _ => {
                let client = OpenAIClient::new(&self.config.api_key, &self.config.base_url);
                client.chat_completion_stream(
                    messages,
                    self.config.model.clone(),
                    self.config.temperature,
                    self.config.max_tokens,
                    tools,
                )
            }
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
