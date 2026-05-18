use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive},
        Json, Sse,
    },
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;
use tokio::time::Duration as TokioDuration;

use crate::api::chapter_regeneration_query_error_mapper::map_regeneration_tasks_query_error;
use crate::api::chapters_error_mapper::{
    map_apply_partial_regenerate_error, map_prepare_chapter_regeneration_stream_error,
    map_prepare_partial_regeneration_stream_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_access_http_service::load_accessible_chapter_or_404;
use crate::services::chapter_regeneration_apply_service::apply_partial_regenerate_payload;
use crate::services::chapter_regeneration_full_stream_service::{
    build_full_chapter_regeneration_stream, FullChapterRegenerationStreamInput,
};
use crate::services::chapter_regeneration_partial_stream_service::{
    build_partial_chapter_regeneration_stream, PartialChapterRegenerationStreamInput,
};
use crate::services::chapter_regeneration_prepare_service::{
    prepare_chapter_regeneration_stream, prepare_partial_regeneration_stream,
};
use crate::services::chapter_regeneration_query_service::load_regeneration_tasks_payload;

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
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let payload = apply_partial_regenerate_payload(
        &db,
        &chapter_id,
        &claims.sub,
        &chapter,
        body.new_text.as_deref().unwrap_or_default(),
        body.start_position.unwrap_or(0),
        body.end_position.unwrap_or(0),
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
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let prepared = prepare_chapter_regeneration_stream(&db, &claims.sub, &chapter, &body)
        .await
        .map_err(map_prepare_chapter_regeneration_stream_error)?;

    let stream = build_full_chapter_regeneration_stream(FullChapterRegenerationStreamInput {
        task_label: "Chapter Rewrite".to_string(),
        chapter_id: chapter_id.clone(),
        chapter_word_count: chapter.word_count as usize,
        prompt: prepared.prompt,
        ai_service: prepared.ai_service,
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(TokioDuration::from_secs(10))))
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
    let chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let stream_prepared = prepare_partial_regeneration_stream(
        &db,
        &claims.sub,
        &chapter,
        &body.selected_text,
        body.start_position,
        body.end_position,
        body.context_chars.unwrap_or(500),
        &body.user_instructions,
        body.length_mode.as_deref(),
        body.target_word_count,
        body.style_id,
        body.enable_web_research.unwrap_or(false),
        body.web_research_query.as_deref(),
    )
    .await
    .map_err(map_prepare_partial_regeneration_stream_error)?;
    let prepared = stream_prepared.prepared;
    let stream = build_partial_chapter_regeneration_stream(
        PartialChapterRegenerationStreamInput {
            target_words: prepared.target_words,
            original_word_count: prepared.original_word_count,
            start_position: body.start_position,
            end_position: body.end_position,
            prompt: prepared.prompt,
            ai_service: stream_prepared.ai_service,
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(TokioDuration::from_secs(10))))
}

async fn get_regeneration_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<RegenerationTasksQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _chapter = load_accessible_chapter_or_404(&db, &chapter_id, &claims.sub).await?;
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let payload = load_regeneration_tasks_payload(&db, &chapter_id, limit)
        .await
        .map_err(map_regeneration_tasks_query_error)?;
    Ok(Json(payload))
}

pub fn routes() -> Router {
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
