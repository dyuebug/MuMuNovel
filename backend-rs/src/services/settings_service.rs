use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::config::AIConfig;
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
            .map_err(|e| format!("读取设置失败: {}", e))?
            .ok_or("用户设置不存在，请先在设置页配置AI")?;

        Ok(AIConfig {
            provider: provider_override.map(|s| s.to_string()).unwrap_or(s.api_provider),
            api_key: s.api_key,
            base_url: s.api_base_url,
            model: model_override.map(|s| s.to_string()).unwrap_or(s.llm_model),
            temperature: temperature_override.unwrap_or(s.temperature),
            max_tokens: s.max_tokens as u32,
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

// ========== API Presets (stored in preferences JSON) ==========

const API_PRESETS_KEY: &str = "api_presets";

fn get_api_presets(prefs_json: Option<&str>) -> (Vec<Value>, String) {
    let prefs: Value = prefs_json.and_then(|s| serde_json::from_str(s).ok()).unwrap_or(json!({}));
    let api_presets = prefs.get(API_PRESETS_KEY)
        .and_then(|ap| ap.get("presets"))
        .and_then(|p| p.as_array().cloned())
        .unwrap_or_default();
    let version = prefs.get(API_PRESETS_KEY)
        .and_then(|ap| ap.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("1.0")
        .to_string();
    (api_presets, version)
}

fn set_api_presets(prefs_json: Option<&str>, presets: &[Value]) -> serde_json::Result<String> {
    let mut prefs: Value = prefs_json.and_then(|s| serde_json::from_str(s).ok()).unwrap_or(json!({}));
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

        let (presets, _version) = get_api_presets(settings.as_ref().and_then(|s| s.preferences.as_deref()));
        let active_preset_id = presets.iter()
            .find(|p| p.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false))
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
        body: &Value,
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
            "name": body.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            "description": body.get("description"),
            "is_active": false,
            "created_at": now.to_rfc3339(),
            "config": body.get("config").cloned().unwrap_or(json!({})),
        });

        presets.push(new_preset.clone());
        let new_prefs = set_api_presets(settings.preferences.as_deref(), &presets)?;

        let mut active: settings::ActiveModel = settings.into();
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now());
        active.update(db).await?;

        Ok(new_preset)
    }

    pub async fn update_preset(
        db: &DatabaseConnection,
        user_id: &str,
        preset_id: &str,
        body: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let settings = settings::Entity::find()
            .filter(settings::Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or("settings not found")?;

        let (mut presets, _version) = get_api_presets(settings.preferences.as_deref());

        let idx = presets.iter().position(|p| p.get("id").and_then(|v| v.as_str()) == Some(preset_id))
            .ok_or("preset not found")?;

        let target = &mut presets[idx];
        if let Some(v) = body.get("name").and_then(|v| v.as_str()) {
            target["name"] = json!(v);
        }
        if body.get("description").is_some() {
            target["description"] = body["description"].clone();
        }
        if let Some(config) = body.get("config") {
            target["config"] = config.clone();
        }

        let result = target.clone();
        let new_prefs = set_api_presets(settings.preferences.as_deref(), &presets)?;

        let mut active: settings::ActiveModel = settings.into();
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now());
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
        active.updated_at = Set(Utc::now());
        active.update(db).await?;

        Ok(json!({"success": true, "message": "预设已删除"}))
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
            if let Some(v) = cfg.get("api_provider").and_then(|v| v.as_str()) { active.api_provider = Set(v.to_string()); }
            if let Some(v) = cfg.get("api_key").and_then(|v| v.as_str()) {
                if !is_placeholder(v) { active.api_key = Set(v.to_string()); }
            }
            if let Some(v) = cfg.get("api_base_url").and_then(|v| v.as_str()) { active.api_base_url = Set(v.to_string()); }
            if let Some(v) = cfg.get("api_backup_urls") {
                let urls: Vec<String> = v.as_array().map(|arr| arr.iter().filter_map(|u| u.as_str().map(String::from)).collect()).unwrap_or_default();
                active.api_backup_urls = Set(serialize_api_backup_urls(&urls));
            }
            if let Some(v) = cfg.get("provider_type").and_then(|v| v.as_str()) { active.provider_type = Set(v.to_string()); }
            if let Some(v) = cfg.get("fallback_strategy").and_then(|v| v.as_str()) { active.fallback_strategy = Set(v.to_string()); }
            if let Some(v) = cfg.get("azure_api_version").and_then(|v| v.as_str()) { active.azure_api_version = Set(Some(v.to_string())); }
            if let Some(v) = cfg.get("llm_model").and_then(|v| v.as_str()) { active.llm_model = Set(v.to_string()); }
            if let Some(v) = cfg.get("temperature").and_then(|v| v.as_f64()) { active.temperature = Set(v); }
            if let Some(v) = cfg.get("max_tokens").and_then(|v| v.as_i64()) { active.max_tokens = Set(v as i32); }
            if let Some(v) = cfg.get("system_prompt").and_then(|v| v.as_str()) { active.system_prompt = Set(Some(v.to_string())); }
        }
        active.preferences = Set(Some(new_prefs));
        active.updated_at = Set(Utc::now());
        active.update(db).await?;

        Ok(json!({"success": true, "message": "预设已激活", "preset": result}))
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
        active.updated_at = Set(Utc::now());
        active.update(db).await?;

        Ok(new_preset)
    }
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
