use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::{
    character, organization, organization_member, project, relationship, relationship_type,
};
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

#[derive(Deserialize)]
struct ProjectRelationshipQuery {
    character_id: Option<String>,
}

async fn verify_project_access(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<bool, String> {
    project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map(|project| project.is_some())
        .map_err(|e| e.to_string())
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
    let types = relationship_type::Entity::find()
        .order_by_asc(relationship_type::Column::Category)
        .order_by_asc(relationship_type::Column::Id)
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    Ok(Json(json!(types)))
}

async fn list_project_relationships(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<ProjectRelationshipQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !verify_project_access(&db, &project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("项目不存在或无权限"));
    }

    let mut selector = relationship::Entity::find()
        .filter(relationship::Column::ProjectId.eq(&project_id))
        .order_by_desc(relationship::Column::CreatedAt);
    if let Some(character_id) = query.character_id {
        selector = selector.filter(
            relationship::Column::CharacterFromId
                .eq(character_id.clone())
                .or(relationship::Column::CharacterToId.eq(character_id)),
        );
    }
    let relationships = selector
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    Ok(Json(json!(relationships)))
}

async fn relationship_graph(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !verify_project_access(&db, &project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("项目不存在或无权限"));
    }

    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&project_id))
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let nodes: Vec<Value> = characters
        .iter()
        .map(|item| {
            json!({
                "id": item.id,
                "name": item.name,
                "type": if item.is_organization { "organization" } else { "character" },
                "role_type": item.role_type,
                "avatar": item.avatar_url,
            })
        })
        .collect();

    let relationships = relationship::Entity::find()
        .filter(relationship::Column::ProjectId.eq(&project_id))
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let mut links: Vec<Value> = relationships
        .iter()
        .map(|item| {
            json!({
                "source": item.character_from_id,
                "target": item.character_to_id,
                "relationship": item.relationship_name.as_deref().unwrap_or("未知关系"),
                "intimacy": item.intimacy_level,
                "status": item.status,
            })
        })
        .collect();

    let orgs = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(&project_id))
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    for org in orgs {
        let members = organization_member::Entity::find()
            .filter(organization_member::Column::OrganizationId.eq(&org.id))
            .all(&db)
            .await
            .map_err(|e| server_error(e.to_string()))?;
        links.extend(members.into_iter().map(|member| {
            json!({
                "source": org.character_id,
                "target": member.character_id,
                "relationship": format!("组织成员·{}", member.position),
                "intimacy": member.loyalty,
                "status": member.status,
            })
        }));
    }

    Ok(Json(json!({"nodes": nodes, "links": links})))
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
