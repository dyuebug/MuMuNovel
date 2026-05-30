use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Default, Clone, Debug)]
pub struct CreatePresetFromCurrentRouteQuery {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
pub struct CreatePresetFromCurrentRouteBody {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CreatePresetFromCurrentRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateSettingsPresetRequest {
    name: String,
    description: Option<Value>,
    config: Value,
}

impl CreateSettingsPresetRequest {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&Value> {
        self.description.as_ref()
    }

    pub fn config(&self) -> &Value {
        &self.config
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpdateSettingsPresetRequest {
    name: Option<String>,
    description: Option<Value>,
    has_description: bool,
    config: Option<Value>,
}

impl UpdateSettingsPresetRequest {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&Value> {
        self.description.as_ref()
    }

    pub fn has_description(&self) -> bool {
        self.has_description
    }

    pub fn config(&self) -> Option<&Value> {
        self.config.as_ref()
    }
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub struct CreateSettingsPresetRouteRequest {
    #[serde(default)]
    pub name: Option<Value>,
    #[serde(default)]
    pub description: Option<Value>,
    #[serde(default)]
    pub config: Option<Value>,
}

impl CreateSettingsPresetRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "config": self.config,
        })
    }
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub struct UpdateSettingsPresetRouteRequest {
    #[serde(default)]
    pub name: Option<Value>,
    #[serde(default)]
    pub description: Option<Value>,
    #[serde(default)]
    pub config: Option<Value>,
}

impl UpdateSettingsPresetRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "config": self.config,
        })
    }
}

pub fn build_create_preset_from_current_request(
    query: CreatePresetFromCurrentRouteQuery,
    body: Option<CreatePresetFromCurrentRouteBody>,
) -> CreatePresetFromCurrentRequest {
    let body_name = body
        .as_ref()
        .and_then(|payload| payload.name.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string);
    let body_description = body.and_then(|payload| payload.description);

    let name = query
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or(body_name)
        .unwrap_or_else(|| "My Preset".to_string());
    let description = query
        .description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or(body_description);

    CreatePresetFromCurrentRequest { name, description }
}

pub fn build_create_settings_preset_request_from_route_payload(
    body: &Value,
) -> CreateSettingsPresetRequest {
    CreateSettingsPresetRequest {
        name: body
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        description: body.get("description").cloned(),
        config: body.get("config").cloned().unwrap_or_else(|| json!({})),
    }
}

pub fn build_create_settings_preset_request_from_typed_route_payload(
    body: CreateSettingsPresetRouteRequest,
) -> CreateSettingsPresetRequest {
    build_create_settings_preset_request_from_route_payload(&body.into_body())
}

pub fn build_update_settings_preset_request_from_route_payload(
    body: &Value,
) -> UpdateSettingsPresetRequest {
    UpdateSettingsPresetRequest {
        name: body
            .get("name")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        description: body.get("description").cloned(),
        has_description: body.get("description").is_some(),
        config: body.get("config").cloned(),
    }
}

