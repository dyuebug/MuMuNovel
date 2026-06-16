use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolChoiceFunction {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Function(ToolChoiceFunction),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AIResponse {
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: Option<String>,
    pub transport_diagnostics: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AIStreamChunk {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub done: bool,
    pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AIRequestError {
    pub message: String,
    pub transport_diagnostics: Option<Value>,
    pub status_code: Option<u16>,
}

impl AIRequestError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            transport_diagnostics: None,
            status_code: None,
        }
    }

    pub fn with_transport_status(
        message: impl Into<String>,
        transport_diagnostics: Value,
        status_code: Option<u16>,
    ) -> Self {
        Self {
            message: message.into(),
            transport_diagnostics: Some(transport_diagnostics),
            status_code,
        }
    }
}

impl fmt::Display for AIRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AIRequestError {}
