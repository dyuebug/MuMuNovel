use std::fmt;

use sea_orm::DatabaseConnection;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::services::{
    chapter_analysis_runtime_service::trigger_runtime_owner::{
        generate_chapter_analysis_payload_for_autopilot_typed, ChapterAnalysisAutopilotRuntimeError,
    },
    cooperative_cancellation_service::CooperativeCancellationToken,
    novel_autopilot::{
        failure_diagnostic::{NovelAutopilotFailureDiagnostic, NovelAutopilotProviderFailureHint},
        types::NovelAutopilotQualityDecision,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterAnalysisCandidate {
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) payload: Value,
    pub(crate) result_digest: String,
    pub(crate) overall_score: f64,
    pub(crate) quality_decision: NovelAutopilotQualityDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChapterAnalysisGenerationError {
    Cancelled,
    InvalidInput(&'static str),
    Context(&'static str),
    Configuration {
        message: String,
        provider_hint: Option<NovelAutopilotProviderFailureHint>,
    },
    Provider {
        message: String,
        provider_hint: NovelAutopilotProviderFailureHint,
    },
    InvalidResult {
        field: &'static str,
        provider_hint: Option<NovelAutopilotProviderFailureHint>,
    },
}

impl ChapterAnalysisGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::InvalidInput(_) => "invalid_input",
            Self::Context(_) => "context_error",
            Self::Configuration { .. } => "configuration_error",
            Self::Provider { .. } => "generation_error",
            Self::InvalidResult { .. } => "invalid_result",
        }
    }

    pub(crate) fn failure_diagnostic(&self) -> NovelAutopilotFailureDiagnostic {
        match self {
            Self::Cancelled => NovelAutopilotFailureDiagnostic::context_invalid("cancelled"),
            Self::InvalidInput(_) => {
                NovelAutopilotFailureDiagnostic::context_invalid("invalid_input")
            }
            Self::Context(_) => NovelAutopilotFailureDiagnostic::context_invalid("context_error"),
            Self::Configuration {
                message,
                provider_hint,
            } => NovelAutopilotFailureDiagnostic::configuration_failure_with_hint(
                "configuration_error",
                provider_hint.clone(),
                Some(message),
            ),
            Self::Provider {
                message,
                provider_hint,
            } => NovelAutopilotFailureDiagnostic::provider_failure(
                "generation_error",
                Some(provider_hint.clone()),
                Some(message),
            ),
            Self::InvalidResult { provider_hint, .. } => {
                NovelAutopilotFailureDiagnostic::response_invalid_with_hint(
                    "invalid_result",
                    provider_hint.clone(),
                )
            }
        }
    }
}

impl fmt::Display for ChapterAnalysisGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("chapter analysis was cancelled"),
            Self::InvalidInput(field) => {
                write!(formatter, "invalid chapter analysis input: {field}")
            }
            Self::Context(_) => formatter.write_str("chapter analysis context invalid"),
            Self::Configuration { .. } => {
                formatter.write_str("chapter analysis configuration invalid")
            }
            Self::Provider { .. } => formatter.write_str("chapter analysis generation failed"),
            Self::InvalidResult { field, .. } => {
                write!(formatter, "invalid chapter analysis result: {field}")
            }
        }
    }
}

impl std::error::Error for ChapterAnalysisGenerationError {}

pub(crate) async fn generate_chapter_analysis_candidate_for_autopilot(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<ChapterAnalysisCandidate, ChapterAnalysisGenerationError> {
    ensure_not_cancelled(cancellation_token)?;
    if user_id.trim().is_empty() {
        return Err(ChapterAnalysisGenerationError::InvalidInput("user_id"));
    }
    if chapter_id.trim().is_empty() {
        return Err(ChapterAnalysisGenerationError::InvalidInput("chapter_id"));
    }

    let (chapter, payload, provider_hint) = generate_chapter_analysis_payload_for_autopilot_typed(
        db,
        user_id,
        chapter_id,
        additional_guidance,
        cancellation_token,
    )
    .await
    .map_err(|error| match error {
        ChapterAnalysisAutopilotRuntimeError::Cancelled => {
            ChapterAnalysisGenerationError::Cancelled
        }
        ChapterAnalysisAutopilotRuntimeError::Context(source) => {
            ChapterAnalysisGenerationError::Context(source)
        }
        ChapterAnalysisAutopilotRuntimeError::Configuration {
            message,
            provider_hint,
        } => ChapterAnalysisGenerationError::Configuration {
            message,
            provider_hint,
        },
        ChapterAnalysisAutopilotRuntimeError::Provider {
            message,
            provider_hint,
        } => ChapterAnalysisGenerationError::Provider {
            message,
            provider_hint,
        },
        ChapterAnalysisAutopilotRuntimeError::ResponseInvalid { provider_hint } => {
            ChapterAnalysisGenerationError::InvalidResult {
                field: "payload",
                provider_hint: Some(provider_hint),
            }
        }
    })?;
    ensure_not_cancelled(cancellation_token)?;

    let overall_score = payload
        .get("scores")
        .and_then(|scores| scores.get("overall"))
        .and_then(Value::as_f64)
        .filter(|score| score.is_finite() && (0.0..=10.0).contains(score))
        .ok_or(ChapterAnalysisGenerationError::InvalidResult {
            field: "scores.overall",
            provider_hint: Some(provider_hint.clone()),
        })?;
    let quality_decision = if overall_score >= 8.0 {
        NovelAutopilotQualityDecision::Accept
    } else if overall_score >= 6.0 {
        NovelAutopilotQualityDecision::AutoRepair
    } else {
        NovelAutopilotQualityDecision::ManualReview
    };
    let serialized = serde_json::to_vec(&payload).map_err(|_| {
        ChapterAnalysisGenerationError::InvalidResult {
            field: "payload",
            provider_hint: Some(provider_hint),
        }
    })?;
    let result_digest = format!("sha256:{}", hex::encode(Sha256::digest(serialized)));

    Ok(ChapterAnalysisCandidate {
        chapter_id: chapter.id,
        chapter_number: chapter.chapter_number,
        payload,
        result_digest,
        overall_score,
        quality_decision,
    })
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), ChapterAnalysisGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        return Err(ChapterAnalysisGenerationError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ChapterAnalysisGenerationError;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(
            ChapterAnalysisGenerationError::Cancelled.code(),
            "cancelled"
        );
        assert_eq!(
            ChapterAnalysisGenerationError::InvalidResult {
                field: "scores.overall",
                provider_hint: None,
            }
            .code(),
            "invalid_result"
        );
    }

    #[test]
    fn generation_error_diagnostic_maps_http_status_without_leaking_raw_message() {
        use crate::services::novel_autopilot::failure_diagnostic::{
            NovelAutopilotFailureDomain, NovelAutopilotProviderFailureHint,
        };

        let error = ChapterAnalysisGenerationError::Provider {
            message: "HTTP 503 Service Unavailable api_key=secret prompt=完整正文".to_string(),
            provider_hint: NovelAutopilotProviderFailureHint {
                provider: Some("openai".to_string()),
                model: Some("gpt-5.1".to_string()),
                http_status: None,
            },
        };
        let diagnostic = error.failure_diagnostic();
        let serialized = serde_json::to_string(&diagnostic.to_value()).expect("serialize");

        assert_eq!(
            diagnostic.reason_code(NovelAutopilotFailureDomain::ChapterAnalysis),
            "chapter_analysis_provider_upstream_unavailable"
        );
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("完整正文"));
    }
}
