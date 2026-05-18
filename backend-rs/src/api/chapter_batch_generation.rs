use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{sse::KeepAlive, Json, Sse},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::chapter_batch_generation_error_mapper::{
    map_active_batch_generation_query_error, map_batch_generation_status_query_error,
    map_active_batch_generation_task_list_query_error,
    map_batch_generation_status_stream_access_error,
    map_cancel_batch_generation_workflow_error,
    map_create_batch_generation_workflow_error,
    map_prepare_batch_generation_resume_request_error,
    map_single_chapter_generation_request_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_batch_generation_active_list_query_service::{
    load_active_batch_generation_task_list_query,
};
use crate::services::chapter_batch_generation_active_query_service::load_active_batch_generation_query;
use crate::services::chapter_batch_generation_request_compat_service::{
    consume_batch_generation_request_compat_fields, BatchGenerationRequestCompatFields,
};
use crate::services::chapter_batch_generation_cancel_service::cancel_owned_batch_generation_task;
use crate::services::chapter_batch_generation_create_workflow_service::{
    create_batch_generation_workflow, BatchGenerationCreateWorkflowRequest,
};
use crate::services::chapter_batch_generation_dispatch_service::{
    dispatch_batch_generation_runtime, dispatch_single_chapter_generation_runtime,
};
use crate::services::chapter_batch_generation_resume_service::{
    prepare_batch_generation_resume_request,
};
use crate::services::chapter_batch_generation_task_command_service::{
    create_single_generation_background_task_plan,
    ResumeExecutionPlan,
};
use crate::services::chapter_batch_generation_status_query_service::load_batch_generation_status_query;
use crate::services::chapter_batch_generation_status_stream_service::build_batch_generation_status_stream;
use crate::services::chapter_batch_generation_stream_access_service::ensure_batch_generation_status_stream_access;
use crate::services::chapter_single_generation_request_service::{
    prepare_single_chapter_generation_request, SingleChapterGenerationRequest,
};
use crate::services::chapter_single_generation_stream_service::build_single_chapter_generation_stream;

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

async fn create_batch_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<BatchGenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    consume_batch_generation_request_compat_fields(&BatchGenerationRequestCompatFields {
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
    });

    let workflow = create_batch_generation_workflow(
        &db,
        &project_id,
        &claims.sub,
        &BatchGenerationCreateWorkflowRequest {
            start_chapter_number: body.start_chapter_number,
            count: body.count,
            style_id: body.style_id,
            target_word_count: body.target_word_count,
            enable_analysis: body.enable_analysis,
            max_retries: body.max_retries,
            model_override: body.model.clone(),
        },
    )
    .await
    .map_err(map_create_batch_generation_workflow_error)?;

    dispatch_batch_generation_runtime(
        db.clone(),
        workflow.created_task_id.clone(),
        claims.sub.clone(),
        workflow.chapter_ids,
        workflow.target_word_count,
        workflow.ai_config,
        workflow.provider_payload,
    );

    Ok(Json(workflow.response_payload))
}

async fn generate_chapter_content_background(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ChapterGenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _ = body.enable_analysis;

    let prepared = prepare_single_chapter_generation_request(
        &db,
        &chapter_id,
        &claims.sub,
        &SingleChapterGenerationRequest {
            target_word_count: body.target_word_count,
            model: body.model.clone(),
        },
    )
    .await
        .map_err(map_single_chapter_generation_request_error)?;

    let plan = create_single_generation_background_task_plan(
        &db,
        &claims.sub,
        &prepared.chapter_model,
        prepared.target_word_count,
    )
    .await
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        )
    })?;

    dispatch_single_chapter_generation_runtime(
        db.clone(),
        plan.created_task.id.clone(),
        claims.sub.clone(),
        prepared.chapter_model.id.clone(),
        plan.target_word_count,
        prepared.ai_config,
        prepared.provider_payload,
    );

    Ok(Json(plan.response_payload))
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
    let prepared = prepare_single_chapter_generation_request(
        &db,
        &chapter_id,
        &claims.sub,
        &SingleChapterGenerationRequest {
            target_word_count: body.target_word_count,
            model: body.model.clone(),
        },
    )
    .await
    .map_err(map_single_chapter_generation_request_error)?;
    let stream = build_single_chapter_generation_stream(
        db.clone(),
        claims.sub.clone(),
        prepared.chapter_model.id.clone(),
        prepared.target_word_count,
        prepared.ai_config,
        prepared.provider_payload,
    );

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new().interval(std::time::Duration::from_secs(10)),
    ))
}

async fn get_batch_generation_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_batch_generation_status_query(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_batch_generation_status_query_error)?;

    Ok(Json(result.response_payload))
}

async fn stream_batch_generation_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    ensure_batch_generation_status_stream_access(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_batch_generation_status_stream_access_error)?;

    let stream =
        build_batch_generation_status_stream(db.clone(), batch_id.clone(), claims.sub.clone());

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

async fn get_active_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_active_batch_generation_query(&db, &project_id, &claims.sub)
        .await
        .map_err(map_active_batch_generation_query_error)?;

    Ok(Json(result.response_payload))
}

async fn list_active_batch_generation_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ActiveQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let result = load_active_batch_generation_task_list_query(&db, &claims.sub, limit)
        .await
        .map_err(map_active_batch_generation_task_list_query_error)?;
    Ok(Json(result.response_payload))
}

async fn cancel_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = cancel_owned_batch_generation_task(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_cancel_batch_generation_workflow_error)?;

    Ok(Json(result.response_payload))
}

async fn resume_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let prepared = prepare_batch_generation_resume_request(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_prepare_batch_generation_resume_request_error)?;
    let response_payload = prepared.response_payload.clone();
    let ai_config = prepared.ai_config;
    let provider_payload = prepared.provider_payload;

    match prepared.execution {
        ResumeExecutionPlan::SingleChapter {
            chapter_id,
            target_word_count,
            user_id,
        } => {
            dispatch_single_chapter_generation_runtime(
                db.clone(),
                batch_id.clone(),
                user_id,
                chapter_id,
                target_word_count,
                ai_config,
                provider_payload,
            );
        }
        ResumeExecutionPlan::Batch {
            chapter_ids,
            target_word_count,
            user_id,
        } => {
            dispatch_batch_generation_runtime(
                db.clone(),
                batch_id.clone(),
                user_id,
                chapter_ids,
                target_word_count,
                ai_config,
                provider_payload,
            );
        }
    }

    Ok(Json(response_payload))
}

pub fn routes() -> Router {
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
