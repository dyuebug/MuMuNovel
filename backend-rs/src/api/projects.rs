use axum::{
    extract::{Extension, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, character, organization, organization_member, project};
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

fn json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn json_i32(value: &Value, key: &str, default: i32) -> i32 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(default)
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
        Ok(project) => Ok((StatusCode::CREATED, Json(json!(project)))),
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
        Ok(projects) => Ok(Json(json!(projects))),
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
        Ok(Some(project)) => Ok(Json(json!(project))),
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
        Ok(Some(project)) => Ok(Json(json!(project))),
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
        Ok(Some(())) => Ok(Json(
            json!({"success": true, "message": "Project deleted successfully"}),
        )),
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
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found"})),
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
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found"})),
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
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
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

async fn import_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut file_data: Vec<u8> = Vec::new();
    let mut file_found = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": format!("Failed to read uploaded file: {}", error)})),
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
    if file_data.len() > 50 * 1024 * 1024 {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"detail": "文件大小超过50MB限制"})),
        ));
    }

    let data: Value = serde_json::from_slice(&file_data).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("Invalid JSON: {}", error)})),
        )
    })?;
    let project_data = data.get("project").ok_or((
        StatusCode::BAD_REQUEST,
        Json(json!({"detail": "Missing project field"})),
    ))?;

    let now = Utc::now().naive_utc();
    let project_id = Uuid::new_v4().to_string();
    let title = json_string(project_data, "title").unwrap_or_else(|| "导入项目".to_string());
    let target_words = json_i32(project_data, "target_words", 100_000);
    let chapters = data
        .get("chapters")
        .or_else(|| project_data.get("chapters"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let imported_project =
        project::ActiveModel {
            id: Set(project_id.clone()),
            user_id: Set(claims.sub.clone()),
            title: Set(title),
            description: Set(json_string(project_data, "description")),
            theme: Set(json_string(project_data, "theme")),
            genre: Set(json_string(project_data, "genre")),
            target_words: Set(target_words),
            current_words: Set(0),
            status: Set(json_string(project_data, "status").unwrap_or_else(|| "draft".to_string())),
            wizard_status: Set(json_string(project_data, "wizard_status")
                .unwrap_or_else(|| "completed".to_string())),
            wizard_step: Set(json_i32(project_data, "wizard_step", 0)),
            outline_mode: Set(json_string(project_data, "outline_mode")
                .unwrap_or_else(|| "traditional".to_string())),
            world_time_period: Set(json_string(project_data, "world_time_period")),
            world_location: Set(json_string(project_data, "world_location")),
            world_atmosphere: Set(json_string(project_data, "world_atmosphere")),
            world_rules: Set(json_string(project_data, "world_rules")),
            chapter_count: Set(Some(chapters.len() as i32)),
            narrative_perspective: Set(json_string(project_data, "narrative_perspective")),
            character_count: Set(0),
            default_creative_mode: Set(json_string(project_data, "default_creative_mode")),
            default_story_focus: Set(json_string(project_data, "default_story_focus")),
            default_plot_stage: Set(json_string(project_data, "default_plot_stage")),
            default_story_creation_brief: Set(json_string(
                project_data,
                "default_story_creation_brief",
            )),
            default_quality_preset: Set(json_string(project_data, "default_quality_preset")),
            default_quality_notes: Set(json_string(project_data, "default_quality_notes")),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    let mut current_words = 0i32;
    for (index, chapter_data) in chapters.iter().enumerate() {
        let content = json_string(chapter_data, "content");
        let word_count = chapter_data
            .get("word_count")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or_else(|| {
                content
                    .as_ref()
                    .map(|value| value.chars().count() as i32)
                    .unwrap_or(0)
            });
        current_words += word_count;

        chapter::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(imported_project.id.clone()),
            chapter_number: Set(json_i32(chapter_data, "chapter_number", index as i32 + 1)),
            title: Set(
                json_string(chapter_data, "title").unwrap_or_else(|| format!("第{}章", index + 1))
            ),
            content: Set(content),
            summary: Set(json_string(chapter_data, "summary")),
            word_count: Set(word_count),
            status: Set(json_string(chapter_data, "status").unwrap_or_else(|| "draft".to_string())),
            outline_id: Set(None),
            sub_index: Set(json_i32(chapter_data, "sub_index", 0)),
            expansion_plan: Set(json_string(chapter_data, "expansion_plan")),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;
    }

    let mut active_project: project::ActiveModel = imported_project.into();
    active_project.current_words = Set(current_words);
    active_project.update(&db).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error.to_string()})),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "project_id": project_id,
        "message": "项目导入成功",
        "statistics": {
            "chapters": chapters.len(),
        },
        "warnings": [],
    })))
}

