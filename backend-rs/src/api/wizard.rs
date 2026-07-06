use axum::response::sse::Event;
use axum::{
    extract::{Extension, Path},
    response::Sse,
    routing::post,
    Json, Router,
};
use sea_orm::DatabaseConnection;
use serde::{de, Deserialize, Deserializer};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::api::careers::{execute_career_system_request, CareerSystemRequest};
use crate::api::outlines::execute_outline_request;
use crate::services::auth::Claims;
use crate::services::project_service::ProjectService;
use crate::services::wizard_service;

const WIZARD_WORLD_BUILDING_ROUTE: &str = "/wizard-stream/world-building";
const WIZARD_WORLD_BUILDING_PROJECT_ROUTE: &str = "/wizard-stream/world-building/{project_id}";
const WIZARD_WORLD_BUILDING_REGENERATE_ROUTE: &str =
    "/wizard-stream/world-building/{project_id}/regenerate";
const WIZARD_CAREER_SYSTEM_ROUTE: &str = "/wizard-stream/career-system";
const WIZARD_CHARACTERS_ROUTE: &str = "/wizard-stream/characters";
const WIZARD_OUTLINE_ROUTE: &str = "/wizard-stream/outline";
const WIZARD_CLEANUP_ROUTE: &str = "/wizard-stream/cleanup/{project_id}";

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct WorldBuildingRequest {
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) theme: Option<String>,
    pub(crate) genre: Option<Value>,
    #[serde(alias = "narrativePerspective")]
    pub(crate) narrative_perspective: Option<String>,
    #[serde(
        default,
        alias = "targetWords",
        deserialize_with = "deserialize_optional_i32_from_number_or_string"
    )]
    pub(crate) target_words: Option<i32>,
    #[serde(
        default,
        alias = "chapterCount",
        deserialize_with = "deserialize_optional_i32_from_number_or_string"
    )]
    pub(crate) chapter_count: Option<i32>,
    #[serde(
        default,
        alias = "characterCount",
        deserialize_with = "deserialize_optional_i32_from_number_or_string"
    )]
    pub(crate) character_count: Option<i32>,
    #[serde(alias = "outlineMode")]
    pub(crate) outline_mode: Option<String>,
    #[serde(alias = "defaultCreativeMode")]
    pub(crate) default_creative_mode: Option<String>,
    #[serde(alias = "defaultStoryFocus")]
    pub(crate) default_story_focus: Option<String>,
    #[serde(alias = "defaultPlotStage")]
    pub(crate) default_plot_stage: Option<String>,
    #[serde(alias = "defaultStoryCreationBrief")]
    pub(crate) default_story_creation_brief: Option<String>,
    #[serde(alias = "defaultQualityPreset")]
    pub(crate) default_quality_preset: Option<String>,
    #[serde(alias = "defaultQualityNotes")]
    pub(crate) default_quality_notes: Option<String>,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    #[serde(alias = "userId")]
    pub(crate) user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub(crate) enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub(crate) enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub(crate) web_research_query: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct CharactersRequest {
    #[serde(alias = "projectId")]
    pub(crate) project_id: String,
    #[serde(
        default = "default_count",
        deserialize_with = "deserialize_usize_from_number_or_string"
    )]
    pub(crate) count: usize,
    #[serde(alias = "worldContext")]
    pub(crate) world_context: Option<Value>,
    pub(crate) theme: Option<String>,
    pub(crate) genre: Option<String>,
    pub(crate) requirements: Option<String>,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    #[serde(alias = "userId")]
    pub(crate) user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub(crate) enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub(crate) enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub(crate) web_research_query: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct OutlineRequest {
    #[serde(alias = "projectId")]
    pub(crate) project_id: String,
    #[serde(
        default = "default_outline_count",
        alias = "chapterCount",
        deserialize_with = "deserialize_usize_from_number_or_string"
    )]
    pub(crate) chapter_count: usize,
    #[serde(alias = "narrativePerspective")]
    pub(crate) narrative_perspective: Option<String>,
    #[serde(
        default = "default_target_words",
        alias = "targetWords",
        deserialize_with = "deserialize_i32_from_number_or_string"
    )]
    pub(crate) target_words: i32,
    pub(crate) requirements: Option<String>,
    #[serde(alias = "creativeMode")]
    pub(crate) creative_mode: Option<String>,
    #[serde(alias = "storyFocus")]
    pub(crate) story_focus: Option<String>,
    #[serde(alias = "plotStage")]
    pub(crate) plot_stage: Option<String>,
    #[serde(alias = "storyCreationBrief")]
    pub(crate) story_creation_brief: Option<String>,
    #[serde(alias = "qualityPreset")]
    pub(crate) quality_preset: Option<String>,
    #[serde(alias = "qualityNotes")]
    pub(crate) quality_notes: Option<String>,
    #[serde(alias = "compactMode")]
    pub(crate) compact_mode: Option<bool>,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    #[serde(alias = "userId")]
    pub(crate) user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub(crate) enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub(crate) enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub(crate) web_research_query: Option<String>,
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
pub(crate) struct RegenerateWorldBuildingRequest {
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    #[serde(alias = "userId")]
    pub(crate) user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub(crate) enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub(crate) enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub(crate) web_research_query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct CleanupWizardDataRouteRequest {
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
pub(crate) struct CleanupWizardDataRequest {
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

fn deserialize_usize_from_number_or_string<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| de::Error::custom("expected non-negative integer for usize")),
        Value::String(raw) => raw
            .trim()
            .parse::<usize>()
            .map_err(|_| de::Error::custom("expected numeric string for usize")),
        _ => Err(de::Error::custom("expected integer or numeric string")),
    }
}

