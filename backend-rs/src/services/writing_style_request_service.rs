use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Default, Clone, Debug)]
pub struct SetDefaultStyleRouteQuery {
    pub project_id: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
pub struct SetDefaultStyleRouteBody {
    pub project_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BuildSetDefaultStyleRequestError {
    MissingProjectId,
}

pub fn build_set_default_style_project_id(
    query: SetDefaultStyleRouteQuery,
    body: Option<SetDefaultStyleRouteBody>,
) -> Result<String, BuildSetDefaultStyleRequestError> {
    let project_id = body
        .and_then(|payload| payload.project_id)
        .or(query.project_id)
        .unwrap_or_default();

    if project_id.is_empty() {
        return Err(BuildSetDefaultStyleRequestError::MissingProjectId);
    }

    Ok(project_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateWritingStyleRequest {
    preset_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    prompt_content: Option<String>,
    style_type: Option<String>,
}

impl CreateWritingStyleRequest {
    pub fn preset_id(&self) -> Option<&str> {
        self.preset_id.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn prompt_content(&self) -> Option<&str> {
        self.prompt_content.as_deref()
    }

    pub fn style_type(&self) -> Option<&str> {
        self.style_type.as_deref()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateWritingStyleRequest {
    name: Option<String>,
    description: Option<String>,
    prompt_content: Option<String>,
    order_index: Option<i32>,
}

impl UpdateWritingStyleRequest {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn prompt_content(&self) -> Option<&str> {
        self.prompt_content.as_deref()
    }

    pub fn order_index(&self) -> Option<i32> {
        self.order_index
    }
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct CreateWritingStyleRouteRequest {
    #[serde(default)]
    pub preset_id: Option<Value>,
    #[serde(default)]
    pub name: Option<Value>,
    #[serde(default)]
    pub description: Option<Value>,
    #[serde(default)]
    pub prompt_content: Option<Value>,
    #[serde(default)]
    pub style_type: Option<Value>,
}

impl CreateWritingStyleRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "preset_id": self.preset_id,
            "name": self.name,
            "description": self.description,
            "prompt_content": self.prompt_content,
            "style_type": self.style_type,
        })
    }
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct UpdateWritingStyleRouteRequest {
    #[serde(default)]
    pub name: Option<Value>,
    #[serde(default)]
    pub description: Option<Value>,
    #[serde(default)]
    pub prompt_content: Option<Value>,
    #[serde(default)]
    pub order_index: Option<Value>,
}

impl UpdateWritingStyleRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "prompt_content": self.prompt_content,
            "order_index": self.order_index,
        })
    }
}

fn optional_non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToString::to_string)
}

pub fn build_create_writing_style_request_from_route_payload(
    body: &Value,
) -> CreateWritingStyleRequest {
    CreateWritingStyleRequest {
        preset_id: optional_string(body.get("preset_id")),
        name: optional_string(body.get("name")),
        description: optional_string(body.get("description")),
        prompt_content: optional_string(body.get("prompt_content")),
        style_type: optional_non_empty_string(body.get("style_type")),
    }
}

pub fn build_create_writing_style_request_from_typed_route_payload(
    route_request: CreateWritingStyleRouteRequest,
) -> CreateWritingStyleRequest {
    build_create_writing_style_request_from_route_payload(&route_request.into_body())
}

pub fn build_update_writing_style_request_from_route_payload(
    body: &Value,
) -> UpdateWritingStyleRequest {
    UpdateWritingStyleRequest {
        name: optional_string(body.get("name")),
        description: optional_string(body.get("description")),
        prompt_content: optional_string(body.get("prompt_content")),
        order_index: body
            .get("order_index")
            .and_then(Value::as_i64)
            .map(|value| value as i32),
    }
}

pub fn build_update_writing_style_request_from_typed_route_payload(
    route_request: UpdateWritingStyleRouteRequest,
) -> UpdateWritingStyleRequest {
    build_update_writing_style_request_from_route_payload(&route_request.into_body())
}

#[cfg(test)]
mod tests {
    use super::{
        build_create_writing_style_request_from_route_payload,
        build_create_writing_style_request_from_typed_route_payload,
        build_set_default_style_project_id, build_update_writing_style_request_from_route_payload,
        build_update_writing_style_request_from_typed_route_payload,
        BuildSetDefaultStyleRequestError, CreateWritingStyleRouteRequest, SetDefaultStyleRouteBody,
        SetDefaultStyleRouteQuery, UpdateWritingStyleRouteRequest,
    };
    use serde_json::json;

