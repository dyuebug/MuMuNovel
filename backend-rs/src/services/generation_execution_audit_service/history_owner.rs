use serde_json::{Map, Value};

use super::{
    GenerationExecutionAuditError, GenerationExecutionAuditV1,
    GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION,
};

pub const GENERATION_EXECUTION_AUDIT_HISTORY_FIELD: &str = "generation_execution_audit";

pub fn merge_generation_execution_audit(
    history_payload: &mut Value,
    audit: &GenerationExecutionAuditV1,
) -> Result<(), GenerationExecutionAuditError> {
    if audit.schema_version != GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION {
        return Err(GenerationExecutionAuditError::UnsupportedAuditSchema(
            audit.schema_version.clone(),
        ));
    }

    if history_payload.is_null() {
        *history_payload = Value::Object(Map::new());
    }
    let payload = history_payload
        .as_object_mut()
        .ok_or(GenerationExecutionAuditError::InvalidHistoryPayload)?;
    let audit_value = serde_json::to_value(audit)
        .map_err(|error| GenerationExecutionAuditError::Serialization(error.to_string()))?;
    payload.insert(
        GENERATION_EXECUTION_AUDIT_HISTORY_FIELD.to_string(),
        audit_value,
    );
    Ok(())
}

pub fn read_generation_execution_audit(
    history_payload: &Value,
) -> Result<Option<GenerationExecutionAuditV1>, GenerationExecutionAuditError> {
    let Some(audit_value) = history_payload
        .as_object()
        .and_then(|payload| payload.get(GENERATION_EXECUTION_AUDIT_HISTORY_FIELD))
    else {
        return Ok(None);
    };
    let schema = audit_value
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if schema != GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION {
        return Err(GenerationExecutionAuditError::UnsupportedAuditSchema(
            schema.to_string(),
        ));
    }

    serde_json::from_value(audit_value.clone())
        .map(Some)
        .map_err(|error| GenerationExecutionAuditError::Serialization(error.to_string()))
}
