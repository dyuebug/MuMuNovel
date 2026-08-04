use serde::Serialize;

use crate::ai::AIConfig;

pub(crate) const NOVEL_AUTOPILOT_FAILURE_DIAGNOSTIC_SCHEMA_VERSION: &str =
    "novel-autopilot-failure-diagnostic/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NovelAutopilotFailureDomain {
    ChapterAnalysis,
    ChapterRepair,
}

impl NovelAutopilotFailureDomain {
    const fn prefix(self) -> &'static str {
        match self {
            Self::ChapterAnalysis => "chapter_analysis",
            Self::ChapterRepair => "chapter_repair",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NovelAutopilotFailureCategory {
    Timeout,
    RateLimited,
    UpstreamUnavailable,
    AuthenticationOrConfiguration,
    ResponseInvalid,
    ContextInvalid,
    Unknown,
}

impl NovelAutopilotFailureCategory {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::RateLimited => "rate_limited",
            Self::UpstreamUnavailable => "upstream_unavailable",
            Self::AuthenticationOrConfiguration => "authentication_or_configuration",
            Self::ResponseInvalid => "response_invalid",
            Self::ContextInvalid => "context_invalid",
            Self::Unknown => "unknown",
        }
    }

    const fn retryable(self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::RateLimited | Self::UpstreamUnavailable | Self::Unknown
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotProviderFailureHint {
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) http_status: Option<u16>,
}

impl NovelAutopilotProviderFailureHint {
    pub(crate) fn from_ai_config(ai_config: &AIConfig) -> Self {
        Self {
            provider: sanitize_identifier(&ai_config.provider),
            model: sanitize_identifier(&ai_config.model),
            http_status: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct NovelAutopilotFailureDiagnostic {
    pub(crate) schema_version: &'static str,
    pub(crate) source_code: &'static str,
    pub(crate) category: NovelAutopilotFailureCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) http_status: Option<u16>,
    pub(crate) retryable: bool,
}

impl NovelAutopilotFailureDiagnostic {
    pub(crate) fn provider_failure(
        source_code: &'static str,
        hint: Option<NovelAutopilotProviderFailureHint>,
        message: Option<&str>,
    ) -> Self {
        let http_status = hint
            .as_ref()
            .and_then(|hint| hint.http_status)
            .or_else(|| message.and_then(extract_http_status_code));
        let category = classify_provider_failure(message, http_status);
        Self::new(source_code, category, hint, http_status)
    }

    pub(crate) fn response_invalid(source_code: &'static str) -> Self {
        Self::response_invalid_with_hint(source_code, None)
    }

    pub(crate) fn response_invalid_with_hint(
        source_code: &'static str,
        hint: Option<NovelAutopilotProviderFailureHint>,
    ) -> Self {
        Self::new(
            source_code,
            NovelAutopilotFailureCategory::ResponseInvalid,
            hint,
            None,
        )
    }

    pub(crate) fn context_invalid(source_code: &'static str) -> Self {
        Self::new(
            source_code,
            NovelAutopilotFailureCategory::ContextInvalid,
            None,
            None,
        )
    }

    pub(crate) fn configuration_failure(source_code: &'static str, message: Option<&str>) -> Self {
        Self::configuration_failure_with_hint(source_code, None, message)
    }

    pub(crate) fn configuration_failure_with_hint(
        source_code: &'static str,
        hint: Option<NovelAutopilotProviderFailureHint>,
        message: Option<&str>,
    ) -> Self {
        let http_status = message.and_then(extract_http_status_code);
        Self::new(
            source_code,
            NovelAutopilotFailureCategory::AuthenticationOrConfiguration,
            hint,
            http_status,
        )
    }

    pub(crate) fn reason_code(&self, domain: NovelAutopilotFailureDomain) -> String {
        match self.category {
            NovelAutopilotFailureCategory::Timeout
            | NovelAutopilotFailureCategory::RateLimited
            | NovelAutopilotFailureCategory::UpstreamUnavailable
            | NovelAutopilotFailureCategory::AuthenticationOrConfiguration => {
                format!("{}_provider_{}", domain.prefix(), self.category.as_str())
            }
            NovelAutopilotFailureCategory::ResponseInvalid => {
                format!("{}_result_invalid", domain.prefix())
            }
            NovelAutopilotFailureCategory::ContextInvalid => {
                format!("{}_context_invalid", domain.prefix())
            }
            NovelAutopilotFailureCategory::Unknown => {
                format!("{}_provider_failed", domain.prefix())
            }
        }
    }

    pub(crate) fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    pub(crate) const fn counts_as_provider_failure(&self) -> bool {
        !matches!(
            self.category,
            NovelAutopilotFailureCategory::ResponseInvalid
                | NovelAutopilotFailureCategory::ContextInvalid
        )
    }

    fn new(
        source_code: &'static str,
        category: NovelAutopilotFailureCategory,
        hint: Option<NovelAutopilotProviderFailureHint>,
        http_status: Option<u16>,
    ) -> Self {
        Self {
            schema_version: NOVEL_AUTOPILOT_FAILURE_DIAGNOSTIC_SCHEMA_VERSION,
            source_code,
            category,
            provider: hint
                .as_ref()
                .and_then(|hint| hint.provider.as_deref())
                .and_then(sanitize_identifier),
            model: hint
                .as_ref()
                .and_then(|hint| hint.model.as_deref())
                .and_then(sanitize_identifier),
            http_status: http_status.filter(|status| (400..=599).contains(status)),
            retryable: category.retryable(),
        }
    }
}

fn classify_provider_failure(
    message: Option<&str>,
    http_status: Option<u16>,
) -> NovelAutopilotFailureCategory {
    match http_status {
        Some(401 | 403) => {
            return NovelAutopilotFailureCategory::AuthenticationOrConfiguration;
        }
        Some(429) => return NovelAutopilotFailureCategory::RateLimited,
        Some(500..=599) => return NovelAutopilotFailureCategory::UpstreamUnavailable,
        Some(408) => return NovelAutopilotFailureCategory::Timeout,
        _ => {}
    }
    let normalized = message.unwrap_or_default().to_ascii_lowercase();
    if normalized.contains("timeout")
        || normalized.contains("timed out")
        || normalized.contains("deadline")
        || normalized.contains("超时")
    {
        return NovelAutopilotFailureCategory::Timeout;
    }
    if normalized.contains("rate limit")
        || normalized.contains("too many requests")
        || normalized.contains("限流")
    {
        return NovelAutopilotFailureCategory::RateLimited;
    }
    if normalized.contains("unauthorized")
        || normalized.contains("forbidden")
        || normalized.contains("invalid api key")
        || normalized.contains("api key")
        || normalized.contains("鉴权")
    {
        return NovelAutopilotFailureCategory::AuthenticationOrConfiguration;
    }
    if normalized.contains("service unavailable")
        || normalized.contains("bad gateway")
        || normalized.contains("gateway error")
        || normalized.contains("上游")
    {
        return NovelAutopilotFailureCategory::UpstreamUnavailable;
    }
    NovelAutopilotFailureCategory::Unknown
}

fn extract_http_status_code(message: &str) -> Option<u16> {
    let normalized = message.to_ascii_lowercase();
    for marker in ["status_code", "status code", "status", "http", "状态码"] {
        let mut offset = 0;
        while let Some(relative_index) = normalized[offset..].find(marker) {
            let marker_end = offset + relative_index + marker.len();
            let suffix = &normalized[marker_end..];
            let digit_start = suffix
                .char_indices()
                .take_while(|(index, _)| *index <= 12)
                .find_map(|(index, character)| character.is_ascii_digit().then_some(index));
            if let Some(digit_start) = digit_start {
                let digits = suffix[digit_start..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>();
                if digits.len() == 3 {
                    if let Ok(status) = digits.parse::<u16>() {
                        if (400..=599).contains(&status) {
                            return Some(status);
                        }
                    }
                }
            }
            offset = marker_end;
        }
    }
    None
}

fn sanitize_identifier(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.contains("api_key")
        || normalized.contains("apikey")
        || normalized.contains("authorization")
        || normalized.contains("bearer")
        || normalized.contains("secret")
        || normalized.contains("token")
        || normalized.contains("http://")
        || normalized.contains("https://")
        || normalized.contains('?')
        || normalized.contains('&')
        || normalized.contains('=')
    {
        return None;
    }
    let sanitized = value
        .trim()
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(*character, '-' | '_' | '.' | '/')
        })
        .take(80)
        .collect::<String>();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::ai::AIConfig;

    use super::{
        NovelAutopilotFailureDiagnostic, NovelAutopilotFailureDomain,
        NovelAutopilotProviderFailureHint,
    };

    #[test]
    fn provider_failure_maps_http_status_to_stable_reason_code() {
        let diagnostic = NovelAutopilotFailureDiagnostic::provider_failure(
            "generation_error",
            Some(NovelAutopilotProviderFailureHint {
                http_status: Some(429),
                ..NovelAutopilotProviderFailureHint::from_ai_config(&AIConfig {
                    provider: "openai".to_string(),
                    model: "gpt-5.1".to_string(),
                    ..Default::default()
                })
            }),
            Some("HTTP 429 Too Many Requests"),
        );

        assert_eq!(
            diagnostic.reason_code(NovelAutopilotFailureDomain::ChapterRepair),
            "chapter_repair_provider_rate_limited"
        );
        assert_eq!(diagnostic.to_value()["http_status"], json!(429));
        assert_eq!(diagnostic.to_value()["provider"], json!("openai"));
        assert_eq!(diagnostic.to_value()["model"], json!("gpt-5.1"));
    }

    #[test]
    fn response_and_context_failures_use_non_provider_reason_codes() {
        assert_eq!(
            NovelAutopilotFailureDiagnostic::response_invalid("invalid_result")
                .reason_code(NovelAutopilotFailureDomain::ChapterAnalysis),
            "chapter_analysis_result_invalid"
        );
        assert_eq!(
            NovelAutopilotFailureDiagnostic::context_invalid("context_error")
                .reason_code(NovelAutopilotFailureDomain::ChapterRepair),
            "chapter_repair_context_invalid"
        );
    }

    #[test]
    fn timeout_and_authentication_failures_have_stable_retry_semantics() {
        let timeout = NovelAutopilotFailureDiagnostic::provider_failure(
            "generation_error",
            None,
            Some("request timed out while waiting for upstream"),
        );
        let authentication = NovelAutopilotFailureDiagnostic::provider_failure(
            "generation_error",
            Some(NovelAutopilotProviderFailureHint {
                provider: Some("openai".to_string()),
                model: Some("gpt-5.1".to_string()),
                http_status: Some(401),
            }),
            None,
        );

        assert_eq!(
            timeout.reason_code(NovelAutopilotFailureDomain::ChapterAnalysis),
            "chapter_analysis_provider_timeout"
        );
        assert!(timeout.retryable);
        assert_eq!(
            authentication.reason_code(NovelAutopilotFailureDomain::ChapterRepair),
            "chapter_repair_provider_authentication_or_configuration"
        );
        assert!(!authentication.retryable);
    }

    #[test]
    fn unknown_provider_failure_keeps_compatible_aggregate_code() {
        let diagnostic = NovelAutopilotFailureDiagnostic::provider_failure(
            "generation_error",
            None,
            Some("connection closed before a typed status was available"),
        );

        assert_eq!(
            diagnostic.reason_code(NovelAutopilotFailureDomain::ChapterAnalysis),
            "chapter_analysis_provider_failed"
        );
        assert_eq!(diagnostic.category.as_str(), "unknown");
        assert!(diagnostic.retryable);
        assert_eq!(diagnostic.http_status, None);
    }

    #[test]
    fn diagnostic_does_not_serialize_raw_sensitive_message() {
        let diagnostic = NovelAutopilotFailureDiagnostic::provider_failure(
            "generation_error",
            Some(NovelAutopilotProviderFailureHint {
                provider: Some("openai?api_key=secret".to_string()),
                model: Some("gpt-5;Authorization".to_string()),
                http_status: Some(503),
            }),
            Some("HTTP 503 api_key=secret prompt=完整正文 response=raw body"),
        );
        let serialized = serde_json::to_string(&diagnostic.to_value()).expect("serialize");

        assert_eq!(
            diagnostic.reason_code(NovelAutopilotFailureDomain::ChapterAnalysis),
            "chapter_analysis_provider_upstream_unavailable"
        );
        for forbidden in ["secret", "完整正文", "raw body", "prompt=", "api_key"] {
            assert!(
                !serialized.contains(forbidden),
                "leaked {forbidden}: {serialized}"
            );
        }
    }

    #[test]
    fn typed_http_status_has_priority_over_conflicting_message_keywords() {
        let authentication = NovelAutopilotFailureDiagnostic::provider_failure(
            "generation_error",
            Some(NovelAutopilotProviderFailureHint {
                provider: Some("openai".to_string()),
                model: Some("gpt-5.1".to_string()),
                http_status: Some(401),
            }),
            Some("request timed out after rate limit handling"),
        );
        let upstream = NovelAutopilotFailureDiagnostic::provider_failure(
            "generation_error",
            Some(NovelAutopilotProviderFailureHint {
                provider: Some("openai".to_string()),
                model: Some("gpt-5.1".to_string()),
                http_status: Some(503),
            }),
            Some("invalid api key after timeout"),
        );

        assert_eq!(
            authentication.reason_code(NovelAutopilotFailureDomain::ChapterAnalysis),
            "chapter_analysis_provider_authentication_or_configuration"
        );
        assert_eq!(
            upstream.reason_code(NovelAutopilotFailureDomain::ChapterRepair),
            "chapter_repair_provider_upstream_unavailable"
        );
    }

    #[test]
    fn string_status_fallback_requires_an_http_or_status_boundary() {
        for message in [
            "connect to http://127.0.0.1:5000 failed",
            "request id 503 was closed",
            "model build 429 completed",
        ] {
            let diagnostic = NovelAutopilotFailureDiagnostic::provider_failure(
                "generation_error",
                None,
                Some(message),
            );
            assert_eq!(
                diagnostic.http_status, None,
                "misread status from {message}"
            );
            assert_eq!(diagnostic.category.as_str(), "unknown");
        }

        let diagnostic = NovelAutopilotFailureDiagnostic::provider_failure(
            "generation_error",
            None,
            Some("upstream returned HTTP status 503"),
        );
        assert_eq!(diagnostic.http_status, Some(503));
        assert_eq!(diagnostic.category.as_str(), "upstream_unavailable");
    }

    #[test]
    fn response_invalid_keeps_safe_provider_and_model_hints() {
        let diagnostic = NovelAutopilotFailureDiagnostic::response_invalid_with_hint(
            "invalid_result",
            Some(NovelAutopilotProviderFailureHint {
                provider: Some("openai".to_string()),
                model: Some("gpt-5.1".to_string()),
                http_status: None,
            }),
        );

        assert_eq!(diagnostic.provider.as_deref(), Some("openai"));
        assert_eq!(diagnostic.model.as_deref(), Some("gpt-5.1"));
        assert_eq!(diagnostic.category.as_str(), "response_invalid");
        assert!(!diagnostic.retryable);
    }
}
