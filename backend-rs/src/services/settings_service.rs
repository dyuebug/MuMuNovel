use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::settings;

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
    placeholders.contains(&key) || key.starts_with("sk-placeholder")
}

fn mask_api_key(key: &str) -> String {
    if key.is_empty() || is_placeholder(key) {
        key.to_string()
    } else {
        PLACEHOLDER_MASK.to_string()
    }
}

fn parse_api_backup_urls(raw: Option<&str>) -> Vec<String> {
    raw.and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default()
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

impl SettingsService {
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
                let now = Utc::now();
                let prefs_str = serde_json::to_string(&json!({"web_research": web_research_defaults()}))?;

                let model = settings::ActiveModel {
                    id: Set(id.clone()),
                    user_id: Set(user_id.to_string()),
                    api_provider: Set("openai".into()),
                    api_key: Set(String::new()),
                    api_base_url: Set(String::new()),
                    api_backup_urls: Set(None),
                    provider_type: Set("openai".into()),
                    fallback_strategy: Set("auto".into()),
                    azure_api_version: Set(None),
                    llm_model: Set("gpt-4".into()),
                    temperature: Set(0.7),
                    max_tokens: Set(2000),
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
        body: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?;

        match existing {
            Some(s) => {
                let current_prefs = s.preferences.clone().unwrap_or_default();
                let wr_patch = extract_web_research_patch(body);
                let new_prefs = if wr_patch.is_object() && wr_patch.as_object().map(|o| o.len()).unwrap_or(0) > 0 {
                    Some(set_web_research(Some(&current_prefs), &wr_patch)?)
                } else {
                    None
                };

                let backup_urls = body.get("api_backup_urls")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|| parse_api_backup_urls(s.api_backup_urls.as_deref()));

                let mut active: settings::ActiveModel = s.into();
                if let Some(v) = body.get("api_provider").and_then(|v| v.as_str()) { active.api_provider = Set(v.to_string()); }
                if let Some(v) = body.get("api_key").and_then(|v| v.as_str()) {
                    if !is_placeholder(v) { active.api_key = Set(v.to_string()); }
                }
                if let Some(v) = body.get("api_base_url").and_then(|v| v.as_str()) { active.api_base_url = Set(v.to_string()); }
                active.api_backup_urls = Set(serialize_api_backup_urls(&backup_urls));
                if let Some(v) = body.get("provider_type").and_then(|v| v.as_str()) { active.provider_type = Set(v.to_string()); }
                if let Some(v) = body.get("fallback_strategy").and_then(|v| v.as_str()) { active.fallback_strategy = Set(v.to_string()); }
                if let Some(v) = body.get("azure_api_version").and_then(|v| v.as_str()) { active.azure_api_version = Set(Some(v.to_string())); }
                if let Some(v) = body.get("llm_model").and_then(|v| v.as_str()) { active.llm_model = Set(v.to_string()); }
                if let Some(v) = body.get("temperature").and_then(|v| v.as_f64()) { active.temperature = Set(v); }
                if let Some(v) = body.get("max_tokens").and_then(|v| v.as_i64()) { active.max_tokens = Set(v as i32); }
                if let Some(v) = body.get("system_prompt").and_then(|v| v.as_str()) { active.system_prompt = Set(Some(v.to_string())); }
                if let Some(v) = body.get("preferences").and_then(|v| v.as_str()) { active.preferences = Set(Some(v.to_string())); }
                if let Some(p) = new_prefs { active.preferences = Set(Some(p)); }
                active.updated_at = Set(Utc::now());

                let saved = active.update(db).await?;
                let web_research = merge_web_research(saved.preferences.as_deref());
                let backup_urls = parse_api_backup_urls(saved.api_backup_urls.as_deref());
                Ok(build_response(&saved, &web_research, &backup_urls))
            }
            None => {
                let id = Uuid::new_v4().to_string();
                let now = Utc::now();
                let wr_patch = extract_web_research_patch(body);
                let default_prefs = serde_json::to_string(&json!({"web_research": web_research_defaults()}))?;
                let prefs = if wr_patch.is_object() && wr_patch.as_object().map(|o| o.len()).unwrap_or(0) > 0 {
                    Some(set_web_research(Some(&default_prefs), &wr_patch)?)
                } else {
                    Some(default_prefs)
                };

                let backup_urls: Vec<String> = body.get("api_backup_urls")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let api_key = body.get("api_key").and_then(|v| v.as_str()).unwrap_or("").to_string();

                let model = settings::ActiveModel {
                    id: Set(id.clone()),
                    user_id: Set(user_id.to_string()),
                    api_provider: Set(body.get("api_provider").and_then(|v| v.as_str()).unwrap_or("openai").to_string()),
                    api_key: Set(api_key),
                    api_base_url: Set(body.get("api_base_url").and_then(|v| v.as_str()).unwrap_or("").to_string()),
                    api_backup_urls: Set(serialize_api_backup_urls(&backup_urls)),
                    provider_type: Set(body.get("provider_type").and_then(|v| v.as_str()).unwrap_or("openai").to_string()),
                    fallback_strategy: Set(body.get("fallback_strategy").and_then(|v| v.as_str()).unwrap_or("auto").to_string()),
                    azure_api_version: Set(body.get("azure_api_version").and_then(|v| v.as_str()).map(String::from)),
                    llm_model: Set(body.get("llm_model").and_then(|v| v.as_str()).unwrap_or("gpt-4").to_string()),
                    temperature: Set(body.get("temperature").and_then(|v| v.as_f64()).unwrap_or(0.7)),
                    max_tokens: Set(body.get("max_tokens").and_then(|v| v.as_i64()).unwrap_or(2000) as i32),
                    system_prompt: Set(body.get("system_prompt").and_then(|v| v.as_str()).map(String::from)),
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
                Ok(json!({"message": "设置已删除", "user_id": user_id}))
            }
            None => Ok(json!({"message": "无设置可删除", "user_id": user_id})),
        }
    }
}

fn build_response(saved: &settings::Model, web_research: &Value, backup_urls: &[String]) -> Value {
    json!({
        "id": saved.id,
        "user_id": saved.user_id,
        "api_provider": saved.api_provider,
        "api_key": mask_api_key(&saved.api_key),
        "api_base_url": saved.api_base_url,
        "api_backup_urls": backup_urls,
        "provider_type": saved.provider_type,
        "fallback_strategy": saved.fallback_strategy,
        "azure_api_version": saved.azure_api_version,
        "llm_model": saved.llm_model,
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
        "created_at": saved.created_at.to_rfc3339(),
        "updated_at": saved.updated_at.to_rfc3339(),
    })
}

fn extract_web_research_patch(body: &Value) -> Value {
    let web_research_keys = [
        "web_research_enabled", "web_research_exa_enabled", "web_research_grok_enabled",
        "web_research_exa_api_key", "web_research_exa_base_url",
        "web_research_grok_api_key", "web_research_grok_base_url",
        "web_research_grok_model", "web_research_grok_search_enabled",
    ];
    let mut patch = json!({});
    if let Some(obj) = body.as_object() {
        if let Some(patch_obj) = patch.as_object_mut() {
            for key in &web_research_keys {
                if let Some(v) = obj.get(*key) {
                    patch_obj.insert(key.to_string(), v.clone());
                }
            }
        }
    }
    patch
}
