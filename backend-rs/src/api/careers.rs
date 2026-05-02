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
use crate::services::career_service::CareerService;

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    name: String,
    career_type: String,
    stages: String,
    description: Option<String>,
    category: Option<String>,
    max_stage: Option<i32>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    name: Option<String>,
    description: Option<String>,
    stages: Option<String>,
    max_stage: Option<i32>,
    category: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

async fn create_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match CareerService::create(
        &db,
        &body.project_id,
        &claims.sub,
        &body.name,
        &body.career_type,
        &body.stages,
        body.description.as_deref(),
        body.category.as_deref(),
        body.max_stage,
    )
    .await
    {
        Ok(Some(career)) => Ok((StatusCode::CREATED, Json(json!({"success": true, "data": career})))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "项目不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn list_careers(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CareerService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(careers)) => Ok(Json(json!({"success": true, "data": careers, "total": careers.len()}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "项目不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn get_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(career_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CareerService::get(&db, &career_id, &claims.sub).await {
        Ok(Some(career)) => Ok(Json(json!({"success": true, "data": career}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "职业不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn update_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(career_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CareerService::update(
        &db, &career_id, &claims.sub,
        body.name.as_deref(), body.description.as_deref(),
        body.stages.as_deref(), body.max_stage, body.category.as_deref(),
    )
    .await
    {
        Ok(Some(career)) => Ok(Json(json!({"success": true, "data": career}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "职业不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn delete_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(career_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CareerService::delete(&db, &career_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "职业已删除"}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "职业不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/careers", post(create_career).get(list_careers))
        .route("/careers/{career_id}", get(get_career).put(update_career).delete(delete_career))
}
