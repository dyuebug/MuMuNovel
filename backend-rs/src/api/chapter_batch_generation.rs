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
    map_active_batch_generation_task_list_query_error, map_cancel_batch_generation_workflow_error,
    map_create_batch_generation_workflow_error, map_owned_batch_generation_task_route_error,
    map_project_access_query_route_error, map_resume_batch_generation_task_command_config_route_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_batch_generation_cancel_service::cancel_owned_batch_generation_task;
use crate::services::chapter_batch_generation_read_context_service::load_owned_batch_generation_status_payload;
use crate::services::chapter_batch_generation_status_stream_service::{
    load_owned_batch_generation_status_stream,
};
use crate::services::chapter_batch_generation_task_view_query_service::{
    load_active_batch_generation_query, load_active_user_batch_generation_task_list_view,
    ActiveBatchGenerationTaskListQueryRequest,
};
use crate::services::chapter_batch_generation_write_workflow_service::{
    resume_owned_batch_generation_write_workflow, start_owned_batch_generation_write_workflow,
    BatchGenerationCreateWorkflowRequest,
};
use crate::utils::sse::named_sse_keep_alive;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct ActiveBatchGenerationTaskListRouteQuery {
    limit: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct BatchGenerationCreateRouteRequest {
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

async fn create_batch_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<BatchGenerationCreateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = BatchGenerationCreateWorkflowRequest::from_route_payload(
        body.start_chapter_number,
        body.count,
        body.style_id,
        body.target_word_count,
        body.enable_analysis,
        body.enable_mcp,
        body.enable_web_research,
        body.web_research_query,
        body.max_retries,
        body.model,
        body.creative_mode,
        body.story_focus,
        body.plot_stage,
        body.story_creation_brief,
        body.quality_preset,
        body.quality_notes,
        body.story_repair_summary,
        body.story_repair_targets,
        body.story_preserve_strengths,
    );
    let result =
        start_owned_batch_generation_write_workflow(&db, &project_id, &claims.sub, request)
            .await
            .map_err(map_create_batch_generation_workflow_error)?;

    Ok(Json(result))
}

async fn get_batch_generation_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_owned_batch_generation_status_payload(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_owned_batch_generation_task_route_error)?;

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
    let stream = load_owned_batch_generation_status_stream(
        db.clone(),
        batch_id.clone(),
        claims.sub.clone(),
    )
    .await
    .map_err(map_owned_batch_generation_task_route_error)?;

    Ok(Sse::new(stream).keep_alive(named_sse_keep_alive("keep-alive")))
}

async fn get_active_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_active_batch_generation_query(&db, &project_id, &claims.sub)
        .await
        .map_err(map_project_access_query_route_error)?;

    Ok(Json(result))
}

