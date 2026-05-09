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

use serde_json::Value;

use crate::services::auth::Claims;
use crate::services::wizard_service;

#[derive(Deserialize)]
#[allow(dead_code)]
struct WorldBuildingRequest {
    title: Option<String>,
    description: Option<String>,
    theme: Option<String>,
    genre: Option<String>,
    narrative_perspective: Option<String>,
    target_words: Option<i32>,
    chapter_count: Option<i32>,
    character_count: Option<i32>,
    outline_mode: Option<String>,
    default_creative_mode: Option<String>,
    default_story_focus: Option<String>,
    default_plot_stage: Option<String>,
    default_story_creation_brief: Option<String>,
    default_quality_preset: Option<String>,
    default_quality_notes: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    user_id: Option<String>,
    enable_mcp: Option<bool>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct CareerSystemRequest {
    project_id: String,
    provider: Option<String>,
    model: Option<String>,
    user_id: Option<String>,
    enable_mcp: Option<bool>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct CharactersRequest {
    project_id: String,
    #[serde(default = "default_count")]
    count: usize,
    world_context: Option<Value>,
    theme: Option<String>,
    genre: Option<String>,
    requirements: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    user_id: Option<String>,
    enable_mcp: Option<bool>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
}

fn default_count() -> usize {
    5
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OutlineRequest {
    project_id: String,
    #[serde(default = "default_outline_count")]
    chapter_count: usize,
    narrative_perspective: Option<String>,
    #[serde(default = "default_target_words")]
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
    user_id: Option<String>,
    enable_mcp: Option<bool>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
}

fn default_outline_count() -> usize {
    3
}
fn default_target_words() -> i32 {
    100000
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
    let genre = body.genre.unwrap_or_default();

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
struct RegenerateWorldBuildingRequest {
    provider: Option<String>,
    model: Option<String>,
    user_id: Option<String>,
    enable_mcp: Option<bool>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
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
}
