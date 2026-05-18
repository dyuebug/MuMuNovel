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
    map_create_chapter_payload_error, map_delete_chapter_payload_error,
    map_get_chapter_payload_error, map_list_chapters_by_project_path_payload_error,
    map_list_chapters_payload_error, map_update_chapter_payload_error,
    map_update_expansion_plan_payload_error,
};
use crate::api::chapters_error_mapper::{
    map_load_annotations_payload_error, map_load_can_generate_payload_error,
    map_load_navigation_payload_error, map_load_quality_trend_payload_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_crud_service::{
    create_chapter_payload, delete_chapter_payload, get_chapter_payload,
    list_chapters_by_project_path_payload, list_chapters_payload, update_chapter_payload,
    update_expansion_plan_payload,
};
use crate::services::chapter_annotation_query_service::load_annotations_payload;
use crate::services::chapter_query_service::{
    load_can_generate_payload, load_navigation_payload,
};
use crate::services::chapter_quality_query_service::load_quality_trend_payload;

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    title: String,
    chapter_number: i32,
    content: Option<String>,
    summary: Option<String>,
    outline_id: Option<String>,
    sub_index: Option<i32>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    title: Option<String>,
    content: Option<String>,
    summary: Option<String>,
    status: Option<String>,
    chapter_number: Option<i32>,
    expansion_plan: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

#[derive(Deserialize)]
struct ExpansionPlanRequest {
    plan: String,
}

async fn create_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let payload = create_chapter_payload(
        &db,
        &body.project_id,
        &claims.sub,
        &body.title,
        body.chapter_number,
        body.content.as_deref(),
        body.summary.as_deref(),
        body.outline_id.as_deref(),
        body.sub_index,
    )
    .await
    .map_err(map_create_chapter_payload_error)?;
    Ok((StatusCode::CREATED, Json(payload)))
}

async fn list_chapters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = list_chapters_payload(&db, &query.project_id, &claims.sub)
        .await
        .map_err(map_list_chapters_payload_error)?;
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
        .map_err(map_get_chapter_payload_error)?;
    Ok(Json(payload))
}

async fn update_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = update_chapter_payload(
        &db,
        &chapter_id,
        &claims.sub,
        body.title.as_deref(),
        body.content.as_deref(),
        body.summary.as_deref(),
        body.status.as_deref(),
        body.chapter_number,
        body.expansion_plan.as_deref(),
    )
    .await
    .map_err(map_update_chapter_payload_error)?;
    Ok(Json(payload))
}

async fn delete_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = delete_chapter_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_delete_chapter_payload_error)?;
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
    Json(body): Json<ExpansionPlanRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = update_expansion_plan_payload(&db, &chapter_id, &claims.sub, &body.plan)
        .await
        .map_err(map_update_expansion_plan_payload_error)?;
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

pub fn routes() -> Router {
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
