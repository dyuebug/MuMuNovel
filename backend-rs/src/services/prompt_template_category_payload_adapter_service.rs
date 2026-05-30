use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::prompt_template;
use crate::services::prompt_template_service::SystemTemplate;

pub fn build_prompt_template_categories_payload(
    user_id: &str,
    user_templates: &[prompt_template::Model],
    system_templates: &[SystemTemplate],
    now: DateTime<Utc>,
) -> Value {
    let user_keys: BTreeSet<&str> = user_templates
        .iter()
        .map(|template| template.template_key.as_str())
        .collect();

    let mut category_map: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    for template in user_templates {
        let category_key = template
            .category
            .clone()
            .unwrap_or_else(|| "未分类".to_string());

        category_map.entry(category_key).or_default().push(json!({
            "id": template.id,
            "user_id": template.user_id,
            "template_key": template.template_key,
            "template_name": template.template_name,
            "template_content": template.template_content,
            "description": template.description,
            "category": template.category,
            "parameters": template.parameters,
            "is_active": template.is_active,
            "is_system_default": false,
            "created_at": template.created_at.and_utc().to_rfc3339(),
            "updated_at": template.updated_at.and_utc().to_rfc3339(),
        }));
    }

    for system in system_templates {
        if user_keys.contains(system.template_key.as_str()) {
            continue;
        }

        let category_key = if system.category.is_empty() {
            "未分类".to_string()
        } else {
            system.category.clone()
        };
        let params_str = serde_json::to_string(&system.parameters).unwrap_or_default();

        category_map.entry(category_key).or_default().push(json!({
            "id": system.template_key,
            "user_id": user_id,
            "template_key": system.template_key,
            "template_name": system.template_name,
            "template_content": system.content,
            "description": system.description,
            "category": system.category,
            "parameters": params_str,
            "is_active": true,
            "is_system_default": true,
            "created_at": now,
            "updated_at": now,
        }));
    }

    let mut result = Vec::new();
    for (category, mut templates) in category_map {
        templates.sort_by(|left, right| {
            left["template_key"]
                .as_str()
                .unwrap_or("")
                .cmp(right["template_key"].as_str().unwrap_or(""))
        });

        result.push(json!({
            "category": category,
            "count": templates.len(),
            "templates": templates,
        }));
    }

    json!(result)
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    use serde_json::Value;

    use crate::models::prompt_template;
    use crate::services::prompt_template_service::SystemTemplate;

    use super::build_prompt_template_categories_payload;

    fn test_datetime() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-05-22T03:00:00", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn user_template(
        template_key: &str,
        category: Option<&str>,
        template_name: &str,
    ) -> prompt_template::Model {
        prompt_template::Model {
            id: format!("id-{template_key}"),
            user_id: "user-1".to_string(),
            template_key: template_key.to_string(),
            template_name: template_name.to_string(),
            template_content: format!("content-{template_key}"),
            description: Some(format!("desc-{template_key}")),
            category: category.map(str::to_string),
            parameters: Some("[\"tone\"]".to_string()),
            is_active: true,
            is_system_default: false,
            created_at: test_datetime(),
            updated_at: test_datetime(),
        }
    }

    fn system_templates() -> Vec<SystemTemplate> {
        vec![
            SystemTemplate {
                template_key: "alpha".to_string(),
                template_name: "Alpha".to_string(),
                category: "分类A".to_string(),
                description: "system alpha".to_string(),
                parameters: vec!["tone".to_string()],
                content: "alpha system".to_string(),
                content_hash: "hash-alpha".to_string(),
            },
            SystemTemplate {
                template_key: "gamma".to_string(),
                template_name: "Gamma".to_string(),
                category: String::new(),
                description: "system gamma".to_string(),
                parameters: vec!["style".to_string()],
                content: "gamma system".to_string(),
                content_hash: "hash-gamma".to_string(),
            },
        ]
    }

    #[test]
    fn build_prompt_template_categories_payload_groups_and_sorts_templates() {
        let payload = build_prompt_template_categories_payload(
            "user-1",
            &[
                user_template("beta", Some("分类A"), "Beta"),
                user_template("delta", None, "Delta"),
            ],
            &system_templates(),
            Utc.with_ymd_and_hms(2026, 5, 22, 3, 0, 0)
                .single()
                .expect("datetime should be valid"),
        );

        let groups = payload.as_array().expect("groups should be an array");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0]["category"], "分类A");
        assert_eq!(groups[0]["count"], 2);
        assert_eq!(groups[0]["templates"][0]["template_key"], "alpha");
        assert_eq!(groups[0]["templates"][0]["is_system_default"], true);
        assert_eq!(groups[0]["templates"][1]["template_key"], "beta");
        assert_eq!(groups[0]["templates"][1]["is_system_default"], false);

        assert_eq!(groups[1]["category"], "未分类");
        assert_eq!(groups[1]["count"], 2);
        assert_eq!(groups[1]["templates"][0]["template_key"], "delta");
        assert_eq!(groups[1]["templates"][0]["category"], Value::Null);
        assert_eq!(groups[1]["templates"][1]["template_key"], "gamma");
        assert_eq!(groups[1]["templates"][1]["user_id"], "user-1");
        assert_eq!(groups[1]["templates"][1]["category"], "");
        assert_eq!(groups[1]["templates"][1]["parameters"], "[\"style\"]");
    }

    #[test]
    fn build_prompt_template_categories_payload_skips_system_defaults_overridden_by_user() {
        let payload = build_prompt_template_categories_payload(
            "user-1",
            &[user_template("alpha", Some("分类A"), "Alpha Custom")],
            &system_templates(),
            Utc.with_ymd_and_hms(2026, 5, 22, 3, 0, 0)
                .single()
                .expect("datetime should be valid"),
        );

        let groups = payload.as_array().expect("groups should be an array");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0]["category"], "分类A");
        assert_eq!(groups[0]["count"], 1);
        assert_eq!(groups[0]["templates"][0]["template_key"], "alpha");
        assert_eq!(groups[0]["templates"][0]["is_system_default"], false);
    }
}