async fn fix_missing_organization_records(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<(usize, usize), sea_orm::DbErr> {
    let org_characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .filter(character::Column::IsOrganization.eq(true))
        .all(db)
        .await?;

    let mut fixed = 0usize;
    for character_model in &org_characters {
        let existing = organization::Entity::find()
            .filter(organization::Column::CharacterId.eq(&character_model.id))
            .one(db)
            .await?;
        if existing.is_some() {
            continue;
        }

        let now = Utc::now().naive_utc();
        organization::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            character_id: Set(character_model.id.clone()),
            project_id: Set(project_id.to_string()),
            parent_org_id: Set(None),
            level: Set(1),
            power_level: Set(50),
            member_count: Set(0),
            location: Set(None),
            motto: Set(None),
            color: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await?;
        fixed += 1;
    }

    Ok((fixed, org_characters.len()))
}

async fn fix_organization_member_counts(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<(usize, usize), sea_orm::DbErr> {
    let organizations = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .all(db)
        .await?;

    let mut fixed = 0usize;
    for org in &organizations {
        let actual_count = organization_member::Entity::find()
            .filter(organization_member::Column::OrganizationId.eq(&org.id))
            .filter(organization_member::Column::Status.eq("active"))
            .count(db)
            .await? as i32;

        if org.member_count == actual_count {
            continue;
        }

        let mut active: organization::ActiveModel = org.clone().into();
        active.member_count = Set(actual_count);
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await?;
        fixed += 1;
    }

    Ok((fixed, organizations.len()))
}

async fn fix_project_organizations(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ProjectService::get(&db, &project_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found"})),
        ))?;

    let (fixed, total) = fix_missing_organization_records(&db, &project_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    Ok(Json(json!({
        "message": "组织记录修复完成",
        "fixed": fixed,
        "total": total,
    })))
}

async fn fix_project_member_counts(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ProjectService::get(&db, &project_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found"})),
        ))?;

    let (fixed, total) = fix_organization_member_counts(&db, &project_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    Ok(Json(json!({
        "message": "成员计数修复完成",
        "fixed": fixed,
        "total": total,
    })))
}

async fn check_project_consistency(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ProjectService::get(&db, &project_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found"})),
        ))?;

    let auto_fix = query
        .get("auto_fix")
        .map(|value| value != "false" && value != "0")
        .unwrap_or(true);

    let (org_fixed, org_total) = if auto_fix {
        fix_missing_organization_records(&db, &project_id)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": error.to_string()})),
                )
            })?
    } else {
        let total = character::Entity::find()
            .filter(character::Column::ProjectId.eq(&project_id))
            .filter(character::Column::IsOrganization.eq(true))
            .count(&db)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": error.to_string()})),
                )
            })? as usize;
        (0, total)
    };

    let (member_fixed, member_total) = if auto_fix {
        fix_organization_member_counts(&db, &project_id)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": error.to_string()})),
                )
            })?
    } else {
        let total = organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(&project_id))
            .count(&db)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": error.to_string()})),
                )
            })? as usize;
        (0, total)
    };

    Ok(Json(json!({
        "project_id": project_id,
        "checks": {
            "organization_records": {
                "checked": org_total,
                "fixed": org_fixed,
                "status": if org_fixed == 0 { "ok" } else { "fixed" },
            },
            "member_counts": {
                "checked": member_total,
                "fixed": member_fixed,
                "status": if member_fixed == 0 { "ok" } else { "fixed" },
            },
        },
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
        .route(
            "/projects/{project_id}/export-data",
            post(export_project_data),
        )
        .route(
            "/projects/{project_id}/check-consistency",
            post(check_project_consistency),
        )
        .route(
            "/projects/{project_id}/fix-organizations",
            post(fix_project_organizations),
        )
        .route(
            "/projects/{project_id}/fix-member-counts",
            post(fix_project_member_counts),
        )
        .route("/projects/validate-import", post(validate_import))
        .route("/projects/import", post(import_project))
}