    #[test]
    fn build_set_default_style_project_id_prefers_body_over_query() {
        let project_id = build_set_default_style_project_id(
            SetDefaultStyleRouteQuery {
                project_id: Some("project-from-query".to_string()),
            },
            Some(SetDefaultStyleRouteBody {
                project_id: Some("project-from-body".to_string()),
            }),
        )
        .expect("project_id should be built");

        assert_eq!(project_id, "project-from-body");
    }

    #[test]
    fn build_set_default_style_project_id_accepts_query_only() {
        let project_id = build_set_default_style_project_id(
            SetDefaultStyleRouteQuery {
                project_id: Some("project-only".to_string()),
            },
            None,
        )
        .expect("project_id should be built");

        assert_eq!(project_id, "project-only");
    }

    #[test]
    fn build_set_default_style_project_id_rejects_missing_value() {
        let error = build_set_default_style_project_id(
            SetDefaultStyleRouteQuery { project_id: None },
            Some(SetDefaultStyleRouteBody { project_id: None }),
        )
        .expect_err("missing project_id should fail");

        assert_eq!(error, BuildSetDefaultStyleRequestError::MissingProjectId);
    }

    #[test]
    fn build_create_writing_style_request_from_route_payload_keeps_optional_fields() {
        let request = build_create_writing_style_request_from_route_payload(&json!({
            "preset_id": "preset-1",
            "name": " 风格A ",
            "description": "描述",
            "prompt_content": "正文",
            "style_type": " custom "
        }));

        assert_eq!(request.preset_id(), Some("preset-1"));
        assert_eq!(request.name(), Some(" 风格A "));
        assert_eq!(request.description(), Some("描述"));
        assert_eq!(request.prompt_content(), Some("正文"));
        assert_eq!(request.style_type(), Some("custom"));
    }

    #[test]
    fn build_create_writing_style_request_from_route_payload_treats_blank_style_type_as_missing() {
        let request = build_create_writing_style_request_from_route_payload(&json!({
            "name": "风格B",
            "style_type": "   "
        }));

        assert_eq!(request.name(), Some("风格B"));
        assert_eq!(request.style_type(), None);
    }

    #[test]
    fn build_update_writing_style_request_from_route_payload_keeps_partial_updates() {
        let request = build_update_writing_style_request_from_route_payload(&json!({
            "name": "新标题",
            "description": "新描述",
            "prompt_content": "新内容",
            "order_index": 7
        }));

        assert_eq!(request.name(), Some("新标题"));
        assert_eq!(request.description(), Some("新描述"));
        assert_eq!(request.prompt_content(), Some("新内容"));
        assert_eq!(request.order_index(), Some(7));
    }

    #[test]
    fn build_create_writing_style_request_from_typed_route_payload_keeps_existing_shape() {
        let request = build_create_writing_style_request_from_typed_route_payload(
            CreateWritingStyleRouteRequest {
                preset_id: Some(json!("preset-1")),
                name: Some(json!(" 风格A ")),
                description: Some(json!("描述")),
                prompt_content: Some(json!("正文")),
                style_type: Some(json!(" custom ")),
            },
        );

        assert_eq!(request.preset_id(), Some("preset-1"));
        assert_eq!(request.name(), Some(" 风格A "));
        assert_eq!(request.description(), Some("描述"));
        assert_eq!(request.prompt_content(), Some("正文"));
        assert_eq!(request.style_type(), Some("custom"));
    }

    #[test]
    fn build_update_writing_style_request_from_typed_route_payload_keeps_compat_parsing() {
        let request = build_update_writing_style_request_from_typed_route_payload(
            UpdateWritingStyleRouteRequest {
                name: Some(json!("新标题")),
                description: Some(json!("新描述")),
                prompt_content: Some(json!("新内容")),
                order_index: Some(json!("invalid")),
            },
        );

        assert_eq!(request.name(), Some("新标题"));
        assert_eq!(request.description(), Some("新描述"));
        assert_eq!(request.prompt_content(), Some("新内容"));
        assert_eq!(request.order_index(), None);
    }
}
