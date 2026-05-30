use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;

use crate::api::chapter_crud_error_mapper::{
    map_chapter_crud_success_message_error, map_list_chapters_by_project_path_payload_error,
    map_project_crud_success_message_error,
};
use crate::api::chapters_error_mapper::{
    map_load_annotations_payload_error, map_load_can_generate_payload_error,
    map_load_navigation_payload_error, map_load_quality_trend_payload_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_crud_workflow_service::{
    create_chapter_payload, delete_chapter_payload, get_chapter_payload,
    list_chapters_by_project_path_payload, list_chapters_payload, update_chapter_payload,
    update_expansion_plan_payload, CreateChapterRequest, ListChaptersRequest,
    UpdateChapterRequest, UpdateExpansionPlanRequest,
};
use crate::services::chapter_query_service::{
    load_annotations_payload, load_can_generate_payload, load_navigation_payload,
    load_quality_trend_payload,
};

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct CreateChapterRouteRequest {
    project_id: String,
    title: String,
    chapter_number: i32,
    content: Option<String>,
    summary: Option<String>,
    outline_id: Option<String>,
    sub_index: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct UpdateChapterRouteRequest {
    title: Option<String>,
    content: Option<String>,
    summary: Option<String>,
    status: Option<String>,
    chapter_number: Option<i32>,
    expansion_plan: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct UpdateExpansionPlanRouteRequest {
    plan: String,
}

async fn create_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateChapterRouteRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let request = CreateChapterRequest::from_route_payload(
        body.project_id,
        body.title,
        body.chapter_number,
        body.content,
        body.summary,
        body.outline_id,
        body.sub_index,
    );
    let payload = create_chapter_payload(&db, &claims.sub, &request)
        .await
        .map_err(map_project_crud_success_message_error)?;
    Ok((StatusCode::CREATED, Json(payload)))
}

async fn list_chapters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = ListChaptersRequest::from_route_payload(query.project_id);
    let payload = list_chapters_payload(&db, &request, &claims.sub)
        .await
        .map_err(map_project_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn list_chapters_by_project_path(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = list_chapters_by_project_path_payload(&db, &project_id, &claims.sub)
        .await
        .map_err(map_list_chapters_by_project_path_payload_error)?;
    Ok(Json(payload))
}

async fn get_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = get_chapter_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_chapter_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn update_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<UpdateChapterRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = UpdateChapterRequest::from_route_payload(
        body.title,
        body.content,
        body.summary,
        body.status,
        body.chapter_number,
        body.expansion_plan,
    );
    let payload = update_chapter_payload(&db, &chapter_id, &claims.sub, &request)
        .await
        .map_err(map_chapter_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn delete_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = delete_chapter_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_chapter_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn get_navigation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_navigation_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_load_navigation_payload_error)?;
    Ok(Json(payload))
}

async fn update_expansion_plan(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<UpdateExpansionPlanRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = UpdateExpansionPlanRequest::from_route_payload(body.plan);
    let payload = update_expansion_plan_payload(&db, &chapter_id, &claims.sub, &request)
        .await
        .map_err(map_chapter_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn get_annotations(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_annotations_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_load_annotations_payload_error)?;
    Ok(Json(payload))
}

async fn get_quality_trend(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_quality_trend_payload(&db, &project_id, &claims.sub)
        .await
        .map_err(map_load_quality_trend_payload_error)?;
    Ok(Json(payload))
}

async fn get_can_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_can_generate_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_load_can_generate_payload_error)?;
    Ok(Json(payload))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(
            "/chapters/project/{project_id}",
            get(list_chapters_by_project_path),
        )
        .route(
            "/chapters/project/{project_id}/quality-trend",
            get(get_quality_trend),
        )
        .route("/chapters/{chapter_id}/navigation", get(get_navigation))
        .route(
            "/chapters/{chapter_id}/expansion-plan",
            axum::routing::put(update_expansion_plan),
        )
        .route("/chapters/{chapter_id}/annotations", get(get_annotations))
        .route("/chapters/{chapter_id}/can-generate", get(get_can_generate))
        .route(
            "/chapters",
            axum::routing::get(list_chapters).post(create_chapter),
        )
        .route(
            "/chapters/{chapter_id}",
            get(get_chapter).put(update_chapter).delete(delete_chapter),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        CreateChapterRouteRequest, ListQuery, UpdateExpansionPlanRouteRequest,
    };
    use crate::services::chapter_crud_workflow_service::{
        CreateChapterRequest, ListChaptersRequest, UpdateChapterRequest,
        UpdateExpansionPlanRequest,
    };

    #[test]
    fn should_build_create_chapter_request_from_route_payload() {
        let request = CreateChapterRequest::from_route_payload(
            CreateChapterRouteRequest {
                project_id: "project-1".to_string(),
                title: "第一章".to_string(),
                chapter_number: 1,
                content: Some("正文".to_string()),
                summary: Some("摘要".to_string()),
                outline_id: Some("outline-1".to_string()),
                sub_index: Some(2),
            }
            .project_id,
            "第一章".to_string(),
            1,
            Some("正文".to_string()),
            Some("摘要".to_string()),
            Some("outline-1".to_string()),
            Some(2),
        );

        assert_eq!(request.project_id(), "project-1");
        assert_eq!(request.title(), "第一章");
        assert_eq!(request.chapter_number(), 1);
        assert_eq!(request.content(), Some("正文"));
        assert_eq!(request.summary(), Some("摘要"));
        assert_eq!(request.outline_id(), Some("outline-1"));
        assert_eq!(request.sub_index(), Some(2));
    }

    #[test]
    fn should_build_update_chapter_request_from_route_payload() {
        let request = UpdateChapterRequest::from_route_payload(
            Some("新标题".to_string()),
            None,
            Some("新摘要".to_string()),
            Some("draft".to_string()),
            Some(3),
            Some("扩写计划".to_string()),
        );

        assert_eq!(request.title(), Some("新标题"));
        assert_eq!(request.content(), None);
        assert_eq!(request.summary(), Some("新摘要"));
        assert_eq!(request.status(), Some("draft"));
        assert_eq!(request.chapter_number(), Some(3));
        assert_eq!(request.expansion_plan(), Some("扩写计划"));
    }

    #[test]
    fn should_build_update_expansion_plan_request_from_route_payload() {
        let request = UpdateExpansionPlanRequest::from_route_payload(
            UpdateExpansionPlanRouteRequest {
                plan: "保持节奏，补足冲突".to_string(),
            }
            .plan,
        );

        assert_eq!(request.plan(), "保持节奏，补足冲突");
    }

    #[test]
    fn should_build_list_chapters_request_from_route_query() {
        let request =
            ListChaptersRequest::from_route_payload(ListQuery { project_id: "project-1".to_string() }.project_id);

        assert_eq!(request.project_id(), "project-1");
    }
}
