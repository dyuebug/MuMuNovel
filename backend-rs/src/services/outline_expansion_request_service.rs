use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;

use crate::ai::service::AIService;
use crate::services::plot_expansion_service::create_plot_expansion_service;
use crate::services::settings_service::SettingsService;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct OutlineExpandRouteRequest {
    pub target_chapter_count: Option<i64>,
    pub expansion_strategy: Option<String>,
    pub auto_create_chapters: Option<bool>,
    pub enable_scene_analysis: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub batch_size: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct OutlineBatchExpandRouteRequest {
    pub project_id: Option<String>,
    pub chapters_per_outline: Option<i64>,
    pub expansion_strategy: Option<String>,
    pub auto_create_chapters: Option<bool>,
    pub enable_scene_analysis: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub outline_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutlineExpandExecutionRequest {
    pub outline_id: String,
    pub target_chapter_count: usize,
    pub expansion_strategy: String,
    pub auto_create_chapters: bool,
    pub enable_scene_analysis: bool,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub batch_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutlineBatchExpandExecutionRequest {
    pub project_id: String,
    pub chapters_per_outline: usize,
    pub expansion_strategy: String,
    pub auto_create_chapters: bool,
    pub enable_scene_analysis: bool,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub outline_ids: Option<Vec<String>>,
}

pub(crate) fn build_outline_expand_execution_request(
    outline_id: impl Into<String>,
    payload: &Value,
) -> OutlineExpandExecutionRequest {
    OutlineExpandExecutionRequest {
        outline_id: outline_id.into(),
        target_chapter_count: payload
            .get("target_chapter_count")
            .and_then(Value::as_i64)
            .unwrap_or_default() as usize,
        expansion_strategy: payload
            .get("expansion_strategy")
            .and_then(Value::as_str)
            .unwrap_or("balanced")
            .to_string(),
        auto_create_chapters: payload
            .get("auto_create_chapters")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        enable_scene_analysis: payload
            .get("enable_scene_analysis")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        provider: payload
            .get("provider")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        model: payload
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        batch_size: payload
            .get("batch_size")
            .and_then(Value::as_i64)
            .filter(|value| *value > 0)
            .unwrap_or(5) as usize,
    }
}

pub(crate) fn build_outline_expand_execution_request_from_route_request(
    outline_id: impl Into<String>,
    request: &OutlineExpandRouteRequest,
) -> OutlineExpandExecutionRequest {
    let mut payload = serde_json::Map::new();

    if let Some(value) = request.target_chapter_count {
        payload.insert(
            "target_chapter_count".to_string(),
            Value::Number(value.into()),
        );
    }
    if let Some(value) = request.expansion_strategy.as_ref() {
        payload.insert(
            "expansion_strategy".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = request.auto_create_chapters {
        payload.insert("auto_create_chapters".to_string(), Value::Bool(value));
    }
    if let Some(value) = request.enable_scene_analysis {
        payload.insert("enable_scene_analysis".to_string(), Value::Bool(value));
    }
    if let Some(value) = request.provider.as_ref() {
        payload.insert("provider".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.model.as_ref() {
        payload.insert("model".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.batch_size {
        payload.insert("batch_size".to_string(), Value::Number(value.into()));
    }

    build_outline_expand_execution_request(outline_id, &Value::Object(payload))
}

pub(crate) fn build_outline_batch_expand_execution_request(
    payload: &Value,
) -> OutlineBatchExpandExecutionRequest {
    OutlineBatchExpandExecutionRequest {
        project_id: payload
            .get("project_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        chapters_per_outline: payload
            .get("chapters_per_outline")
            .and_then(Value::as_i64)
            .unwrap_or_default() as usize,
        expansion_strategy: payload
            .get("expansion_strategy")
            .and_then(Value::as_str)
            .unwrap_or("balanced")
            .to_string(),
        auto_create_chapters: payload
            .get("auto_create_chapters")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        enable_scene_analysis: payload
            .get("enable_scene_analysis")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        provider: payload
            .get("provider")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        model: payload
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        outline_ids: payload
            .get("outline_ids")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            }),
    }
}

pub(crate) fn build_outline_batch_expand_execution_request_from_route_request(
    request: &OutlineBatchExpandRouteRequest,
) -> OutlineBatchExpandExecutionRequest {
    let mut payload = serde_json::Map::new();

    if let Some(value) = request.project_id.as_ref() {
        payload.insert("project_id".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.chapters_per_outline {
        payload.insert(
            "chapters_per_outline".to_string(),
            Value::Number(value.into()),
        );
    }
    if let Some(value) = request.expansion_strategy.as_ref() {
        payload.insert(
            "expansion_strategy".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = request.auto_create_chapters {
        payload.insert("auto_create_chapters".to_string(), Value::Bool(value));
    }
    if let Some(value) = request.enable_scene_analysis {
        payload.insert("enable_scene_analysis".to_string(), Value::Bool(value));
    }
    if let Some(value) = request.provider.as_ref() {
        payload.insert("provider".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.model.as_ref() {
        payload.insert("model".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.outline_ids.as_ref() {
        payload.insert(
            "outline_ids".to_string(),
            Value::Array(value.iter().cloned().map(Value::String).collect()),
        );
    }

    build_outline_batch_expand_execution_request(&Value::Object(payload))
}

pub(crate) async fn execute_outline_expand_request(
    db: &DatabaseConnection,
    user_id: &str,
    request: &OutlineExpandExecutionRequest,
) -> Result<Value, String> {
    let ai_config = SettingsService::build_ai_config(
        db,
        user_id,
        request.provider.as_deref(),
        request.model.as_deref(),
        None,
    )
    .await?;
    let ai_service = AIService::new(ai_config);
    let service = create_plot_expansion_service(&ai_service);

    service
        .expand_outline(
            db,
            user_id,
            &request.outline_id,
            request.target_chapter_count,
            &request.expansion_strategy,
            request.auto_create_chapters,
            request.enable_scene_analysis,
            request.provider.as_deref(),
            request.model.as_deref(),
            request.batch_size,
        )
        .await
}

pub(crate) async fn execute_outline_batch_expand_request(
    db: &DatabaseConnection,
    user_id: &str,
    request: &OutlineBatchExpandExecutionRequest,
) -> Result<Value, String> {
    let ai_config = SettingsService::build_ai_config(
        db,
        user_id,
        request.provider.as_deref(),
        request.model.as_deref(),
        None,
    )
    .await?;
    let ai_service = AIService::new(ai_config);
    let service = create_plot_expansion_service(&ai_service);

    service
        .batch_expand_outlines(
            db,
            user_id,
            &request.project_id,
            request.chapters_per_outline,
            &request.expansion_strategy,
            request.auto_create_chapters,
            request.enable_scene_analysis,
            request.outline_ids.as_deref(),
            request.provider.as_deref(),
            request.model.as_deref(),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::{
        build_outline_batch_expand_execution_request,
        build_outline_batch_expand_execution_request_from_route_request,
        build_outline_expand_execution_request,
        build_outline_expand_execution_request_from_route_request, OutlineBatchExpandRouteRequest,
        OutlineExpandRouteRequest,
    };
    use serde_json::json;

    #[test]
    fn outline_expand_request_keeps_existing_defaults() {
        let request = build_outline_expand_execution_request("outline-1", &json!({}));
        assert_eq!(request.outline_id, "outline-1");
        assert_eq!(request.target_chapter_count, 0);
        assert_eq!(request.expansion_strategy, "balanced");
        assert!(!request.auto_create_chapters);
        assert!(!request.enable_scene_analysis);
        assert_eq!(request.provider, None);
        assert_eq!(request.model, None);
        assert_eq!(request.batch_size, 5);
    }

    #[test]
    fn outline_batch_expand_request_keeps_existing_contract() {
        let request = build_outline_batch_expand_execution_request(&json!({
            "project_id": "project-1",
            "chapters_per_outline": 4,
            "expansion_strategy": "climax",
            "auto_create_chapters": true,
            "enable_scene_analysis": true,
            "provider": "openai",
            "model": "gpt-4.1",
            "outline_ids": ["outline-a", "outline-b", 1]
        }));
        assert_eq!(request.project_id, "project-1");
        assert_eq!(request.chapters_per_outline, 4);
        assert_eq!(request.expansion_strategy, "climax");
        assert!(request.auto_create_chapters);
        assert!(request.enable_scene_analysis);
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.model.as_deref(), Some("gpt-4.1"));
        assert_eq!(
            request.outline_ids,
            Some(vec!["outline-a".to_string(), "outline-b".to_string()])
        );
    }

    #[test]
    fn outline_batch_expand_request_defaults_missing_project_id_to_empty_string() {
        let request = build_outline_batch_expand_execution_request(&json!({
            "chapters_per_outline": 2
        }));

        assert_eq!(request.project_id, "");
        assert_eq!(request.chapters_per_outline, 2);
    }

    #[test]
    fn outline_expand_route_request_builder_keeps_existing_contract() {
        let request = build_outline_expand_execution_request_from_route_request(
            "outline-1",
            &OutlineExpandRouteRequest {
                target_chapter_count: Some(6),
                expansion_strategy: Some("climax".to_string()),
                auto_create_chapters: Some(true),
                enable_scene_analysis: Some(true),
                provider: Some("openai".to_string()),
                model: Some("gpt-4.1".to_string()),
                batch_size: Some(8),
            },
        );

        assert_eq!(request.outline_id, "outline-1");
        assert_eq!(request.target_chapter_count, 6);
        assert_eq!(request.expansion_strategy, "climax");
        assert!(request.auto_create_chapters);
        assert!(request.enable_scene_analysis);
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.model.as_deref(), Some("gpt-4.1"));
        assert_eq!(request.batch_size, 8);
    }

    #[test]
    fn outline_expand_route_request_builder_keeps_defaults() {
        let request = build_outline_expand_execution_request_from_route_request(
            "outline-1",
            &OutlineExpandRouteRequest {
                target_chapter_count: None,
                expansion_strategy: None,
                auto_create_chapters: None,
                enable_scene_analysis: None,
                provider: None,
                model: None,
                batch_size: None,
            },
        );

        assert_eq!(request.target_chapter_count, 0);
        assert_eq!(request.expansion_strategy, "balanced");
        assert!(!request.auto_create_chapters);
        assert!(!request.enable_scene_analysis);
        assert_eq!(request.provider, None);
        assert_eq!(request.model, None);
        assert_eq!(request.batch_size, 5);
    }

    #[test]
    fn outline_batch_expand_route_request_builder_keeps_existing_contract() {
        let request = build_outline_batch_expand_execution_request_from_route_request(
            &OutlineBatchExpandRouteRequest {
                project_id: Some("project-1".to_string()),
                chapters_per_outline: Some(4),
                expansion_strategy: Some("climax".to_string()),
                auto_create_chapters: Some(true),
                enable_scene_analysis: Some(true),
                provider: Some("openai".to_string()),
                model: Some("gpt-4.1".to_string()),
                outline_ids: Some(vec!["outline-a".to_string(), "outline-b".to_string()]),
            },
        );

        assert_eq!(request.project_id, "project-1");
        assert_eq!(request.chapters_per_outline, 4);
        assert_eq!(request.expansion_strategy, "climax");
        assert!(request.auto_create_chapters);
        assert!(request.enable_scene_analysis);
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.model.as_deref(), Some("gpt-4.1"));
        assert_eq!(
            request.outline_ids,
            Some(vec!["outline-a".to_string(), "outline-b".to_string()])
        );
    }

    #[test]
    fn outline_batch_expand_route_request_builder_defaults_missing_project_id() {
        let request = build_outline_batch_expand_execution_request_from_route_request(
            &OutlineBatchExpandRouteRequest {
                project_id: None,
                chapters_per_outline: Some(2),
                expansion_strategy: None,
                auto_create_chapters: None,
                enable_scene_analysis: None,
                provider: None,
                model: None,
                outline_ids: None,
            },
        );

        assert_eq!(request.project_id, "");
        assert_eq!(request.chapters_per_outline, 2);
    }
}
