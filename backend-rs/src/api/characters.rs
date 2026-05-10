use axum::{
    extract::{Extension, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{Json, Response},
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

#[derive(Deserialize)]
struct CharactersExportRequest {
    character_ids: Vec<String>,
}

#[derive(Deserialize)]
struct ImportCharactersQuery {
    project_id: String,
}

fn value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
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
        Ok(Some(characters)) => Ok(Json(
            json!({"success": true, "data": characters, "items": characters, "total": characters.len()}),
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

async fn validate_characters_import(
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

    let data: Value = serde_json::from_slice(&file_data).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "valid": false,
                "version": null,
                "statistics": {},
                "errors": ["JSON解析失败"],
                "warnings": [],
            })),
        )
    })?;

    let version = data.get("version").and_then(|v| v.as_str());
    let export_type = data.get("export_type").and_then(|v| v.as_str());
    let items = data.get("data").and_then(|d| d.as_array());
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if version.is_none() {
        errors.push("缺少version字段".to_string());
    }
    if export_type != Some("characters") {
        errors.push(format!(
            "export_type应为'characters'，当前为{:?}",
            export_type
        ));
    }
    if items.is_none() {
        errors.push("缺少data字段或data不是数组".to_string());
    } else if let Some(arr) = items {
        if arr.is_empty() {
            warnings.push("没有需要导入的角色数据".to_string());
        }
        for (i, item) in arr.iter().enumerate() {
            if item
                .get("name")
                .and_then(|n| n.as_str())
                .map_or(true, |n| n.is_empty())
            {
                errors.push(format!("第{}项缺少name字段", i + 1));
            }
        }
    }

    let char_count = items.map_or(0, |a| a.len());
    let org_count = items.map_or(0, |a| {
        a.iter()
            .filter(|i| {
                i.get("is_organization")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .count()
    });

    Ok(Json(json!({
        "valid": errors.is_empty(),
        "version": version,
        "statistics": {
            "total": char_count,
            "characters": char_count - org_count,
            "organizations": org_count,
        },
        "errors": errors,
        "warnings": warnings,
    })))
}

async fn export_characters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CharactersExportRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    if body.character_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "请至少选择一个角色/组织"})),
        ));
    }

    let mut items = Vec::new();
    for character_id in &body.character_ids {
        let character = CharacterService::get(&db, character_id, &claims.sub)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": error})),
                )
            })?
            .ok_or((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": format!("角色不存在: {}", character_id)})),
            ))?;
        items.push(serde_json::to_value(character).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?);
    }

    let payload = json!({
        "version": "rust-strangler-1",
        "export_type": "characters",
        "data": items,
        "statistics": {
            "total": items.len(),
            "characters": items.iter().filter(|item| !item.get("is_organization").and_then(Value::as_bool).unwrap_or(false)).count(),
            "organizations": items.iter().filter(|item| item.get("is_organization").and_then(Value::as_bool).unwrap_or(false)).count(),
        },
    });
    let body = serde_json::to_vec_pretty(&payload).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error.to_string()})),
        )
    })?;
    let filename = format!("characters_export_{}.json", items.len());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename={}", filename),
        )
        .body(axum::body::Body::from(body))
        .unwrap())
}

async fn import_characters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ImportCharactersQuery>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let existing = CharacterService::list(&db, &query.project_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "项目不存在或无权限"})),
        ))?;
    let mut existing_names: std::collections::HashSet<String> =
        existing.into_iter().map(|item| item.name).collect();

    let mut file_data = Vec::new();
    let mut file_found = false;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": format!("读取文件失败: {}", error)})),
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

    let data: Value = serde_json::from_slice(&file_data).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("JSON格式错误: {}", error)})),
        )
    })?;
    let items = data.get("data").and_then(Value::as_array).ok_or((
        StatusCode::BAD_REQUEST,
        Json(json!({"detail": "缺少data字段或data不是数组"})),
    ))?;

    let mut imported_characters = Vec::new();
    let mut imported_organizations = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();

    for item in items {
        let Some(name) = value_string(item, "name") else {
            errors.push("缺少name字段".to_string());
            continue;
        };
        if existing_names.contains(&name) {
            skipped.push(format!("名称已存在: {}", name));
            continue;
        }

        let is_organization = item
            .get("is_organization")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        match CharacterService::create(
            &db,
            &query.project_id,
            &claims.sub,
            &name,
            is_organization,
            value_string(item, "role_type").as_deref(),
            value_string(item, "personality").as_deref(),
            value_string(item, "background").as_deref(),
            value_string(item, "appearance").as_deref(),
            value_string(item, "age").as_deref(),
            value_string(item, "gender").as_deref(),
        )
        .await
        {
            Ok(Some(_)) => {
                existing_names.insert(name.clone());
                if is_organization {
                    imported_organizations.push(name);
                } else {
                    imported_characters.push(name);
                }
            }
            Ok(None) => errors.push(format!("项目不存在或无权限: {}", name)),
            Err(error) => errors.push(format!("{}: {}", name, error)),
        }
    }

    let imported = imported_characters.len() + imported_organizations.len();
    Ok(Json(json!({
        "success": errors.is_empty(),
        "message": format!("导入完成：成功{}，跳过{}，错误{}", imported, skipped.len(), errors.len()),
        "statistics": {
            "total": items.len(),
            "imported": imported,
            "skipped": skipped.len(),
            "errors": errors.len(),
        },
        "details": {
            "imported_characters": imported_characters,
            "imported_organizations": imported_organizations,
            "skipped": skipped,
            "errors": errors,
        },
        "warnings": [],
    })))
}

async fn list_characters_by_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CharacterService::list(&db, &project_id, &claims.sub).await {
        Ok(Some(characters)) => Ok(Json(
            json!({"success": true, "data": characters, "items": characters, "total": characters.len()}),
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

pub fn routes() -> Router {
    Router::new()
        .route(
            "/characters/project/{project_id}",
            get(list_characters_by_project),
        )
        .route("/characters", post(create_character).get(list_characters))
        .route(
            "/characters/{character_id}",
            get(get_character)
                .put(update_character)
                .delete(delete_character),
        )
        .route(
            "/characters/validate-import",
            post(validate_characters_import),
        )
        .route("/characters/export", post(export_characters))
        .route("/characters/import", post(import_characters))
}
