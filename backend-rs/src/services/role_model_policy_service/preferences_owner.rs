use serde_json::{Map, Value};

use super::canonical_owner::{normalize_role_model_policy, RoleModelPolicyError};
use super::schema_owner::RoleModelPolicyV1;

pub const ROLE_MODEL_POLICY_PREFERENCES_KEY: &str = "role_model_policy";

pub fn read_role_model_policy(
    preferences: Option<&str>,
) -> Result<RoleModelPolicyV1, RoleModelPolicyError> {
    let Some(raw_preferences) = preferences
        .map(str::trim)
        .filter(|preferences| !preferences.is_empty())
    else {
        return Ok(RoleModelPolicyV1::default());
    };

    let preferences_value: Value = serde_json::from_str(raw_preferences)
        .map_err(|error| RoleModelPolicyError::InvalidPreferences(error.to_string()))?;
    let preferences_object = preferences_value.as_object().ok_or_else(|| {
        RoleModelPolicyError::InvalidPreferences("preferences must be a JSON object".to_owned())
    })?;

    let Some(policy_value) = preferences_object.get(ROLE_MODEL_POLICY_PREFERENCES_KEY) else {
        return Ok(RoleModelPolicyV1::default());
    };
    let policy: RoleModelPolicyV1 = serde_json::from_value(policy_value.clone())
        .map_err(|error| RoleModelPolicyError::InvalidPolicy(error.to_string()))?;
    normalize_role_model_policy(&policy)
}

pub fn set_role_model_policy(
    preferences: Option<&str>,
    policy: &RoleModelPolicyV1,
) -> Result<String, RoleModelPolicyError> {
    let normalized = normalize_role_model_policy(policy)?;
    let mut preferences_object = parse_preferences_object(preferences)?;
    let policy_value = serde_json::to_value(normalized)
        .map_err(|error| RoleModelPolicyError::Serialization(error.to_string()))?;
    preferences_object.insert(ROLE_MODEL_POLICY_PREFERENCES_KEY.to_owned(), policy_value);
    serde_json::to_string(&Value::Object(preferences_object))
        .map_err(|error| RoleModelPolicyError::Serialization(error.to_string()))
}

fn parse_preferences_object(
    preferences: Option<&str>,
) -> Result<Map<String, Value>, RoleModelPolicyError> {
    let Some(raw_preferences) = preferences
        .map(str::trim)
        .filter(|preferences| !preferences.is_empty())
    else {
        return Ok(Map::new());
    };

    let value: Value = serde_json::from_str(raw_preferences)
        .map_err(|error| RoleModelPolicyError::InvalidPreferences(error.to_string()))?;
    value.as_object().cloned().ok_or_else(|| {
        RoleModelPolicyError::InvalidPreferences("preferences must be a JSON object".to_owned())
    })
}
