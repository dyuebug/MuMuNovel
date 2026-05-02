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
use crate::services::outline_service::OutlineService;

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    title: String,
    content: Option<String>,
    order_index: Option<i32>,
    structure: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    title: Option<String>,
    content: Option<String>,
    order_index: Option<i32>,
    structure: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

async fn create_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match OutlineService::create(
        &db,
        &body.project_id,
        &claims.sub,
        &body.title,
        body.content.as_deref(),
        body.order_index,
        body.structure.as_deref(),
    )
    .await
    {
        Ok(Some(outline)) => Ok((
            StatusCode::CREATED,
            Json(json!({"success": true, "data": outline})),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn list_outlines(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(outlines)) => Ok(Json(json!({"success": true, "data": outlines, "total": outlines.len()}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn get_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::get(&db, &outline_id, &claims.sub).await {
        Ok(Some(outline)) => Ok(Json(json!({"success": true, "data": outline}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::update(
        &db,
        &outline_id,
        &claims.sub,
        body.title.as_deref(),
        body.content.as_deref(),
        body.order_index,
        body.structure.as_deref(),
    )
    .await
    {
        Ok(Some(outline)) => Ok(Json(json!({"success": true, "data": outline}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::delete(&db, &outline_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "大纲已删除"}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/outlines", post(create_outline).get(list_outlines))
        .route(
            "/outlines/{outline_id}",
            get(get_outline).put(update_outline).delete(delete_outline),
        )
}
