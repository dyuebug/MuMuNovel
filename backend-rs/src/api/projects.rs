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
use crate::services::project_service::ProjectService;

#[derive(Deserialize)]
struct CreateRequest {
    title: String,
    description: Option<String>,
    theme: Option<String>,
    genre: Option<String>,
    outline_mode: Option<String>,
    target_words: Option<i32>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    title: Option<String>,
    description: Option<String>,
    theme: Option<String>,
    genre: Option<String>,
    status: Option<String>,
    target_words: Option<i32>,
    outline_mode: Option<String>,
    narrative_perspective: Option<String>,
    default_creative_mode: Option<String>,
    default_story_focus: Option<String>,
    default_plot_stage: Option<String>,
    default_story_creation_brief: Option<String>,
    default_quality_preset: Option<String>,
    default_quality_notes: Option<String>,
}

async fn create_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match ProjectService::create(
        &db,
        &claims.sub,
        &body.title,
        body.description.as_deref(),
        body.theme.as_deref(),
        body.genre.as_deref(),
        body.outline_mode.as_deref(),
        body.target_words,
    )
    .await
    {
        Ok(project) => Ok((
            StatusCode::CREATED,
            Json(json!({"success": true, "data": project})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    user_id: Option<String>,
}

async fn list_projects(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let uid = query.user_id.as_deref().unwrap_or(&claims.sub);
    match ProjectService::list(&db, uid).await {
        Ok(projects) => Ok(Json(json!({"success": true, "data": projects, "total": projects.len()}))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn get_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ProjectService::get(&db, &project_id, &claims.sub).await {
        Ok(Some(project)) => Ok(Json(json!({"success": true, "data": project}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ProjectService::update(
        &db,
        &project_id,
        &claims.sub,
        body.title.as_deref(),
        body.description.as_deref(),
        body.theme.as_deref(),
        body.genre.as_deref(),
        body.status.as_deref(),
        body.target_words,
        body.outline_mode.as_deref(),
        body.narrative_perspective.as_deref(),
        body.default_creative_mode.as_deref(),
        body.default_story_focus.as_deref(),
        body.default_plot_stage.as_deref(),
        body.default_story_creation_brief.as_deref(),
        body.default_quality_preset.as_deref(),
        body.default_quality_notes.as_deref(),
    )
    .await
    {
        Ok(Some(project)) => Ok(Json(json!({"success": true, "data": project}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ProjectService::delete(&db, &project_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "项目已删除"}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/projects", post(create_project).get(list_projects))
        .route(
            "/projects/{project_id}",
            get(get_project).put(update_project).delete(delete_project),
        )
}
