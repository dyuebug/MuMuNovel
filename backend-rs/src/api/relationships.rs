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
use crate::services::relationship_service::RelationshipService;

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    character_from_id: String,
    character_to_id: String,
    relationship_type_id: Option<i32>,
    relationship_name: Option<String>,
    intimacy_level: Option<i32>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    relationship_name: Option<String>,
    intimacy_level: Option<i32>,
    status: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

async fn create_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match RelationshipService::create(
        &db, &body.project_id, &claims.sub,
        &body.character_from_id, &body.character_to_id,
        body.relationship_type_id, body.relationship_name.as_deref(),
        body.intimacy_level, body.description.as_deref(),
    )
    .await
    {
        Ok(Some(rel)) => Ok((StatusCode::CREATED, Json(json!({"success": true, "data": rel})))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "项目不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn list_relationships(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(rels)) => Ok(Json(json!({"success": true, "data": rels, "total": rels.len()}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "项目不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn get_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(rel_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::get(&db, &rel_id, &claims.sub).await {
        Ok(Some(rel)) => Ok(Json(json!({"success": true, "data": rel}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "关系不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn update_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(rel_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::update(
        &db, &rel_id, &claims.sub,
        body.relationship_name.as_deref(), body.intimacy_level,
        body.status.as_deref(), body.description.as_deref(),
    )
    .await
    {
        Ok(Some(rel)) => Ok(Json(json!({"success": true, "data": rel}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "关系不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn delete_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(rel_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::delete(&db, &rel_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "关系已删除"}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "关系不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/relationships", post(create_relationship).get(list_relationships))
        .route("/relationships/{rel_id}", get(get_relationship).put(update_relationship).delete(delete_relationship))
}
