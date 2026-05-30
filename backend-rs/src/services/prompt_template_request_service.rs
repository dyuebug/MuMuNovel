use serde::Deserialize;
use serde_json::Value;

use crate::models::prompt_template;

#[derive(Debug, PartialEq, Eq)]
pub enum BuildPromptTemplateImportRequestError {
    MissingTemplates,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct PromptTemplateImportRouteRequest {
    #[serde(default)]
    templates: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct PromptTemplateUpsertRouteRequest {
    #[serde(flatten)]
    body: Value,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct PromptTemplateUpdateRouteRequest {
    #[serde(flatten)]
    body: Value,
}

impl PromptTemplateUpsertRouteRequest {
    pub fn into_body(self) -> Value {
        self.body
    }
}

impl PromptTemplateUpdateRouteRequest {
    pub fn into_body(self) -> Value {
        self.body
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptTemplateImportItemRequest {
    raw_item: Value,
    template_key: String,
    template_name: Value,
    is_customized: bool,
    imported_content: String,
}

impl PromptTemplateImportItemRequest {
    pub fn template_key(&self) -> &str {
        self.template_key.as_str()
    }

    pub fn template_name_value(&self) -> &Value {
        &self.template_name
    }

    pub fn is_customized(&self) -> bool {
        self.is_customized
    }

    pub fn imported_content(&self) -> &str {
        self.imported_content.as_str()
    }

    pub fn upsert_payload(&self) -> Value {
        let mut payload = self.raw_item.clone();

        if let Value::Object(ref mut map) = payload {
            map.insert(
                "template_key".to_string(),
                Value::String(self.template_key.clone()),
            );
        }

        payload
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptTemplateImportRequest {
    templates: Vec<PromptTemplateImportItemRequest>,
}

impl PromptTemplateImportRequest {
    pub fn templates(&self) -> &[PromptTemplateImportItemRequest] {
        self.templates.as_slice()
    }
}

pub fn build_prompt_template_upsert_payload_from_route_body(body: &Value) -> Value {
    body.clone()
}

pub fn build_prompt_template_upsert_payload_from_route_payload(
    route_request: PromptTemplateUpsertRouteRequest,
) -> Value {
    build_prompt_template_upsert_payload_from_route_body(&route_request.into_body())
}

pub fn build_prompt_template_update_payload_from_route_body(
    body: &Value,
    existing: &prompt_template::Model,
) -> Value {
    let mut merged = serde_json::to_value(existing).unwrap_or(Value::Null);

    if let Value::Object(ref mut map) = merged {
        if let Value::Object(body_map) = body {
            for (key, value) in body_map {
                map.insert(key.clone(), value.clone());
            }
        }
    }

    merged
}

pub fn build_prompt_template_update_payload_from_route_payload(
    route_request: PromptTemplateUpdateRouteRequest,
    existing: &prompt_template::Model,
) -> Value {
    build_prompt_template_update_payload_from_route_body(&route_request.into_body(), existing)
}

pub fn build_prompt_template_import_request_from_route_body(
    body: &Value,
) -> Result<PromptTemplateImportRequest, BuildPromptTemplateImportRequestError> {
    let route_request = if body.is_object() {
        PromptTemplateImportRouteRequest {
            templates: body.get("templates").cloned(),
        }
    } else {
        PromptTemplateImportRouteRequest::default()
    };

    build_prompt_template_import_request_from_route_payload(route_request)
}

pub fn build_prompt_template_import_request_from_route_payload(
    route_request: PromptTemplateImportRouteRequest,
) -> Result<PromptTemplateImportRequest, BuildPromptTemplateImportRequestError> {
    let templates = route_request
        .templates
        .as_ref()
        .and_then(Value::as_array)
        .ok_or(BuildPromptTemplateImportRequestError::MissingTemplates)?
        .iter()
        .map(|item| PromptTemplateImportItemRequest {
            raw_item: item.clone(),
            template_key: item
                .get("template_key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            template_name: item.get("template_name").cloned().unwrap_or(Value::Null),
            is_customized: item
                .get("is_customized")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            imported_content: item
                .get("template_content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
        })
        .collect();

    Ok(PromptTemplateImportRequest { templates })
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use serde_json::json;

    use super::{
        build_prompt_template_import_request_from_route_body,
        build_prompt_template_import_request_from_route_payload,
        build_prompt_template_update_payload_from_route_body,
        build_prompt_template_update_payload_from_route_payload,
        build_prompt_template_upsert_payload_from_route_body,
        build_prompt_template_upsert_payload_from_route_payload,
        BuildPromptTemplateImportRequestError, PromptTemplateImportRouteRequest,
        PromptTemplateUpdateRouteRequest, PromptTemplateUpsertRouteRequest,
    };
    use crate::models::prompt_template;

    fn sample_prompt_template() -> prompt_template::Model {
        prompt_template::Model {
            id: "template-id".to_string(),
            user_id: "user-1".to_string(),
            template_key: "chapter_generate".to_string(),
            template_name: "章节生成".to_string(),
            template_content: "existing content".to_string(),
            description: Some("existing description".to_string()),
            category: Some("writing".to_string()),
            parameters: Some("[\"chapter_title\"]".to_string()),
            is_active: true,
            is_system_default: false,
            created_at: DateTime::from_timestamp(0, 0)
                .expect("valid time")
                .naive_utc(),
            updated_at: DateTime::from_timestamp(0, 0)
                .expect("valid time")
                .naive_utc(),
        }
    }

    #[test]
    fn build_prompt_template_upsert_payload_from_route_body_keeps_payload_shape() {
        let body = json!({
            "template_key": "chapter_generate",
            "template_name": "章节生成",
            "template_content": "new content",
            "description": null,
            "category": "writing",
            "parameters": {
                "vars": ["chapter_title"]
            },
            "is_active": false
        });

        let payload = build_prompt_template_upsert_payload_from_route_body(&body);

        assert_eq!(payload, body);
    }

    #[test]
    fn build_prompt_template_upsert_payload_from_route_payload_keeps_payload_shape() {
        let payload = build_prompt_template_upsert_payload_from_route_payload(
            PromptTemplateUpsertRouteRequest {
                body: json!({
                    "template_key": "chapter_generate",
                    "template_name": "章节生成",
                    "template_content": "new content",
                    "description": null,
                    "category": "writing",
                    "parameters": {
                        "vars": ["chapter_title"]
                    },
                    "is_active": false
                }),
            },
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["template_content"], "new content");
    }

    #[test]
    fn build_prompt_template_update_payload_from_route_body_keeps_existing_fields_when_missing() {
        let existing = sample_prompt_template();

        let payload = build_prompt_template_update_payload_from_route_body(
            &json!({
                "template_content": "updated content"
            }),
            &existing,
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["template_name"], "章节生成");
        assert_eq!(payload["template_content"], "updated content");
        assert_eq!(payload["description"], "existing description");
        assert_eq!(payload["category"], "writing");
        assert_eq!(payload["parameters"], "[\"chapter_title\"]");
        assert_eq!(payload["is_active"], true);
    }

    #[test]
    fn build_prompt_template_update_payload_from_route_body_prefers_route_values() {
        let existing = sample_prompt_template();

        let payload = build_prompt_template_update_payload_from_route_body(
            &json!({
                "template_key": "chapter_rewrite",
                "template_name": "章节改写",
                "description": null,
                "parameters": {
                    "vars": ["scene"]
                },
                "is_active": false
            }),
            &existing,
        );

        assert_eq!(payload["template_key"], "chapter_rewrite");
        assert_eq!(payload["template_name"], "章节改写");
        assert!(payload["description"].is_null());
        assert_eq!(payload["parameters"], json!({"vars": ["scene"]}));
        assert_eq!(payload["is_active"], false);
    }

    #[test]
    fn build_prompt_template_update_payload_from_route_body_ignores_non_object_body() {
        let existing = sample_prompt_template();

        let payload = build_prompt_template_update_payload_from_route_body(
            &json!("not-an-object"),
            &existing,
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["template_content"], "existing content");
        assert_eq!(payload["description"], "existing description");
    }

    #[test]
    fn build_prompt_template_update_payload_from_route_payload_keeps_existing_fields_when_missing()
    {
        let existing = sample_prompt_template();

        let payload = build_prompt_template_update_payload_from_route_payload(
            PromptTemplateUpdateRouteRequest {
                body: json!({
                    "template_content": "updated content"
                }),
            },
            &existing,
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["template_name"], "章节生成");
        assert_eq!(payload["template_content"], "updated content");
        assert_eq!(payload["description"], "existing description");
    }

    #[test]
    fn build_prompt_template_import_request_from_route_body_requires_templates_array() {
        let error = build_prompt_template_import_request_from_route_body(&json!({}))
            .expect_err("missing templates should fail");

        assert_eq!(
            error,
            BuildPromptTemplateImportRequestError::MissingTemplates
        );
    }

    #[test]
    fn build_prompt_template_import_request_from_route_body_projects_import_items() {
        let request = build_prompt_template_import_request_from_route_body(&json!({
            "templates": [
                {
                    "template_key": "chapter_generate",
                    "template_name": "章节生成",
                    "template_content": "  imported content  ",
                    "is_customized": true,
                    "category": "writing"
                }
            ]
        }))
        .expect("templates should be parsed");

        let item = &request.templates()[0];

        assert_eq!(item.template_key(), "chapter_generate");
        assert_eq!(item.template_name_value(), "章节生成");
        assert_eq!(item.imported_content(), "imported content");
        assert!(item.is_customized());
        assert_eq!(
            item.upsert_payload(),
            json!({
                "template_key": "chapter_generate",
                "template_name": "章节生成",
                "template_content": "  imported content  ",
                "is_customized": true,
                "category": "writing"
            })
        );
    }

    #[test]
    fn build_prompt_template_import_request_from_route_payload_projects_import_items() {
        let request = build_prompt_template_import_request_from_route_payload(
            PromptTemplateImportRouteRequest {
                templates: Some(json!([
                    {
                        "template_key": "chapter_generate",
                        "template_name": "章节生成",
                        "template_content": "  imported content  ",
                        "is_customized": true,
                        "category": "writing"
                    }
                ])),
            },
        )
        .expect("templates should be parsed");

        let item = &request.templates()[0];

        assert_eq!(item.template_key(), "chapter_generate");
        assert_eq!(item.template_name_value(), "章节生成");
        assert_eq!(item.imported_content(), "imported content");
        assert!(item.is_customized());
    }

    #[test]
    fn build_prompt_template_import_request_from_route_payload_requires_templates_array() {
        let error = build_prompt_template_import_request_from_route_payload(
            PromptTemplateImportRouteRequest { templates: None },
        )
        .expect_err("missing templates should fail");

        assert_eq!(
            error,
            BuildPromptTemplateImportRequestError::MissingTemplates
        );
    }

    #[test]
    fn build_prompt_template_import_request_from_route_body_keeps_non_object_item_payload() {
        let request = build_prompt_template_import_request_from_route_body(&json!({
            "templates": ["raw-template"]
        }))
        .expect("templates should be parsed");

        let item = &request.templates()[0];

        assert_eq!(item.template_key(), "");
        assert!(item.template_name_value().is_null());
        assert_eq!(item.imported_content(), "");
        assert!(!item.is_customized());
        assert_eq!(item.upsert_payload(), json!("raw-template"));
    }
}
