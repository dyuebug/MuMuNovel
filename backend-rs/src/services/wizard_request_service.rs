use sea_orm::DatabaseConnection;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::services::wizard_service;
use crate::utils::sse::SseChannel;

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct WorldBuildingRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub theme: Option<String>,
    pub genre: Option<serde_json::Value>,
    #[serde(alias = "narrativePerspective")]
    pub narrative_perspective: Option<String>,
    #[serde(alias = "targetWords")]
    pub target_words: Option<i32>,
    #[serde(alias = "chapterCount")]
    pub chapter_count: Option<i32>,
    #[serde(alias = "characterCount")]
    pub character_count: Option<i32>,
    #[serde(alias = "outlineMode")]
    pub outline_mode: Option<String>,
    #[serde(alias = "defaultCreativeMode")]
    pub default_creative_mode: Option<String>,
    #[serde(alias = "defaultStoryFocus")]
    pub default_story_focus: Option<String>,
    #[serde(alias = "defaultPlotStage")]
    pub default_plot_stage: Option<String>,
    #[serde(alias = "defaultStoryCreationBrief")]
    pub default_story_creation_brief: Option<String>,
    #[serde(alias = "defaultQualityPreset")]
    pub default_quality_preset: Option<String>,
    #[serde(alias = "defaultQualityNotes")]
    pub default_quality_notes: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(alias = "userId")]
    pub user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub web_research_query: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct CareerSystemRequest {
    #[serde(alias = "projectId")]
    pub project_id: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(alias = "userId")]
    pub user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub web_research_query: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct CharactersRequest {
    #[serde(alias = "projectId")]
    pub project_id: String,
    #[serde(default = "default_count")]
    pub count: usize,
    #[serde(alias = "worldContext")]
    pub world_context: Option<Value>,
    pub theme: Option<String>,
    pub genre: Option<String>,
    pub requirements: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(alias = "userId")]
    pub user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub web_research_query: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct OutlineRequest {
    #[serde(alias = "projectId")]
    pub project_id: String,
    #[serde(default = "default_outline_count")]
    #[serde(alias = "chapterCount")]
    pub chapter_count: usize,
    #[serde(alias = "narrativePerspective")]
    pub narrative_perspective: Option<String>,
    #[serde(default = "default_target_words")]
    #[serde(alias = "targetWords")]
    pub target_words: i32,
    pub requirements: Option<String>,
    #[serde(alias = "creativeMode")]
    pub creative_mode: Option<String>,
    #[serde(alias = "storyFocus")]
    pub story_focus: Option<String>,
    #[serde(alias = "plotStage")]
    pub plot_stage: Option<String>,
    #[serde(alias = "storyCreationBrief")]
    pub story_creation_brief: Option<String>,
    #[serde(alias = "qualityPreset")]
    pub quality_preset: Option<String>,
    #[serde(alias = "qualityNotes")]
    pub quality_notes: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(alias = "userId")]
    pub user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub web_research_query: Option<String>,
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
pub struct RegenerateWorldBuildingRequest {
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(alias = "userId")]
    pub user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub web_research_query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CleanupWizardDataRouteRequest {
    body: Value,
}

impl<'de> Deserialize<'de> for CleanupWizardDataRouteRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(|body| Self { body })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupWizardDataRequest {
    body: Value,
}

fn default_count() -> usize {
    5
}

fn default_outline_count() -> usize {
    3
}

fn default_target_words() -> i32 {
    100000
}

pub fn resolve_effective_user_id(request_user_id: Option<String>, default_user_id: &str) -> String {
    request_user_id.unwrap_or_else(|| default_user_id.to_string())
}

pub fn normalize_genre_input(value: Option<serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text,
        Some(serde_json::Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| item.as_str().map(str::trim).map(ToString::to_string))
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("、"),
        Some(other) => {
            let text = other.to_string();
            if text == "null" {
                String::new()
            } else {
                text
            }
        }
        None => String::new(),
    }
}

pub async fn execute_world_building_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    body: WorldBuildingRequest,
) {
    let title = body.title.unwrap_or_default();
    let description = body.description.unwrap_or_default();
    let theme = body.theme.unwrap_or_default();
    let genre = normalize_genre_input(body.genre);

    wizard_service::generate_world_building(
        db,
        channel,
        user_id,
        &title,
        &description,
        &theme,
        &genre,
        body.narrative_perspective.as_deref(),
        body.target_words,
        body.chapter_count,
        body.character_count,
        body.outline_mode.as_deref(),
        body.default_creative_mode.as_deref(),
        body.default_story_focus.as_deref(),
        body.default_plot_stage.as_deref(),
        body.default_story_creation_brief.as_deref(),
        body.default_quality_preset.as_deref(),
        body.default_quality_notes.as_deref(),
        body.provider.as_deref(),
        body.model.as_deref(),
    )
    .await;
}

pub async fn execute_career_system_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    body: CareerSystemRequest,
) {
    wizard_service::generate_career_system(
        db,
        channel,
        user_id,
        &body.project_id,
        body.provider.as_deref(),
        body.model.as_deref(),
    )
    .await;
}

pub async fn execute_characters_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    body: CharactersRequest,
) {
    wizard_service::generate_characters(
        db,
        channel,
        user_id,
        &body.project_id,
        body.count,
        body.world_context,
        body.theme.as_deref(),
        body.genre.as_deref(),
        body.requirements.as_deref(),
        body.provider.as_deref(),
        body.model.as_deref(),
    )
    .await;
}

