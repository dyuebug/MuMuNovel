use serde_json::Value;

use super::{
    validate_business_checkpoint, BusinessCheckpointError, BusinessCheckpointV1,
    BUSINESS_CHECKPOINT_SCHEMA_VERSION,
};

pub(crate) const BUSINESS_CHECKPOINT_RUNTIME_FIELD: &str = "business_checkpoint";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BusinessCheckpointRead {
    Missing,
    Valid(BusinessCheckpointV1),
    UnsupportedSchema { schema_version: String },
    Invalid,
}

pub(crate) fn read_business_checkpoint_runtime_state(
    workflow_runtime_state: &Value,
) -> BusinessCheckpointRead {
    let Some(value) = workflow_runtime_state.get(BUSINESS_CHECKPOINT_RUNTIME_FIELD) else {
        return BusinessCheckpointRead::Missing;
    };
    let Some(object) = value.as_object() else {
        return BusinessCheckpointRead::Invalid;
    };
    let Some(schema_version) = object.get("schema_version").and_then(Value::as_str) else {
        return BusinessCheckpointRead::Invalid;
    };
    if schema_version != BUSINESS_CHECKPOINT_SCHEMA_VERSION {
        return BusinessCheckpointRead::UnsupportedSchema {
            schema_version: schema_version.to_owned(),
        };
    }

    let Ok(checkpoint) = serde_json::from_value::<BusinessCheckpointV1>(value.clone()) else {
        return BusinessCheckpointRead::Invalid;
    };
    if validate_business_checkpoint(&checkpoint).is_err() {
        return BusinessCheckpointRead::Invalid;
    }
    BusinessCheckpointRead::Valid(checkpoint)
}

pub(crate) fn merge_business_checkpoint_runtime_state(
    workflow_runtime_state: &mut Value,
    checkpoint: &BusinessCheckpointV1,
) -> Result<(), BusinessCheckpointError> {
    validate_business_checkpoint(checkpoint)?;
    let object = workflow_runtime_state
        .as_object_mut()
        .ok_or(BusinessCheckpointError::InvalidRuntimeState)?;
    object.insert(
        BUSINESS_CHECKPOINT_RUNTIME_FIELD.to_owned(),
        serde_json::to_value(checkpoint)
            .map_err(|error| BusinessCheckpointError::Serialization(error.to_string()))?,
    );
    Ok(())
}
