use serde_json::Value;

#[derive(Debug, PartialEq)]
pub struct TestPresetConnectionRequest {
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub provider: Option<String>,
    pub llm_model: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub api_backup_urls: Option<Vec<String>>,
    pub fallback_strategy: Option<String>,
}

pub fn build_test_preset_connection_request(config: &Value) -> TestPresetConnectionRequest {
    TestPresetConnectionRequest {
        api_key: config
            .get("api_key")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        api_base_url: config
            .get("api_base_url")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        provider: config
            .get("api_provider")
            .or_else(|| config.get("provider"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        llm_model: config
            .get("llm_model")
            .or_else(|| config.get("model"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        temperature: config.get("temperature").and_then(Value::as_f64),
        max_tokens: config
            .get("max_tokens")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        api_backup_urls: config.get("api_backup_urls").and_then(|value| {
            value.as_array().map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .collect()
            })
        }),
        fallback_strategy: config
            .get("fallback_strategy")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::build_test_preset_connection_request;

    #[test]
    fn build_test_preset_connection_request_prefers_existing_provider_keys() {
        let request = build_test_preset_connection_request(&json!({
            "api_key": "sk-test",
            "api_base_url": "https://api.example.com/v1",
            "api_provider": "openai",
            "provider": "gemini",
            "llm_model": "gpt-4o",
            "model": "gemini-2.5-pro",
            "temperature": 0.7,
            "max_tokens": 1024,
            "api_backup_urls": ["https://backup-1.example.com/v1", 1, "https://backup-2.example.com/v1"],
            "fallback_strategy": "manual"
        }));

        assert_eq!(request.api_key.as_deref(), Some("sk-test"));
        assert_eq!(
            request.api_base_url.as_deref(),
            Some("https://api.example.com/v1")
        );
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.llm_model.as_deref(), Some("gpt-4o"));
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(1024));
        assert_eq!(
            request.api_backup_urls,
            Some(vec![
                "https://backup-1.example.com/v1".to_string(),
                "https://backup-2.example.com/v1".to_string()
            ])
        );
        assert_eq!(request.fallback_strategy.as_deref(), Some("manual"));
    }

    #[test]
    fn build_test_preset_connection_request_falls_back_to_legacy_keys() {
        let request = build_test_preset_connection_request(&json!({
            "provider": "anthropic",
            "model": "claude-3-5-sonnet-latest",
            "temperature": 0.125
        }));

        assert_eq!(request.provider.as_deref(), Some("anthropic"));
        assert_eq!(
            request.llm_model.as_deref(),
            Some("claude-3-5-sonnet-latest")
        );
        assert_eq!(request.temperature, Some(0.125));
        assert_eq!(request.max_tokens, None);
        assert_eq!(request.api_backup_urls, None);
        assert_eq!(request.fallback_strategy, None);
    }

    #[test]
    fn build_test_preset_connection_request_ignores_invalid_max_tokens() {
        let request = build_test_preset_connection_request(&json!({
            "max_tokens": 5000000000u64,
            "temperature": "bad"
        }));

        assert_eq!(request.max_tokens, None);
        assert_eq!(request.temperature, None);
    }
}
