use std::env;

use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::config::AIConfig;
use crate::models::settings;
use crate::services::settings_preset_request_service::{
    CreateSettingsPresetRequest, UpdateSettingsPresetRequest,
};
use crate::services::settings_update_request_service::{
    SettingsApiBackupUrlsField, SettingsUpdateRequest,
};

const PLACEHOLDER_MASK: &str = "********";

const WEB_RESEARCH_PREF_KEY: &str = "web_research";

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

fn normalize_api_key(key: Option<String>) -> Option<String> {
    key.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !is_placeholder(value))
}

fn normalize_non_empty_string(value: Option<&str>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
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

fn normalize_openai_compatible_base_url(base_url: &str) -> String {
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

impl SettingsService {
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
        let existing = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?;

        match existing {
            Some(s) => {
                let web_research = merge_web_research(s.preferences.as_deref());
                let backup_urls = parse_api_backup_urls(s.api_backup_urls.as_deref());
                Ok(build_response(&s, &web_research, &backup_urls))
            }
            None => {
                let id = Uuid::new_v4().to_string();
                let now = Utc::now().naive_utc();
                let prefs_str =
                    serde_json::to_string(&json!({"web_research": web_research_defaults()}))?;
                let default_provider = default_ai_provider();
                let default_key = env_api_key_for_provider(&default_provider).unwrap_or_default();
                let default_base_url = resolve_base_url(&default_provider, "");

                let model = settings::ActiveModel {
                    id: Set(id.clone()),
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
                let saved = model.insert(db).await?;
                let web_research = web_research_defaults();
                let backup_urls: Vec<String> = vec![];
                Ok(build_response(&saved, &web_research, &backup_urls))
            }
        }
    }

    pub async fn update(
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
                let existing_provider = s.provider_type.trim().to_lowercase();
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

                let mut active: settings::ActiveModel = s.into();
                if let Some(v) = request.api_provider.as_ref() {
                    active.api_provider = Set(v.clone());
                }
                if request.clear_api_key {
                    active.api_key = Set(String::new());
                }
                if let Some(v) = request.api_key.as_ref() {
                    let trimmed = v.trim();
                    if !trimmed.is_empty() && !is_placeholder(trimmed) {
                        active.api_key = Set(trimmed.to_string());
                    }
                }
                if let Some(v) = request.api_base_url.as_ref() {
                    active.api_base_url = Set(v.clone());
                }
                active.api_backup_urls = Set(serialize_api_backup_urls(&backup_urls));
                if let Some(v) = request.provider_type.as_ref() {
                    active.provider_type = Set(v.clone());
                }
                if let Some(v) = request.fallback_strategy.as_ref() {
                    active.fallback_strategy = Set(v.clone());
                }
                if let Some(v) = request.azure_api_version.as_ref() {
                    active.azure_api_version = Set(Some(v.clone()));
                }
                let target_provider = request
                    .provider_type
                    .as_deref()
                    .or(request.api_provider.as_deref())
                    .map(|v| v.trim().to_lowercase())
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| {
                        if existing_provider.is_empty() {
                            "openai".to_string()
                        } else {
                            existing_provider.clone()
                        }
                    });
                if let Some(v) = normalize_non_empty_string(request.llm_model.as_deref()) {
                    active.llm_model = Set(v);
                } else if request.provider_switch_requested {
                    active.llm_model = Set(default_model_for_provider(&target_provider));
                }
                if let Some(v) = request.temperature {
                    active.temperature = Set(v);
                }
                if let Some(v) = request.max_tokens {
                    active.max_tokens = Set(v as i32);
                }
                if let Some(v) = request.system_prompt.as_ref() {
                    active.system_prompt = Set(Some(v.clone()));
                }
                if let Some(v) = request.preferences.as_ref() {
                    active.preferences = Set(Some(v.clone()));
                }
                if let Some(p) = new_prefs {
                    active.preferences = Set(Some(p));
                }
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
                let default_provider = default_ai_provider();
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

                let model = settings::ActiveModel {
                    id: Set(id.clone()),
                    user_id: Set(user_id.to_string()),
                    api_provider: Set(request
                        .api_provider
                        .as_deref()
                        .unwrap_or(&default_provider)
                        .to_string()),
                    api_key: Set(api_key),
                    api_base_url: Set(request
                        .api_base_url
                        .clone()
                        .unwrap_or_else(|| resolve_base_url(&default_provider, ""))),
                    api_backup_urls: Set(serialize_api_backup_urls(&backup_urls)),
                    provider_type: Set(request
                        .provider_type
                        .as_deref()
                        .unwrap_or(&default_provider)
                        .to_string()),
                    fallback_strategy: Set(request
                        .fallback_strategy
                        .as_deref()
                        .unwrap_or("auto")
                        .to_string()),
                    azure_api_version: Set(request.azure_api_version.clone()),
                    llm_model: Set(normalize_non_empty_string(request.llm_model.as_deref())
                        .unwrap_or_else(|| {
                            let provider = request
                                .provider_type
                                .as_deref()
                                .or(request.api_provider.as_deref())
                                .unwrap_or(&default_provider);
                            default_model_for_provider(provider)
                        })),
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

    pub async fn build_ai_config(
        db: &DatabaseConnection,
        user_id: &str,
        provider_override: Option<&str>,
        model_override: Option<&str>,
        temperature_override: Option<f64>,
    ) -> Result<AIConfig, String> {
        let s = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("failed to load settings: {}", e))?
            .ok_or("settings not found")?;

        let provider = resolve_provider(provider_override, &s.provider_type, &s.api_provider);
        let api_key = normalize_api_key(Some(s.api_key))
            .or_else(|| env_api_key_for_provider(&provider))
            .unwrap_or_default();
        if api_key.is_empty() {
            return Err(format!(
                "current AI settings are missing a usable API key for provider {}",
                provider
            ));
        }
        let base_url = resolve_base_url(&provider, &s.api_base_url);
        let model = model_override
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                let stored = s.llm_model.trim().to_string();
                if stored.is_empty() {
                    default_model_for_provider(&provider)
                } else {
                    stored
                }
            });
        let max_tokens = if s.max_tokens > 0 {
            s.max_tokens as u32
        } else {
            default_max_tokens()
        };

        Ok(AIConfig {
            provider,
            api_key,
            base_url,
            model,
            temperature: temperature_override.unwrap_or(s.temperature),
            max_tokens,
            system_prompt: s.system_prompt,
            max_retries: 3,
            request_delay_ms: 200,
        })
    }

    pub async fn delete(
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
            None => Ok(json!({"message": "no settings to delete", "user_id": user_id})),
        }
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
        let settings = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?;

        let (presets, _version) =
            get_api_presets(settings.as_ref().and_then(|s| s.preferences.as_deref()));
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
        let settings = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or("settings not found")?;

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
        let settings = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or("settings not found")?;

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
        let settings = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or("settings not found")?;

        let (mut presets, _version) = get_api_presets(settings.preferences.as_deref());
        presets.retain(|p| p.get("id").and_then(|v| v.as_str()) != Some(preset_id));
        let new_prefs = set_api_presets(settings.preferences.as_deref(), &presets)?;

        let mut active: settings::ActiveModel = settings.into();
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now().naive_utc());
        active.update(db).await?;

        Ok(json!({"success": true, "message": "preset deleted"}))
    }

    pub async fn activate_preset(
        db: &DatabaseConnection,
        user_id: &str,
        preset_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or("settings not found")?;

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
                let trimmed = v.trim();
                if !trimmed.is_empty() && !is_placeholder(trimmed) {
                    active.api_key = Set(trimmed.to_string());
                }
            }
            if let Some(v) = cfg.get("api_base_url").and_then(|v| v.as_str()) {
                active.api_base_url = Set(v.to_string());
            }
            if let Some(v) = cfg.get("api_backup_urls") {
                let urls: Vec<String> = v
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|u| u.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                active.api_backup_urls = Set(serialize_api_backup_urls(&urls));
            }
            if let Some(v) = cfg.get("provider_type").and_then(|v| v.as_str()) {
                active.provider_type = Set(v.to_string());
            }
            if let Some(v) = cfg.get("fallback_strategy").and_then(|v| v.as_str()) {
                active.fallback_strategy = Set(v.to_string());
            }
            if let Some(v) = cfg.get("azure_api_version").and_then(|v| v.as_str()) {
                active.azure_api_version = Set(Some(v.to_string()));
            }
            if let Some(v) =
                normalize_non_empty_string(cfg.get("llm_model").and_then(|v| v.as_str()))
            {
                active.llm_model = Set(v);
            }
            if let Some(v) = cfg.get("temperature").and_then(|v| v.as_f64()) {
                active.temperature = Set(v);
            }
            if let Some(v) = cfg.get("max_tokens").and_then(|v| v.as_i64()) {
                active.max_tokens = Set(v as i32);
            }
            if let Some(v) = cfg.get("system_prompt").and_then(|v| v.as_str()) {
                active.system_prompt = Set(Some(v.to_string()));
            }
        }
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now().naive_utc());
        active.update(db).await?;

        Ok(json!({"success": true, "message": "preset activated", "preset": result}))
    }

    pub async fn create_preset_from_current(
        db: &DatabaseConnection,
        user_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or("settings not found")?;

        let backup_urls = parse_api_backup_urls(settings.api_backup_urls.as_deref());
        let config = json!({
            "api_provider": settings.api_provider,
            "api_key": settings.api_key,
            "api_base_url": settings.api_base_url,
            "api_backup_urls": backup_urls,
            "provider_type": settings.provider_type,
            "fallback_strategy": settings.fallback_strategy,
            "azure_api_version": settings.azure_api_version,
            "llm_model": resolve_stored_model(&settings.llm_model, &settings.provider_type),
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
