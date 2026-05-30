use axum::{http::StatusCode, response::Json};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{json, Value};

use crate::models::settings;

pub(crate) type SettingsRouteError = (StatusCode, Json<Value>);

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct EffectiveSettingsOverrides {
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedEffectiveSettings {
    pub provider: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
}

async fn load_settings_model(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<Option<settings::Model>, String> {
    settings::Entity::find()
        .filter(settings::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
}

fn trim_to_non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn map_internal_error(detail: String) -> SettingsRouteError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"detail": detail})),
    )
}

fn map_bad_request(detail: impl Into<String>) -> SettingsRouteError {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"detail": detail.into()})),
    )
}

fn resolve_provider_base_url(provider: &str, raw_base_url: &str) -> String {
    match provider {
        "gemini" => normalize_gemini_base_url(raw_base_url),
        "anthropic" => normalize_anthropic_base_url(raw_base_url),
        _ => normalize_openai_compatible_base_url(raw_base_url),
    }
}

pub(crate) fn normalize_openai_compatible_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://api.openai.com/v1".to_string();
    }

    if let Ok(mut url) = reqwest::Url::parse(trimmed) {
        let path = url.path().trim_matches('/');
        if path.is_empty() {
            url.set_path("/v1");
            return url.to_string().trim_end_matches('/').to_string();
        }
    }

    trimmed.to_string()
}

fn normalize_anthropic_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://api.anthropic.com".to_string();
    }

    trimmed.to_string()
}

fn normalize_gemini_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://generativelanguage.googleapis.com/v1beta".to_string();
    }

    trimmed.to_string()
}

fn default_runtime_model_for_provider(provider: &str) -> String {
    match provider {
        "anthropic" => "claude-3-5-sonnet-latest".to_string(),
        "gemini" => "gemini-2.5-flash".to_string(),
        _ => "gpt-4o-mini".to_string(),
    }
}

pub(crate) async fn resolve_effective_runtime_settings(
    db: &DatabaseConnection,
    user_id: &str,
    overrides: EffectiveSettingsOverrides,
) -> Result<ResolvedEffectiveSettings, SettingsRouteError> {
    let stored = load_settings_model(db, user_id)
        .await
        .map_err(map_internal_error)?;

    let stored_provider = stored
        .as_ref()
        .map(|model| model.provider_type.clone())
        .unwrap_or_else(|| "openai".to_string());
    let effective_provider = trim_to_non_empty(overrides.provider)
        .map(|value| value.to_lowercase())
        .unwrap_or(stored_provider);

    let stored_key = stored
        .as_ref()
        .map(|model| model.api_key.trim().to_string())
        .unwrap_or_default();
    let effective_key = trim_to_non_empty(overrides.api_key).unwrap_or(stored_key);
    if effective_key.is_empty() {
        return Err(map_bad_request("API key is required"));
    }

    let stored_base = stored
        .as_ref()
        .map(|model| model.api_base_url.trim().to_string())
        .unwrap_or_default();
    let raw_base_url = trim_to_non_empty(overrides.api_base_url).unwrap_or(stored_base);
    let effective_base_url = resolve_provider_base_url(&effective_provider, &raw_base_url);

    let stored_model = stored
        .as_ref()
        .map(|model| model.llm_model.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_runtime_model_for_provider(&effective_provider));
    let effective_model = trim_to_non_empty(overrides.model).unwrap_or(stored_model);

    Ok(ResolvedEffectiveSettings {
        provider: effective_provider,
        api_key: effective_key,
        base_url: effective_base_url,
        model: effective_model,
        temperature: overrides.temperature.unwrap_or(
            stored
                .as_ref()
                .map(|model| model.temperature)
                .unwrap_or(0.7),
        ),
        max_tokens: overrides.max_tokens.unwrap_or(
            stored
                .as_ref()
                .map(|model| model.max_tokens as u32)
                .unwrap_or(32000),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        default_runtime_model_for_provider, normalize_anthropic_base_url,
        normalize_gemini_base_url, normalize_openai_compatible_base_url,
    };

    #[test]
    fn openai_compatible_base_url_defaults_to_v1() {
        assert_eq!(
            normalize_openai_compatible_base_url(""),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_openai_compatible_base_url("https://example.com"),
            "https://example.com/v1"
        );
    }

    #[test]
    fn anthropic_and_gemini_base_urls_keep_provider_specific_defaults() {
        assert_eq!(
            normalize_anthropic_base_url(""),
            "https://api.anthropic.com"
        );
        assert_eq!(
            normalize_gemini_base_url(""),
            "https://generativelanguage.googleapis.com/v1beta"
        );
    }

    #[test]
    fn runtime_default_model_mapping_keeps_existing_probe_defaults() {
        assert_eq!(
            default_runtime_model_for_provider("anthropic"),
            "claude-3-5-sonnet-latest"
        );
        assert_eq!(
            default_runtime_model_for_provider("gemini"),
            "gemini-2.5-flash"
        );
        assert_eq!(default_runtime_model_for_provider("openai"), "gpt-4o-mini");
    }
}
