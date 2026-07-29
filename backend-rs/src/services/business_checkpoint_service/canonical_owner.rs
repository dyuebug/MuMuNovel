use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::services::generation_contract_service::canonical_json_string;

use super::{
    validate_business_checkpoint, BusinessCheckpointBoundary, BusinessCheckpointError,
    BusinessCheckpointOutputReferenceV1, BusinessCheckpointV1, BUSINESS_CHECKPOINT_SCHEMA_VERSION,
};

#[derive(Serialize)]
struct BusinessCheckpointIdempotencyPreimage<'a> {
    schema_version: &'static str,
    batch_task_id: &'a str,
    boundary: BusinessCheckpointBoundary,
    revision: u64,
    input_digest: &'a str,
    output_reference: &'a BusinessCheckpointOutputReferenceV1,
}

pub(crate) fn build_business_checkpoint(
    batch_task_id: &str,
    boundary: BusinessCheckpointBoundary,
    revision: u64,
    input_digest: &str,
    output_reference: BusinessCheckpointOutputReferenceV1,
    recorded_at: DateTime<Utc>,
) -> Result<BusinessCheckpointV1, BusinessCheckpointError> {
    if batch_task_id.trim().is_empty() {
        return Err(BusinessCheckpointError::InvalidTaskId);
    }

    let idempotency_key = compute_business_checkpoint_idempotency_key(
        batch_task_id,
        boundary,
        revision,
        input_digest,
        &output_reference,
    )?;
    let checkpoint = BusinessCheckpointV1 {
        schema_version: BUSINESS_CHECKPOINT_SCHEMA_VERSION.to_owned(),
        boundary,
        revision,
        idempotency_key,
        input_digest: input_digest.to_owned(),
        output_reference,
        recorded_at: recorded_at.to_rfc3339_opts(SecondsFormat::Millis, true),
    };
    validate_business_checkpoint(&checkpoint)?;
    Ok(checkpoint)
}

pub(crate) fn compute_business_checkpoint_idempotency_key(
    batch_task_id: &str,
    boundary: BusinessCheckpointBoundary,
    revision: u64,
    input_digest: &str,
    output_reference: &BusinessCheckpointOutputReferenceV1,
) -> Result<String, BusinessCheckpointError> {
    if batch_task_id.trim().is_empty() {
        return Err(BusinessCheckpointError::InvalidTaskId);
    }
    if revision == 0 {
        return Err(BusinessCheckpointError::InvalidRevision);
    }

    let value = serde_json::to_value(BusinessCheckpointIdempotencyPreimage {
        schema_version: BUSINESS_CHECKPOINT_SCHEMA_VERSION,
        batch_task_id,
        boundary,
        revision,
        input_digest,
        output_reference,
    })
    .map_err(|error| BusinessCheckpointError::Serialization(error.to_string()))?;
    let canonical = canonical_json_string(value)
        .map_err(|error| BusinessCheckpointError::Serialization(error.to_string()))?;
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(canonical.as_bytes()))
    ))
}
