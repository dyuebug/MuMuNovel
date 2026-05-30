use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::models::writing_style;

pub fn normalize_tags_value(tags: Option<&Value>) -> Option<String> {
    let value = tags?;

    match value {
        Value::Null => None,
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }

            if trimmed.starts_with('[') {
                return Some(trimmed.to_string());
            }

            let items: Vec<String> = trimmed
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();

            if items.is_empty() {
                None
            } else {
                serde_json::to_string(&items).ok()
            }
        }
        Value::Array(items) => {
            let normalized: Vec<String> = items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();

            if normalized.is_empty() {
                None
            } else {
                serde_json::to_string(&normalized).ok()
            }
        }
        _ => None,
    }
}

pub fn workshop_instance_id() -> String {
    std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string())
}

pub fn workshop_user_identifier(user_id: &str) -> String {
    format!("{}:{}", workshop_instance_id(), user_id)
}

pub fn build_workshop_download_payload(instance_id: &str, user_identifier: &str) -> Value {
    json!({
        "instance_id": instance_id,
        "user_identifier": user_identifier,
    })
}

pub fn default_workshop_category() -> String {
    "general".to_string()
}

#[derive(Debug, PartialEq)]
pub struct PreparedSubmitPromptRequest {
    pub user_identifier: String,
    pub submitter_name: String,
    pub normalized_tags: Option<String>,
    pub proxy_payload: Value,
}

pub fn prepare_submit_prompt_request(
    instance_id: &str,
    user_id: &str,
    name: &str,
    description: Option<&str>,
    prompt_content: &str,
    category: &str,
    tags: Option<&Value>,
    author_display_name: Option<&str>,
    is_anonymous: bool,
) -> PreparedSubmitPromptRequest {
    let user_identifier = workshop_user_identifier(user_id);
    let submitter_name = author_display_name
        .map(str::to_string)
        .unwrap_or_else(|| user_id.to_string());
    let normalized_tags = normalize_tags_value(tags);

    let mut payload = Map::new();
    payload.insert("instance_id".to_string(), json!(instance_id));
    payload.insert("submitter_id".to_string(), json!(user_identifier));
    payload.insert("submitter_name".to_string(), json!(submitter_name));
    payload.insert("name".to_string(), json!(name));
    payload.insert("description".to_string(), json!(description));
    payload.insert("prompt_content".to_string(), json!(prompt_content));
    payload.insert("category".to_string(), json!(category));
    payload.insert(
        "author_display_name".to_string(),
        json!(author_display_name),
    );
    payload.insert("is_anonymous".to_string(), json!(is_anonymous));
    payload.insert(
        "tags".to_string(),
        normalized_tags
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .unwrap_or(Value::Null),
    );

    PreparedSubmitPromptRequest {
        user_identifier,
        submitter_name,
        normalized_tags,
        proxy_payload: Value::Object(payload),
    }
}

#[derive(Deserialize)]
pub struct PromptWorkshopAdminReviewRouteRequest {
    pub action: String,
    pub review_note: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Value>,
}

#[derive(Debug, PartialEq)]
pub struct PreparedPromptWorkshopAdminReviewRequest {
    pub action: String,
    pub review_note: Option<String>,
    pub category: Option<String>,
    pub normalized_tags: Option<String>,
}

pub fn prepare_admin_review_submission_request(
    body: PromptWorkshopAdminReviewRouteRequest,
) -> PreparedPromptWorkshopAdminReviewRequest {
    let normalized_tags = normalize_tags_value(body.tags.as_ref());

    PreparedPromptWorkshopAdminReviewRequest {
        action: body.action,
        review_note: body.review_note,
        category: body.category,
        normalized_tags,
    }
}

#[derive(Deserialize)]
pub struct PromptWorkshopAdminCreateItemRouteRequest {
    pub name: String,
    pub description: Option<String>,
    pub prompt_content: String,
    #[serde(default = "default_workshop_category")]
    pub category: String,
    pub tags: Option<Value>,
}

#[derive(Debug, PartialEq)]
pub struct PreparedPromptWorkshopAdminCreateItemRequest {
    pub name: String,
    pub description: Option<String>,
    pub prompt_content: String,
    pub category: String,
    pub normalized_tags: Option<String>,
}

pub fn prepare_admin_create_item_request(
    body: PromptWorkshopAdminCreateItemRouteRequest,
) -> PreparedPromptWorkshopAdminCreateItemRequest {
    let normalized_tags = normalize_tags_value(body.tags.as_ref());

    PreparedPromptWorkshopAdminCreateItemRequest {
        name: body.name,
        description: body.description,
        prompt_content: body.prompt_content,
        category: body.category,
        normalized_tags,
    }
}

