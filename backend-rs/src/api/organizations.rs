use std::collections::HashMap;

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{character, organization, organization_member, project};
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

#[derive(Deserialize)]
struct MemberCreateRequest {
    character_id: String,
    position: String,
    rank: Option<i32>,
    status: Option<String>,
    joined_at: Option<String>,
    left_at: Option<String>,
    loyalty: Option<i32>,
    contribution: Option<i32>,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct MemberUpdateRequest {
    position: Option<String>,
    rank: Option<i32>,
    status: Option<String>,
    joined_at: Option<String>,
    left_at: Option<String>,
    loyalty: Option<i32>,
    contribution: Option<i32>,
    notes: Option<String>,
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

fn org_detail_json(org: &organization::Model, char_model: Option<&character::Model>) -> Value {
    let name = char_model
        .map(|model| model.name.clone())
        .unwrap_or_else(|| format!("未关联组织角色 ({})", org.id));
    let organization_type = char_model
        .and_then(|model| model.organization_type.clone())
        .unwrap_or_else(|| "未设置".to_string());
    let purpose = char_model
        .and_then(|model| model.organization_purpose.clone())
        .unwrap_or_default();

    json!({
        "id": org.id,
        "character_id": org.character_id,
        "name": name,
        "type": organization_type,
        "purpose": purpose,
        "member_count": org.member_count,
        "power_level": org.power_level,
        "location": org.location,
        "motto": org.motto,
        "color": org.color,
    })
}

fn member_detail_json(
    member: &organization_member::Model,
    char_model: Option<&character::Model>,
) -> Value {
    let character_name = char_model
        .map(|model| model.name.clone())
        .unwrap_or_else(|| format!("??????? ({})", member.character_id));

    json!({
        "id": member.id,
        "character_id": member.character_id,
        "character_name": character_name,
        "position": member.position,
        "rank": member.rank,
        "loyalty": member.loyalty,
        "contribution": member.contribution,
        "status": member.status,
        "joined_at": member.joined_at,
        "left_at": member.left_at,
        "notes": member.notes,
    })
}

async fn create_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match OrganizationService::create(
        &db,
        &body.project_id,
        &body.character_id,
        &claims.sub,
        body.parent_org_id.as_deref(),
        body.level,
        body.power_level,
        body.location.as_deref(),
        body.motto.as_deref(),
        body.color.as_deref(),
    )
    .await
    {
        Ok(Some(org)) => Ok((StatusCode::CREATED, Json(json!(org)))),
        Ok(None) => Err(forbidden_or_missing("项目不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn list_orgs(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(orgs)) => Ok(Json(
            json!({"success": true, "data": orgs, "total": orgs.len()}),
        )),
        Ok(None) => Err(forbidden_or_missing("项目不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn list_project_orgs(
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

    let orgs = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(&project_id))
        .order_by_asc(organization::Column::Level)
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    let character_ids: Vec<String> = orgs.iter().map(|org| org.character_id.clone()).collect();
    let characters = character::Entity::find()
        .filter(character::Column::Id.is_in(character_ids))
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let character_map: HashMap<String, character::Model> = characters
        .into_iter()
        .map(|char_model| (char_model.id.clone(), char_model))
        .collect();

    let payload: Vec<Value> = orgs
        .iter()
        .map(|org| org_detail_json(org, character_map.get(&org.character_id)))
        .collect();
    Ok(Json(json!(payload)))
}

async fn get_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::get(&db, &org_id, &claims.sub).await {
        Ok(Some(org)) => Ok(Json(json!(org))),
        Ok(None) => Err(forbidden_or_missing("组织不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn update_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::update(
        &db,
        &org_id,
        &claims.sub,
        body.parent_org_id.as_deref(),
        body.level,
        body.power_level,
        body.location.as_deref(),
        body.motto.as_deref(),
        body.color.as_deref(),
    )
    .await
    {
        Ok(Some(org)) => Ok(Json(json!([org]))),
        Ok(None) => Err(forbidden_or_missing("组织不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn delete_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::delete(&db, &org_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"message": "组织删除成功", "id": org_id}))),
        Ok(None) => Err(forbidden_or_missing("组织不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn get_member_org(
    db: &DatabaseConnection,
    member_id: &str,
) -> Result<Option<(organization_member::Model, organization::Model)>, String> {
    let Some(member) = organization_member::Entity::find_by_id(member_id)
        .one(db)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let org = organization::Entity::find_by_id(&member.organization_id)
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(org.map(|org| (member, org)))
}

async fn list_members(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(org) = organization::Entity::find_by_id(&org_id)
        .one(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?
    else {
        return Err(forbidden_or_missing("组织不存在"));
    };
    if !verify_project_access(&db, &org.project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("组织不存在或无权限"));
    }

    let members = organization_member::Entity::find()
        .filter(organization_member::Column::OrganizationId.eq(&org_id))
        .order_by_desc(organization_member::Column::Rank)
        .order_by_asc(organization_member::Column::CreatedAt)
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let character_ids: Vec<String> = members
        .iter()
        .map(|member| member.character_id.clone())
        .collect();
    let characters = character::Entity::find()
        .filter(character::Column::Id.is_in(character_ids))
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let character_map: HashMap<String, character::Model> = characters
        .into_iter()
        .map(|char_model| (char_model.id.clone(), char_model))
        .collect();
    let payload: Vec<Value> = members
        .iter()
        .map(|member| member_detail_json(member, character_map.get(&member.character_id)))
        .collect();
    Ok(Json(json!(payload)))
}

async fn add_member(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(body): Json<MemberCreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let Some(org) = organization::Entity::find_by_id(&org_id)
        .one(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?
    else {
        return Err(forbidden_or_missing("组织不存在"));
    };
    if !verify_project_access(&db, &org.project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("组织不存在或无权限"));
    }

    let Some(char_model) = character::Entity::find_by_id(&body.character_id)
        .one(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在"})),
        ));
    };
    if char_model.is_organization {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "不能将组织添加为成员"})),
        ));
    }

    let duplicate = organization_member::Entity::find()
        .filter(organization_member::Column::OrganizationId.eq(&org_id))
        .filter(organization_member::Column::CharacterId.eq(&body.character_id))
        .one(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    if duplicate.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "该角色已在组织中"})),
        ));
    }

    let now = Utc::now().naive_utc();
    let active = organization_member::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        organization_id: Set(org_id.clone()),
        character_id: Set(body.character_id),
        position: Set(body.position),
        rank: Set(body.rank.unwrap_or(0)),
        status: Set(body.status.unwrap_or_else(|| "active".to_string())),
        joined_at: Set(body.joined_at),
        left_at: Set(body.left_at),
        loyalty: Set(body.loyalty.unwrap_or(50)),
        contribution: Set(body.contribution.unwrap_or(0)),
        source: Set("manual".to_string()),
        notes: Set(body.notes),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    };
    let member = active
        .insert(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    let next_member_count = org.member_count + 1;
    let mut org_active: organization::ActiveModel = org.into();
    org_active.member_count = Set(next_member_count);
    org_active.updated_at = Set(Some(now));
    org_active
        .update(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(json!(member))))
}