pub async fn execute_outline_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    body: OutlineRequest,
) {
    wizard_service::generate_outline(
        db,
        channel,
        user_id,
        &body.project_id,
        body.chapter_count,
        body.narrative_perspective.as_deref(),
        body.target_words,
        body.requirements.as_deref(),
        body.creative_mode.as_deref(),
        body.story_focus.as_deref(),
        body.plot_stage.as_deref(),
        body.story_creation_brief.as_deref(),
        body.quality_preset.as_deref(),
        body.quality_notes.as_deref(),
        body.provider.as_deref(),
        body.model.as_deref(),
    )
    .await;
}

pub fn legacy_career_system_query_to_request(
    project_id: String,
    provider: Option<String>,
    model: Option<String>,
    user_id: Option<String>,
    enable_mcp: Option<bool>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
) -> CareerSystemRequest {
    CareerSystemRequest {
        project_id,
        provider,
        model,
        user_id,
        enable_mcp,
        enable_web_research,
        web_research_query,
    }
}

pub fn outline_generate_request_to_wizard_request(
    project_id: String,
    chapter_count: usize,
    narrative_perspective: Option<String>,
    target_words: i32,
    requirements: Option<String>,
    creative_mode: Option<String>,
    story_focus: Option<String>,
    plot_stage: Option<String>,
    story_creation_brief: Option<String>,
    quality_preset: Option<String>,
    quality_notes: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> OutlineRequest {
    OutlineRequest {
        project_id,
        chapter_count,
        narrative_perspective,
        target_words,
        requirements,
        creative_mode,
        story_focus,
        plot_stage,
        story_creation_brief,
        quality_preset,
        quality_notes,
        provider,
        model,
        user_id: None,
        enable_mcp: None,
        enable_web_research: None,
        web_research_query: None,
    }
}

pub fn build_cleanup_wizard_data_request_from_route_payload(
    body: CleanupWizardDataRouteRequest,
) -> CleanupWizardDataRequest {
    CleanupWizardDataRequest { body: body.body }
}

impl CleanupWizardDataRequest {
    pub fn body(&self) -> &Value {
        &self.body
    }
}

pub async fn execute_regenerate_world_building_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    project_id: &str,
    body: RegenerateWorldBuildingRequest,
) {
    wizard_service::regenerate_world_building(
        db,
        channel,
        user_id,
        project_id,
        body.provider.as_deref(),
        body.model.as_deref(),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::{
        build_cleanup_wizard_data_request_from_route_payload,
        legacy_career_system_query_to_request, normalize_genre_input,
        outline_generate_request_to_wizard_request, resolve_effective_user_id,
        CleanupWizardDataRouteRequest,
    };
    use serde_json::json;

    #[test]
    fn normalize_genre_input_keeps_existing_transport_compatibility() {
        assert_eq!(
            normalize_genre_input(Some(json!(["玄幻", "  冒险  ", ""]))),
            "玄幻、冒险"
        );
        assert_eq!(normalize_genre_input(Some(json!("科幻"))), "科幻");
        assert_eq!(normalize_genre_input(Some(json!(null))), "");
        assert_eq!(normalize_genre_input(None), "");
    }

    #[test]
    fn effective_user_id_prefers_explicit_request_value() {
        assert_eq!(
            resolve_effective_user_id(Some("request-user".to_string()), "claims-user"),
            "request-user"
        );
        assert_eq!(
            resolve_effective_user_id(None, "claims-user"),
            "claims-user"
        );
    }

    #[test]
    fn legacy_career_system_query_adapter_preserves_existing_fields() {
        let request = legacy_career_system_query_to_request(
            "project-1".to_string(),
            Some("openai".to_string()),
            Some("gpt-4o-mini".to_string()),
            Some("user-1".to_string()),
            Some(true),
            None,
            None,
        );

        assert_eq!(request.project_id, "project-1");
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.model.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(request.user_id.as_deref(), Some("user-1"));
        assert_eq!(request.enable_mcp, Some(true));
    }

    #[test]
    fn outline_generate_adapter_keeps_outline_execution_inputs() {
        let request = outline_generate_request_to_wizard_request(
            "project-2".to_string(),
            8,
            Some("first_person".to_string()),
            120_000,
            Some("more twists".to_string()),
            Some("balanced".to_string()),
            Some("growth".to_string()),
            Some("midpoint".to_string()),
            Some("brief".to_string()),
            Some("high".to_string()),
            Some("notes".to_string()),
            Some("openai".to_string()),
            Some("gpt-4.1".to_string()),
        );

        assert_eq!(request.project_id, "project-2");
        assert_eq!(request.chapter_count, 8);
        assert_eq!(request.target_words, 120_000);
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.model.as_deref(), Some("gpt-4.1"));
        assert!(request.user_id.is_none());
    }

    #[test]
    fn cleanup_wizard_data_route_request_preserves_arbitrary_body_shape() {
        let object_request =
            build_cleanup_wizard_data_request_from_route_payload(CleanupWizardDataRouteRequest {
                body: json!({"dry_run": true, "ids": [1, 2, 3]}),
            });
        assert_eq!(
            object_request.body(),
            &json!({"dry_run": true, "ids": [1, 2, 3]})
        );

        let null_request =
            build_cleanup_wizard_data_request_from_route_payload(CleanupWizardDataRouteRequest {
                body: json!(null),
            });
        assert_eq!(null_request.body(), &json!(null));
    }
}