#[derive(Deserialize)]
pub struct PromptWorkshopAdminUpdateItemRouteRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub prompt_content: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Value>,
    pub status: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct PreparedPromptWorkshopAdminUpdateItemRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub prompt_content: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Value>,
    pub status: Option<String>,
}

pub fn prepare_admin_update_item_request(
    body: PromptWorkshopAdminUpdateItemRouteRequest,
) -> PreparedPromptWorkshopAdminUpdateItemRequest {
    PreparedPromptWorkshopAdminUpdateItemRequest {
        name: body.name,
        description: body.description,
        prompt_content: body.prompt_content,
        category: body.category,
        tags: body.tags,
        status: body.status,
    }
}

pub fn required_workshop_text<'a>(item: &'a Value, field: &str) -> Result<&'a str, String> {
    item.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("云端提示词缺少必要字段: {}", field))
}

pub async fn create_writing_style_from_workshop_item(
    db: &DatabaseConnection,
    item: &Value,
    custom_name: Option<&str>,
    user_id: &str,
) -> Result<Value, String> {
    let name = required_workshop_text(item, "name")?;
    let prompt_content = required_workshop_text(item, "prompt_content")?;
    let description = item
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let count = writing_style::Entity::find()
        .filter(writing_style::Column::UserId.eq(user_id))
        .count(db)
        .await
        .map_err(|error| format!("{}", error))?;

    let inserted = writing_style::ActiveModel {
        user_id: Set(Some(user_id.to_string())),
        name: Set(custom_name.unwrap_or(name).to_string()),
        style_type: Set("custom".to_string()),
        description: Set(Some(format!("从提示词工坊导入: {}", description))),
        prompt_content: Set(prompt_content.to_string()),
        order_index: Set(count as i32 + 1),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|error| format!("{}", error))?;

    Ok(json!({
        "success": true,
        "message": "导入成功",
        "writing_style": {
            "id": inserted.id,
            "name": inserted.name,
            "style_type": inserted.style_type,
            "prompt_content": inserted.prompt_content,
        }
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_workshop_download_payload, default_workshop_category, normalize_tags_value,
        prepare_admin_create_item_request, prepare_admin_review_submission_request,
        prepare_admin_update_item_request, prepare_submit_prompt_request, required_workshop_text,
        workshop_instance_id, workshop_user_identifier, PromptWorkshopAdminCreateItemRouteRequest,
        PromptWorkshopAdminReviewRouteRequest, PromptWorkshopAdminUpdateItemRouteRequest,
    };

    #[test]
    fn normalize_tags_value_keeps_csv_and_array_inputs_compatible() {
        assert_eq!(
            normalize_tags_value(Some(&json!("tag-1, tag-2 , ,tag-3"))),
            Some("[\"tag-1\",\"tag-2\",\"tag-3\"]".to_string())
        );
        assert_eq!(
            normalize_tags_value(Some(&json!(["a", " b ", "", 1]))),
            Some("[\"a\",\"b\"]".to_string())
        );
    }

    #[test]
    fn normalize_tags_value_keeps_json_string_and_empty_inputs_behavior() {
        assert_eq!(
            normalize_tags_value(Some(&json!("[\"x\",\"y\"]"))),
            Some("[\"x\",\"y\"]".to_string())
        );
        assert_eq!(normalize_tags_value(Some(&json!("   "))), None);
        assert_eq!(normalize_tags_value(Some(&json!(null))), None);
    }

    #[test]
    fn workshop_user_identifier_uses_instance_prefix() {
        let original = std::env::var("INSTANCE_ID").ok();
        unsafe {
            std::env::set_var("INSTANCE_ID", "test-instance");
        }

        assert_eq!(workshop_instance_id(), "test-instance");
        assert_eq!(workshop_user_identifier("user-7"), "test-instance:user-7");

        if let Some(value) = original {
            unsafe {
                std::env::set_var("INSTANCE_ID", value);
            }
        } else {
            unsafe {
                std::env::remove_var("INSTANCE_ID");
            }
        }
    }

    #[test]
    fn required_workshop_text_rejects_missing_or_blank_fields() {
        let item = json!({
            "name": "风格 A",
            "prompt_content": "内容",
            "blank": "   "
        });

        assert_eq!(
            required_workshop_text(&item, "name").expect("name should exist"),
            "风格 A"
        );
        assert_eq!(
            required_workshop_text(&item, "missing").expect_err("missing should fail"),
            "云端提示词缺少必要字段: missing"
        );
        assert_eq!(
            required_workshop_text(&item, "blank").expect_err("blank should fail"),
            "云端提示词缺少必要字段: blank"
        );
    }

    #[test]
    fn build_workshop_download_payload_keeps_existing_shape() {
        let payload = build_workshop_download_payload("instance-a", "instance-a:user-1");

        assert_eq!(payload["instance_id"], "instance-a");
        assert_eq!(payload["user_identifier"], "instance-a:user-1");
    }

    #[test]
    fn prepare_submit_prompt_request_keeps_proxy_payload_contract() {
        let original = std::env::var("INSTANCE_ID").ok();
        unsafe {
            std::env::set_var("INSTANCE_ID", "test-instance");
        }

        let request = prepare_submit_prompt_request(
            "instance-a",
            "user-1",
            "提示词名",
            Some("描述"),
            "内容",
            "general",
            Some(&json!(["tag-1", " tag-2 "])),
            Some("展示名"),
            true,
        );

        assert_eq!(request.user_identifier, "test-instance:user-1");
        assert_eq!(request.submitter_name, "展示名");
        assert_eq!(
            request.normalized_tags,
            Some("[\"tag-1\",\"tag-2\"]".to_string())
        );
        assert_eq!(request.proxy_payload["instance_id"], "instance-a");
        assert_eq!(
            request.proxy_payload["submitter_id"],
            "test-instance:user-1"
        );
        assert_eq!(request.proxy_payload["submitter_name"], "展示名");
        assert_eq!(request.proxy_payload["name"], "提示词名");
        assert_eq!(request.proxy_payload["description"], "描述");
        assert_eq!(request.proxy_payload["prompt_content"], "内容");
        assert_eq!(request.proxy_payload["category"], "general");
        assert_eq!(request.proxy_payload["author_display_name"], "展示名");
        assert_eq!(request.proxy_payload["is_anonymous"], true);
        assert_eq!(request.proxy_payload["tags"][0], "tag-1");
        assert_eq!(request.proxy_payload["tags"][1], "tag-2");

        if let Some(value) = original {
            unsafe {
                std::env::set_var("INSTANCE_ID", value);
            }
        } else {
            unsafe {
                std::env::remove_var("INSTANCE_ID");
            }
        }
    }

    #[test]
    fn prepare_admin_review_submission_request_normalizes_tags() {
        let request =
            prepare_admin_review_submission_request(PromptWorkshopAdminReviewRouteRequest {
                action: "approve".to_string(),
                review_note: Some("通过".to_string()),
                category: Some("romance".to_string()),
                tags: Some(json!(["tag-1", " tag-2 ", ""])),
            });

        assert_eq!(request.action, "approve");
        assert_eq!(request.review_note.as_deref(), Some("通过"));
        assert_eq!(request.category.as_deref(), Some("romance"));
        assert_eq!(
            request.normalized_tags,
            Some("[\"tag-1\",\"tag-2\"]".to_string())
        );
    }

    #[test]
    fn prepare_admin_create_item_request_keeps_default_category_and_normalized_tags() {
        let request =
            prepare_admin_create_item_request(PromptWorkshopAdminCreateItemRouteRequest {
                name: "官方提示词".to_string(),
                description: Some("描述".to_string()),
                prompt_content: "内容".to_string(),
                category: default_workshop_category(),
                tags: Some(json!("tag-1, tag-2")),
            });

        assert_eq!(request.name, "官方提示词");
        assert_eq!(request.description.as_deref(), Some("描述"));
        assert_eq!(request.prompt_content, "内容");
        assert_eq!(request.category, "general");
        assert_eq!(
            request.normalized_tags,
            Some("[\"tag-1\",\"tag-2\"]".to_string())
        );
    }

    #[test]
    fn prepare_admin_update_item_request_keeps_partial_update_contract() {
        let request =
            prepare_admin_update_item_request(PromptWorkshopAdminUpdateItemRouteRequest {
                name: Some("Prompt".to_string()),
                description: None,
                prompt_content: Some("Updated prompt".to_string()),
                category: Some("writing".to_string()),
                tags: Some(json!(["tag-1", "tag-2"])),
                status: Some("inactive".to_string()),
            });

        assert_eq!(request.name.as_deref(), Some("Prompt"));
        assert_eq!(request.description, None);
        assert_eq!(request.prompt_content.as_deref(), Some("Updated prompt"));
        assert_eq!(request.category.as_deref(), Some("writing"));
        assert_eq!(request.tags, Some(json!(["tag-1", "tag-2"])));
        assert_eq!(request.status.as_deref(), Some("inactive"));
    }
}
