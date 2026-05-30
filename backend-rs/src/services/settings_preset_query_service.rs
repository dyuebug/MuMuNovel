use serde_json::{json, Value};

use crate::services::settings_service::get_api_presets;

#[derive(Debug, PartialEq, Eq)]
pub enum FindPresetConfigError {
    PresetNotFound,
}

pub(crate) fn find_preset_config(
    preferences: Option<&str>,
    preset_id: &str,
) -> Result<Value, FindPresetConfigError> {
    let (presets, _version) = get_api_presets(preferences);

    presets
        .into_iter()
        .find(|preset| preset.get("id").and_then(Value::as_str) == Some(preset_id))
        .map(|preset| preset.get("config").cloned().unwrap_or_else(|| json!({})))
        .ok_or(FindPresetConfigError::PresetNotFound)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{find_preset_config, FindPresetConfigError};

    #[test]
    fn find_preset_config_returns_matching_config() {
        let config = find_preset_config(
            Some(
                &json!({
                    "api_presets": {
                        "version": "1.0",
                        "presets": [
                            {
                                "id": "preset_1",
                                "config": {
                                    "api_provider": "openai",
                                    "llm_model": "gpt-4o"
                                }
                            }
                        ]
                    }
                })
                .to_string(),
            ),
            "preset_1",
        )
        .expect("preset config should exist");

        assert_eq!(config["api_provider"], "openai");
        assert_eq!(config["llm_model"], "gpt-4o");
    }

    #[test]
    fn find_preset_config_defaults_to_empty_object_when_config_missing() {
        let config = find_preset_config(
            Some(
                &json!({
                    "api_presets": {
                        "presets": [
                            { "id": "preset_1" }
                        ]
                    }
                })
                .to_string(),
            ),
            "preset_1",
        )
        .expect("preset without config should still resolve");

        assert_eq!(config, json!({}));
    }

    #[test]
    fn find_preset_config_rejects_unknown_preset() {
        let error =
            find_preset_config(Some("{}"), "missing").expect_err("missing preset should fail");

        assert_eq!(error, FindPresetConfigError::PresetNotFound);
    }
}
