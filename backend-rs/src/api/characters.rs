use axum::{
    extract::{Extension, Multipart, Path, Query},
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
}
