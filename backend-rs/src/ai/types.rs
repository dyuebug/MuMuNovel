use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

pub(crate) const AI_REQUEST_RETRY_AFTER_MAX_SECONDS: u64 = 15 * 60;

pub(crate) fn retry_after_seconds_from_headers(
    headers: &HeaderMap,
    now: DateTime<Utc>,
) -> Option<u64> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        let seconds = value.bytes().fold(0_u64, |seconds, byte| {
            seconds
                .saturating_mul(10)
                .saturating_add(u64::from(byte - b'0'))
                .min(AI_REQUEST_RETRY_AFTER_MAX_SECONDS)
        });
        return Some(seconds);
    }

    let not_before = DateTime::parse_from_rfc2822(value)
        .ok()?
        .with_timezone(&Utc);
    let seconds = not_before.signed_duration_since(now).num_seconds();
    (seconds > 0).then(|| (seconds as u64).min(AI_REQUEST_RETRY_AFTER_MAX_SECONDS))
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: Option<String>,
    pub transport_diagnostics: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AIStreamChunk {
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub done: bool,
    pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AIRequestError {
    pub message: String,
    pub transport_diagnostics: Option<Value>,
    pub status_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

impl AIRequestError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            transport_diagnostics: None,
            status_code: None,
            retry_after_seconds: None,
        }
    }

    pub fn with_transport_status(
        message: impl Into<String>,
        transport_diagnostics: Value,
        status_code: Option<u16>,
    ) -> Self {
        Self::with_transport_status_and_retry_after(
            message,
            transport_diagnostics,
            status_code,
            None,
        )
    }

    pub fn with_transport_status_and_retry_after(
        message: impl Into<String>,
        transport_diagnostics: Value,
        status_code: Option<u16>,
        retry_after_seconds: Option<u64>,
    ) -> Self {
        Self {
            message: message.into(),
            transport_diagnostics: Some(transport_diagnostics),
            status_code,
            retry_after_seconds,
        }
    }
}

impl fmt::Display for AIRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AIRequestError {}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};

    use super::{retry_after_seconds_from_headers, AI_REQUEST_RETRY_AFTER_MAX_SECONDS};

    fn retry_after(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            RETRY_AFTER,
            HeaderValue::from_str(value).expect("valid test header"),
        );
        headers
    }

    #[test]
    fn retry_after_parses_delta_seconds_and_caps_large_values() {
        let now = Utc.with_ymd_and_hms(2026, 8, 7, 12, 0, 0).unwrap();

        assert_eq!(
            retry_after_seconds_from_headers(&retry_after("120"), now),
            Some(120)
        );
        assert_eq!(
            retry_after_seconds_from_headers(&retry_after("0"), now),
            Some(0)
        );
        assert_eq!(
            retry_after_seconds_from_headers(&retry_after("999999999999999999999999"), now),
            Some(AI_REQUEST_RETRY_AFTER_MAX_SECONDS)
        );
    }

    #[test]
    fn retry_after_parses_http_date_relative_to_fixed_now() {
        let now = Utc.with_ymd_and_hms(2026, 8, 7, 12, 0, 0).unwrap();

        assert_eq!(
            retry_after_seconds_from_headers(&retry_after("Fri, 07 Aug 2026 12:02:00 GMT"), now,),
            Some(120)
        );
    }

    #[test]
    fn retry_after_rejects_invalid_or_elapsed_http_dates() {
        let now = Utc.with_ymd_and_hms(2026, 8, 7, 12, 0, 0).unwrap();

        assert_eq!(
            retry_after_seconds_from_headers(&retry_after("invalid"), now),
            None
        );
        assert_eq!(
            retry_after_seconds_from_headers(&retry_after("Fri, 07 Aug 2026 11:59:59 GMT"), now,),
            None
        );
        assert_eq!(
            retry_after_seconds_from_headers(&HeaderMap::new(), now),
            None
        );
    }
}
