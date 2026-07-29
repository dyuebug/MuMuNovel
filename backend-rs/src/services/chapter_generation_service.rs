use std::fmt;

use sea_orm::DatabaseConnection;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::services::{
    chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig,
    chapter_generation_execution_contract_service::{
        build_prompt_overrides_from_compat_options, PreparedGenerationExecutionConfig,
        SingleChapterGenerationCompatOptions,
    },
    chapter_generation_runtime_service::runtime_execution_owner::load_generation_context,
    cooperative_cancellation_service::CooperativeCancellationToken,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterGeneratedDraft {
    pub chapter_id: String,
    pub chapter_number: i32,
    pub title: String,
    pub content: String,
    pub word_count: i32,
    pub chapter_status: String,
    pub quality_metrics: Option<Value>,
    pub quality_gate_action: Option<String>,
    pub quality_gate_message: Option<String>,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChapterGenerationError {
    Cancelled,
    InvalidInput(&'static str),
    Context(String),
    Generation(String),
    InvalidResult(&'static str),
}

impl ChapterGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::InvalidInput(_) => "invalid_input",
            Self::Context(_) => "context_error",
            Self::Generation(_) => "generation_error",
            Self::InvalidResult(_) => "invalid_result",
        }
    }
}

impl fmt::Display for ChapterGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("chapter generation was cancelled"),
            Self::InvalidInput(field) => {
                write!(formatter, "invalid chapter generation input: {field}")
            }
            Self::Context(_) => formatter.write_str("failed to load chapter generation context"),
            Self::Generation(_) => formatter.write_str("chapter candidate generation failed"),
            Self::InvalidResult(field) => {
                write!(formatter, "invalid generated chapter result: {field}")
            }
        }
    }
}

impl std::error::Error for ChapterGenerationError {}

pub(crate) async fn generate_chapter_candidate_for_autopilot(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    compat_options: &SingleChapterGenerationCompatOptions,
    execution_config: PreparedGenerationExecutionConfig,
    additional_guidance: Option<&str>,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<ChapterGeneratedDraft, ChapterGenerationError> {
    ensure_not_cancelled(cancellation_token)?;
    if user_id.trim().is_empty() {
        return Err(ChapterGenerationError::InvalidInput("user_id"));
    }
    if chapter_id.trim().is_empty() {
        return Err(ChapterGenerationError::InvalidInput("chapter_id"));
    }
    if target_word_count <= 0 {
        return Err(ChapterGenerationError::InvalidInput("target_word_count"));
    }

    let context = load_generation_context(db, user_id, chapter_id)
        .await
        .map_err(|error| ChapterGenerationError::Context(error.into_runtime_message()))?;
    ensure_not_cancelled(cancellation_token)?;

    let overrides = build_prompt_overrides_from_compat_options(compat_options);
    let PreparedGenerationExecutionConfig {
        ai_config,
        provider_payload,
        role_policy_context,
    } = execution_config;
    let generated = context
        .generate_candidate_only_with_guidance(
            ai_config,
            target_word_count,
            provider_payload,
            &overrides,
            additional_guidance,
            gateway_config,
            role_policy_context,
        )
        .await
        .map_err(ChapterGenerationError::Generation)?;
    ensure_not_cancelled(cancellation_token)?;

    if generated.chapter_id != context.chapter_model.id {
        return Err(ChapterGenerationError::InvalidResult("chapter_id"));
    }
    if generated.chapter_number != context.chapter_model.chapter_number {
        return Err(ChapterGenerationError::InvalidResult("chapter_number"));
    }
    if generated.content.trim().is_empty() {
        return Err(ChapterGenerationError::InvalidResult("content"));
    }
    if generated.word_count <= 0 {
        return Err(ChapterGenerationError::InvalidResult("word_count"));
    }

    let content_digest = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(generated.content.as_bytes()))
    );
    Ok(ChapterGeneratedDraft {
        chapter_id: generated.chapter_id,
        chapter_number: generated.chapter_number,
        title: generated.title,
        content: generated.content,
        word_count: generated.word_count,
        chapter_status: generated.chapter_status,
        quality_metrics: generated.quality_metrics,
        quality_gate_action: generated.quality_gate_action,
        quality_gate_message: generated.quality_gate_message,
        content_digest,
    })
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), ChapterGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        return Err(ChapterGenerationError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_not_cancelled, ChapterGenerationError};

    #[test]
    fn cancellation_guard_allows_missing_token() {
        assert_eq!(ensure_not_cancelled(None), Ok(()));
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(ChapterGenerationError::Cancelled.code(), "cancelled");
        assert_eq!(
            ChapterGenerationError::InvalidResult("content").code(),
            "invalid_result"
        );
    }
}
