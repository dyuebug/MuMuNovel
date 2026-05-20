use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{Json, Sse},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;

use crate::api::chapter_batch_generation_error_mapper::{
    map_active_batch_generation_query_error, map_active_batch_generation_task_list_query_error,
    map_batch_generation_status_query_error, map_batch_generation_status_stream_access_error,
    map_cancel_batch_generation_workflow_error, map_create_batch_generation_workflow_error,
    map_create_single_generation_background_workflow_error,
    map_prepare_batch_generation_resume_request_error, map_single_chapter_generation_request_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_batch_generation_active_list_query_service::load_owned_active_batch_generation_task_list_query;
use crate::services::chapter_batch_generation_active_query_service::load_active_batch_generation_query;
use crate::services::chapter_batch_generation_cancel_service::cancel_owned_batch_generation_task;
use crate::services::chapter_batch_generation_create_workflow_service::start_owned_batch_generation_workflow;
use crate::services::chapter_batch_generation_request_compat_service::BatchGenerationRequestCompatFields;
use crate::services::chapter_batch_generation_resume_service::resume_owned_batch_generation_task;
use crate::services::chapter_batch_generation_status_query_service::load_batch_generation_status_query;
use crate::services::chapter_batch_generation_status_stream_service::create_owned_batch_generation_status_stream;
use crate::services::chapter_single_generation_background_workflow_service::start_owned_single_generation_background_workflow;
use crate::services::chapter_single_generation_request_service::{
    build_single_chapter_generation_request, consume_single_chapter_generation_request_compat_fields,
    SingleChapterGenerationRequest, SingleChapterGenerationRequestCompatFields,
};
use crate::services::chapter_single_generation_stream_workflow_service::create_single_generation_stream_workflow;
use crate::utils::sse::{default_sse_keep_alive, named_sse_keep_alive};

#[derive(Deserialize)]
struct BatchGenerateRequest {
    start_chapter_number: i32,
    count: i32,
    style_id: Option<i32>,
    target_word_count: Option<i32>,
    enable_analysis: Option<bool>,
    enable_mcp: Option<bool>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
    max_retries: Option<i32>,
    model: Option<String>,
    creative_mode: Option<String>,
    story_focus: Option<String>,
    plot_stage: Option<String>,
    story_creation_brief: Option<String>,
    quality_preset: Option<String>,
    quality_notes: Option<String>,
    story_repair_summary: Option<String>,
    story_repair_targets: Option<Vec<String>>,
    story_preserve_strengths: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ActiveQuery {
    limit: Option<u64>,
}

#[derive(Deserialize)]
struct ChapterGenerateRequest {
    target_word_count: Option<i32>,
    model: Option<String>,
    #[serde(default)]
    enable_analysis: Option<bool>,
}

fn build_batch_generation_request_compat_fields(
    body: &BatchGenerateRequest,
) -> BatchGenerationRequestCompatFields {
    BatchGenerationRequestCompatFields {
        enable_mcp: body.enable_mcp,
        enable_web_research: body.enable_web_research,
        web_research_query: body.web_research_query.clone(),
        creative_mode: body.creative_mode.clone(),
        story_focus: body.story_focus.clone(),
        plot_stage: body.plot_stage.clone(),
        story_creation_brief: body.story_creation_brief.clone(),
        quality_preset: body.quality_preset.clone(),
        quality_notes: body.quality_notes.clone(),
        story_repair_summary: body.story_repair_summary.clone(),
        story_repair_targets: body.story_repair_targets.clone(),
        story_preserve_strengths: body.story_preserve_strengths.clone(),
    }
}

async fn create_batch_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<BatchGenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = start_owned_batch_generation_workflow(
        &db,
        &project_id,
        &claims.sub,
        body.start_chapter_number,
        body.count,
        body.style_id,
        body.target_word_count,
        body.enable_analysis,
        body.max_retries,
        body.model.clone(),
        build_batch_generation_request_compat_fields(&body),
    )
    .await
    .map_err(map_create_batch_generation_workflow_error)?;

    Ok(Json(result))
}

async fn generate_chapter_content_background(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ChapterGenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    consume_single_chapter_generation_request_compat_fields(
        &SingleChapterGenerationRequestCompatFields {
            enable_analysis: body.enable_analysis,
        },
    );
    let request = build_single_chapter_generation_request(body.target_word_count, body.model.clone());
    let result = start_owned_single_generation_background_workflow(
        &db,
        &chapter_id,
        &claims.sub,
        request,
    )
    .await
    .map_err(map_create_single_generation_background_workflow_error)?;

    Ok(Json(result))
}

async fn generate_chapter_content_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ChapterGenerateRequest>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    consume_single_chapter_generation_request_compat_fields(
        &SingleChapterGenerationRequestCompatFields {
            enable_analysis: body.enable_analysis,
        },
    );
    let request: SingleChapterGenerationRequest = build_single_chapter_generation_request(
        body.target_word_count,
        body.model.clone(),
    );
    let stream = create_single_generation_stream_workflow(
        db.clone(),
        claims.sub.clone(),
        chapter_id.clone(),
        request,
    )
    .await
    .map_err(map_single_chapter_generation_request_error)?;

    Ok(Sse::new(stream).keep_alive(default_sse_keep_alive()))
}

async fn get_batch_generation_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_batch_generation_status_query(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_batch_generation_status_query_error)?;

    Ok(Json(result))
}

async fn stream_batch_generation_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let stream = create_owned_batch_generation_status_stream(
        db.clone(),
        batch_id.clone(),
        claims.sub.clone(),
    )
    .await
    .map_err(map_batch_generation_status_stream_access_error)?;

    Ok(Sse::new(stream).keep_alive(named_sse_keep_alive("keep-alive")))
}

async fn get_active_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_active_batch_generation_query(&db, &project_id, &claims.sub)
        .await
        .map_err(map_active_batch_generation_query_error)?;

    Ok(Json(result))
}

async fn list_active_batch_generation_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ActiveQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_owned_active_batch_generation_task_list_query(&db, &claims.sub, query.limit)
        .await
        .map_err(map_active_batch_generation_task_list_query_error)?;
    Ok(Json(result))
}

async fn cancel_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = cancel_owned_batch_generation_task(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_cancel_batch_generation_workflow_error)?;

    Ok(Json(result))
}

async fn resume_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = resume_owned_batch_generation_task(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_prepare_batch_generation_resume_request_error)?;

    Ok(Json(result))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(
            "/chapters/project/{project_id}/batch-generate",
            post(create_batch_generate),
        )
        .route(
            "/chapters/{chapter_id}/generate-stream",
            post(generate_chapter_content_stream),
        )
        .route(
            "/chapters/{chapter_id}/generate-background",
            post(generate_chapter_content_background),
        )
        .route(
            "/chapters/batch-generate/{batch_id}/status",
            get(get_batch_generation_status),
        )
        .route(
            "/chapters/batch-generate/{batch_id}/stream",
            get(stream_batch_generation_status),
        )
        .route(
            "/chapters/project/{project_id}/batch-generate/active",
            get(get_active_batch_generation),
        )
        .route(
            "/chapters/batch-generate/active-tasks",
            get(list_active_batch_generation_tasks),
        )
        .route(
            "/chapters/batch-generate/{batch_id}/cancel",
            post(cancel_batch_generation),
        )
        .route(
            "/chapters/batch-generate/{batch_id}/resume",
            post(resume_batch_generation),
        )
}
