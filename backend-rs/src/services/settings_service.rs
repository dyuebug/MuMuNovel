use std::env;

use axum::{http::StatusCode, response::Json};
use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::config::AIConfig;
use crate::models::settings;
use crate::services::generation_contract_service::GenerationIntentKind;
use crate::services::role_model_policy_service::{
    read_role_model_policy, resolve_role_model_policy, ResolvedRoleModelPolicyV1,
    RoleModelPolicyV1, RoleModelResolutionInput,
};

const PLACEHOLDER_MASK: &str = "********";
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

const WEB_RESEARCH_PREF_KEY: &str = "web_research";
pub(crate) const SETTINGS_UPDATE_MISSING_DETAIL: &str = "设置不存在，请先创建设置";
pub(crate) const SETTINGS_DELETE_MISSING_DETAIL: &str = "设置不存在";
pub(crate) type SettingsRouteError = (StatusCode, Json<Value>);

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

#[derive(Debug, Clone)]
pub struct RoleAwareAIConfig {
    pub ai_config: AIConfig,
    pub resolved_policy: ResolvedRoleModelPolicyV1,
    pub allow_model_fallback: bool,
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

fn web_research_defaults() -> Value {
    json!({
        "web_research_enabled": false,
        "web_research_exa_enabled": true,
        "web_research_grok_enabled": true,
        "web_research_exa_api_key": "",
        "web_research_exa_base_url": "",
        "web_research_grok_api_key": "",
        "web_research_grok_base_url": "",
        "web_research_grok_model": "grok-4.1-fast",
        "web_research_grok_search_enabled": false,
    })
}

fn is_placeholder(key: &str) -> bool {
    let placeholders = [
        "your_openai_api_key_here",
        "your_anthropic_api_key_here",
        "your_gemini_api_key_here",
        "your_api_key_here",
    ];
    placeholders.contains(&key) || key == PLACEHOLDER_MASK || key.starts_with("sk-placeholder")
}

fn mask_api_key(key: &str) -> String {
    if key.is_empty() || is_placeholder(key) {
        key.to_string()
    } else {
        PLACEHOLDER_MASK.to_string()
    }
}

fn parse_api_backup_urls(raw: Option<&str>) -> Vec<String> {
    raw.and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default()
}

fn serialize_api_backup_urls(urls: &[String]) -> Option<String> {
    if urls.is_empty() {
        None
    } else {
        serde_json::to_string(urls).ok()
    }
}

fn merge_web_research(prefs_json: Option<&str>) -> Value {
    let stored: Value = prefs_json
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(json!({}));
    let mut merged = web_research_defaults();
    if let Some(wr) = stored.get(WEB_RESEARCH_PREF_KEY) {
        if let Some(wr_obj) = wr.as_object() {
            if let Some(merged_obj) = merged.as_object_mut() {
                for (k, v) in wr_obj {
                    merged_obj.insert(k.clone(), v.clone());
                }
            }
        }
    }
    merged
}

fn set_web_research(prefs_json: Option<&str>, wr: &Value) -> serde_json::Result<String> {
    let mut prefs: Value = prefs_json
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(json!({}));
    prefs[WEB_RESEARCH_PREF_KEY] = wr.clone();
    serde_json::to_string(&prefs)
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

pub(crate) fn build_create_settings_preset_request_from_route_payload(
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

pub(crate) fn build_update_settings_preset_request_from_route_payload(
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

pub struct SettingsService;

fn format_timestamp(value: NaiveDateTime) -> String {
    DateTime::<Utc>::from_naive_utc_and_offset(value, Utc).to_rfc3339()
}

fn env_string(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

fn trim_to_non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn map_settings_internal_error(detail: String) -> SettingsRouteError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"detail": detail})),
    )
}

fn map_settings_bad_request(detail: impl Into<String>) -> SettingsRouteError {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"detail": detail.into()})),
    )
}

fn normalize_api_key(key: Option<String>) -> Option<String> {
    key.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !is_placeholder(value))
}

fn normalize_non_empty_string(value: Option<&str>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn resolve_settings_provider(
    request: &SettingsUpdateRequest,
    existing_provider: Option<&str>,
) -> String {
    request
        .api_provider
        .as_deref()
        .or(request.provider_type.as_deref())
        .or(existing_provider)
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_ai_provider)
}

fn resolve_updated_api_key(request: &SettingsUpdateRequest, existing_api_key: &str) -> String {
    if request.clear_api_key {
        return String::new();
    }

    match request.api_key.as_deref() {
        Some(value) => {
            let trimmed = value.trim();
            if !trimmed.is_empty() && !is_placeholder(trimmed) {
                trimmed.to_string()
            } else {
                existing_api_key.to_string()
            }
        }
        None => existing_api_key.to_string(),
    }
}

fn resolve_updated_llm_model(
    request: &SettingsUpdateRequest,
    provider: &str,
    existing_llm_model: &str,
) -> String {
    if request.llm_model.is_some() {
        return normalize_non_empty_string(request.llm_model.as_deref())
            .unwrap_or_else(|| default_model_for_provider(provider));
    }

    if request.provider_switch_requested {
        return default_model_for_provider(provider);
    }

    existing_llm_model.to_string()
}

fn maybe_deactivate_changed_active_preset(
    prefs_json: Option<&str>,
    api_provider: &str,
    api_key: &str,
    api_base_url: &str,
    llm_model: &str,
    temperature: f64,
    max_tokens: i32,
) -> serde_json::Result<Option<String>> {
    let mut prefs: Value = prefs_json
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or(json!({}));

    let Some(presets) = prefs
        .get_mut(API_PRESETS_KEY)
        .and_then(|node| node.get_mut("presets"))
        .and_then(Value::as_array_mut)
    else {
        return Ok(None);
    };

    let Some(active_preset) = presets.iter_mut().find(|preset| {
        preset
            .get("is_active")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }) else {
        return Ok(None);
    };

    let preset_config = active_preset.get("config");
    let config_changed = preset_config
        .and_then(Value::as_object)
        .map(|config| {
            config.get("api_provider").and_then(Value::as_str) != Some(api_provider)
                || config.get("api_key").and_then(Value::as_str) != Some(api_key)
                || config.get("api_base_url").and_then(Value::as_str) != Some(api_base_url)
                || config.get("llm_model").and_then(Value::as_str) != Some(llm_model)
                || config.get("temperature").and_then(Value::as_f64) != Some(temperature)
                || config.get("max_tokens").and_then(Value::as_i64) != Some(max_tokens as i64)
        })
        .unwrap_or(true);

    if !config_changed {
        return Ok(None);
    }

    active_preset["is_active"] = json!(false);
    Ok(Some(serde_json::to_string(&prefs)?))
}

