use std::fmt;

use sea_orm::DatabaseConnection;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::services::{
    chapter_analysis_runtime_service::trigger_runtime_owner::generate_chapter_analysis_payload_for_autopilot,
    cooperative_cancellation_service::CooperativeCancellationToken,
    novel_autopilot::types::NovelAutopilotQualityDecision,
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
    Generation(String),
    InvalidResult(&'static str),
}

impl ChapterAnalysisGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::InvalidInput(_) => "invalid_input",
            Self::Generation(_) => "generation_error",
            Self::InvalidResult(_) => "invalid_result",
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
            Self::Generation(_) => formatter.write_str("chapter analysis generation failed"),
            Self::InvalidResult(field) => {
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

    let (chapter, payload) = generate_chapter_analysis_payload_for_autopilot(
        db,
        user_id,
        chapter_id,
        additional_guidance,
        cancellation_token,
    )
    .await
    .map_err(|error| {
        if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
            ChapterAnalysisGenerationError::Cancelled
        } else {
            ChapterAnalysisGenerationError::Generation(error)
        }
    })?;
    ensure_not_cancelled(cancellation_token)?;

    let overall_score = payload
        .get("scores")
        .and_then(|scores| scores.get("overall"))
        .and_then(Value::as_f64)
        .filter(|score| score.is_finite() && (0.0..=10.0).contains(score))
        .ok_or(ChapterAnalysisGenerationError::InvalidResult(
            "scores.overall",
        ))?;
    let quality_decision = if overall_score >= 8.0 {
        NovelAutopilotQualityDecision::Accept
    } else if overall_score >= 6.0 {
        NovelAutopilotQualityDecision::AutoRepair
    } else {
        NovelAutopilotQualityDecision::ManualReview
    };
    let serialized = serde_json::to_vec(&payload)
        .map_err(|_| ChapterAnalysisGenerationError::InvalidResult("payload"))?;
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
            ChapterAnalysisGenerationError::InvalidResult("scores.overall").code(),
            "invalid_result"
        );
    }
}
