use serde::Deserialize;
use serde_json::{json, Map, Value};

const WEB_RESEARCH_KEYS: [&str; 9] = [
    "web_research_enabled",
    "web_research_exa_enabled",
    "web_research_grok_enabled",
    "web_research_exa_api_key",
    "web_research_exa_base_url",
    "web_research_grok_api_key",
    "web_research_grok_base_url",
    "web_research_grok_model",
    "web_research_grok_search_enabled",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SettingsApiBackupUrlsField {
    Missing,
    Invalid,
    Provided(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SettingsUpdateRequest {
    pub api_provider: Option<String>,
    pub clear_api_key: bool,
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub api_backup_urls: SettingsApiBackupUrlsField,
    pub provider_type: Option<String>,
    pub fallback_strategy: Option<String>,
    pub azure_api_version: Option<String>,
    pub llm_model: Option<String>,
    pub provider_switch_requested: bool,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    pub system_prompt: Option<String>,
    pub preferences: Option<String>,
    pub web_research_patch: Value,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub(crate) struct SettingsUpdateRouteRequest {
    #[serde(default)]
    pub api_provider: Option<Value>,
    #[serde(default)]
    pub clear_api_key: Option<Value>,
    #[serde(default)]
    pub api_key: Option<Value>,
    #[serde(default)]
    pub api_base_url: Option<Value>,
    #[serde(default)]
    pub api_backup_urls: Option<Value>,
    #[serde(default)]
    pub provider_type: Option<Value>,
    #[serde(default)]
    pub fallback_strategy: Option<Value>,
    #[serde(default)]
    pub azure_api_version: Option<Value>,
    #[serde(default)]
    pub llm_model: Option<Value>,
    #[serde(default)]
    pub temperature: Option<Value>,
    #[serde(default)]
    pub max_tokens: Option<Value>,
    #[serde(default)]
    pub system_prompt: Option<Value>,
    #[serde(default)]
    pub preferences: Option<Value>,
    #[serde(default)]
    pub web_research_enabled: Option<Value>,
    #[serde(default)]
    pub web_research_exa_enabled: Option<Value>,
    #[serde(default)]
    pub web_research_grok_enabled: Option<Value>,
    #[serde(default)]
    pub web_research_exa_api_key: Option<Value>,
    #[serde(default)]
    pub web_research_exa_base_url: Option<Value>,
    #[serde(default)]
    pub web_research_grok_api_key: Option<Value>,
    #[serde(default)]
    pub web_research_grok_base_url: Option<Value>,
    #[serde(default)]
    pub web_research_grok_model: Option<Value>,
    #[serde(default)]
    pub web_research_grok_search_enabled: Option<Value>,
}

impl SettingsUpdateRouteRequest {
    fn into_body(self) -> Value {
        let mut body = Map::new();

        insert_present_field(&mut body, "api_provider", self.api_provider);
        insert_present_field(&mut body, "clear_api_key", self.clear_api_key);
        insert_present_field(&mut body, "api_key", self.api_key);
        insert_present_field(&mut body, "api_base_url", self.api_base_url);
        insert_present_field(&mut body, "api_backup_urls", self.api_backup_urls);
        insert_present_field(&mut body, "provider_type", self.provider_type);
        insert_present_field(&mut body, "fallback_strategy", self.fallback_strategy);
        insert_present_field(&mut body, "azure_api_version", self.azure_api_version);
        insert_present_field(&mut body, "llm_model", self.llm_model);
        insert_present_field(&mut body, "temperature", self.temperature);
        insert_present_field(&mut body, "max_tokens", self.max_tokens);
        insert_present_field(&mut body, "system_prompt", self.system_prompt);
        insert_present_field(&mut body, "preferences", self.preferences);
        insert_present_field(&mut body, "web_research_enabled", self.web_research_enabled);
        insert_present_field(
            &mut body,
            "web_research_exa_enabled",
            self.web_research_exa_enabled,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_enabled",
            self.web_research_grok_enabled,
        );
        insert_present_field(
            &mut body,
            "web_research_exa_api_key",
            self.web_research_exa_api_key,
        );
        insert_present_field(
            &mut body,
            "web_research_exa_base_url",
            self.web_research_exa_base_url,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_api_key",
            self.web_research_grok_api_key,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_base_url",
            self.web_research_grok_base_url,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_model",
            self.web_research_grok_model,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_search_enabled",
            self.web_research_grok_search_enabled,
        );

        Value::Object(body)
    }
}

fn insert_present_field(body: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        body.insert(key.to_string(), value);
    }
}

pub(crate) fn build_settings_update_request_from_route_body(body: &Value) -> SettingsUpdateRequest {
    SettingsUpdateRequest {
        api_provider: body
            .get("api_provider")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        clear_api_key: body
            .get("clear_api_key")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        api_key: body
            .get("api_key")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        api_base_url: body
            .get("api_base_url")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        api_backup_urls: build_api_backup_urls_field(body.get("api_backup_urls")),
        provider_type: body
            .get("provider_type")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        fallback_strategy: body
            .get("fallback_strategy")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        azure_api_version: body
            .get("azure_api_version")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        llm_model: body
            .get("llm_model")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        provider_switch_requested: body.get("provider_type").is_some()
            || body.get("api_provider").is_some(),
        temperature: body.get("temperature").and_then(Value::as_f64),
        max_tokens: body.get("max_tokens").and_then(Value::as_i64),
        system_prompt: body
            .get("system_prompt")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        preferences: body
            .get("preferences")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        web_research_patch: extract_web_research_patch_from_route_body(body),
    }
}

pub(crate) fn build_settings_update_request_from_typed_route_payload(
    body: SettingsUpdateRouteRequest,
) -> SettingsUpdateRequest {
    build_settings_update_request_from_route_body(&body.into_body())
}

fn build_api_backup_urls_field(value: Option<&Value>) -> SettingsApiBackupUrlsField {
    match value {
        None => SettingsApiBackupUrlsField::Missing,
        Some(Value::Array(items)) => SettingsApiBackupUrlsField::Provided(
            items
                .iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect(),
        ),
        Some(_) => SettingsApiBackupUrlsField::Invalid,
    }
}

fn extract_web_research_patch_from_route_body(body: &Value) -> Value {
    let mut patch = json!({});

    if let Some(obj) = body.as_object() {
        if let Some(patch_obj) = patch.as_object_mut() {
            for key in &WEB_RESEARCH_KEYS {
                if let Some(value) = obj.get(*key) {
                    patch_obj.insert((*key).to_string(), value.clone());
                }
            }
        }
    }

    patch
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_settings_update_request_from_route_body,
        build_settings_update_request_from_typed_route_payload, SettingsApiBackupUrlsField,
        SettingsUpdateRouteRequest,
    };

    #[test]
    fn build_settings_update_request_from_route_body_keeps_existing_contract() {
        let request = build_settings_update_request_from_route_body(&json!({
            "api_provider": "openai",
            "clear_api_key": true,
            "api_key": "  sk-live  ",
            "api_base_url": "https://api.example.com/v1",
            "api_backup_urls": ["https://a.example.com", 1, "https://b.example.com"],
            "provider_type": "openai",
            "fallback_strategy": "auto",
            "azure_api_version": "2024-10-21",
            "llm_model": "gpt-4.1",
            "temperature": 0.3,
            "max_tokens": 2048,
            "system_prompt": "system",
            "preferences": "{\"theme\":\"dark\"}",
            "web_research_enabled": true,
            "web_research_grok_model": "grok-4.1-fast"
        }));

        assert_eq!(request.api_provider.as_deref(), Some("openai"));
        assert!(request.clear_api_key);
        assert_eq!(request.api_key.as_deref(), Some("  sk-live  "));
        assert_eq!(
            request.api_base_url.as_deref(),
            Some("https://api.example.com/v1")
        );
        assert_eq!(
            request.api_backup_urls,
            SettingsApiBackupUrlsField::Provided(vec![
                "https://a.example.com".to_string(),
                "https://b.example.com".to_string(),
            ])
        );
        assert_eq!(request.provider_type.as_deref(), Some("openai"));
        assert_eq!(request.fallback_strategy.as_deref(), Some("auto"));
        assert_eq!(request.azure_api_version.as_deref(), Some("2024-10-21"));
        assert_eq!(request.llm_model.as_deref(), Some("gpt-4.1"));
        assert!(request.provider_switch_requested);
        assert_eq!(request.temperature, Some(0.3));
        assert_eq!(request.max_tokens, Some(2048));
        assert_eq!(request.system_prompt.as_deref(), Some("system"));
        assert_eq!(request.preferences.as_deref(), Some("{\"theme\":\"dark\"}"));
        assert_eq!(request.web_research_patch["web_research_enabled"], true);
        assert_eq!(
            request.web_research_patch["web_research_grok_model"],
            "grok-4.1-fast"
        );
    }

    #[test]
    fn build_settings_update_request_from_route_body_preserves_backup_urls_compat_shape() {
        let missing = build_settings_update_request_from_route_body(&json!({}));
        assert_eq!(missing.api_backup_urls, SettingsApiBackupUrlsField::Missing);
        assert!(!missing.provider_switch_requested);

        let invalid = build_settings_update_request_from_route_body(&json!({
            "api_backup_urls": "not-an-array",
            "provider_type": 1,
            "api_provider": false
        }));
        assert_eq!(invalid.api_backup_urls, SettingsApiBackupUrlsField::Invalid);
        assert!(invalid.provider_switch_requested);
    }

    #[test]
    fn build_settings_update_request_from_route_body_ignores_non_string_scalar_fields() {
        let request = build_settings_update_request_from_route_body(&json!({
            "api_provider": 1,
            "api_key": true,
            "api_base_url": [],
            "fallback_strategy": {},
            "azure_api_version": null,
            "llm_model": 99,
            "system_prompt": false,
            "preferences": ["bad"],
            "temperature": "bad",
            "max_tokens": "bad"
        }));

        assert_eq!(request.api_provider, None);
        assert_eq!(request.api_key, None);
        assert_eq!(request.api_base_url, None);
        assert_eq!(request.fallback_strategy, None);
        assert_eq!(request.azure_api_version, None);
        assert_eq!(request.llm_model, None);
        assert_eq!(request.system_prompt, None);
        assert_eq!(request.preferences, None);
        assert_eq!(request.temperature, None);
        assert_eq!(request.max_tokens, None);
    }

    #[test]
    fn build_settings_update_request_from_typed_route_payload_preserves_route_compat_semantics() {
        let request =
            build_settings_update_request_from_typed_route_payload(SettingsUpdateRouteRequest {
                api_provider: Some(json!("openai")),
                clear_api_key: Some(json!(true)),
                api_key: Some(Value::Null),
                api_base_url: Some(json!("https://api.example.com/v1")),
                api_backup_urls: Some(json!(["https://a.example.com", 1, "https://b.example.com"])),
                provider_type: Some(json!(false)),
                fallback_strategy: Some(json!("auto")),
                azure_api_version: Some(json!("2024-10-21")),
                llm_model: Some(json!("gpt-4.1")),
                temperature: Some(json!(0.3)),
                max_tokens: Some(json!(2048)),
                system_prompt: Some(json!("system")),
                preferences: Some(json!("{\"theme\":\"dark\"}")),
                web_research_enabled: Some(json!(true)),
                web_research_grok_model: Some(json!("grok-4.1-fast")),
                ..Default::default()
            });

        assert_eq!(request.api_provider.as_deref(), Some("openai"));
        assert!(request.clear_api_key);
        assert_eq!(request.api_key, None);
        assert_eq!(
            request.api_base_url.as_deref(),
            Some("https://api.example.com/v1")
        );
        assert_eq!(
            request.api_backup_urls,
            SettingsApiBackupUrlsField::Provided(vec![
                "https://a.example.com".to_string(),
                "https://b.example.com".to_string(),
            ])
        );
        assert_eq!(request.provider_type, None);
        assert!(request.provider_switch_requested);
        assert_eq!(request.fallback_strategy.as_deref(), Some("auto"));
        assert_eq!(request.azure_api_version.as_deref(), Some("2024-10-21"));
        assert_eq!(request.llm_model.as_deref(), Some("gpt-4.1"));
        assert_eq!(request.temperature, Some(0.3));
        assert_eq!(request.max_tokens, Some(2048));
        assert_eq!(request.system_prompt.as_deref(), Some("system"));
        assert_eq!(request.preferences.as_deref(), Some("{\"theme\":\"dark\"}"));
        assert_eq!(request.web_research_patch["web_research_enabled"], true);
        assert_eq!(
            request.web_research_patch["web_research_grok_model"],
            "grok-4.1-fast"
        );
    }

    #[test]
    fn build_settings_update_request_from_typed_route_payload_keeps_missing_and_invalid_shape() {
        let missing = build_settings_update_request_from_typed_route_payload(
            SettingsUpdateRouteRequest::default(),
        );
        assert_eq!(missing.api_backup_urls, SettingsApiBackupUrlsField::Missing);
        assert!(!missing.provider_switch_requested);

        let invalid =
            build_settings_update_request_from_typed_route_payload(SettingsUpdateRouteRequest {
                api_provider: Some(json!(false)),
                provider_type: Some(json!(1)),
                api_backup_urls: Some(json!("not-an-array")),
                temperature: Some(json!("bad")),
                max_tokens: Some(json!("bad")),
                ..Default::default()
            });
        assert_eq!(invalid.api_backup_urls, SettingsApiBackupUrlsField::Invalid);
        assert!(invalid.provider_switch_requested);
        assert_eq!(invalid.temperature, None);
        assert_eq!(invalid.max_tokens, None);
    }
}
