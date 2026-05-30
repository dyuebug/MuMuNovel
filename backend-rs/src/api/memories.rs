use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;

use crate::api::memories_error_mapper::{
    map_analyze_chapter_memories_write_workflow_error, map_memories_project_write_context_error,
    map_owned_project_memories_query_error, map_project_chapter_analysis_payload_error,
};
use crate::services::auth::Claims;
use crate::services::memories_query_service::{
    load_owned_memory_stats_payload, load_owned_project_chapter_analysis_payload,
    load_owned_project_memories_payload, load_owned_unresolved_foreshadows_payload,
    search_owned_project_memories_payload, MemoryListRequest, SearchMemoriesRequest,
};
use crate::services::memories_write_workflow_service::{
    analyze_chapter_memories_write_workflow, delete_chapter_memories_write_workflow,
};

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct MemoryListRouteQuery {
    pub memory_type: Option<String>,
    pub chapter_id: Option<String>,
    pub limit: Option<u64>,
}

fn normalize_optional_route_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn normalize_memory_list_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(50).clamp(1, 500)
}

fn build_memory_list_request_from_route_query(
    route_query: MemoryListRouteQuery,
) -> MemoryListRequest {
    MemoryListRequest::new(
        normalize_optional_route_string(route_query.memory_type),
        normalize_optional_route_string(route_query.chapter_id),
        normalize_memory_list_limit(route_query.limit),
    )
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct SearchMemoriesRouteRequest {
    pub query: Option<String>,
    pub limit: Option<u64>,
    pub min_importance: Option<f64>,
    pub memory_types: Option<Vec<String>>,
}

fn normalize_search_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(10).clamp(1, 100)
}

fn normalize_search_query(query: Option<String>) -> String {
    query.unwrap_or_default().trim().to_owned()
}

fn normalize_search_memory_types(memory_types: Option<Vec<String>>) -> Vec<String> {
    memory_types
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

fn build_search_memories_request_from_route_payload(
    route_request: SearchMemoriesRouteRequest,
) -> SearchMemoriesRequest {
    SearchMemoriesRequest::new(
        normalize_search_query(route_request.query),
        normalize_search_limit(route_request.limit),
        route_request.min_importance.unwrap_or(0.0),
        normalize_search_memory_types(route_request.memory_types),
    )
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ForeshadowListRouteQuery {
    pub current_chapter: Option<i32>,
}

async fn get_project_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<MemoryListRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_memory_list_request_from_route_query(query);
    let payload = load_owned_project_memories_payload(&db, &project_id, &claims.sub, request)
        .await
        .map_err(map_owned_project_memories_query_error)?;
    Ok(Json(payload))
}

async fn analyze_chapter_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((project_id, chapter_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload =
        analyze_chapter_memories_write_workflow(&db, &project_id, &chapter_id, &claims.sub)
            .await
            .map_err(map_analyze_chapter_memories_write_workflow_error)?;
    Ok(Json(payload))
}

async fn get_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((project_id, chapter_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload =
        load_owned_project_chapter_analysis_payload(&db, &project_id, &chapter_id, &claims.sub)
            .await
            .map_err(map_project_chapter_analysis_payload_error)?;
    Ok(Json(payload))
}

async fn search_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<SearchMemoriesRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_search_memories_request_from_route_payload(body);
    let payload = search_owned_project_memories_payload(&db, &project_id, &claims.sub, request)
        .await
        .map_err(map_owned_project_memories_query_error)?;
    Ok(Json(payload))
}

async fn get_unresolved_foreshadows(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<ForeshadowListRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let current_chapter = query.current_chapter;
    let payload =
        load_owned_unresolved_foreshadows_payload(&db, &project_id, &claims.sub, current_chapter)
            .await
            .map_err(map_owned_project_memories_query_error)?;
    Ok(Json(payload))
}

async fn get_memory_stats(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_owned_memory_stats_payload(&db, &project_id, &claims.sub)
        .await
        .map_err(map_owned_project_memories_query_error)?;
    Ok(Json(payload))
}

async fn delete_chapter_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((project_id, chapter_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload =
        delete_chapter_memories_write_workflow(&db, &project_id, &chapter_id, &claims.sub)
            .await
            .map_err(map_memories_project_write_context_error)?;
    Ok(Json(payload))
}

pub fn routes() -> Router {
    Router::new()
        .route(
            "/memories/projects/{project_id}/analyze-chapter/{chapter_id}",
            post(analyze_chapter_memories),
        )
        .route(
            "/memories/projects/{project_id}/memories",
            get(get_project_memories),
        )
        .route(
            "/memories/projects/{project_id}/analysis/{chapter_id}",
            get(get_chapter_analysis),
        )
        .route(
            "/memories/projects/{project_id}/search",
            post(search_memories),
        )
        .route(
            "/memories/projects/{project_id}/foreshadows",
            get(get_unresolved_foreshadows),
        )
        .route(
            "/memories/projects/{project_id}/stats",
            get(get_memory_stats),
        )
        .route(
            "/memories/projects/{project_id}/chapters/{chapter_id}/memories",
            delete(delete_chapter_memories),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        build_memory_list_request_from_route_query,
        build_search_memories_request_from_route_payload, ForeshadowListRouteQuery,
        MemoryListRouteQuery, SearchMemoriesRouteRequest,
    };

    #[test]
    fn should_build_memory_list_request_from_route_query() {
        let request = build_memory_list_request_from_route_query(MemoryListRouteQuery {
            memory_type: Some(" summary ".to_string()),
            chapter_id: Some(" chapter-1 ".to_string()),
            limit: Some(900),
        });

        assert_eq!(request.memory_type(), Some("summary"));
        assert_eq!(request.chapter_id(), Some("chapter-1"));
        assert_eq!(request.limit(), 500);
    }

    #[test]
    fn should_build_search_memories_request_from_route_payload() {
        let request =
            build_search_memories_request_from_route_payload(SearchMemoriesRouteRequest {
                query: Some("  conflict  ".to_string()),
                limit: Some(0),
                min_importance: Some(0.75),
                memory_types: Some(vec![
                    " clue ".to_string(),
                    "".to_string(),
                    " setup ".to_string(),
                ]),
            });

        assert_eq!(request.query(), "conflict");
        assert_eq!(request.limit(), 1);
        assert_eq!(request.min_importance(), 0.75);
        assert_eq!(
            request.memory_types(),
            &["clue".to_string(), "setup".to_string()]
        );
    }

    #[test]
    fn should_keep_foreshadow_list_route_query_current_chapter() {
        let query = ForeshadowListRouteQuery {
            current_chapter: Some(12),
        };

        assert_eq!(query.current_chapter, Some(12));
    }
}
