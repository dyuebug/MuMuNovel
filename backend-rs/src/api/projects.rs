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
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "项目不存在"})),
        ))?;

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
            Json(json!({"detail": "项目暂无章节"})),
        ));
    }

    let mut text = String::new();
    text.push_str(&format!("项目：《{}》\n", project.title));
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
        text.push_str(&format!("第{}章 {}\n\n", ch.chapter_number, ch.title));
        if let Some(ref content) = ch.content {
            text.push_str(content);
        }
        text.push_str("\n\n---\n\n");
    }

    let safe_title: String = project.title.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
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
                    Json(json!({"detail": format!("读取文件失败: {}", e)})),
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
            Json(json!({"detail": "请上传JSON文件"})),
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
                "errors": [format!("JSON解析失败: {}", e)],
                "warnings": [],
            })),
        )
    })?;

    let version = data.get("version").and_then(|v| v.as_str());
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if version.is_none() {
        errors.push("缺少version字段".to_string());
    }

    let project = data.get("project");
    if project.is_none() {
        errors.push("缺少project字段".to_string());
    } else if project
        .and_then(|p| p.get("title"))
        .and_then(|t| t.as_str())
        .map_or(true, |t| t.is_empty())
    {
        errors.push("project.title不能为空".to_string());
    }

    if let Some(ver) = version {
        if !["1.0.0", "1.1.0"].contains(&ver) {
            warnings.push(format!("不支持的版本 {}，可能会有兼容性问题", ver));
        }
    }

    let stats = if let Some(proj) = project {
        json!({
            "chapters": proj.get("chapters").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "characters": proj.get("characters").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "outlines": proj.get("outlines").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "relationships": proj.get("relationships").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "organizations": proj.get("organizations").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "organization_members": proj.get("organization_members").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "writing_styles": proj.get("writing_styles").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "generation_history": proj.get("generation_history").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "careers": proj.get("careers").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "character_careers": proj.get("character_careers").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "story_memories": proj.get("story_memories").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "plot_analysis": proj.get("plot_analysis").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "has_project_default_style": proj.get("project_default_style").is_some(),
        })
    } else {
        json!({})
    };

    if project
        .and_then(|p| p.get("chapters"))
        .and_then(|c| c.as_array())
        .map_or(true, |a| a.is_empty())
        && project
            .and_then(|p| p.get("characters"))
            .and_then(|c| c.as_array())
            .map_or(true, |a| a.is_empty())
    {
        warnings.push("导入数据没有章节和角色".to_string());
    }

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
        .route("/projects/validate-import", post(validate_import))
}
