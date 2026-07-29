use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::schema_owner::{
    RoleModelPolicyV1, RoleModelSelectionV1, ROLE_MODEL_POLICY_SCHEMA_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleModelPolicyError {
    Serialization(String),
    InvalidPreferences(String),
    InvalidPolicy(String),
    UnsupportedSchemaVersion(String),
}

impl fmt::Display for RoleModelPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(message) => {
                write!(
                    formatter,
                    "role model policy serialization failed: {message}"
                )
            }
            Self::InvalidPreferences(message) => {
                write!(formatter, "invalid settings preferences: {message}")
            }
            Self::InvalidPolicy(message) => {
                write!(formatter, "invalid role model policy: {message}")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported role model policy schema version: {version}"
                )
            }
        }
    }
}

impl Error for RoleModelPolicyError {}

pub fn normalize_role_model_policy(
    policy: &RoleModelPolicyV1,
) -> Result<RoleModelPolicyV1, RoleModelPolicyError> {
    if policy.schema_version != ROLE_MODEL_POLICY_SCHEMA_VERSION {
        return Err(RoleModelPolicyError::UnsupportedSchemaVersion(
            policy.schema_version.clone(),
        ));
    }

    let roles = policy
        .roles
        .iter()
        .filter_map(|(role, selection)| {
            let normalized = RoleModelSelectionV1 {
                provider: normalize_provider(selection.provider.as_deref()),
                model: normalize_model(selection.model.as_deref()),
            };
            (normalized.provider.is_some() || normalized.model.is_some())
                .then_some((*role, normalized))
        })
        .collect();

    Ok(RoleModelPolicyV1 {
        schema_version: ROLE_MODEL_POLICY_SCHEMA_VERSION.to_owned(),
        roles,
    })
}

pub fn compute_role_model_policy_digest(
    policy: &RoleModelPolicyV1,
) -> Result<String, RoleModelPolicyError> {
    let normalized = normalize_role_model_policy(policy)?;
    let value = serde_json::to_value(normalized)
        .map_err(|error| RoleModelPolicyError::Serialization(error.to_string()))?;
    let canonical = canonicalize_value(value);
    let serialized = serde_json::to_vec(&canonical)
        .map_err(|error| RoleModelPolicyError::Serialization(error.to_string()))?;
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(serialized))
    ))
}

pub(super) fn normalize_provider(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase)
}

pub(super) fn normalize_model(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted = object
                .into_iter()
                .map(|(key, value)| (key, canonicalize_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect::<Map<_, _>>())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(canonicalize_value).collect()),
        other => other,
    }
}