async fn list_active_batch_generation_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ActiveBatchGenerationTaskListRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = ActiveBatchGenerationTaskListQueryRequest::from_route_limit(query.limit);
    let result = load_active_user_batch_generation_task_list_view(&db, &claims.sub, request)
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
    let result = resume_owned_batch_generation_write_workflow(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_resume_batch_generation_task_command_config_route_error)?;

    Ok(Json(result))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(
            "/chapters/project/{project_id}/batch-generate",
            post(create_batch_generate),
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

#[cfg(test)]
mod tests {
    use crate::services::chapter_batch_generation_write_workflow_service::BatchGenerationCreateWorkflowRequest;
    use crate::services::chapter_batch_generation_task_view_query_service::ActiveBatchGenerationTaskListQueryRequest;

    use super::{ActiveBatchGenerationTaskListRouteQuery, BatchGenerationCreateRouteRequest};

    #[test]
    fn should_normalize_active_batch_generation_task_list_limit() {
        let default_request = ActiveBatchGenerationTaskListQueryRequest::from_route_limit(
            ActiveBatchGenerationTaskListRouteQuery { limit: None }.limit,
        )
        .limit();
        let min_request = ActiveBatchGenerationTaskListQueryRequest::from_route_limit(
            ActiveBatchGenerationTaskListRouteQuery { limit: Some(0) }.limit,
        )
        .limit();
        let preserved_request = ActiveBatchGenerationTaskListQueryRequest::from_route_limit(
            ActiveBatchGenerationTaskListRouteQuery { limit: Some(25) }.limit,
        )
        .limit();
        let capped_request = ActiveBatchGenerationTaskListQueryRequest::from_route_limit(
            ActiveBatchGenerationTaskListRouteQuery { limit: Some(500) }.limit,
        )
        .limit();

        assert_eq!(default_request, 20);
        assert_eq!(min_request, 1);
        assert_eq!(preserved_request, 25);
        assert_eq!(capped_request, 100);
    }

    #[test]
    fn should_keep_batch_generation_create_route_payload_contract() {
        let route_request = BatchGenerationCreateRouteRequest {
            start_chapter_number: 5,
            count: 3,
            style_id: Some(9),
            target_word_count: Some(3200),
            enable_analysis: Some(true),
            enable_mcp: Some(true),
            enable_web_research: Some(false),
            web_research_query: Some("ignored".to_string()),
            max_retries: Some(6),
            model: Some("gpt-4.1-mini".to_string()),
            creative_mode: Some("dramatic".to_string()),
            story_focus: Some("battle".to_string()),
            plot_stage: Some("climax".to_string()),
            story_creation_brief: Some("brief".to_string()),
            quality_preset: Some("strict".to_string()),
            quality_notes: Some("notes".to_string()),
            story_repair_summary: Some("repair".to_string()),
            story_repair_targets: Some(vec!["target-a".to_string()]),
            story_preserve_strengths: Some(vec!["strength-a".to_string()]),
        };
        let request = BatchGenerationCreateWorkflowRequest::from_route_payload(
            route_request.start_chapter_number,
            route_request.count,
            route_request.style_id,
            route_request.target_word_count,
            route_request.enable_analysis,
            route_request.enable_mcp,
            route_request.enable_web_research,
            route_request.web_research_query,
            route_request.max_retries,
            route_request.model,
            route_request.creative_mode,
            route_request.story_focus,
            route_request.plot_stage,
            route_request.story_creation_brief,
            route_request.quality_preset,
            route_request.quality_notes,
            route_request.story_repair_summary,
            route_request.story_repair_targets,
            route_request.story_preserve_strengths,
        );

        assert_eq!(request.start_chapter_number, 5);
        assert_eq!(request.count, 3);
        assert_eq!(request.style_id, Some(9));
        assert_eq!(request.target_word_count, Some(3200));
        assert!(request.enable_analysis);
        assert_eq!(request.max_retries, 6);
        assert_eq!(request.model_override.as_deref(), Some("gpt-4.1-mini"));
    }

    #[test]
    fn should_keep_batch_generation_create_workflow_request_contract_minimal() {
        let route_request = BatchGenerationCreateRouteRequest {
            start_chapter_number: 3,
            count: 2,
            style_id: Some(7),
            target_word_count: Some(2800),
            enable_analysis: None,
            enable_mcp: Some(true),
            enable_web_research: Some(true),
            web_research_query: Some("ignored".to_string()),
            max_retries: None,
            model: Some("gpt-4.1".to_string()),
            creative_mode: Some("dramatic".to_string()),
            story_focus: Some("battle".to_string()),
            plot_stage: Some("climax".to_string()),
            story_creation_brief: Some("brief".to_string()),
            quality_preset: Some("strict".to_string()),
            quality_notes: Some("notes".to_string()),
            story_repair_summary: Some("repair".to_string()),
            story_repair_targets: Some(vec!["target-a".to_string()]),
            story_preserve_strengths: Some(vec!["strength-a".to_string()]),
        };
        let request = BatchGenerationCreateWorkflowRequest::from_route_payload(
            route_request.start_chapter_number,
            route_request.count,
            route_request.style_id,
            route_request.target_word_count,
            route_request.enable_analysis,
            route_request.enable_mcp,
            route_request.enable_web_research,
            route_request.web_research_query,
            route_request.max_retries,
            route_request.model,
            route_request.creative_mode,
            route_request.story_focus,
            route_request.plot_stage,
            route_request.story_creation_brief,
            route_request.quality_preset,
            route_request.quality_notes,
            route_request.story_repair_summary,
            route_request.story_repair_targets,
            route_request.story_preserve_strengths,
        );

        assert_eq!(request.start_chapter_number, 3);
        assert_eq!(request.count, 2);
        assert_eq!(request.style_id, Some(7));
        assert_eq!(request.target_word_count, Some(2800));
        assert!(!request.enable_analysis);
        assert_eq!(request.max_retries, 3);
        assert_eq!(request.model_override.as_deref(), Some("gpt-4.1"));
    }
}