fn deserialize_i32_from_number_or_string<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    deserialize_i32_from_value(value)
}

fn deserialize_optional_i32_from_number_or_string<'de, D>(
    deserializer: D,
) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(None),
        Value::String(raw) if raw.trim().is_empty() => Ok(None),
        other => deserialize_i32_from_value(other).map(Some),
    }
}

fn deserialize_i32_from_value<E>(value: Value) -> Result<i32, E>
where
    E: de::Error,
{
    match value {
        Value::Number(number) => number
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| E::custom("expected integer for i32")),
        Value::String(raw) => raw
            .trim()
            .parse::<i32>()
            .map_err(|_| E::custom("expected numeric string for i32")),
        _ => Err(E::custom("expected integer or numeric string")),
    }
}

pub(crate) fn resolve_effective_user_id(
    request_user_id: Option<String>,
    default_user_id: &str,
) -> String {
    request_user_id.unwrap_or_else(|| default_user_id.to_string())
}

pub(crate) fn normalize_genre_input(value: Option<Value>) -> String {
    match value {
        Some(Value::String(text)) => text,
        Some(Value::Array(items)) => items
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

pub(crate) fn build_cleanup_wizard_data_request_from_route_payload(
    body: CleanupWizardDataRouteRequest,
) -> CleanupWizardDataRequest {
    CleanupWizardDataRequest { body: body.body }
}

impl CleanupWizardDataRequest {
    pub(crate) fn body(&self) -> &Value {
        &self.body
    }
}

pub(crate) async fn execute_world_building_request(
    db: &DatabaseConnection,
    channel: &crate::utils::sse::SseChannel,
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

pub(crate) async fn execute_characters_request(
    db: &DatabaseConnection,
    channel: &crate::utils::sse::SseChannel,
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

pub(crate) async fn execute_regenerate_world_building_request(
    db: &DatabaseConnection,
    channel: &crate::utils::sse::SseChannel,
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
fn build_wizard_stream_route_owner_contract() -> serde_json::Value {
    json!({
        "owner": "wizard-stream",
        "rust_owner": "backend-rs/src/api/wizard.rs",
        "route_prefix": "/api",
        "routes": {
            "world_building": WIZARD_WORLD_BUILDING_ROUTE,
            "world_building_project": WIZARD_WORLD_BUILDING_PROJECT_ROUTE,
            "world_building_regenerate": WIZARD_WORLD_BUILDING_REGENERATE_ROUTE,
            "career_system": WIZARD_CAREER_SYSTEM_ROUTE,
            "characters": WIZARD_CHARACTERS_ROUTE,
            "outline": WIZARD_OUTLINE_ROUTE,
            "cleanup": WIZARD_CLEANUP_ROUTE
        },
        "method_contract": {
            "world_building": ["POST"],
            "world_building_project": ["POST"],
            "world_building_regenerate": ["POST"],
            "career_system": ["POST"],
            "characters": ["POST"],
            "outline": ["POST"],
            "cleanup": ["POST"]
        },
        "service_handoffs": {
            "wizard_runtime_owner": "backend-rs/src/api/wizard.rs",
            "career_runtime_owner": "backend-rs/src/api/careers.rs",
            "outline_runtime_owner": "backend-rs/src/api/outlines.rs",
            "project_cleanup_owner": "backend-rs/src/services/project_service.rs",
            "sse_transport_owner": "backend-rs/src/utils/sse.rs"
        },
        "readiness_probes": [
            "wizard-stream-outline-auth-guard-rust",
            "wizard-stream-world-building-auth-guard-rust",
            "wizard-stream-world-building-regenerate-auth-guard-rust",
            "wizard-stream-cleanup-auth-guard-rust",
            "wizard-stream-career-system-auth-guard-rust",
            "wizard-stream-characters-auth-guard-rust",
            "wizard-stream-setup-project-business-rust",
            "wizard-stream-world-building-business-rust",
            "wizard-stream-world-building-regenerate-business-rust",
            "wizard-stream-career-system-business-rust",
            "wizard-stream-characters-business-rust",
            "wizard-stream-outline-business-rust",
            "wizard-stream-cleanup-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-wizard-stream-business-owner",
            "business_probes": [
                "wizard-stream-setup-project-business-rust",
                "wizard-stream-world-building-business-rust",
                "wizard-stream-world-building-regenerate-business-rust",
                "wizard-stream-career-system-business-rust",
                "wizard-stream-characters-business-rust",
                "wizard-stream-outline-business-rust",
                "wizard-stream-cleanup-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "source_map_files": [],
        "next_cutover_gate": "wizard-stream route source-map shell deleted; surviving Python closeout work is outside this route group",
        "migration_policy": "Wizard-stream SSE business smoke is covered by phase5-wizard-stream-business-owner; the Python wizard-stream route shell and its explicit bootstrap rollback registration have been physically deleted.",
        "smoke_gap": "Dedicated business owner-profile smoke now exists for setup-project, world-building, world-building-regenerate, career-system, characters, outline, and cleanup; surviving Python closeout work is outside the wizard-stream route group.",
        "rollback_boundary": {
            "source_map_policy": "wizard_stream_route_source_map_deleted_after_owner_profile_business_smoke",
            "python_route_files_status": "wizard_stream_route_source_map_deleted_after_frozen_closeout",
            "python_bootstrap_status": "wizard_stream_runtime_registration_deleted_no_python_route_shell_remains",
            "source_map_freeze_status": "physical_closeout_completed",
            "source_map_physical_closeout_action": "delete_completed",
            "top_level_surviving_shells": [],
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "rollback_reference": "No Python wizard-stream route shell remains; rollback must happen through Rust route ownership or deployment changes."
        },
        "business_smoke_status": {
            "owner_profile": "phase5-wizard-stream-business-owner",
            "business_probe_count": 7,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        }
    })
}

fn spawn_world_building_stream(
    claims: Claims,
    db: DatabaseConnection,
    body: WorldBuildingRequest,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_world_building_request(&db, &channel, &user_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn world_building(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<WorldBuildingRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    spawn_world_building_stream(claims, db, body)
}

async fn world_building_with_project_id(
    Path(_project_id): Path<String>,
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<WorldBuildingRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    spawn_world_building_stream(claims, db, body)
}

async fn career_system(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<CareerSystemRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_career_system_request(&db, &channel, &user_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn characters(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<CharactersRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_characters_request(&db, &channel, &user_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn outline(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<OutlineRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_outline_request(&db, &channel, &user_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn regenerate_world_building(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Json(body): Json<RegenerateWorldBuildingRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_regenerate_world_building_request(&db, &channel, &user_id, &project_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn cleanup_wizard_data(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Json(body): Json<CleanupWizardDataRouteRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);
    let request = build_cleanup_wizard_data_request_from_route_payload(body);

    tokio::spawn(async move {
        let _raw_body = request.body();

        channel
            .progress("正在清理旧的向导数据...", 0, "processing")
            .await;

        match ProjectService::cleanup_wizard_data(&db, &project_id, &claims.sub).await {
            Ok(Some(deleted)) => {
                let response = json!({
                    "message": "向导旧数据清理完成",
                    "deleted": {
                        "characters": deleted.characters,
                        "outlines": deleted.outlines,
                        "chapters": deleted.chapters,
                    }
                });
                channel.progress("清理完成", 100, "success").await;
                channel.result(&response).await;
                channel.done().await;
            }
            Ok(None) => {
                channel.error("项目不存在或无权访问", 404).await;
            }
            Err(e) => {
                channel.error(&format!("清理失败: {}", e), 500).await;
            }
        }
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

pub fn routes() -> Router {
    Router::new()
        .route(WIZARD_WORLD_BUILDING_ROUTE, post(world_building))
        .route(
            WIZARD_WORLD_BUILDING_PROJECT_ROUTE,
            post(world_building_with_project_id),
        )
        .route(
            WIZARD_WORLD_BUILDING_REGENERATE_ROUTE,
            post(regenerate_world_building),
        )
        .route(WIZARD_CAREER_SYSTEM_ROUTE, post(career_system))
        .route(WIZARD_CHARACTERS_ROUTE, post(characters))
        .route(WIZARD_OUTLINE_ROUTE, post(outline))
        .route(WIZARD_CLEANUP_ROUTE, post(cleanup_wizard_data))
}

#[cfg(test)]
mod tests {
    use super::{
        build_cleanup_wizard_data_request_from_route_payload,
        build_wizard_stream_route_owner_contract, normalize_genre_input, resolve_effective_user_id,
        CharactersRequest, CleanupWizardDataRouteRequest, OutlineRequest, WorldBuildingRequest,
        WIZARD_CAREER_SYSTEM_ROUTE, WIZARD_CHARACTERS_ROUTE, WIZARD_CLEANUP_ROUTE,
        WIZARD_OUTLINE_ROUTE, WIZARD_WORLD_BUILDING_PROJECT_ROUTE,
        WIZARD_WORLD_BUILDING_REGENERATE_ROUTE, WIZARD_WORLD_BUILDING_ROUTE,
    };
    use serde_json::json;

    #[test]
    fn should_publish_wizard_stream_route_owner_contract() {
        let contract = build_wizard_stream_route_owner_contract();

        assert_eq!(contract["owner"], "wizard-stream");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/wizard.rs");
        assert_eq!(
            contract["routes"]["world_building"],
            WIZARD_WORLD_BUILDING_ROUTE
        );
        assert_eq!(
            contract["routes"]["world_building_regenerate"],
            WIZARD_WORLD_BUILDING_REGENERATE_ROUTE
        );
        assert_eq!(
            contract["routes"]["career_system"],
            WIZARD_CAREER_SYSTEM_ROUTE
        );
        assert_eq!(contract["routes"]["characters"], WIZARD_CHARACTERS_ROUTE);
        assert_eq!(contract["routes"]["outline"], WIZARD_OUTLINE_ROUTE);
        assert_eq!(contract["routes"]["cleanup"], WIZARD_CLEANUP_ROUTE);
        assert_eq!(contract["readiness_probes"].as_array().unwrap().len(), 13);
        assert_eq!(
            contract["readiness_probes"][12],
            "wizard-stream-cleanup-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-wizard-stream-business-owner"
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("wizard stream business probes should be present");
        assert_eq!(business_probes.len(), 7);
        assert_eq!(
            contract["owner_profile"]["business_probes"][2],
            "wizard-stream-world-building-regenerate-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 0);
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_status"],
            "physical_closeout_completed"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_physical_closeout_action"],
            "delete_completed"
        );
        assert_eq!(
            contract["rollback_boundary"]["top_level_surviving_shells"],
            json!([])
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(true)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(7)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "wizard-stream route source-map shell deleted; surviving Python closeout work is outside this route group"
        );
        assert_eq!(
            contract["migration_policy"],
            "Wizard-stream SSE business smoke is covered by phase5-wizard-stream-business-owner; the Python wizard-stream route shell and its explicit bootstrap rollback registration have been physically deleted."
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"],
            json!([])
        );
        assert!(contract["smoke_gap"]
            .as_str()
            .unwrap_or_default()
            .contains("surviving Python closeout work is outside"));
    }

    #[test]
    fn should_keep_wizard_stream_route_group_paths_stable() {
        assert_eq!(WIZARD_WORLD_BUILDING_ROUTE, "/wizard-stream/world-building");
        assert_eq!(
            WIZARD_WORLD_BUILDING_PROJECT_ROUTE,
            "/wizard-stream/world-building/{project_id}"
        );
        assert_eq!(
            WIZARD_WORLD_BUILDING_REGENERATE_ROUTE,
            "/wizard-stream/world-building/{project_id}/regenerate"
        );
        assert_eq!(WIZARD_CAREER_SYSTEM_ROUTE, "/wizard-stream/career-system");
        assert_eq!(WIZARD_CHARACTERS_ROUTE, "/wizard-stream/characters");
        assert_eq!(WIZARD_OUTLINE_ROUTE, "/wizard-stream/outline");
        assert_eq!(WIZARD_CLEANUP_ROUTE, "/wizard-stream/cleanup/{project_id}");
    }

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
    fn wizard_outline_request_accepts_numeric_string_counts_from_background_payload() {
        let request: OutlineRequest = serde_json::from_value(json!({
            "project_id": "project-1",
            "chapter_count": "5",
            "targetWords": "120000",
            "narrative_perspective": "third_person",
        }))
        .expect("outline request should accept numeric strings");

        assert_eq!(request.project_id, "project-1");
        assert_eq!(request.chapter_count, 5);
        assert_eq!(request.target_words, 120000);
        assert_eq!(
            request.narrative_perspective.as_deref(),
            Some("third_person")
        );
    }

    #[test]
    fn wizard_character_and_world_requests_accept_numeric_string_counts() {
        let characters: CharactersRequest = serde_json::from_value(json!({
            "project_id": "project-1",
            "count": "7",
        }))
        .expect("characters request should accept numeric count strings");
        assert_eq!(characters.count, 7);

        let world: WorldBuildingRequest = serde_json::from_value(json!({
            "chapterCount": "5",
            "characterCount": "8",
            "targetWords": "90000",
        }))
        .expect("world request should accept numeric option strings");
        assert_eq!(world.chapter_count, Some(5));
        assert_eq!(world.character_count, Some(8));
        assert_eq!(world.target_words, Some(90000));
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
