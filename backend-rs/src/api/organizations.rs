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
use crate::services::organization_service::OrganizationService;

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    character_id: String,
    parent_org_id: Option<String>,
    level: Option<i32>,
    power_level: Option<i32>,
    location: Option<String>,
    motto: Option<String>,
    color: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    parent_org_id: Option<String>,
    level: Option<i32>,
    power_level: Option<i32>,
    location: Option<String>,
    motto: Option<String>,
    color: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

async fn create_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match OrganizationService::create(
        &db, &body.project_id, &body.character_id, &claims.sub,
        body.parent_org_id.as_deref(), body.level, body.power_level,
        body.location.as_deref(), body.motto.as_deref(), body.color.as_deref(),
    )
    .await
    {
        Ok(Some(org)) => Ok((StatusCode::CREATED, Json(json!({"success": true, "data": org})))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "项目不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn list_orgs(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(orgs)) => Ok(Json(json!({"success": true, "data": orgs, "total": orgs.len()}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "项目不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn get_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::get(&db, &org_id, &claims.sub).await {
        Ok(Some(org)) => Ok(Json(json!({"success": true, "data": org}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "组织不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn update_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::update(
        &db, &org_id, &claims.sub,
        body.parent_org_id.as_deref(), body.level, body.power_level,
        body.location.as_deref(), body.motto.as_deref(), body.color.as_deref(),
    )
    .await
    {
        Ok(Some(org)) => Ok(Json(json!({"success": true, "data": org}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "组织不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

async fn delete_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::delete(&db, &org_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "组织已删除"}))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "组织不存在或无权限"})))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e})))),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/organizations", post(create_org).get(list_orgs))
        .route("/organizations/{org_id}", get(get_org).put(update_org).delete(delete_org))
}
