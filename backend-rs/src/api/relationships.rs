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
use crate::services::relationship_query_service::{
    build_relationship_graph_payload, list_project_relationship_models, list_relationship_types,
    verify_relationship_project_access,
};
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

#[derive(Deserialize)]
struct ProjectRelationshipQuery {
    character_id: Option<String>,
}

fn forbidden_or_missing(message: &str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"success": false, "message": message})),
    )
}

fn server_error(error: String) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"success": false, "message": error})),
    )
}

async fn create_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match RelationshipService::create(
        &db,
        &body.project_id,
        &claims.sub,
        &body.character_from_id,
        &body.character_to_id,
        body.relationship_type_id,
        body.relationship_name.as_deref(),
        body.intimacy_level,
        body.description.as_deref(),
    )
    .await
    {
        Ok(Some(rel)) => Ok((StatusCode::CREATED, Json(json!(rel)))),
        Ok(None) => Err(forbidden_or_missing("项目不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn list_relationships(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(rels)) => Ok(Json(json!(rels))),
        Ok(None) => Err(forbidden_or_missing("项目不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn list_types(
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let types = list_relationship_types(&db).await.map_err(server_error)?;
    Ok(Json(json!(types)))
}

async fn list_project_relationships(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<ProjectRelationshipQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !verify_relationship_project_access(&db, &project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("项目不存在或无权限"));
    }

    let relationships =
        list_project_relationship_models(&db, &project_id, query.character_id.as_deref())
            .await
            .map_err(server_error)?;
    Ok(Json(json!(relationships)))
}

async fn relationship_graph(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !verify_relationship_project_access(&db, &project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("项目不存在或无权限"));
    }

    build_relationship_graph_payload(&db, &project_id)
        .await
        .map(Json)
        .map_err(server_error)
}

async fn get_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(rel_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::get(&db, &rel_id, &claims.sub).await {
        Ok(Some(rel)) => Ok(Json(json!(rel))),
        Ok(None) => Err(forbidden_or_missing("关系不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn update_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(rel_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::update(
        &db,
        &rel_id,
        &claims.sub,
        body.relationship_name.as_deref(),
        body.intimacy_level,
        body.status.as_deref(),
        body.description.as_deref(),
    )
    .await
    {
        Ok(Some(rel)) => Ok(Json(json!(rel))),
        Ok(None) => Err(forbidden_or_missing("关系不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn delete_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(rel_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::delete(&db, &rel_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"message": "关系删除成功", "id": rel_id}))),
        Ok(None) => Err(forbidden_or_missing("关系不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(
            "/relationships",
            post(create_relationship).get(list_relationships),
        )
        .route("/relationships/", post(create_relationship))
        .route("/relationships/types", get(list_types))
        .route(
            "/relationships/project/{project_id}",
            get(list_project_relationships),
        )
        .route("/relationships/graph/{project_id}", get(relationship_graph))
        .route(
            "/relationships/{rel_id}",
            get(get_relationship)
                .put(update_relationship)
                .delete(delete_relationship),
        )
}
