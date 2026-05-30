use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::prompt_template;
use crate::services::prompt_template_service::SystemTemplate;

pub fn build_prompt_template_export_payload(
    user_templates: &[prompt_template::Model],
    system_templates: &[SystemTemplate],
    export_time: DateTime<Utc>,
) -> Value {
    let user_keys: BTreeSet<&str> = user_templates
        .iter()
        .map(|template| template.template_key.as_str())
        .collect();

    let mut export_items = Vec::new();
    let mut customized_count = 0u32;
    let mut system_default_count = 0u32;

    for template in user_templates {
        let system_hash = system_templates
            .iter()
            .find(|system| system.template_key == template.template_key)
            .map(|system| system.content_hash.as_str());

        export_items.push(json!({
            "template_key": template.template_key,
            "template_name": template.template_name,
            "template_content": template.template_content,
            "description": template.description,
            "category": template.category,
            "parameters": template.parameters,
            "is_active": template.is_active,
            "is_customized": true,
            "system_content_hash": system_hash,
        }));
        customized_count += 1;
    }

    for system in system_templates {
        if user_keys.contains(system.template_key.as_str()) {
            continue;
        }

        let params_str = serde_json::to_string(&system.parameters).unwrap_or_default();
        export_items.push(json!({
            "template_key": system.template_key,
            "template_name": system.template_name,
            "template_content": system.content,
            "description": system.description,
            "category": system.category,
            "parameters": params_str,
            "is_active": true,
            "is_customized": false,
            "system_content_hash": system.content_hash,
        }));
        system_default_count += 1;
    }

    json!({
        "templates": export_items,
        "export_time": export_time,
        "version": "2.0",
        "statistics": {
            "total": customized_count + system_default_count,
            "customized": customized_count,
            "system_default": system_default_count,
        },
    })
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDateTime, TimeZone, Utc};

    use crate::models::prompt_template;
    use crate::services::prompt_template_service::SystemTemplate;

    use super::build_prompt_template_export_payload;

    fn test_datetime() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-05-22T02:15:00", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn user_template() -> prompt_template::Model {
        prompt_template::Model {
            id: "template-1".to_string(),
            user_id: "user-1".to_string(),
            template_key: "chapter_generate".to_string(),
            template_name: "章节生成".to_string(),
            template_content: "custom content".to_string(),
            description: Some("自定义描述".to_string()),
            category: Some("生成".to_string()),
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
                template_key: "chapter_generate".to_string(),
                template_name: "章节生成".to_string(),
                category: "生成".to_string(),
                description: "默认描述".to_string(),
                parameters: vec!["tone".to_string()],
                content: "system content".to_string(),
                content_hash: "sys-hash-1".to_string(),
            },
            SystemTemplate {
                template_key: "chapter_rewrite".to_string(),
                template_name: "章节重写".to_string(),
                category: "改写".to_string(),
                description: "默认改写".to_string(),
                parameters: vec!["style".to_string()],
                content: "rewrite content".to_string(),
                content_hash: "sys-hash-2".to_string(),
            },
        ]
    }

    #[test]
    fn build_prompt_template_export_payload_keeps_custom_and_system_items() {
        let payload = build_prompt_template_export_payload(
            &[user_template()],
            &system_templates(),
            Utc.with_ymd_and_hms(2026, 5, 22, 2, 15, 0)
                .single()
                .expect("datetime should be valid"),
        );

        assert_eq!(payload["version"], "2.0");
        assert_eq!(payload["statistics"]["total"], 2);
        assert_eq!(payload["statistics"]["customized"], 1);
        assert_eq!(payload["statistics"]["system_default"], 1);

        let items = payload["templates"]
            .as_array()
            .expect("templates should be an array");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["template_key"], "chapter_generate");
        assert_eq!(items[0]["is_customized"], true);
        assert_eq!(items[0]["system_content_hash"], "sys-hash-1");
        assert_eq!(items[1]["template_key"], "chapter_rewrite");
        assert_eq!(items[1]["is_customized"], false);
        assert_eq!(items[1]["parameters"], "[\"style\"]");
    }

    #[test]
    fn build_prompt_template_export_payload_omits_duplicate_system_default_items() {
        let mut second_user_template = user_template();
        second_user_template.template_key = "chapter_rewrite".to_string();
        second_user_template.template_name = "章节重写-用户版".to_string();

        let payload = build_prompt_template_export_payload(
            &[user_template(), second_user_template],
            &system_templates(),
            Utc.with_ymd_and_hms(2026, 5, 22, 2, 15, 0)
                .single()
                .expect("datetime should be valid"),
        );

        let items = payload["templates"]
            .as_array()
            .expect("templates should be an array");
        assert_eq!(items.len(), 2);
        assert_eq!(payload["statistics"]["customized"], 2);
        assert_eq!(payload["statistics"]["system_default"], 0);
    }
}
