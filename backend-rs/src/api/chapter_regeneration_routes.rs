use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{sse::Event, Json, Sse},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;

use crate::api::chapter_regeneration_query_error_mapper::map_regeneration_tasks_query_error;
use crate::api::chapters_error_mapper::{
    map_apply_partial_regenerate_error, map_create_chapter_regeneration_stream_workflow_error,
    map_create_partial_regeneration_stream_workflow_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_regeneration_apply_service::{
    apply_owned_partial_regenerate_payload,
    ApplyPartialRegenerateRequest as ApplyPartialRegenerateServiceRequest,
};
use crate::services::chapter_regeneration_query_service::load_owned_regeneration_tasks_payload;
use crate::services::chapter_regeneration_stream_workflow_service::{
    create_chapter_regeneration_stream_workflow, create_partial_regeneration_stream_workflow,
    PartialRegenerationStreamWorkflowRequest,
};
use crate::utils::sse::default_sse_keep_alive;

#[derive(Deserialize)]
struct ApplyPartialRegenerateRequest {
    new_text: Option<String>,
    start_position: Option<usize>,
    end_position: Option<usize>,
}

#[derive(Deserialize)]
struct PartialRegenerateRequest {
    selected_text: String,
    start_position: usize,
    end_position: usize,
    user_instructions: String,
    context_chars: Option<usize>,
    style_id: Option<i32>,
    length_mode: Option<String>,
    target_word_count: Option<usize>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
}

#[derive(Deserialize)]
struct RegenerationTasksQuery {
    limit: Option<u64>,
}

async fn apply_partial_regenerate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ApplyPartialRegenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = apply_owned_partial_regenerate_payload(
        &db,
        &chapter_id,
        &claims.sub,
        ApplyPartialRegenerateServiceRequest {
            new_text: body.new_text.as_deref(),
            start_position: body.start_position,
            end_position: body.end_position,
        },
    )
    .await
    .map_err(map_apply_partial_regenerate_error)?;
    Ok(Json(payload))
}

async fn regenerate_chapter_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let stream = create_chapter_regeneration_stream_workflow(&db, &claims.sub, &chapter_id, &body)
        .await
        .map_err(map_create_chapter_regeneration_stream_workflow_error)?;

    Ok(Sse::new(stream).keep_alive(default_sse_keep_alive()))
}

async fn partial_regenerate_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<PartialRegenerateRequest>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let stream = create_partial_regeneration_stream_workflow(
        &db,
        &claims.sub,
        &chapter_id,
        PartialRegenerationStreamWorkflowRequest {
            selected_text: &body.selected_text,
            start_position: body.start_position,
            end_position: body.end_position,
            context_chars: body.context_chars,
            user_instructions: &body.user_instructions,
            length_mode: body.length_mode.as_deref(),
            target_word_count: body.target_word_count,
            style_id: body.style_id,
            enable_web_research: body.enable_web_research,
            web_research_query: body.web_research_query.as_deref(),
        },
    )
    .await
    .map_err(map_create_partial_regeneration_stream_workflow_error)?;

    Ok(Sse::new(stream).keep_alive(default_sse_keep_alive()))
}

async fn get_regeneration_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<RegenerationTasksQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_owned_regeneration_tasks_payload(&db, &chapter_id, &claims.sub, query.limit)
        .await
        .map_err(map_regeneration_tasks_query_error)?;
    Ok(Json(payload))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(
            "/chapters/{chapter_id}/regenerate-stream",
            post(regenerate_chapter_stream),
        )
        .route(
            "/chapters/{chapter_id}/partial-regenerate-stream",
            post(partial_regenerate_stream),
        )
        .route(
            "/chapters/{chapter_id}/apply-partial-regenerate",
            post(apply_partial_regenerate),
        )
        .route(
            "/chapters/{chapter_id}/regeneration/tasks",
            get(get_regeneration_tasks),
        )
}
