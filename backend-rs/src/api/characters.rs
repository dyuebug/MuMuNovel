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
use crate::services::character_service::CharacterService;

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    name: String,
    #[serde(default)]
    is_organization: bool,
    role_type: Option<String>,
    personality: Option<String>,
    background: Option<String>,
    appearance: Option<String>,
    age: Option<String>,
    gender: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    name: Option<String>,
    role_type: Option<String>,
    personality: Option<String>,
    background: Option<String>,
    appearance: Option<String>,
    age: Option<String>,
    gender: Option<String>,
    status: Option<String>,
    is_organization: Option<bool>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

async fn create_character(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match CharacterService::create(
        &db,
        &body.project_id,
        &claims.sub,
        &body.name,
        body.is_organization,
        body.role_type.as_deref(),
        body.personality.as_deref(),
        body.background.as_deref(),
        body.appearance.as_deref(),
        body.age.as_deref(),
        body.gender.as_deref(),
    )
    .await
    {
        Ok(Some(character)) => Ok((
            StatusCode::CREATED,
            Json(json!({"success": true, "data": character})),
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

async fn list_characters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CharacterService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(characters)) => Ok(Json(json!({"success": true, "data": characters, "total": characters.len()}))),
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

async fn get_character(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(character_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CharacterService::get(&db, &character_id, &claims.sub).await {
        Ok(Some(character)) => Ok(Json(json!({"success": true, "data": character}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_character(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(character_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CharacterService::update(
        &db,
        &character_id,
        &claims.sub,
        body.name.as_deref(),
        body.role_type.as_deref(),
        body.personality.as_deref(),
        body.background.as_deref(),
        body.appearance.as_deref(),
        body.age.as_deref(),
        body.gender.as_deref(),
        body.status.as_deref(),
        body.is_organization,
    )
    .await
    {
        Ok(Some(character)) => Ok(Json(json!({"success": true, "data": character}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_character(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(character_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CharacterService::delete(&db, &character_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "角色已删除"}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/characters", post(create_character).get(list_characters))
        .route(
            "/characters/{character_id}",
            get(get_character).put(update_character).delete(delete_character),
        )
}
