use std::collections::HashMap;
use std::sync::OnceLock;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::prompt_template;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemTemplate {
    pub template_key: String,
    pub template_name: String,
    pub category: String,
    pub description: String,
    pub parameters: Vec<String>,
    pub content: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSyncRule {
    pub legacy_hashes: Vec<String>,
    pub current_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemplatesData {
    templates: Vec<SystemTemplate>,
    sync_rules: HashMap<String, TemplateSyncRule>,
    managed_keys: Vec<String>,
}

fn templates_data() -> &'static TemplatesData {
    static DATA: OnceLock<TemplatesData> = OnceLock::new();
    DATA.get_or_init(|| {
        let json = include_str!("system_templates_data.json");
        serde_json::from_str(json).expect("Failed to parse system_templates_data.json")
    })
}

pub struct PromptTemplateService;

impl PromptTemplateService {
    pub fn all_system_templates() -> &'static [SystemTemplate] {
        &templates_data().templates
    }

    pub fn system_template_info(template_key: &str) -> Option<&SystemTemplate> {
        templates_data()
            .templates
            .iter()
            .find(|t| t.template_key == template_key)
    }

    pub fn managed_keys() -> &'static [String] {
        &templates_data().managed_keys
    }

    pub fn sync_rule(template_key: &str) -> Option<&TemplateSyncRule> {
        templates_data().sync_rules.get(template_key)
    }

    pub fn calculate_content_hash(content: &str) -> String {
        let normalized = content.trim();
        let hash = Sha256::digest(normalized.as_bytes());
        hex::encode(&hash[..8])
    }

    pub fn is_legacy_hash(template_key: &str, content_hash: &str) -> bool {
        if let Some(rule) = Self::sync_rule(template_key) {
            if !content_hash.is_empty()
                && rule.legacy_hashes.contains(&content_hash.to_string())
                && !rule.current_hashes.contains(&content_hash.to_string())
            {
                return true;
            }
        }
        false
    }

    pub fn build_sync_status(
        template_key: &str,
        user_template: Option<&prompt_template::Model>,
    ) -> serde_json::Value {
        let info = Self::system_template_info(template_key);
        let template_name = info
            .map(|i| i.template_name.as_str())
            .unwrap_or(template_key);
        let category = info.map(|i| i.category.as_str());
        let system_hash = info.map(|i| i.content_hash.as_str());

        let system_content = info.map(|i| i.content.as_str());

        if system_content.is_none() {
            return serde_json::json!({
                "template_key": template_key,
                "template_name": template_name,
                "category": category,
                "has_custom_template": user_template.is_some(),
                "is_active": user_template.map_or(true, |u| u.is_active),
                "sync_status": "system_template_missing",
                "is_diff_from_system": false,
                "is_legacy_default": false,
                "can_auto_sync": false,
                "can_sync_to_default": user_template.is_some(),
                "user_content_hash": user_template.map(|u| Self::calculate_content_hash(&u.template_content)),
                "system_content_hash": null,
                "updated_at": user_template.map(|u| u.updated_at.and_utc().to_rfc3339()),
            });
        }

        if user_template.is_none() {
            return serde_json::json!({
                "template_key": template_key,
                "template_name": template_name,
                "category": category,
                "has_custom_template": false,
                "is_active": true,
                "sync_status": "system_default",
                "is_diff_from_system": false,
                "is_legacy_default": false,
                "can_auto_sync": false,
                "can_sync_to_default": false,
                "user_content_hash": null,
                "system_content_hash": system_hash,
                "updated_at": null,
            });
        }

        let user = user_template.unwrap();
        let user_hash = Self::calculate_content_hash(&user.template_content);
        let is_diff = user_hash != system_hash.unwrap_or("");
        let is_legacy = if is_diff {
            Self::is_legacy_hash(template_key, &user_hash)
        } else {
            false
        };
        let can_auto_sync = is_legacy;

        let sync_status = if !is_diff {
            "up_to_date"
        } else if is_legacy {
            "legacy_default"
        } else {
            "customized"
        };

        serde_json::json!({
            "template_key": template_key,
            "template_name": template_name,
            "category": category,
            "has_custom_template": true,
            "is_active": user.is_active,
            "sync_status": sync_status,
            "is_diff_from_system": is_diff,
            "is_legacy_default": is_legacy,
            "can_auto_sync": can_auto_sync,
            "can_sync_to_default": true,
            "user_content_hash": user_hash,
            "system_content_hash": system_hash,
            "updated_at": user.updated_at.and_utc().to_rfc3339(),
        })
    }

    pub async fn sync_managed_templates_for_user(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<u32, String> {
        let mut synced = 0u32;
        for template_key in Self::managed_keys() {
            let info = match Self::system_template_info(template_key) {
                Some(i) => i,
                None => continue,
            };

            let user_template = prompt_template::Entity::find()
                .filter(prompt_template::Column::UserId.eq(user_id))
                .filter(prompt_template::Column::TemplateKey.eq(template_key.as_str()))
                .one(db)
                .await
                .map_err(|e| format!("{}", e))?;

            let user_tmpl = match user_template {
                Some(t) => t,
                None => continue,
            };

            let current_hash = Self::calculate_content_hash(&user_tmpl.template_content);
            if !Self::is_legacy_hash(template_key, &current_hash) {
                continue;
            }

            let normalized_user = user_tmpl.template_content.trim();
            let normalized_system = info.content.trim();
            if normalized_user == normalized_system {
                continue;
            }

            let mut active: prompt_template::ActiveModel = user_tmpl.into();
            active.template_content = Set(info.content.clone());
            active.template_name = Set(info.template_name.clone());
            if !info.description.is_empty() {
                active.description = Set(Some(info.description.clone()));
            }
            if !info.category.is_empty() {
                active.category = Set(Some(info.category.clone()));
            }
            let params = serde_json::to_string(&info.parameters).unwrap_or_default();
            active.parameters = Set(Some(params));
            active.updated_at = Set(Utc::now().naive_utc());

            active.update(db).await.map_err(|e| format!("{}", e))?;
            synced += 1;
        }
        Ok(synced)
    }

    pub async fn list_user_templates(
        db: &DatabaseConnection,
        user_id: &str,
        category: Option<&str>,
        is_active: Option<bool>,
    ) -> Result<(Vec<prompt_template::Model>, Vec<String>), String> {
        use sea_orm::QueryOrder;

        let mut query =
            prompt_template::Entity::find().filter(prompt_template::Column::UserId.eq(user_id));

        if let Some(cat) = category {
            query = query.filter(prompt_template::Column::Category.eq(cat));
        }
        if let Some(active) = is_active {
            query = query.filter(prompt_template::Column::IsActive.eq(active));
        }

        let templates = query
            .order_by_asc(prompt_template::Column::Category)
            .order_by_asc(prompt_template::Column::TemplateKey)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))?;

        let categories: Vec<String> = templates
            .iter()
            .filter_map(|t| t.category.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        Ok((templates, categories))
    }

    pub async fn find_user_template(
        db: &DatabaseConnection,
        user_id: &str,
        template_key: &str,
    ) -> Result<Option<prompt_template::Model>, String> {
        prompt_template::Entity::find()
            .filter(prompt_template::Column::UserId.eq(user_id))
            .filter(prompt_template::Column::TemplateKey.eq(template_key))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))
    }

    pub async fn upsert_template(
        db: &DatabaseConnection,
        user_id: &str,
        data: &serde_json::Value,
    ) -> Result<prompt_template::Model, String> {
        let template_key = data["template_key"].as_str().unwrap_or("");
        let existing = Self::find_user_template(db, user_id, template_key).await?;

        if let Some(tmpl) = existing {
            let mut active: prompt_template::ActiveModel = tmpl.into();
            if let Some(v) = data.get("template_name").and_then(|v| v.as_str()) {
                active.template_name = Set(v.to_string());
            }
            if let Some(v) = data.get("template_content").and_then(|v| v.as_str()) {
                active.template_content = Set(v.to_string());
            }
            if let Some(v) = data.get("description") {
                if v.is_null() {
                    active.description = Set(None);
                } else if let Some(s) = v.as_str() {
                    active.description = Set(Some(s.to_string()));
                }
            }
            if let Some(v) = data.get("category") {
                if v.is_null() {
                    active.category = Set(None);
                } else if let Some(s) = v.as_str() {
                    active.category = Set(Some(s.to_string()));
                }
            }
            if let Some(v) = data.get("parameters") {
                if v.is_null() {
                    active.parameters = Set(None);
                } else {
                    active.parameters = Set(Some(v.to_string()));
                }
            }
            if let Some(v) = data.get("is_active").and_then(|v| v.as_bool()) {
                active.is_active = Set(v);
            }
            active.updated_at = Set(Utc::now().naive_utc());
            active.update(db).await.map_err(|e| format!("{}", e))
        } else {
            let id = Uuid::new_v4().to_string();
            let params = data
                .get("parameters")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_string());
            let active = prompt_template::ActiveModel {
                id: Set(id),
                user_id: Set(user_id.to_string()),
                template_key: Set(template_key.to_string()),
                template_name: Set(data
                    .get("template_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()),
                template_content: Set(data
                    .get("template_content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()),
                description: Set(data
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())),
                category: Set(data
                    .get("category")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())),
                parameters: Set(Some(params)),
                is_active: Set(data
                    .get("is_active")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true)),
                is_system_default: Set(false),
                created_at: Set(Utc::now().naive_utc()),
                updated_at: Set(Utc::now().naive_utc()),
            };
            active.insert(db).await.map_err(|e| format!("{}", e))
        }
    }

    pub async fn delete_user_template(
        db: &DatabaseConnection,
        user_id: &str,
        template_key: &str,
    ) -> Result<bool, String> {
        let existing = Self::find_user_template(db, user_id, template_key).await?;
        if let Some(tmpl) = existing {
            prompt_template::Entity::delete_by_id(tmpl.id)
                .exec(db)
                .await
                .map_err(|e| format!("{}", e))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn format_prompt(
        template: &str,
        parameters: &HashMap<String, String>,
    ) -> Result<String, String> {
        let mut result = template.to_string();
        for (key, value) in parameters {
            let placeholder = format!("{{{}}}", key);
            if !result.contains(&placeholder) {
                continue;
            }
            result = result.replace(&placeholder, value);
        }
        Ok(result)
    }
}
