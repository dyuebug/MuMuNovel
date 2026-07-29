use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::canonical_owner::{validate_generation_contract_snapshot, GenerationContractError};
use super::schema_owner::{
    GenerationContractSnapshotV1, GenerationIntentKind, GenerationTarget, StoryPacketSource,
    GENERATION_CONTRACT_SCHEMA_VERSION,
};

pub const GENERATION_CONTRACT_RUNTIME_NAMESPACE: &str = "story_packet";
pub const GENERATION_CONTRACT_HISTORY_FIELD: &str = "story_packet";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationContractSnapshotRead {
    Missing,
    Legacy,
    UnsupportedVersion(String),
    Malformed(String),
    Valid(GenerationContractSnapshotV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationContractHistorySummaryV1 {
    pub schema_version: String,
    pub input_digest: String,
    pub target: GenerationTarget,
    pub intent_kind: GenerationIntentKind,
    #[serde(default)]
    pub sources: Vec<StoryPacketSource>,
}

pub fn generation_contract_runtime_value(
    snapshot: &GenerationContractSnapshotV1,
) -> Result<Value, GenerationContractError> {
    validate_generation_contract_snapshot(snapshot)?;
    serde_json::to_value(snapshot)
        .map_err(|error| GenerationContractError::Serialization(error.to_string()))
}

pub fn merge_generation_contract_runtime_snapshot(
    workflow_runtime_state: &mut Value,
    snapshot: &GenerationContractSnapshotV1,
) -> Result<(), GenerationContractError> {
    let snapshot_value = generation_contract_runtime_value(snapshot)?;
    if workflow_runtime_state.is_null() {
        *workflow_runtime_state = Value::Object(Map::new());
    }
    let runtime_object = workflow_runtime_state.as_object_mut().ok_or_else(|| {
        GenerationContractError::Serialization(
            "workflow_runtime_state must be a JSON object".to_owned(),
        )
    })?;
    runtime_object.insert(
        GENERATION_CONTRACT_RUNTIME_NAMESPACE.to_owned(),
        snapshot_value,
    );
    Ok(())
}

pub fn read_generation_contract_runtime_snapshot(
    workflow_runtime_state: &Value,
) -> GenerationContractSnapshotRead {
    let Some(snapshot_value) = workflow_runtime_state
        .as_object()
        .and_then(|runtime| runtime.get(GENERATION_CONTRACT_RUNTIME_NAMESPACE))
    else {
        return GenerationContractSnapshotRead::Missing;
    };
    read_generation_contract_snapshot_value(snapshot_value)
}

pub fn read_generation_contract_snapshot_value(
    snapshot_value: &Value,
) -> GenerationContractSnapshotRead {
    let Some(snapshot_object) = snapshot_value.as_object() else {
        return GenerationContractSnapshotRead::Legacy;
    };
    let Some(version) = snapshot_object
        .get("schema_version")
        .and_then(Value::as_str)
    else {
        return GenerationContractSnapshotRead::Legacy;
    };
    if version != GENERATION_CONTRACT_SCHEMA_VERSION {
        return GenerationContractSnapshotRead::UnsupportedVersion(version.to_owned());
    }

    let snapshot =
        match serde_json::from_value::<GenerationContractSnapshotV1>(snapshot_value.clone()) {
            Ok(snapshot) => snapshot,
            Err(error) => return GenerationContractSnapshotRead::Malformed(error.to_string()),
        };
    match validate_generation_contract_snapshot(&snapshot) {
        Ok(()) => GenerationContractSnapshotRead::Valid(snapshot),
        Err(error) => GenerationContractSnapshotRead::Malformed(error.to_string()),
    }
}

pub fn generation_contract_history_summary(
    snapshot: &GenerationContractSnapshotV1,
) -> Result<GenerationContractHistorySummaryV1, GenerationContractError> {
    validate_generation_contract_snapshot(snapshot)?;
    Ok(GenerationContractHistorySummaryV1 {
        schema_version: snapshot.schema_version.clone(),
        input_digest: snapshot.input_digest.clone(),
        target: snapshot.generation_intent.target.clone(),
        intent_kind: snapshot.generation_intent.kind,
        sources: snapshot.story_packet.sources.clone(),
    })
}

pub fn merge_generation_contract_history_summary(
    history_payload: &mut Value,
    snapshot: &GenerationContractSnapshotV1,
) -> Result<(), GenerationContractError> {
    if history_payload.is_null() {
        *history_payload = Value::Object(Map::new());
    }
    let history_object = history_payload.as_object_mut().ok_or_else(|| {
        GenerationContractError::Serialization(
            "generation history payload must be a JSON object".to_owned(),
        )
    })?;
    let summary = generation_contract_history_summary(snapshot)?;
    history_object.insert(
        GENERATION_CONTRACT_HISTORY_FIELD.to_owned(),
        serde_json::to_value(summary)
            .map_err(|error| GenerationContractError::Serialization(error.to_string()))?,
    );
    Ok(())
}

pub fn read_generation_contract_history_summary(
    history_payload: &Value,
) -> Result<Option<GenerationContractHistorySummaryV1>, GenerationContractError> {
    let Some(summary_value) = history_payload
        .as_object()
        .and_then(|payload| payload.get(GENERATION_CONTRACT_HISTORY_FIELD))
    else {
        return Ok(None);
    };
    if summary_value.get("schema_version").and_then(Value::as_str)
        != Some(GENERATION_CONTRACT_SCHEMA_VERSION)
    {
        return Ok(None);
    }
    serde_json::from_value(summary_value.clone())
        .map(Some)
        .map_err(|error| GenerationContractError::Serialization(error.to_string()))
}
