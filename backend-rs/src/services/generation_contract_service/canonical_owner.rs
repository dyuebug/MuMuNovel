use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::schema_owner::{
    GenerationContractSnapshotV1, GenerationIntentV1, GenerationTarget, GenerationTargetKind,
    StoryPacketV1, GENERATION_CONTRACT_SCHEMA_VERSION,
};

const SENSITIVE_FIELD_NAMES: &[&str] = &[
    "api_key",
    "authorization",
    "cookie",
    "access_token",
    "refresh_token",
    "client_secret",
    "jwt_secret",
];

const RUNTIME_ONLY_FIELD_NAMES: &[&str] = &[
    "created_at",
    "updated_at",
    "timestamp",
    "task_id",
    "request_id",
    "task_progress",
    "progress",
    "retry_count",
    "provider_result",
    "provider",
    "model",
    "fallback",
    "fallback_chain",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationContractError {
    Serialization(String),
    UnsupportedSchemaVersion(String),
    InvalidTarget(String),
    ProjectMismatch { expected: String, actual: String },
    SensitiveField(String),
    DigestMismatch { expected: String, actual: String },
}

impl fmt::Display for GenerationContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(message) => write!(
                formatter,
                "generation contract serialization failed: {message}"
            ),
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported generation contract schema version: {version}"
                )
            }
            Self::InvalidTarget(message) => {
                write!(formatter, "invalid generation target: {message}")
            }
            Self::ProjectMismatch { expected, actual } => write!(
                formatter,
                "generation contract project mismatch: expected {expected}, got {actual}"
            ),
            Self::SensitiveField(field) => {
                write!(
                    formatter,
                    "generation contract contains sensitive field: {field}"
                )
            }
            Self::DigestMismatch { expected, actual } => write!(
                formatter,
                "generation contract digest mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl Error for GenerationContractError {}

pub fn build_generation_contract_snapshot(
    story_packet: StoryPacketV1,
    generation_intent: GenerationIntentV1,
) -> Result<GenerationContractSnapshotV1, GenerationContractError> {
    validate_contract_inputs(&story_packet, &generation_intent)?;
    let input_digest = compute_input_digest(&story_packet, &generation_intent)?;
    Ok(GenerationContractSnapshotV1 {
        schema_version: GENERATION_CONTRACT_SCHEMA_VERSION.to_owned(),
        story_packet,
        generation_intent,
        input_digest,
    })
}

pub fn validate_generation_contract_snapshot(
    snapshot: &GenerationContractSnapshotV1,
) -> Result<(), GenerationContractError> {
    validate_schema_version(&snapshot.schema_version)?;
    validate_contract_inputs(&snapshot.story_packet, &snapshot.generation_intent)?;
    let actual = compute_input_digest(&snapshot.story_packet, &snapshot.generation_intent)?;
    if actual != snapshot.input_digest {
        return Err(GenerationContractError::DigestMismatch {
            expected: snapshot.input_digest.clone(),
            actual,
        });
    }
    Ok(())
}

pub fn compute_input_digest(
    story_packet: &StoryPacketV1,
    generation_intent: &GenerationIntentV1,
) -> Result<String, GenerationContractError> {
    validate_contract_inputs(story_packet, generation_intent)?;

    let mut preimage = BTreeMap::new();
    preimage.insert(
        "schema_version",
        Value::String(GENERATION_CONTRACT_SCHEMA_VERSION.to_owned()),
    );
    preimage.insert("story_packet", serialize_to_value(story_packet)?);
    preimage.insert("generation_intent", serialize_to_value(generation_intent)?);

    let normalized = normalize_canonical_value(Value::Object(
        preimage
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    ));
    if let Some(field) = find_sensitive_field(&normalized) {
        return Err(GenerationContractError::SensitiveField(field));
    }
    let digest_input = strip_runtime_only_fields(normalized);
    let serialized = serde_json::to_vec(&digest_input)
        .map_err(|error| GenerationContractError::Serialization(error.to_string()))?;
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(serialized))
    ))
}

pub fn canonicalize_json_value(value: Value) -> Result<Value, GenerationContractError> {
    let normalized = normalize_canonical_value(value);
    if let Some(field) = find_sensitive_field(&normalized) {
        return Err(GenerationContractError::SensitiveField(field));
    }
    Ok(strip_runtime_only_fields(normalized))
}

pub fn canonical_json_string(value: Value) -> Result<String, GenerationContractError> {
    serde_json::to_string(&canonicalize_json_value(value)?)
        .map_err(|error| GenerationContractError::Serialization(error.to_string()))
}