pub fn build_update_settings_preset_request_from_typed_route_payload(
    body: UpdateSettingsPresetRouteRequest,
) -> UpdateSettingsPresetRequest {
    build_update_settings_preset_request_from_route_payload(&body.into_body())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_create_preset_from_current_request,
        build_create_settings_preset_request_from_route_payload,
        build_create_settings_preset_request_from_typed_route_payload,
        build_update_settings_preset_request_from_route_payload,
        build_update_settings_preset_request_from_typed_route_payload,
        CreatePresetFromCurrentRouteBody, CreatePresetFromCurrentRouteQuery,
        CreateSettingsPresetRouteRequest, UpdateSettingsPresetRouteRequest,
    };

    #[test]
    fn build_create_preset_from_current_request_prefers_non_empty_query_values() {
        let request = build_create_preset_from_current_request(
            CreatePresetFromCurrentRouteQuery {
                name: Some("Query Preset".to_string()),
                description: Some("Query Description".to_string()),
            },
            Some(CreatePresetFromCurrentRouteBody {
                name: Some("Body Preset".to_string()),
                description: Some("Body Description".to_string()),
            }),
        );

        assert_eq!(request.name, "Query Preset");
        assert_eq!(request.description.as_deref(), Some("Query Description"));
    }

    #[test]
    fn build_create_preset_from_current_request_falls_back_to_body_and_default_name() {
        let from_body = build_create_preset_from_current_request(
            CreatePresetFromCurrentRouteQuery::default(),
            Some(CreatePresetFromCurrentRouteBody {
                name: Some("Body Preset".to_string()),
                description: Some("Body Description".to_string()),
            }),
        );
        assert_eq!(from_body.name, "Body Preset");
        assert_eq!(from_body.description.as_deref(), Some("Body Description"));

        let defaulted = build_create_preset_from_current_request(
            CreatePresetFromCurrentRouteQuery::default(),
            Some(CreatePresetFromCurrentRouteBody {
                name: Some("   ".to_string()),
                description: None,
            }),
        );
        assert_eq!(defaulted.name, "My Preset");
        assert_eq!(defaulted.description, None);
    }

    #[test]
    fn build_create_preset_from_current_request_treats_blank_query_as_missing() {
        let request = build_create_preset_from_current_request(
            CreatePresetFromCurrentRouteQuery {
                name: Some("   ".to_string()),
                description: Some("  ".to_string()),
            },
            Some(CreatePresetFromCurrentRouteBody {
                name: Some("Body Preset".to_string()),
                description: Some("".to_string()),
            }),
        );

        assert_eq!(request.name, "Body Preset");
        assert_eq!(request.description.as_deref(), Some(""));
    }

    #[test]
    fn build_create_settings_preset_request_from_route_payload_keeps_existing_shape() {
        let request = build_create_settings_preset_request_from_route_payload(&json!({
            "name": "Preset A",
            "description": "desc",
            "config": {
                "provider": "openai"
            }
        }));

        assert_eq!(request.name(), "Preset A");
        assert_eq!(request.description(), Some(&json!("desc")));
        assert_eq!(request.config()["provider"], "openai");
    }

    #[test]
    fn build_create_settings_preset_request_from_route_payload_defaults_empty_config() {
        let request = build_create_settings_preset_request_from_route_payload(&json!({
            "name": "Preset B"
        }));

        assert_eq!(request.name(), "Preset B");
        assert_eq!(request.description(), None);
        assert_eq!(request.config(), &json!({}));
    }

    #[test]
    fn build_create_settings_preset_request_from_typed_route_payload_keeps_existing_shape() {
        let request = build_create_settings_preset_request_from_typed_route_payload(
            CreateSettingsPresetRouteRequest {
                name: Some(json!("Preset A")),
                description: Some(json!("desc")),
                config: Some(json!({
                    "provider": "openai"
                })),
            },
        );

        assert_eq!(request.name(), "Preset A");
        assert_eq!(request.description(), Some(&json!("desc")));
        assert_eq!(request.config()["provider"], "openai");
    }

    #[test]
    fn build_update_settings_preset_request_from_route_payload_tracks_optional_fields() {
        let request = build_update_settings_preset_request_from_route_payload(&json!({
            "name": "Renamed",
            "description": null,
            "config": {
                "model": "gpt-4o"
            }
        }));

        assert_eq!(request.name(), Some("Renamed"));
        assert!(request.has_description());
        assert_eq!(request.description(), Some(&json!(null)));
        assert_eq!(
            request.config().expect("config should exist")["model"],
            "gpt-4o"
        );
    }

    #[test]
    fn build_update_settings_preset_request_from_typed_route_payload_tracks_optional_fields() {
        let request = build_update_settings_preset_request_from_typed_route_payload(
            UpdateSettingsPresetRouteRequest {
                name: Some(json!("Renamed")),
                description: Some(json!(null)),
                config: Some(json!({
                    "model": "gpt-4o"
                })),
            },
        );

        assert_eq!(request.name(), Some("Renamed"));
        assert!(request.has_description());
        assert_eq!(request.description(), Some(&json!(null)));
        assert_eq!(
            request.config().expect("config should exist")["model"],
            "gpt-4o"
        );
    }
}