async fn update_member(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(member_id): Path<String>,
    Json(body): Json<MemberUpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some((member, org)) = get_member_org(&db, &member_id)
        .await
        .map_err(server_error)?
    else {
        return Err(forbidden_or_missing("成员记录不存在"));
    };
    if !verify_project_access(&db, &org.project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("成员记录不存在或无权限"));
    }

    let mut active: organization_member::ActiveModel = member.into();
    if let Some(value) = body.position {
        active.position = Set(value);
    }
    if let Some(value) = body.rank {
        active.rank = Set(value);
    }
    if let Some(value) = body.status {
        active.status = Set(value);
    }
    if let Some(value) = body.joined_at {
        active.joined_at = Set(Some(value));
    }
    if let Some(value) = body.left_at {
        active.left_at = Set(Some(value));
    }
    if let Some(value) = body.loyalty {
        active.loyalty = Set(value);
    }
    if let Some(value) = body.contribution {
        active.contribution = Set(value);
    }
    if let Some(value) = body.notes {
        active.notes = Set(Some(value));
    }
    active.updated_at = Set(Some(Utc::now().naive_utc()));
    let updated = active
        .update(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    Ok(Json(json!(updated)))
}

async fn delete_member(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(member_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some((member, org)) = get_member_org(&db, &member_id)
        .await
        .map_err(server_error)?
    else {
        return Err(forbidden_or_missing("成员记录不存在"));
    };
    if !verify_project_access(&db, &org.project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("成员记录不存在或无权限"));
    }

    organization_member::Entity::delete_by_id(&member_id)
        .exec(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let next_member_count = (org.member_count - 1).max(0);
    let mut org_active: organization::ActiveModel = org.into();
    org_active.member_count = Set(next_member_count);
    org_active.updated_at = Set(Some(Utc::now().naive_utc()));
    org_active
        .update(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok(Json(json!({"message": "成员移除成功", "id": member.id})))
}

pub fn routes() -> Router {
    Router::new()
        .route("/organizations", post(create_org).get(list_orgs))
        .route(
            "/organizations/project/{project_id}",
            get(list_project_orgs),
        )
        .route(
            "/organizations/members/{member_id}",
            get(|| async { StatusCode::METHOD_NOT_ALLOWED })
                .put(update_member)
                .delete(delete_member),
        )
        .route(
            "/organizations/{org_id}/members",
            get(list_members).post(add_member),
        )
        .route(
            "/organizations/{org_id}",
            get(get_org).put(update_org).delete(delete_org),
        )
}