fn validate_contract_inputs(
    story_packet: &StoryPacketV1,
    generation_intent: &GenerationIntentV1,
) -> Result<(), GenerationContractError> {
    validate_schema_version(&story_packet.schema_version)?;
    validate_schema_version(&generation_intent.schema_version)?;
    validate_target(&story_packet.target)?;
    validate_target(&generation_intent.target)?;

    if story_packet.project_id.trim().is_empty() {
        return Err(GenerationContractError::InvalidTarget(
            "story packet project_id must not be empty".to_owned(),
        ));
    }
    ensure_same_project(&story_packet.project_id, &story_packet.target.project_id)?;
    ensure_same_project(
        &story_packet.project_id,
        &generation_intent.target.project_id,
    )?;
    validate_positive_word_count(
        "story_packet.target_word_count",
        story_packet.target_word_count,
    )?;
    validate_positive_word_count(
        "generation_intent.target_word_count",
        generation_intent.target_word_count,
    )?;

    let packet_value = serialize_to_value(story_packet)?;
    if let Some(field) = find_sensitive_field(&normalize_canonical_value(packet_value)) {
        return Err(GenerationContractError::SensitiveField(field));
    }
    let intent_value = serialize_to_value(generation_intent)?;
    if let Some(field) = find_sensitive_field(&normalize_canonical_value(intent_value)) {
        return Err(GenerationContractError::SensitiveField(field));
    }

    Ok(())
}

fn validate_schema_version(version: &str) -> Result<(), GenerationContractError> {
    if version == GENERATION_CONTRACT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(GenerationContractError::UnsupportedSchemaVersion(
            version.to_owned(),
        ))
    }
}

fn validate_target(target: &GenerationTarget) -> Result<(), GenerationContractError> {
    if target.project_id.trim().is_empty() {
        return Err(GenerationContractError::InvalidTarget(
            "project_id must not be empty".to_owned(),
        ));
    }

    match target.kind {
        GenerationTargetKind::Outline => Ok(()),
        GenerationTargetKind::Chapter => {
            require_non_empty(target.chapter_id.as_deref(), "chapter_id")
        }
        GenerationTargetKind::ChapterBatch => {
            if target.chapter_ids.is_empty()
                || target
                    .chapter_ids
                    .iter()
                    .any(|chapter_id| chapter_id.trim().is_empty())
            {
                Err(GenerationContractError::InvalidTarget(
                    "chapter batch requires non-empty chapter_ids".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
        GenerationTargetKind::ChapterSelection => {
            require_non_empty(target.chapter_id.as_deref(), "chapter_id")?;
            let selection = target.selection.as_ref().ok_or_else(|| {
                GenerationContractError::InvalidTarget(
                    "chapter selection requires selection bounds".to_owned(),
                )
            })?;
            if selection.start_index >= selection.end_index {
                Err(GenerationContractError::InvalidTarget(
                    "selection start_index must be smaller than end_index".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
    }
}

fn validate_positive_word_count(
    field: &str,
    value: Option<u32>,
) -> Result<(), GenerationContractError> {
    if value == Some(0) {
        Err(GenerationContractError::InvalidTarget(format!(
            "{field} must be greater than zero"
        )))
    } else {
        Ok(())
    }
}

fn require_non_empty(value: Option<&str>, field: &str) -> Result<(), GenerationContractError> {
    if value.is_some_and(|value| !value.trim().is_empty()) {
        Ok(())
    } else {
        Err(GenerationContractError::InvalidTarget(format!(
            "{field} must not be empty"
        )))
    }
}

fn ensure_same_project(expected: &str, actual: &str) -> Result<(), GenerationContractError> {
    if expected == actual {
        Ok(())
    } else {
        Err(GenerationContractError::ProjectMismatch {
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

fn serialize_to_value<T: Serialize>(value: &T) -> Result<Value, GenerationContractError> {
    serde_json::to_value(value)
        .map_err(|error| GenerationContractError::Serialization(error.to_string()))
}

fn normalize_canonical_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut normalized = Map::new();
            for (key, value) in entries {
                normalized.insert(key, normalize_canonical_value(value));
            }
            Value::Object(normalized)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(normalize_canonical_value).collect())
        }
        Value::String(value) => normalize_nested_json_string(value),
        scalar => scalar,
    }
}

fn normalize_nested_json_string(value: String) -> Value {
    let trimmed = value.trim();
    let looks_like_json = (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'));
    if looks_like_json {
        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            return normalize_canonical_value(parsed);
        }
    }
    Value::String(value)
}

fn strip_runtime_only_fields(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut stripped = Map::new();
            for (key, value) in object {
                if !matches_field_name(&key, RUNTIME_ONLY_FIELD_NAMES) {
                    stripped.insert(key, strip_runtime_only_fields(value));
                }
            }
            Value::Object(stripped)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(strip_runtime_only_fields).collect())
        }
        scalar => scalar,
    }
}

fn find_sensitive_field(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => object.iter().find_map(|(key, value)| {
            if matches_field_name(key, SENSITIVE_FIELD_NAMES) {
                Some(key.clone())
            } else {
                find_sensitive_field(value)
            }
        }),
        Value::Array(values) => values.iter().find_map(find_sensitive_field),
        _ => None,
    }
}

fn matches_field_name(field: &str, candidates: &[&str]) -> bool {
    let normalized = normalize_field_name(field);
    let compact = normalized.replace('_', "");
    candidates.iter().any(|candidate| {
        let candidate = normalize_field_name(candidate);
        candidate == normalized || candidate.replace('_', "") == compact
    })
}

fn normalize_field_name(field: &str) -> String {
    field
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