fn resolve_stored_model(value: &str, provider: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default_model_for_provider(provider)
    } else {
        trimmed.to_string()
    }
}

fn default_ai_provider() -> String {
    env_string("DEFAULT_AI_PROVIDER").unwrap_or_else(|| "openai".to_string())
}

fn default_model() -> String {
    env_string("DEFAULT_MODEL").unwrap_or_else(|| "gpt-4o-mini".to_string())
}

pub fn default_model_for_provider(provider: &str) -> String {
    match provider.trim().to_lowercase().as_str() {
        "anthropic" => "claude-3-5-sonnet-latest".to_string(),
        "gemini" => "gemini-2.5-pro".to_string(),
        _ => default_model(),
    }
}

fn default_runtime_model_for_provider(provider: &str) -> String {
    match provider {
        "anthropic" => "claude-3-5-sonnet-latest".to_string(),
        "gemini" => "gemini-2.5-flash".to_string(),
        _ => "gpt-4o-mini".to_string(),
    }
}

fn default_temperature() -> f64 {
    env::var("DEFAULT_TEMPERATURE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.7)
}

fn default_max_tokens() -> u32 {
    env_u32("DEFAULT_MAX_TOKENS", 32000)
}

fn env_api_key_for_provider(provider: &str) -> Option<String> {
    match provider {
        "anthropic" => normalize_api_key(env_string("ANTHROPIC_API_KEY")),
        "gemini" => normalize_api_key(env_string("GEMINI_API_KEY")),
        _ => normalize_api_key(env_string("OPENAI_API_KEY")),
    }
}

fn env_base_url_for_provider(provider: &str) -> Option<String> {
    match provider {
        "anthropic" => env_string("ANTHROPIC_BASE_URL"),
        "gemini" => env_string("GEMINI_BASE_URL"),
        _ => env_string("OPENAI_BASE_URL"),
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

fn resolve_effective_provider_base_url(provider: &str, raw_base_url: &str) -> String {
    match provider {
        "gemini" => normalize_gemini_base_url(raw_base_url),
        "anthropic" => normalize_anthropic_base_url(raw_base_url),
        _ => normalize_openai_compatible_base_url(raw_base_url),
    }
}

fn resolve_provider(
    explicit_provider: Option<&str>,
    stored_provider_type: &str,
    stored_provider: &str,
) -> String {
    explicit_provider
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let value = stored_provider_type.trim().to_lowercase();
            (!value.is_empty()).then_some(value)
        })
        .or_else(|| {
            let value = stored_provider.trim().to_lowercase();
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_else(default_ai_provider)
}

fn resolve_base_url(provider: &str, stored_base_url: &str) -> String {
    let candidate = {
        let trimmed = stored_base_url.trim();
        (!trimmed.is_empty())
            .then_some(trimmed.to_string())
            .or_else(|| env_base_url_for_provider(provider))
    };

    match provider {
        "anthropic" => candidate
            .unwrap_or_default()
            .trim_end_matches('/')
            .to_string(),
        _ => normalize_openai_compatible_base_url(
            candidate.as_deref().unwrap_or("https://api.openai.com/v1"),
        ),
    }
}

fn build_ai_config_from_settings(
    stored: &settings::Model,
    provider: String,
    model: String,
    temperature_override: Option<f64>,
    backup_urls: Vec<String>,
) -> Result<AIConfig, String> {
    let api_key = normalize_api_key(Some(stored.api_key.clone()))
        .or_else(|| env_api_key_for_provider(&provider))
        .unwrap_or_default();
    if api_key.is_empty() {
        return Err(format!(
            "current AI settings are missing a usable API key for provider {}",
            provider
        ));
    }

    let max_tokens = if stored.max_tokens > 0 {
        stored.max_tokens as u32
    } else {
        default_max_tokens()
    };

    Ok(AIConfig {
        base_url: resolve_base_url(&provider, &stored.api_base_url),
        provider,
        api_key,
        backup_urls,
        model,
        temperature: temperature_override.unwrap_or(stored.temperature),
        max_tokens,
        system_prompt: stored.system_prompt.clone(),
        prefer_normalized_v1_candidate: false,
        read_timeout_secs: None,
        transport_max_retries: None,
    })
}

fn allows_automatic_model_fallback(fallback_strategy: &str) -> bool {
    fallback_strategy.trim().eq_ignore_ascii_case("auto")
}

async fn load_or_create_settings_model(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<settings::Model, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(existing) = settings::Entity::find()
        .filter(settings::Column::UserId.eq(user_id))
        .one(db)
        .await?
    {
        return Ok(existing);
    }

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().naive_utc();
    let default_provider = default_ai_provider();
    let default_key = env_api_key_for_provider(&default_provider).unwrap_or_default();
    let default_base_url = resolve_base_url(&default_provider, "");
    let prefs_str = serde_json::to_string(&json!({
        "web_research": web_research_defaults()
    }))?;

    let model = settings::ActiveModel {
        id: Set(id),
        user_id: Set(user_id.to_string()),
        api_provider: Set(default_provider.clone()),
        api_key: Set(default_key),
        api_base_url: Set(default_base_url),
        api_backup_urls: Set(None),
        provider_type: Set(default_provider.clone()),
        fallback_strategy: Set("auto".into()),
        azure_api_version: Set(None),
        llm_model: Set(default_model_for_provider(&default_provider)),
        temperature: Set(default_temperature()),
        max_tokens: Set(default_max_tokens() as i32),
        system_prompt: Set(None),
        preferences: Set(Some(prefs_str)),
        created_at: Set(now),
        updated_at: Set(now),
    };

    Ok(model.insert(db).await?)
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

impl SettingsService {
    pub(crate) async fn resolve_effective_runtime_settings(
        db: &DatabaseConnection,
        user_id: &str,
        overrides: EffectiveSettingsOverrides,
    ) -> Result<ResolvedEffectiveSettings, SettingsRouteError> {
        let stored = load_settings_model(db, user_id)
            .await
            .map_err(map_settings_internal_error)?;

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
            return Err(map_settings_bad_request("API key is required"));
        }

        let stored_base = stored
            .as_ref()
            .map(|model| model.api_base_url.trim().to_string())
            .unwrap_or_default();
        let raw_base_url = trim_to_non_empty(overrides.api_base_url).unwrap_or(stored_base);
        let effective_base_url =
            resolve_effective_provider_base_url(&effective_provider, &raw_base_url);

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

    pub(crate) async fn resolve_web_research_settings(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?;

        Ok(existing
            .as_ref()
            .map(|saved| merge_web_research(saved.preferences.as_deref()))
            .unwrap_or_else(web_research_defaults))
    }

    pub async fn resolve_web_research_enabled(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self::resolve_web_research_settings(db, user_id)
            .await?
            .get("web_research_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub async fn get_or_create(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = load_or_create_settings_model(db, user_id).await?;
        let web_research = merge_web_research(settings.preferences.as_deref());
        let backup_urls = parse_api_backup_urls(settings.api_backup_urls.as_deref());
        Ok(build_response(&settings, &web_research, &backup_urls))
    }

    pub async fn create_or_update(
        db: &DatabaseConnection,
        user_id: &str,
        request: &SettingsUpdateRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?;

        match existing {
            Some(s) => {
                let current_prefs = s.preferences.clone().unwrap_or_default();
                let normalized_provider =
                    resolve_settings_provider(request, Some(s.api_provider.as_str()));
                let new_prefs = if request.web_research_patch.is_object()
                    && request
                        .web_research_patch
                        .as_object()
                        .map(|o| o.len())
                        .unwrap_or(0)
                        > 0
                {
                    Some(set_web_research(
                        Some(&current_prefs),
                        &request.web_research_patch,
                    )?)
                } else {
                    None
                };

                let backup_urls = match &request.api_backup_urls {
                    SettingsApiBackupUrlsField::Provided(urls) => urls.clone(),
                    SettingsApiBackupUrlsField::Missing | SettingsApiBackupUrlsField::Invalid => {
                        parse_api_backup_urls(s.api_backup_urls.as_deref())
                    }
                };

                let final_api_key = resolve_updated_api_key(request, &s.api_key);
                let final_api_base_url = request
                    .api_base_url
                    .as_deref()
                    .map(|value| value.trim().to_string())
                    .unwrap_or_else(|| s.api_base_url.clone());
                let final_llm_model =
                    resolve_updated_llm_model(request, &normalized_provider, &s.llm_model);
                let final_temperature = request.temperature.unwrap_or(s.temperature);
                let final_max_tokens = request.max_tokens.unwrap_or(s.max_tokens as i64) as i32;

                let mut final_preferences = request
                    .preferences
                    .clone()
                    .or_else(|| s.preferences.clone());
                if let Some(prefs) = new_prefs.clone() {
                    final_preferences = Some(prefs);
                }
                if let Some(updated_prefs) = maybe_deactivate_changed_active_preset(
                    final_preferences.as_deref(),
                    &normalized_provider,
                    &final_api_key,
                    &final_api_base_url,
                    &final_llm_model,
                    final_temperature,
                    final_max_tokens,
                )? {
                    final_preferences = Some(updated_prefs);
                }

                let mut active: settings::ActiveModel = s.into();
                active.api_provider = Set(normalized_provider.clone());
                active.provider_type = Set(normalized_provider.clone());
                active.api_key = Set(final_api_key);
                active.api_base_url = Set(final_api_base_url);
                active.api_backup_urls = Set(serialize_api_backup_urls(&backup_urls));
                if let Some(v) = request.fallback_strategy.as_ref() {
                    active.fallback_strategy = Set(v.clone());
                }
                if let Some(v) = request.azure_api_version.as_ref() {
                    active.azure_api_version = Set(Some(v.clone()));
                }
                active.llm_model = Set(final_llm_model);
                active.temperature = Set(final_temperature);
                active.max_tokens = Set(final_max_tokens);
                if let Some(v) = request.system_prompt.as_ref() {
                    active.system_prompt = Set(Some(v.clone()));
                }
                active.preferences = Set(final_preferences);
                active.updated_at = Set(Utc::now().naive_utc());

                let saved = active.update(db).await?;
                let web_research = merge_web_research(saved.preferences.as_deref());
                let backup_urls = parse_api_backup_urls(saved.api_backup_urls.as_deref());
                Ok(build_response(&saved, &web_research, &backup_urls))
            }
            None => {
                let id = Uuid::new_v4().to_string();
                let now = Utc::now().naive_utc();
                let default_prefs =
                    serde_json::to_string(&json!({"web_research": web_research_defaults()}))?;
                let normalized_provider = resolve_settings_provider(request, None);
                let prefs = if request.web_research_patch.is_object()
                    && request
                        .web_research_patch
                        .as_object()
                        .map(|o| o.len())
                        .unwrap_or(0)
                        > 0
                {
                    Some(set_web_research(
                        Some(&default_prefs),
                        &request.web_research_patch,
                    )?)
                } else {
                    Some(default_prefs)
                };

                let backup_urls: Vec<String> = match &request.api_backup_urls {
                    SettingsApiBackupUrlsField::Provided(urls) => urls.clone(),
                    SettingsApiBackupUrlsField::Missing | SettingsApiBackupUrlsField::Invalid => {
                        Vec::new()
                    }
                };
                let api_key =
                    normalize_api_key(request.api_key.as_ref().map(|v| v.trim().to_string()))
                        .unwrap_or_default();
                let llm_model = normalize_non_empty_string(request.llm_model.as_deref())
                    .unwrap_or_else(|| default_model_for_provider(&normalized_provider));

                let model = settings::ActiveModel {
                    id: Set(id.clone()),
                    user_id: Set(user_id.to_string()),
                    api_provider: Set(normalized_provider.clone()),
                    api_key: Set(api_key),
                    api_base_url: Set(request
                        .api_base_url
                        .as_deref()
                        .map(|value| value.trim().to_string())
                        .unwrap_or_else(|| resolve_base_url(&normalized_provider, ""))),
                    api_backup_urls: Set(serialize_api_backup_urls(&backup_urls)),
                    provider_type: Set(normalized_provider.clone()),
                    fallback_strategy: Set(request
                        .fallback_strategy
                        .as_deref()
                        .unwrap_or("auto")
                        .to_string()),
                    azure_api_version: Set(request.azure_api_version.clone()),
                    llm_model: Set(llm_model),
                    temperature: Set(request.temperature.unwrap_or(default_temperature())),
                    max_tokens: Set(
                        request.max_tokens.unwrap_or(default_max_tokens() as i64) as i32
                    ),
                    system_prompt: Set(request.system_prompt.clone()),
                    preferences: Set(prefs.clone()),
                    created_at: Set(now),
                    updated_at: Set(now),
                };

                let saved = model.insert(db).await?;
                let web_research = merge_web_research(prefs.as_deref());
                let backup_urls = parse_api_backup_urls(saved.api_backup_urls.as_deref());
                Ok(build_response(&saved, &web_research, &backup_urls))
            }
        }
    }

    pub async fn update_existing(
        db: &DatabaseConnection,
        user_id: &str,
        request: &SettingsUpdateRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?;

        if existing.is_none() {
            return Err(SETTINGS_UPDATE_MISSING_DETAIL.into());
        }

        Self::update_existing_only(db, user_id, request).await
    }

    async fn update_existing_only(
        db: &DatabaseConnection,
        user_id: &str,
        request: &SettingsUpdateRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?;

        match existing {
            Some(s) => {
                let current_prefs = s.preferences.clone().unwrap_or_default();
                let normalized_provider =
                    resolve_settings_provider(request, Some(s.api_provider.as_str()));
                let new_prefs = if request.web_research_patch.is_object()
                    && request
                        .web_research_patch
                        .as_object()
                        .map(|o| o.len())
                        .unwrap_or(0)
                        > 0
                {
                    Some(set_web_research(
                        Some(&current_prefs),
                        &request.web_research_patch,
                    )?)
                } else {
                    None
                };

                let backup_urls = match &request.api_backup_urls {
                    SettingsApiBackupUrlsField::Provided(urls) => urls.clone(),
                    SettingsApiBackupUrlsField::Missing | SettingsApiBackupUrlsField::Invalid => {
                        parse_api_backup_urls(s.api_backup_urls.as_deref())
                    }
                };

                let mut active: settings::ActiveModel = s.clone().into();
                active.api_provider = Set(normalized_provider.clone());
                active.provider_type = Set(normalized_provider.clone());
                active.api_key = Set(resolve_updated_api_key(request, &s.api_key));
                if let Some(value) = request.api_base_url.as_deref() {
                    active.api_base_url = Set(value.trim().to_string());
                }
                active.api_backup_urls = Set(serialize_api_backup_urls(&backup_urls));
                if let Some(v) = request.fallback_strategy.as_ref() {
                    active.fallback_strategy = Set(v.clone());
                }
                if let Some(v) = request.azure_api_version.as_ref() {
                    active.azure_api_version = Set(Some(v.clone()));
                }
                active.llm_model = Set(resolve_updated_llm_model(
                    request,
                    &normalized_provider,
                    &s.llm_model,
                ));
                if let Some(v) = request.temperature {
                    active.temperature = Set(v);
                }
                if let Some(v) = request.max_tokens {
                    active.max_tokens = Set(v as i32);
                }
                if let Some(v) = request.system_prompt.as_ref() {
                    active.system_prompt = Set(Some(v.clone()));
                }

                let mut final_preferences = request.preferences.clone();
                if let Some(prefs) = new_prefs {
                    final_preferences = Some(prefs);
                }
                if let Some(prefs) = final_preferences {
                    active.preferences = Set(Some(prefs));
                }

                active.updated_at = Set(Utc::now().naive_utc());

                let saved = active.update(db).await?;
                let web_research = merge_web_research(saved.preferences.as_deref());
                let backup_urls = parse_api_backup_urls(saved.api_backup_urls.as_deref());
                Ok(build_response(&saved, &web_research, &backup_urls))
            }
            None => Err(SETTINGS_UPDATE_MISSING_DETAIL.into()),
        }
    }

    pub async fn load_role_model_policy(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<RoleModelPolicyV1, String> {
        let stored = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|error| format!("failed to load settings: {error}"))?
            .ok_or_else(|| "settings not found".to_owned())?;

        read_role_model_policy(stored.preferences.as_deref())
            .map_err(|error| format!("failed to load role model policy: {error}"))
    }

    pub async fn build_ai_config(
        db: &DatabaseConnection,
        user_id: &str,
        provider_override: Option<&str>,
        model_override: Option<&str>,
        temperature_override: Option<f64>,
    ) -> Result<AIConfig, String> {
        let stored = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|error| format!("failed to load settings: {error}"))?
            .ok_or_else(|| "settings not found".to_owned())?;

        let provider = resolve_provider(
            provider_override,
            &stored.provider_type,
            &stored.api_provider,
        );
        let model = model_override
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| resolve_stored_model(&stored.llm_model, &provider));

        build_ai_config_from_settings(&stored, provider, model, temperature_override, Vec::new())
    }

    pub async fn build_role_aware_ai_config(
        db: &DatabaseConnection,
        user_id: &str,
        intent_kind: GenerationIntentKind,
        provider_override: Option<&str>,
        model_override: Option<&str>,
        temperature_override: Option<f64>,
    ) -> Result<RoleAwareAIConfig, String> {
        let stored = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|error| format!("failed to load settings: {error}"))?
            .ok_or_else(|| "settings not found".to_owned())?;
        let policy = read_role_model_policy(stored.preferences.as_deref())
            .map_err(|error| format!("failed to load role model policy: {error}"))?;
        let global_provider = resolve_provider(None, &stored.provider_type, &stored.api_provider);
        let runtime_default_provider = default_ai_provider();
        let resolved_policy = resolve_role_model_policy(
            RoleModelResolutionInput {
                intent_kind,
                policy: &policy,
                route_provider: provider_override,
                route_model: model_override,
                global_provider: Some(&global_provider),
                global_model: Some(&stored.llm_model),
                runtime_default_provider: &runtime_default_provider,
            },
            default_runtime_model_for_provider,
        )
        .map_err(|error| format!("failed to resolve role model policy: {error}"))?;
        let backup_urls = parse_api_backup_urls(stored.api_backup_urls.as_deref());
        let allow_model_fallback = allows_automatic_model_fallback(&stored.fallback_strategy);
        let ai_config = build_ai_config_from_settings(
            &stored,
            resolved_policy.resolved_provider.clone(),
            resolved_policy.resolved_model.clone(),
            temperature_override,
            backup_urls,
        )?;

        Ok(RoleAwareAIConfig {
            ai_config,
            resolved_policy,
            allow_model_fallback,
        })
    }

    pub async fn delete_existing(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?;

        match existing {
            Some(s) => {
                settings::Entity::delete_by_id(&s.id).exec(db).await?;
                Ok(json!({"message": "settings deleted", "user_id": user_id}))
            }
            None => Err(SETTINGS_DELETE_MISSING_DETAIL.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{
        ColumnTrait, ConnectionTrait, Database, DbBackend, EntityTrait, QueryFilter, Schema,
    };
    use serde_json::{json, Value};

    use super::*;
    use crate::services::role_model_policy_service::{GenerationRole, ModelSelectionSource};

    async fn setup_settings_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(settings::Entity)))
            .await
            .expect("create settings table");
        db
    }

    async fn load_settings_for_user(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Option<settings::Model> {
        settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await
            .expect("load settings")
    }

    async fn insert_runtime_settings(
        db: &DatabaseConnection,
        user_id: &str,
        provider: &str,
        model: &str,
        preferences: Option<Value>,
        backup_urls: &[&str],
        fallback_strategy: &str,
    ) {
        let now = Utc::now().naive_utc();
        settings::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            api_provider: Set(provider.to_owned()),
            api_key: Set("sk-settings-test-key".to_owned()),
            api_base_url: Set("https://settings.example.test/v1".to_owned()),
            api_backup_urls: Set(serialize_api_backup_urls(
                &backup_urls
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect::<Vec<_>>(),
            )),
            provider_type: Set(provider.to_owned()),
            fallback_strategy: Set(fallback_strategy.to_owned()),
            azure_api_version: Set(None),
            llm_model: Set(model.to_owned()),
            temperature: Set(0.55),
            max_tokens: Set(8192),
            system_prompt: Set(Some("stored system prompt".to_owned())),
            preferences: Set(preferences.map(|value| value.to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .expect("insert runtime settings");
    }

    #[tokio::test]
    async fn build_ai_config_preserves_legacy_backup_url_behavior() {
        let db = setup_settings_db().await;
        insert_runtime_settings(
            &db,
            "legacy-user",
            "openai",
            "stored-model",
            None,
            &["https://backup.example.test/v1"],
            "manual",
        )
        .await;

        let config = SettingsService::build_ai_config(&db, "legacy-user", None, None, Some(0.25))
            .await
            .expect("build legacy ai config");

        assert_eq!(config.provider, "openai");
        assert_eq!(config.model, "stored-model");
        assert_eq!(config.temperature, 0.25);
        assert_eq!(config.max_tokens, 8192);
        assert!(config.backup_urls.is_empty());
    }

    #[tokio::test]
    async fn role_aware_ai_config_empty_policy_inherits_global_settings() {
        let db = setup_settings_db().await;
        insert_runtime_settings(
            &db,
            "default-policy-user",
            "openai",
            "global-model",
            None,
            &["https://backup.example.test/v1"],
            "auto",
        )
        .await;

        let prepared = SettingsService::build_role_aware_ai_config(
            &db,
            "default-policy-user",
            GenerationIntentKind::ChapterGenerate,
            None,
            None,
            None,
        )
        .await
        .expect("build role-aware config");

        assert_eq!(prepared.resolved_policy.role, GenerationRole::Writer);
        assert_eq!(prepared.resolved_policy.resolved_provider, "openai");
        assert_eq!(prepared.resolved_policy.resolved_model, "global-model");
        assert_eq!(
            prepared.resolved_policy.provider_source,
            ModelSelectionSource::GlobalSettings
        );
        assert_eq!(
            prepared.resolved_policy.model_source,
            ModelSelectionSource::GlobalSettings
        );
        assert_eq!(prepared.ai_config.provider, "openai");
        assert_eq!(prepared.ai_config.model, "global-model");
        assert_eq!(
            prepared.ai_config.backup_urls,
            vec!["https://backup.example.test/v1".to_owned()]
        );
        assert!(prepared.allow_model_fallback);
    }

    #[tokio::test]
    async fn role_aware_ai_config_applies_role_override_and_manual_fallback() {
        let db = setup_settings_db().await;
        let preferences = json!({
            "role_model_policy": {
                "schema_version": "role-model-policy/v1",
                "roles": {
                    "writer": {
                        "provider": "gemini",
                        "model": "gemini-writer-model"
                    }
                }
            },
            "unknown_top_level": {"preserved": true}
        });
        insert_runtime_settings(
            &db,
            "role-override-user",
            "openai",
            "global-openai-model",
            Some(preferences),
            &["https://backup.example.test/v1"],
            "manual",
        )
        .await;

        let prepared = SettingsService::build_role_aware_ai_config(
            &db,
            "role-override-user",
            GenerationIntentKind::BatchChapterGenerate,
            None,
            None,
            None,
        )
        .await
        .expect("build role override config");

        assert_eq!(prepared.ai_config.provider, "gemini");
        assert_eq!(prepared.ai_config.model, "gemini-writer-model");
        assert_eq!(
            prepared.resolved_policy.provider_source,
            ModelSelectionSource::RoleOverride
        );
        assert_eq!(
            prepared.resolved_policy.model_source,
            ModelSelectionSource::RoleOverride
        );
        assert!(!prepared.allow_model_fallback);
    }

    #[tokio::test]
    async fn role_aware_ai_config_route_override_wins_over_role_policy() {
        let db = setup_settings_db().await;
        let preferences = json!({
            "role_model_policy": {
                "schema_version": "role-model-policy/v1",
                "roles": {
                    "writer": {
                        "provider": "anthropic",
                        "model": "role-model"
                    }
                }
            }
        });
        insert_runtime_settings(
            &db,
            "route-override-user",
            "openai",
            "global-model",
            Some(preferences),
            &[],
            "auto",
        )
        .await;

        let prepared = SettingsService::build_role_aware_ai_config(
            &db,
            "route-override-user",
            GenerationIntentKind::ChapterRegenerate,
            Some(" Gemini "),
            Some(" route-model "),
            Some(0.4),
        )
        .await
        .expect("build route override config");

        assert_eq!(prepared.ai_config.provider, "gemini");
        assert_eq!(prepared.ai_config.model, "route-model");
        assert_eq!(prepared.ai_config.temperature, 0.4);
        assert_eq!(
            prepared.resolved_policy.provider_source,
            ModelSelectionSource::RouteOverride
        );
        assert_eq!(
            prepared.resolved_policy.model_source,
            ModelSelectionSource::RouteOverride
        );
        assert_eq!(
            prepared.resolved_policy.requested_provider.as_deref(),
            Some("gemini")
        );
        assert_eq!(
            prepared.resolved_policy.requested_model.as_deref(),
            Some("route-model")
        );
    }

    #[tokio::test]
    async fn role_aware_ai_config_uses_target_provider_runtime_default_model() {
        let db = setup_settings_db().await;
        let preferences = json!({
            "role_model_policy": {
                "schema_version": "role-model-policy/v1",
                "roles": {
                    "writer": {
                        "provider": "gemini"
                    }
                }
            }
        });
        insert_runtime_settings(
            &db,
            "provider-default-user",
            "openai",
            "global-openai-model",
            Some(preferences),
            &[],
            "auto",
        )
        .await;

        let prepared = SettingsService::build_role_aware_ai_config(
            &db,
            "provider-default-user",
            GenerationIntentKind::ChapterRepair,
            None,
            None,
            None,
        )
        .await
        .expect("build provider default config");

        assert_eq!(prepared.ai_config.provider, "gemini");
        assert_eq!(prepared.ai_config.model, "gemini-2.5-flash");
        assert_eq!(
            prepared.resolved_policy.model_source,
            ModelSelectionSource::ProviderDefault
        );
    }

    #[tokio::test]
    async fn list_presets_auto_creates_settings_when_missing() {
        let db = setup_settings_db().await;

        let response = SettingsService::list_presets(&db, "user-1")
            .await
            .expect("list presets");

        assert_eq!(response["presets"], json!([]));
        assert_eq!(response["total"], json!(0));
        assert_eq!(response["active_preset_id"], Value::Null);

        let saved = load_settings_for_user(&db, "user-1").await;
        assert!(saved.is_some());
    }

    #[tokio::test]
    async fn create_preset_auto_creates_settings_when_missing() {
        let db = setup_settings_db().await;
        let request = build_create_settings_preset_request_from_route_payload(&json!({
            "name": "Primary Preset",
            "description": "preset description",
            "config": {
                "api_provider": "openai",
                "api_key": "sk-test",
                "api_base_url": "https://api.openai.com/v1",
                "llm_model": "gpt-4o-mini",
                "temperature": 0.4,
                "max_tokens": 1024
            }
        }));

        let created = SettingsService::create_preset(&db, "user-1", &request)
            .await
            .expect("create preset");

        assert_eq!(created["name"], json!("Primary Preset"));
        assert_eq!(created["is_active"], json!(false));

        let listed = SettingsService::list_presets(&db, "user-1")
            .await
            .expect("list presets");
        assert_eq!(listed["total"], json!(1));
        assert_eq!(listed["presets"][0]["name"], json!("Primary Preset"));
    }

    #[tokio::test]
    async fn delete_preset_rejects_active_preset() {
        let db = setup_settings_db().await;
        let request = build_create_settings_preset_request_from_route_payload(&json!({
            "name": "Active Preset",
            "description": "preset description",
            "config": {
                "api_provider": "openai",
                "api_key": "sk-test",
                "api_base_url": "https://api.openai.com/v1",
                "llm_model": "gpt-4o-mini",
                "temperature": 0.4,
                "max_tokens": 1024
            }
        }));

        let created = SettingsService::create_preset(&db, "user-1", &request)
            .await
            .expect("create preset");
        let preset_id = created["id"]
            .as_str()
            .expect("preset id should be string")
            .to_string();

        SettingsService::activate_preset(&db, "user-1", &preset_id)
            .await
            .expect("activate preset");

        let error = SettingsService::delete_preset(&db, "user-1", &preset_id)
            .await
            .expect_err("active preset delete should fail");

        assert!(
            format!("{}", error).contains("无法删除激活中的预设，请先激活其他预设"),
            "unexpected error: {}",
            error
        );
    }

    #[tokio::test]
    async fn create_or_update_deactivates_changed_active_preset() {
        let db = setup_settings_db().await;
        let seed_request = build_settings_update_request_from_route_body(&json!({
            "api_provider": "openai",
            "api_key": "sk-original",
            "api_base_url": "https://api.openai.com/v1",
            "llm_model": "gpt-4o-mini",
            "temperature": 0.4,
            "max_tokens": 1024
        }));
        SettingsService::create_or_update(&db, "user-1", &seed_request)
            .await
            .expect("seed settings");

        let preset_request = build_create_settings_preset_request_from_route_payload(&json!({
            "name": "Active Preset",
            "description": "preset description",
            "config": {
                "api_provider": "openai",
                "api_key": "sk-original",
                "api_base_url": "https://api.openai.com/v1",
                "llm_model": "gpt-4o-mini",
                "temperature": 0.4,
                "max_tokens": 1024
            }
        }));
        let created = SettingsService::create_preset(&db, "user-1", &preset_request)
            .await
            .expect("create preset");
        let preset_id = created["id"].as_str().expect("preset id").to_string();
        SettingsService::activate_preset(&db, "user-1", &preset_id)
            .await
            .expect("activate preset");

        let update_request = build_settings_update_request_from_route_body(&json!({
            "llm_model": "gpt-4.1",
            "api_key": "sk-updated"
        }));
        let updated = SettingsService::create_or_update(&db, "user-1", &update_request)
            .await
            .expect("update settings");

        assert_eq!(updated["llm_model"], json!("gpt-4.1"));
        let saved = load_settings_for_user(&db, "user-1")
            .await
            .expect("saved settings");
        let prefs: Value = serde_json::from_str(saved.preferences.as_deref().unwrap_or("{}"))
            .expect("parse prefs");
        assert_eq!(
            prefs["api_presets"]["presets"][0]["is_active"],
            json!(false)
        );
    }

    #[tokio::test]
    async fn update_existing_rejects_missing_settings() {
        let db = setup_settings_db().await;
        let request = build_settings_update_request_from_route_body(&json!({
            "llm_model": "gpt-4.1"
        }));

        let error = SettingsService::update_existing(&db, "user-1", &request)
            .await
            .expect_err("missing settings should fail");

        assert!(
            format!("{}", error).contains(SETTINGS_UPDATE_MISSING_DETAIL),
            "unexpected error: {}",
            error
        );
    }

    #[tokio::test]
    async fn delete_existing_rejects_missing_settings() {
        let db = setup_settings_db().await;

        let error = SettingsService::delete_existing(&db, "user-1")
            .await
            .expect_err("missing settings delete should fail");

        assert!(
            format!("{}", error).contains(SETTINGS_DELETE_MISSING_DETAIL),
            "unexpected error: {}",
            error
        );
    }

    #[tokio::test]
    async fn create_or_update_normalizes_provider_fields_and_default_model() {
        let db = setup_settings_db().await;
        let request = build_settings_update_request_from_route_body(&json!({
            "provider_type": "Anthropic",
            "api_provider": "OpenAI",
            "api_key": "sk-provider",
            "api_base_url": "https://provider.example.com",
            "llm_model": "   "
        }));

        let response = SettingsService::create_or_update(&db, "user-1", &request)
            .await
            .expect("create settings");

        assert_eq!(response["api_provider"], json!("openai"));
        assert_eq!(response["provider_type"], json!("openai"));
        assert_eq!(
            response["llm_model"],
            json!(default_model_for_provider("openai"))
        );

        let saved = load_settings_for_user(&db, "user-1")
            .await
            .expect("saved settings");
        assert_eq!(saved.api_provider, "openai");
        assert_eq!(saved.provider_type, "openai");
        assert_eq!(saved.llm_model, default_model_for_provider("openai"));
    }

    #[tokio::test]
    async fn activate_preset_only_applies_python_owned_main_fields() {
        let db = setup_settings_db().await;
        let seed_request = build_settings_update_request_from_route_body(&json!({
            "api_provider": "openai",
            "provider_type": "openai",
            "api_key": "sk-before",
            "api_base_url": "https://before.example.com/v1",
            "api_backup_urls": ["https://before-backup.example.com/v1"],
            "fallback_strategy": "manual",
            "azure_api_version": "2024-10-21",
            "llm_model": "before-model",
            "temperature": 0.9,
            "max_tokens": 512,
            "system_prompt": "before-prompt"
        }));
        SettingsService::create_or_update(&db, "user-1", &seed_request)
            .await
            .expect("seed settings");

        let preset_request = build_create_settings_preset_request_from_route_payload(&json!({
            "name": "Preset With Extra Config",
            "config": {
                "api_provider": "anthropic",
                "api_key": "",
                "api_base_url": "https://preset.example.com/v1",
                "api_backup_urls": ["https://preset-backup.example.com/v1"],
                "provider_type": "anthropic",
                "fallback_strategy": "auto",
                "azure_api_version": "2025-01-01",
                "llm_model": "",
                "temperature": 0.2,
                "max_tokens": 333,
                "system_prompt": null
            }
        }));
        let created = SettingsService::create_preset(&db, "user-1", &preset_request)
            .await
            .expect("create preset");
        let preset_id = created["id"].as_str().expect("preset id").to_string();

        SettingsService::activate_preset(&db, "user-1", &preset_id)
            .await
            .expect("activate preset");

        let saved = load_settings_for_user(&db, "user-1")
            .await
            .expect("saved settings");
        assert_eq!(saved.api_provider, "anthropic");
        assert_eq!(saved.api_key, "");
        assert_eq!(saved.api_base_url, "https://preset.example.com/v1");
        assert_eq!(saved.llm_model, "");
        assert_eq!(saved.temperature, 0.2);
        assert_eq!(saved.max_tokens, 333);
        assert_eq!(saved.system_prompt, None);

        assert_eq!(
            parse_api_backup_urls(saved.api_backup_urls.as_deref()),
            vec!["https://before-backup.example.com/v1".to_string()]
        );
        assert_eq!(saved.provider_type, "openai");
        assert_eq!(saved.fallback_strategy, "manual");
        assert_eq!(saved.azure_api_version.as_deref(), Some("2024-10-21"));
    }

    #[tokio::test]
    async fn create_preset_from_current_uses_python_snapshot_shape() {
        let db = setup_settings_db().await;

        let seeded = settings::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set("user-1".to_string()),
            api_provider: Set("anthropic".to_string()),
            api_key: Set("sk-current".to_string()),
            api_base_url: Set("https://current.example.com/v1".to_string()),
            api_backup_urls: Set(Some(
                serde_json::to_string(&vec!["https://backup.example.com/v1"]).expect("backup urls"),
            )),
            provider_type: Set("anthropic".to_string()),
            fallback_strategy: Set("manual".to_string()),
            azure_api_version: Set(Some("2024-10-21".to_string())),
            llm_model: Set(String::new()),
            temperature: Set(0.5),
            max_tokens: Set(2048),
            system_prompt: Set(Some("current-prompt".to_string())),
            preferences: Set(Some("{}".to_string())),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        };
        seeded.insert(&db).await.expect("insert settings");

        let created = SettingsService::create_preset_from_current(
            &db,
            "user-1",
            "Snapshot Preset",
            Some("snapshot description"),
        )
        .await
        .expect("create preset from current");

        assert_eq!(created["name"], json!("Snapshot Preset"));
        assert_eq!(created["description"], json!("snapshot description"));
        assert_eq!(created["config"]["api_provider"], json!("anthropic"));
        assert_eq!(created["config"]["api_key"], json!("sk-current"));
        assert_eq!(
            created["config"]["api_base_url"],
            json!("https://current.example.com/v1")
        );
        assert_eq!(created["config"]["api_backup_urls"], Value::Null);
        assert_eq!(created["config"]["provider_type"], json!("openai"));
        assert_eq!(created["config"]["fallback_strategy"], json!("auto"));
        assert_eq!(created["config"]["azure_api_version"], Value::Null);
        assert_eq!(created["config"]["llm_model"], json!(""));
        assert_eq!(created["config"]["temperature"], json!(0.5));
        assert_eq!(created["config"]["max_tokens"], json!(2048));
        assert_eq!(created["config"]["system_prompt"], json!("current-prompt"));
    }

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
    fn runtime_default_model_mapping_keeps_runtime_probe_defaults() {
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

fn build_response(saved: &settings::Model, web_research: &Value, backup_urls: &[String]) -> Value {
    json!({
        "id": saved.id,
        "user_id": saved.user_id,
        "api_provider": saved.api_provider,
        "api_key": mask_api_key(&saved.api_key),
        "has_api_key": normalize_api_key(Some(saved.api_key.clone())).is_some(),
        "api_base_url": saved.api_base_url,
        "api_backup_urls": backup_urls,
        "provider_type": saved.provider_type,
        "fallback_strategy": saved.fallback_strategy,
        "azure_api_version": saved.azure_api_version,
        "llm_model": resolve_stored_model(&saved.llm_model, &saved.provider_type),
        "temperature": saved.temperature,
        "max_tokens": saved.max_tokens,
        "system_prompt": saved.system_prompt,
        "web_research_enabled": web_research["web_research_enabled"],
        "web_research_exa_enabled": web_research["web_research_exa_enabled"],
        "web_research_grok_enabled": web_research["web_research_grok_enabled"],
        "web_research_exa_api_key": web_research["web_research_exa_api_key"],
        "web_research_exa_base_url": web_research["web_research_exa_base_url"],
        "web_research_grok_api_key": web_research["web_research_grok_api_key"],
        "web_research_grok_base_url": web_research["web_research_grok_base_url"],
        "web_research_grok_model": web_research["web_research_grok_model"],
        "web_research_grok_search_enabled": web_research["web_research_grok_search_enabled"],
        "preferences": saved.preferences,
        "created_at": format_timestamp(saved.created_at),
        "updated_at": format_timestamp(saved.updated_at),
    })
}

// ========== API Presets (stored in preferences JSON) ==========

const API_PRESETS_KEY: &str = "api_presets";

pub(crate) fn get_api_presets(prefs_json: Option<&str>) -> (Vec<Value>, String) {
    let prefs: Value = prefs_json
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(json!({}));
    let api_presets = prefs
        .get(API_PRESETS_KEY)
        .and_then(|ap| ap.get("presets"))
        .and_then(|p| p.as_array().cloned())
        .unwrap_or_default();
    let version = prefs
        .get(API_PRESETS_KEY)
        .and_then(|ap| ap.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("1.0")
        .to_string();
    (api_presets, version)
}

fn set_api_presets(prefs_json: Option<&str>, presets: &[Value]) -> serde_json::Result<String> {
    let mut prefs: Value = prefs_json
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(json!({}));
    prefs[API_PRESETS_KEY] = json!({"presets": presets, "version": "1.0"});
    serde_json::to_string(&prefs)
}

impl SettingsService {
    pub async fn list_presets(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = load_or_create_settings_model(db, user_id).await?;

        let (presets, _version) = get_api_presets(settings.preferences.as_deref());
        let active_preset_id = presets
            .iter()
            .find(|p| {
                p.get("is_active")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .and_then(|p| p.get("id").and_then(|v| v.as_str()))
            .map(String::from);

        Ok(json!({
            "presets": presets,
            "total": presets.len(),
            "active_preset_id": active_preset_id,
        }))
    }

    pub async fn create_preset(
        db: &DatabaseConnection,
        user_id: &str,
        request: &CreateSettingsPresetRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = load_or_create_settings_model(db, user_id).await?;

        let (mut presets, _version) = get_api_presets(settings.preferences.as_deref());

        let now = Utc::now();
        let new_preset = json!({
            "id": format!("preset_{}", now.timestamp_millis()),
            "name": request.name(),
            "description": request.description().cloned().unwrap_or(Value::Null),
            "is_active": false,
            "created_at": now.to_rfc3339(),
            "config": request.config().clone(),
        });

        presets.push(new_preset.clone());
        let new_prefs = set_api_presets(settings.preferences.as_deref(), &presets)?;

        let mut active: settings::ActiveModel = settings.into();
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now().naive_utc());
        active.update(db).await?;

        Ok(new_preset)
    }

    pub async fn update_preset(
        db: &DatabaseConnection,
        user_id: &str,
        preset_id: &str,
        request: &UpdateSettingsPresetRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = load_or_create_settings_model(db, user_id).await?;

        let (mut presets, _version) = get_api_presets(settings.preferences.as_deref());

        let idx = presets
            .iter()
            .position(|p| p.get("id").and_then(|v| v.as_str()) == Some(preset_id))
            .ok_or("preset not found")?;

        let target = &mut presets[idx];
        if let Some(v) = request.name() {
            target["name"] = json!(v);
        }
        if request.has_description() {
            target["description"] = request.description().cloned().unwrap_or(Value::Null);
        }
        if let Some(config) = request.config() {
            target["config"] = config.clone();
        }

        let result = target.clone();
        let new_prefs = set_api_presets(settings.preferences.as_deref(), &presets)?;

        let mut active: settings::ActiveModel = settings.into();
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now().naive_utc());
        active.update(db).await?;

        Ok(result)
    }

    pub async fn delete_preset(
        db: &DatabaseConnection,
        user_id: &str,
        preset_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = load_or_create_settings_model(db, user_id).await?;

        let (mut presets, _version) = get_api_presets(settings.preferences.as_deref());
        let target_preset = presets
            .iter()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(preset_id))
            .ok_or("preset not found")?;
        let is_active = target_preset
            .get("is_active")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_active {
            return Err("无法删除激活中的预设，请先激活其他预设".into());
        }

        presets.retain(|p| p.get("id").and_then(|v| v.as_str()) != Some(preset_id));
        let new_prefs = set_api_presets(settings.preferences.as_deref(), &presets)?;

        let mut active: settings::ActiveModel = settings.into();
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now().naive_utc());
        active.update(db).await?;

        Ok(json!({"message": "预设已删除", "preset_id": preset_id}))
    }

    pub async fn activate_preset(
        db: &DatabaseConnection,
        user_id: &str,
        preset_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = load_or_create_settings_model(db, user_id).await?;

        let (mut presets, _version) = get_api_presets(settings.preferences.as_deref());

        let mut activated: Option<Value> = None;
        for p in &mut presets {
            if p.get("id").and_then(|v| v.as_str()) == Some(preset_id) {
                p["is_active"] = json!(true);
                activated = Some(p.clone());
            } else {
                p["is_active"] = json!(false);
            }
        }

        let result = activated.ok_or("preset not found")?;
        let new_prefs = set_api_presets(settings.preferences.as_deref(), &presets)?;

        // Apply preset config to settings
        let config = result.get("config");
        let mut active: settings::ActiveModel = settings.into();
        if let Some(cfg) = config {
            if let Some(v) = cfg.get("api_provider").and_then(|v| v.as_str()) {
                active.api_provider = Set(v.to_string());
            }
            if let Some(v) = cfg.get("api_key").and_then(|v| v.as_str()) {
                active.api_key = Set(v.to_string());
            }
            if let Some(value) = cfg.get("api_base_url") {
                active.api_base_url = Set(value.as_str().unwrap_or("").to_string());
            }
            if let Some(value) = cfg.get("llm_model") {
                active.llm_model = Set(value.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = cfg.get("temperature").and_then(|v| v.as_f64()) {
                active.temperature = Set(v);
            }
            if let Some(v) = cfg.get("max_tokens").and_then(|v| v.as_i64()) {
                active.max_tokens = Set(v as i32);
            }
            if let Some(value) = cfg.get("system_prompt") {
                active.system_prompt = Set(value.as_str().map(ToString::to_string));
            }
        }
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now().naive_utc());
        active.update(db).await?;

        Ok(json!({
            "message": "预设已激活",
            "preset_id": preset_id,
            "preset_name": result.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        }))
    }

    pub async fn create_preset_from_current(
        db: &DatabaseConnection,
        user_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = load_or_create_settings_model(db, user_id).await?;

        let config = json!({
            "api_provider": settings.api_provider,
            "api_key": settings.api_key,
            "api_base_url": settings.api_base_url,
            "api_backup_urls": Value::Null,
            "provider_type": "openai",
            "fallback_strategy": "auto",
            "azure_api_version": Value::Null,
            "llm_model": settings.llm_model,
            "temperature": settings.temperature,
            "max_tokens": settings.max_tokens,
            "system_prompt": settings.system_prompt,
        });

        let (mut presets, _version) = get_api_presets(settings.preferences.as_deref());

        let now = Utc::now();
        let new_preset = json!({
            "id": format!("preset_{}", now.timestamp_millis()),
            "name": name,
            "description": description,
            "is_active": false,
            "created_at": now.to_rfc3339(),
            "config": config,
        });

        presets.push(new_preset.clone());
        let new_prefs = set_api_presets(settings.preferences.as_deref(), &presets)?;

        let mut active: settings::ActiveModel = settings.into();
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now().naive_utc());
        active.update(db).await?;

        Ok(new_preset)
    }
}
