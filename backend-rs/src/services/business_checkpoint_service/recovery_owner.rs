use super::{
    compute_business_checkpoint_idempotency_key, validate_business_checkpoint,
    BusinessCheckpointError, BusinessCheckpointV1,
};

pub(crate) fn validate_business_checkpoint_idempotency_key(
    batch_task_id: &str,
    checkpoint: &BusinessCheckpointV1,
) -> Result<(), BusinessCheckpointError> {
    validate_business_checkpoint(checkpoint)?;
    let expected = compute_business_checkpoint_idempotency_key(
        batch_task_id,
        checkpoint.boundary,
        checkpoint.revision,
        &checkpoint.input_digest,
        &checkpoint.output_reference,
    )?;

    if checkpoint.idempotency_key != expected {
        return Err(BusinessCheckpointError::IdempotencyKeyMismatch {
            expected,
            actual: checkpoint.idempotency_key.clone(),
        });
    }

    Ok(())
}
