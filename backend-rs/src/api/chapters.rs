use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::chapter_service::ChapterService;

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

async fn create_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match ChapterService::create(
        &db, &body.project_id, &claims.sub, &body.title, body.chapter_number,
        body.content.as_deref(), body.summary.as_deref(),
        body.outline_id.as_deref(), body.sub_index,
    )
    .await
    {
        Ok(Some(chapter)) => Ok((StatusCode::CREATED, Json(json!({"success": true, "data": chapter})))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "项目不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn list_chapters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::list_by_project(&db, &query.project_id, &claims.sub).await {
        Ok(Some(chapters)) => Ok(Json(json!({"success": true, "data": chapters, "total": chapters.len()}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "项目不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn get_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::get(&db, &chapter_id, &claims.sub).await {
        Ok(Some(chapter)) => Ok(Json(json!({"success": true, "data": chapter}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "章节不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn update_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::update(
        &db, &chapter_id, &claims.sub,
        body.title.as_deref(), body.content.as_deref(), body.summary.as_deref(),
        body.status.as_deref(), body.chapter_number, body.expansion_plan.as_deref(),
    )
    .await
    {
        Ok(Some(chapter)) => Ok(Json(json!({"success": true, "data": chapter}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "章节不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn delete_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::delete(&db, &chapter_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "章节已删除"}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "章节不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

#[derive(Deserialize)]
struct ExpansionPlanRequest {
    plan: String,
}

async fn get_navigation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::navigation(&db, &chapter_id, &claims.sub).await {
        Ok(Some((prev, current, next))) => Ok(Json(json!({
            "success": true,
            "data": {
                "prev": prev,
                "current": current,
                "next": next,
            },
        }))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "章节不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn update_expansion_plan(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ExpansionPlanRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::update_expansion_plan(&db, &chapter_id, &claims.sub, &body.plan).await {
        Ok(Some(chapter)) => Ok(Json(json!({"success": true, "data": chapter}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "章节不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn get_annotations(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::get_annotations(&db, &chapter_id, &claims.sub).await {
        Ok(Some(annotations)) => Ok(Json(json!({"success": true, "data": annotations}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "章节不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn get_quality_trend(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::quality_trend(&db, &project_id, &claims.sub).await {
        Ok(Some(trend)) => Ok(Json(json!({"success": true, "data": trend}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "项目不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn get_can_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ChapterService::can_generate(&db, &chapter_id, &claims.sub).await {
        Ok(Some(can)) => Ok(Json(json!({"success": true, "data": {"can_generate": can}}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "章节不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/chapters/project/{project_id}/quality-trend", get(get_quality_trend))
        .route("/chapters/{chapter_id}/navigation", get(get_navigation))
        .route("/chapters/{chapter_id}/expansion-plan", axum::routing::put(update_expansion_plan))
        .route("/chapters/{chapter_id}/annotations", get(get_annotations))
        .route("/chapters/{chapter_id}/can-generate", get(get_can_generate))
        .route("/chapters", post(create_chapter).get(list_chapters))
        .route("/chapters/{chapter_id}", get(get_chapter).put(update_chapter).delete(delete_chapter))
}
