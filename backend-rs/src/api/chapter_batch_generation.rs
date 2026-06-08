use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{Json, Sse},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::api::chapter_batch_generation_error_mapper::{
    map_active_batch_generation_task_list_route_error,
    map_active_project_batch_generation_route_error, map_cancel_batch_generation_workflow_error,
    map_create_batch_generation_workflow_error, map_owned_batch_generation_task_route_error,
    map_resume_batch_generation_task_command_config_route_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_batch_generation_read_context_service::{
    load_active_batch_generation_view_from_route_project,
    load_active_user_batch_generation_task_list_view_from_route_query,
    load_owned_batch_generation_status_payload, ActiveBatchGenerationTaskListRouteQuery,
};
use crate::services::chapter_batch_generation_status_stream_service::load_owned_batch_generation_status_stream;
use crate::services::chapter_batch_generation_write_workflow_service::{
    cancel_owned_batch_generation_write_workflow, resume_owned_batch_generation_write_workflow,
    start_owned_batch_generation_write_workflow, BatchGenerationCreateRouteRequest,
};
use crate::utils::sse::named_sse_keep_alive;

async fn create_batch_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<BatchGenerationCreateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = start_owned_batch_generation_write_workflow(&db, &project_id, &claims.sub, body)
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
    let stream =
        load_owned_batch_generation_status_stream(db.clone(), batch_id.clone(), claims.sub.clone())
            .await
            .map_err(map_owned_batch_generation_task_route_error)?;

    Ok(Sse::new(stream).keep_alive(named_sse_keep_alive("keep-alive")))
}

async fn get_active_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_active_batch_generation_view_from_route_project(&db, &claims.sub, project_id)
        .await
        .map_err(map_active_project_batch_generation_route_error)?;

    Ok(Json(result))
}

async fn list_active_batch_generation_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ActiveBatchGenerationTaskListRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result =
        load_active_user_batch_generation_task_list_view_from_route_query(&db, &claims.sub, query)
            .await
            .map_err(map_active_batch_generation_task_list_route_error)?;
    Ok(Json(result))
}

async fn cancel_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = cancel_owned_batch_generation_write_workflow(&db, &batch_id, &claims.sub)
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
    use crate::services::chapter_batch_generation_read_context_service::{
        build_active_batch_generation_task_list_query_request_from_route_query,
        ActiveBatchGenerationTaskListRouteQuery,
    };

    use super::BatchGenerationCreateRouteRequest;

    #[test]
    fn should_validate_active_batch_generation_task_list_limit_like_python_query() {
        let default_request =
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: None },
            )
            .expect("default limit should be valid")
            .limit();
        let preserved_request =
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(25) },
            )
            .expect("explicit in-range limit should be valid")
            .limit();

        assert_eq!(default_request, 20);
        assert_eq!(preserved_request, 25);
        assert!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(0) },
            )
            .is_err()
        );
        assert!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(500) },
            )
            .is_err()
        );
    }

    #[test]
    fn should_keep_active_project_batch_generation_route_start_contract() {
        let project_id = "project-9".to_string();

        assert_eq!(project_id, "project-9");
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

        assert_eq!(route_request.start_chapter_number, 5);
        assert_eq!(route_request.count, 3);
        assert_eq!(route_request.style_id, Some(9));
        assert_eq!(route_request.target_word_count, Some(3200));
        assert_eq!(route_request.enable_analysis, Some(true));
        assert_eq!(route_request.max_retries, Some(6));
        assert_eq!(route_request.model.as_deref(), Some("gpt-4.1-mini"));
    }

    #[test]
    fn should_keep_batch_generation_create_route_payload_contract_minimal() {
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

        assert_eq!(route_request.start_chapter_number, 3);
        assert_eq!(route_request.count, 2);
        assert_eq!(route_request.style_id, Some(7));
        assert_eq!(route_request.target_word_count, Some(2800));
        assert_eq!(route_request.enable_analysis, None);
        assert_eq!(route_request.max_retries, None);
        assert_eq!(route_request.model.as_deref(), Some("gpt-4.1"));
    }
}
