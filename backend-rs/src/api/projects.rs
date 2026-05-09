use axum::{
    extract::{Extension, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::chapter;
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

#[derive(Deserialize)]
struct ExportOptions {
    #[serde(default)]
    include_generation_history: bool,
    #[serde(default)]
    include_writing_styles: bool,
    #[serde(default)]
    include_careers: bool,
    #[serde(default)]
    include_memories: bool,
    #[serde(default)]
    include_plot_analysis: bool,
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    user_id: Option<String>,
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
        Ok(project) => Ok((StatusCode::CREATED, Json(json!({"success": true, "data": project})))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
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
            Json(json!({"success": false, "message": "Project not found"})),
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
            Json(json!({"success": false, "message": "Project not found"})),
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
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "Project deleted successfully"}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn export_project_data(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(options): Json<ExportOptions>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let project = ProjectService::get(&db, &project_id, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "Project not found"}))))?;

    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&project_id))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let export_payload = json!({
        "version": "rust-strangler-1",
        "export_type": "project",
        "project": project,
        "chapters": chapters,
        "statistics": {
            "chapter_count": chapters.len()
        },
        "options": {
            "include_generation_history": options.include_generation_history,
            "include_writing_styles": options.include_writing_styles,
            "include_careers": options.include_careers,
            "include_memories": options.include_memories,
            "include_plot_analysis": options.include_plot_analysis
        }
    });

    let safe_title: String = project
        .title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let filename = format!("project_{}.json", safe_title.trim().replace(' ', "_"));
    let encoded_filename = filename.clone();
    let body = serde_json::to_vec_pretty(&export_payload).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename*=UTF-8''{}", encoded_filename),
        )
        .body(axum::body::Body::from(body))
        .unwrap())
}

async fn export_project_txt(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let project = ProjectService::get(&db, &project_id, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "Project not found"}))))?;

    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&project_id))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    if chapters.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project has no chapters"})),
        ));
    }

    let mut text = String::new();
    text.push_str(&format!("项目：{}\n", project.title));
    if let Some(ref desc) = project.description {
        if !desc.is_empty() {
            text.push_str(&format!("简介：{}\n", desc));
        }
    }
    if let Some(ref theme) = project.theme {
        if !theme.is_empty() {
            text.push_str(&format!("主题：{}\n", theme));
        }
    }
    if let Some(ref genre) = project.genre {
        if !genre.is_empty() {
            text.push_str(&format!("类型：{}\n", genre));
        }
    }
    text.push_str("\n\n");

    for ch in &chapters {
        text.push_str(&format!("第 {} 章：{}\n\n", ch.chapter_number, ch.title));
        if let Some(ref content) = ch.content {
            text.push_str(content);
        }
        text.push_str("\n\n---\n\n");
    }

    let safe_title: String = project
        .title
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    let filename = format!("{}.txt", safe_title);
    let headers = [
        (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
        (
            header::CONTENT_DISPOSITION,
            &format!("attachment; filename=\"{}\"", filename),
        ),
    ];

    Ok((headers, text).into_response())
}

async fn validate_import(
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut file_data: Vec<u8> = Vec::new();
    let mut file_found = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": format!("Failed to read uploaded file: {}", e)})),
                )
            })?;
            file_data = bytes.to_vec();
            file_found = true;
            break;
        }
    }

    if !file_found {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "Missing file field"})),
        ));
    }

    let data: Value = serde_json::from_slice(&file_data).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "valid": false,
                "version": null,
                "project_name": null,
                "statistics": {},
                "errors": [format!("Invalid JSON: {}", e)],
                "warnings": [],
            })),
        )
    })?;

    let version = data.get("version").and_then(|v| v.as_str());
    let project = data.get("project");
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    if version.is_none() {
        errors.push("Missing version field".to_string());
    }
    if project.is_none() {
        errors.push("Missing project field".to_string());
    }

    if let Some(ver) = version {
        if !["1.0.0", "1.1.0", "rust-strangler-1"].contains(&ver) {
            warnings.push(format!("Unknown export version: {}", ver));
        }
    }

    let stats = if let Some(proj) = project {
        json!({
            "chapters": proj.get("chapters").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "characters": proj.get("characters").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "outlines": proj.get("outlines").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "relationships": proj.get("relationships").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "organizations": proj.get("organizations").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "writing_styles": proj.get("writing_styles").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "generation_history": proj.get("generation_history").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "careers": proj.get("careers").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "memories": proj.get("memories").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "plot_analysis": proj.get("plot_analysis").and_then(|c| c.as_array()).map_or(0, |a| a.len())
        })
    } else {
        json!({})
    };

    Ok(Json(json!({
        "valid": errors.is_empty(),
        "version": version,
        "project_name": project.and_then(|p| p.get("title")).and_then(|t| t.as_str()),
        "statistics": stats,
        "errors": errors,
        "warnings": warnings,
    })))
}

pub fn routes() -> Router {
    Router::new()
        .route("/projects", post(create_project).get(list_projects))
        .route(
            "/projects/{project_id}",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/projects/{project_id}/export", get(export_project_txt))
        .route("/projects/{project_id}/export-data", post(export_project_data))
        .route("/projects/validate-import", post(validate_import))
}
