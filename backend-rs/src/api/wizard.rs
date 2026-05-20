use axum::response::sse::Event;
use axum::{
    extract::{Extension, Path},
    response::Sse,
    routing::post,
    Json, Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use tokio::sync::mpsc;

use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::project_service::ProjectService;
use crate::services::wizard_service;

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct WorldBuildingRequest {
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) theme: Option<String>,
    pub(crate) genre: Option<serde_json::Value>,
    #[serde(alias = "narrativePerspective")]
    pub(crate) narrative_perspective: Option<String>,
    #[serde(alias = "targetWords")]
    pub(crate) target_words: Option<i32>,
    #[serde(alias = "chapterCount")]
    pub(crate) chapter_count: Option<i32>,
    #[serde(alias = "characterCount")]
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
pub(crate) struct CareerSystemRequest {
    #[serde(alias = "projectId")]
    pub(crate) project_id: String,
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
    #[serde(default = "default_count")]
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

fn default_count() -> usize {
    5
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct OutlineRequest {
    #[serde(alias = "projectId")]
    pub(crate) project_id: String,
    #[serde(default = "default_outline_count")]
    #[serde(alias = "chapterCount")]
    pub(crate) chapter_count: usize,
    #[serde(alias = "narrativePerspective")]
    pub(crate) narrative_perspective: Option<String>,
    #[serde(default = "default_target_words")]
    #[serde(alias = "targetWords")]
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

fn default_outline_count() -> usize {
    3
}
fn default_target_words() -> i32 {
    100000
}

pub(crate) fn normalize_genre_input(value: Option<serde_json::Value>) -> String {
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

fn spawn_world_building_stream(
    claims: Claims,
    db: DatabaseConnection,
    body: WorldBuildingRequest,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = body.user_id.unwrap_or_else(|| claims.sub.clone());
    let title = body.title.unwrap_or_default();
    let description = body.description.unwrap_or_default();
    let theme = body.theme.unwrap_or_default();
    let genre = normalize_genre_input(body.genre);

    tokio::spawn(async move {
        wizard_service::generate_world_building(
            &db,
            &channel,
            &user_id,
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
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

#[derive(Deserialize)]
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

    let user_id = body.user_id.unwrap_or_else(|| claims.sub.clone());
    let project_id = body.project_id;

    tokio::spawn(async move {
        wizard_service::generate_career_system(
            &db,
            &channel,
            &user_id,
            &project_id,
            body.provider.as_deref(),
            body.model.as_deref(),
        )
        .await;
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

    let user_id = body.user_id.unwrap_or_else(|| claims.sub.clone());
    let project_id = body.project_id;
    let count = body.count;

    tokio::spawn(async move {
        wizard_service::generate_characters(
            &db,
            &channel,
            &user_id,
            &project_id,
            count,
            body.world_context,
            body.theme.as_deref(),
            body.genre.as_deref(),
            body.requirements.as_deref(),
            body.provider.as_deref(),
            body.model.as_deref(),
        )
        .await;
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

    let user_id = body.user_id.unwrap_or_else(|| claims.sub.clone());
    let project_id = body.project_id;
    let chapter_count = body.chapter_count;
    let target_words = body.target_words;

    tokio::spawn(async move {
        wizard_service::generate_outline(
            &db,
            &channel,
            &user_id,
            &project_id,
            chapter_count,
            body.narrative_perspective.as_deref(),
            target_words,
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

    let user_id = body.user_id.unwrap_or_else(|| claims.sub.clone());

    tokio::spawn(async move {
        wizard_service::regenerate_world_building(
            &db,
            &channel,
            &user_id,
            &project_id,
            body.provider.as_deref(),
            body.model.as_deref(),
        )
        .await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn cleanup_wizard_data(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Json(_body): Json<Value>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    tokio::spawn(async move {
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
        .route("/wizard-stream/world-building", post(world_building))
        .route(
            "/wizard-stream/world-building/{project_id}",
            post(world_building_with_project_id),
        )
        .route(
            "/wizard-stream/world-building/{project_id}/regenerate",
            post(regenerate_world_building),
        )
        .route("/wizard-stream/career-system", post(career_system))
        .route("/wizard-stream/characters", post(characters))
        .route("/wizard-stream/outline", post(outline))
        .route(
            "/wizard-stream/cleanup/{project_id}",
            post(cleanup_wizard_data),
        )
}
