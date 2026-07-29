use std::fmt;

use chrono::DateTime;
use serde::{Deserialize, Serialize};

pub const BUSINESS_CHECKPOINT_SCHEMA_VERSION: &str = "business-checkpoint/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BusinessCheckpointBoundary {
    ChapterDraftSaved,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum BusinessCheckpointOutputReferenceV1 {
    Chapter { id: String },
}

impl BusinessCheckpointOutputReferenceV1 {
    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Chapter { id } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BusinessCheckpointV1 {
    pub(crate) schema_version: String,
    pub(crate) boundary: BusinessCheckpointBoundary,
    pub(crate) revision: u64,
    pub(crate) idempotency_key: String,
    pub(crate) input_digest: String,
    pub(crate) output_reference: BusinessCheckpointOutputReferenceV1,
    pub(crate) recorded_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BusinessCheckpointError {
    UnsupportedSchema(String),
    InvalidRevision,
    InvalidTaskId,
    InvalidInputDigest,
    InvalidIdempotencyKey,
    InvalidOutputReference,
    InvalidRecordedAt,
    Serialization(String),
    InvalidRuntimeState,
    IdempotencyKeyMismatch { expected: String, actual: String },
}

impl fmt::Display for BusinessCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => {
                write!(
                    formatter,
                    "unsupported business checkpoint schema: {schema}"
                )
            }
            Self::InvalidRevision => {
                formatter.write_str("business checkpoint revision must be positive")
            }
            Self::InvalidTaskId => {
                formatter.write_str("business checkpoint task id must not be empty")
            }
            Self::InvalidInputDigest => formatter
                .write_str("business checkpoint input digest must be a sha256-prefixed digest"),
            Self::InvalidIdempotencyKey => formatter
                .write_str("business checkpoint idempotency key must be a sha256-prefixed digest"),
            Self::InvalidOutputReference => {
                formatter.write_str("business checkpoint output reference must not be empty")
            }
            Self::InvalidRecordedAt => formatter
                .write_str("business checkpoint recorded_at must be a valid RFC3339 timestamp"),
            Self::Serialization(message) => {
                write!(
                    formatter,
                    "business checkpoint serialization failed: {message}"
                )
            }
            Self::InvalidRuntimeState => {
                formatter.write_str("business checkpoint runtime state must be an object")
            }
            Self::IdempotencyKeyMismatch { expected, actual } => write!(
                formatter,
                "business checkpoint idempotency key mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for BusinessCheckpointError {}

pub(crate) fn validate_business_checkpoint(
    checkpoint: &BusinessCheckpointV1,
) -> Result<(), BusinessCheckpointError> {
    if checkpoint.schema_version != BUSINESS_CHECKPOINT_SCHEMA_VERSION {
        return Err(BusinessCheckpointError::UnsupportedSchema(
            checkpoint.schema_version.clone(),
        ));
    }
    if checkpoint.revision == 0 {
        return Err(BusinessCheckpointError::InvalidRevision);
    }
    if !is_sha256_digest(&checkpoint.input_digest) {
        return Err(BusinessCheckpointError::InvalidInputDigest);
    }
    if !is_sha256_digest(&checkpoint.idempotency_key) {
        return Err(BusinessCheckpointError::InvalidIdempotencyKey);
    }
    if checkpoint.output_reference.id().trim().is_empty() {
        return Err(BusinessCheckpointError::InvalidOutputReference);
    }
    if DateTime::parse_from_rfc3339(&checkpoint.recorded_at).is_err() {
        return Err(BusinessCheckpointError::InvalidRecordedAt);
    }
    Ok(())
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex_digest) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex_digest.len() == 64
        && hex_digest
            .bytes()
            .all(|character| character.is_ascii_hexdigit())
}
