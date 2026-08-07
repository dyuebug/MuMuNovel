use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::types::{AIRequestError, AIResponse, AIStreamChunk};

pub const AI_EXECUTION_TRACE_SCHEMA_VERSION: &str = "ai-execution-trace/v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AIExecutionOutcome {
    Succeeded,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AIExecutionFallbackKind {
    None,
    ModelFallback,
    EndpointFailover,
    CandidateExecutorFallback,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AIExecutionFallbackSummaryV1 {
    pub kind: AIExecutionFallbackKind,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EndpointExecutionSummaryV1 {
    pub endpoint_role: String,
    pub endpoint_index: usize,
    pub total_attempts: usize,
    pub failover_count: usize,
    pub backup_endpoint_used: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AIExecutionTraceV1 {
    pub schema_version: String,
    pub requested_provider: String,
    pub requested_model: String,
    pub actual_provider: String,
    pub actual_model: String,
    pub outcome: AIExecutionOutcome,
    pub fallbacks: Vec<AIExecutionFallbackSummaryV1>,
    pub endpoint_summary: Option<EndpointExecutionSummaryV1>,
}

#[derive(Debug)]
pub struct TrackedAIResponse {
    pub response: AIResponse,
    pub execution: AIExecutionTraceV1,
}

#[derive(Debug)]
pub struct TrackedAIRequestError {
    pub error: AIRequestError,
    pub execution: AIExecutionTraceV1,
}

pub struct TrackedAIStream {
    pub stream: ReceiverStream<Result<AIStreamChunk, AIRequestError>>,
    pub completion: oneshot::Receiver<AIExecutionTraceV1>,
}

pub(crate) fn build_ai_execution_trace(
    provider: &str,
    requested_model: &str,
    actual_model: &str,
    outcome: AIExecutionOutcome,
    model_fallback_reason: Option<&str>,
    transport_diagnostics: Option<&Value>,
) -> AIExecutionTraceV1 {
    let endpoint_summary = transport_diagnostics.and_then(extract_endpoint_execution_summary);
    let mut fallbacks = Vec::new();

    if let Some(reason) = model_fallback_reason {
        fallbacks.push(AIExecutionFallbackSummaryV1 {
            kind: AIExecutionFallbackKind::ModelFallback,
            reason: reason.to_string(),
        });
    }

    if endpoint_summary
        .as_ref()
        .map(|summary| summary.backup_endpoint_used || summary.failover_count > 0)
        .unwrap_or(false)
    {
        fallbacks.push(AIExecutionFallbackSummaryV1 {
            kind: AIExecutionFallbackKind::EndpointFailover,
            reason: "primary_endpoint_failed".to_string(),
        });
    }

    AIExecutionTraceV1 {
        schema_version: AI_EXECUTION_TRACE_SCHEMA_VERSION.to_string(),
        requested_provider: provider.to_string(),
        requested_model: requested_model.to_string(),
        actual_provider: provider.to_string(),
        actual_model: actual_model.to_string(),
        outcome,
        fallbacks,
        endpoint_summary,
    }
}

pub fn extract_endpoint_execution_summary(
    transport_diagnostics: &Value,
) -> Option<EndpointExecutionSummaryV1> {
    let attempts = transport_diagnostics.get("attempts")?.as_array()?;
    let final_attempt = attempts.last()?;
    let endpoint_role = final_attempt
        .get("endpoint_role")
        .and_then(Value::as_str)
        .filter(|role| matches!(*role, "primary" | "backup"))?;
    let endpoint_index = final_attempt
        .get("endpoint_index")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())?;
    let summary = transport_diagnostics.get("summary");
    let total_attempts = summary
        .and_then(|value| value.get("total_attempts"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(attempts.len());
    let failover_count = summary
        .and_then(|value| value.get("failover_count"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| {
            attempts
                .iter()
                .filter(|attempt| {
                    attempt
                        .get("will_failover")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .count()
        });
    let backup_endpoint_used = summary
        .and_then(|value| value.get("backup_endpoint_used"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            attempts.iter().any(|attempt| {
                attempt
                    .get("endpoint_role")
                    .and_then(Value::as_str)
                    .map(|role| role == "backup")
                    .unwrap_or(false)
            })
        });

    Some(EndpointExecutionSummaryV1 {
        endpoint_role: endpoint_role.to_string(),
        endpoint_index,
        total_attempts,
        failover_count,
        backup_endpoint_used,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_ai_execution_trace, extract_endpoint_execution_summary, AIExecutionFallbackKind,
        AIExecutionOutcome, AI_EXECUTION_TRACE_SCHEMA_VERSION,
    };

    #[test]
    fn endpoint_summary_only_keeps_allowlisted_transport_fields() {
        let diagnostics = json!({
            "api_key": "secret-api-key",
            "authorization": "Bearer secret-token",
            "prompt": "secret prompt",
            "content": "secret response body",
            "events": [{
                "effective_base_url": "https://secret.example/v1",
                "error_message": "secret raw error"
            }],
            "attempts": [
                {
                    "endpoint_role": "primary",
                    "endpoint_index": 1,
                    "base_url": "https://primary.secret.example/v1",
                    "will_failover": true
                },
                {
                    "endpoint_role": "backup",
                    "endpoint_index": 2,
                    "base_url": "https://backup.secret.example/v1",
                    "will_failover": false
                }
            ],
            "summary": {
                "total_attempts": 2,
                "effective_base_url": "https://backup.secret.example/v1",
                "effective_endpoint": "https://backup.secret.example/v1/chat/completions",
                "backup_endpoint_used": true,
                "failover_count": 1
            }
        });

        let summary = extract_endpoint_execution_summary(&diagnostics).expect("summary");
        assert_eq!(summary.endpoint_role, "backup");
        assert_eq!(summary.endpoint_index, 2);
        assert_eq!(summary.total_attempts, 2);
        assert_eq!(summary.failover_count, 1);
        assert!(summary.backup_endpoint_used);

        let serialized = serde_json::to_string(&summary).expect("serialize summary");
        for secret in [
            "secret-api-key",
            "secret-token",
            "secret prompt",
            "secret response body",
            "secret.example",
            "effective_base_url",
            "effective_endpoint",
            "error_message",
        ] {
            assert!(
                !serialized.contains(secret),
                "leaked {secret}: {serialized}"
            );
        }
    }

    #[test]
    fn trace_serialization_is_typed_and_orders_model_before_endpoint_fallback() {
        let diagnostics = json!({
            "attempts": [
                {"endpoint_role": "primary", "endpoint_index": 1, "will_failover": true},
                {"endpoint_role": "backup", "endpoint_index": 2, "will_failover": false}
            ],
            "summary": {
                "total_attempts": 2,
                "backup_endpoint_used": true,
                "failover_count": 1
            }
        });
        let trace = build_ai_execution_trace(
            "openai",
            "requested-model",
            "fallback-model",
            AIExecutionOutcome::Succeeded,
            Some("model_not_found"),
            Some(&diagnostics),
        );

        assert_eq!(trace.schema_version, AI_EXECUTION_TRACE_SCHEMA_VERSION);
        assert_eq!(trace.fallbacks.len(), 2);
        assert_eq!(
            trace.fallbacks[0].kind,
            AIExecutionFallbackKind::ModelFallback
        );
        assert_eq!(
            trace.fallbacks[1].kind,
            AIExecutionFallbackKind::EndpointFailover
        );
        assert_eq!(trace.fallbacks[0].reason, "model_not_found");
        assert_eq!(trace.fallbacks[1].reason, "primary_endpoint_failed");
    }

    #[test]
    fn missing_transport_diagnostics_does_not_invent_endpoint_failover() {
        let trace = build_ai_execution_trace(
            "anthropic",
            "claude-primary",
            "claude-primary",
            AIExecutionOutcome::Succeeded,
            None,
            None,
        );

        assert!(trace.endpoint_summary.is_none());
        assert!(trace.fallbacks.is_empty());
    }
}
