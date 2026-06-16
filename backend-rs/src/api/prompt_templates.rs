use std::collections::{BTreeMap, BTreeSet, HashMap};

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::prompt_template;
use crate::services::auth::Claims;
use crate::services::prompt_template_service::{self, PromptTemplateService, SystemTemplate};

const PROMPT_TEMPLATES_LIST_CREATE_ROUTE: &str = "/prompt-templates";
const PROMPT_TEMPLATES_CATEGORIES_ROUTE: &str = "/prompt-templates/categories";
const PROMPT_TEMPLATES_SYSTEM_DEFAULTS_ROUTE: &str = "/prompt-templates/system-defaults";
const PROMPT_TEMPLATES_SYNC_STATUS_ROUTE: &str = "/prompt-templates/sync-status";
const PROMPT_TEMPLATES_SYNC_TO_DEFAULT_ROUTE: &str =
    "/prompt-templates/{template_key}/sync-to-default";
const PROMPT_TEMPLATES_DETAIL_ROUTE: &str = "/prompt-templates/{template_key}";
const PROMPT_TEMPLATES_RESET_ROUTE: &str = "/prompt-templates/{template_key}/reset";
const PROMPT_TEMPLATES_EXPORT_ROUTE: &str = "/prompt-templates/export";
const PROMPT_TEMPLATES_IMPORT_ROUTE: &str = "/prompt-templates/import";
const PROMPT_TEMPLATES_PREVIEW_ROUTE: &str = "/prompt-templates/{template_key}/preview";

#[derive(Debug, PartialEq, Eq)]
enum BuildPromptTemplateImportRequestError {
    MissingTemplates,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
struct PromptTemplateImportRouteRequest {
    #[serde(default)]
    templates: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
struct PromptTemplateUpsertRouteRequest {
    #[serde(flatten)]
    body: Value,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
struct PromptTemplateUpdateRouteRequest {
    #[serde(flatten)]
    body: Value,
}

impl PromptTemplateUpsertRouteRequest {
    fn into_body(self) -> Value {
        self.body
    }
}

impl PromptTemplateUpdateRouteRequest {
    fn into_body(self) -> Value {
        self.body
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PromptTemplateImportItemRequest {
    raw_item: Value,
    template_key: String,
    template_name: Value,
    is_customized: bool,
    imported_content: String,
}

impl PromptTemplateImportItemRequest {
    fn template_key(&self) -> &str {
        self.template_key.as_str()
    }

    fn template_name_value(&self) -> &Value {
        &self.template_name
    }

    fn is_customized(&self) -> bool {
        self.is_customized
    }

    fn imported_content(&self) -> &str {
        self.imported_content.as_str()
    }

    fn upsert_payload(&self) -> Value {
        let mut payload = self.raw_item.clone();

        if let Value::Object(ref mut map) = payload {
            map.insert(
                "template_key".to_string(),
                Value::String(self.template_key.clone()),
            );
        }

        payload
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PromptTemplateImportRequest {
    templates: Vec<PromptTemplateImportItemRequest>,
}

impl PromptTemplateImportRequest {
    fn templates(&self) -> &[PromptTemplateImportItemRequest] {
        self.templates.as_slice()
    }
}

fn build_prompt_template_upsert_payload_from_route_body(body: &Value) -> Value {
    body.clone()
}

fn build_prompt_template_upsert_payload_from_route_payload(
    route_request: PromptTemplateUpsertRouteRequest,
) -> Value {
    build_prompt_template_upsert_payload_from_route_body(&route_request.into_body())
}

fn build_prompt_template_update_payload_from_route_body(
    body: &Value,
    existing: &prompt_template::Model,
) -> Value {
    let mut merged = serde_json::to_value(existing).unwrap_or(Value::Null);

    if let Value::Object(ref mut map) = merged {
        if let Value::Object(body_map) = body {
            for (key, value) in body_map {
                map.insert(key.clone(), value.clone());
            }
        }
    }

    merged
}

fn build_prompt_template_update_payload_from_route_payload(
    route_request: PromptTemplateUpdateRouteRequest,
    existing: &prompt_template::Model,
) -> Value {
    build_prompt_template_update_payload_from_route_body(&route_request.into_body(), existing)
}

fn build_prompt_template_import_request_from_route_payload(
    route_request: PromptTemplateImportRouteRequest,
) -> Result<PromptTemplateImportRequest, BuildPromptTemplateImportRequestError> {
    let templates = route_request
        .templates
        .as_ref()
        .and_then(Value::as_array)
        .ok_or(BuildPromptTemplateImportRequestError::MissingTemplates)?
        .iter()
        .map(|item| PromptTemplateImportItemRequest {
            raw_item: item.clone(),
            template_key: item
                .get("template_key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            template_name: item.get("template_name").cloned().unwrap_or(Value::Null),
            is_customized: item
                .get("is_customized")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            imported_content: item
                .get("template_content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
        })
        .collect();

    Ok(PromptTemplateImportRequest { templates })
}

fn build_prompt_template_list_payload<T: Serialize>(
    templates: T,
    total: usize,
    categories: Vec<String>,
) -> Value {
    json!({
        "templates": serde_json::to_value(templates).unwrap_or_else(|_| json!([])),
        "total": total,
        "categories": categories,
    })
}

fn build_prompt_template_system_defaults_payload<T: Serialize>(
    templates: T,
    total: usize,
) -> Value {
    json!({
        "templates": serde_json::to_value(templates).unwrap_or_else(|_| json!([])),
        "total": total,
    })
}

fn build_prompt_template_categories_payload(
    user_id: &str,
    user_templates: &[prompt_template::Model],
    system_templates: &[SystemTemplate],
    now: DateTime<Utc>,
) -> Value {
    let user_keys: BTreeSet<&str> = user_templates
        .iter()
        .map(|template| template.template_key.as_str())
        .collect();

    let mut category_map: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    for template in user_templates {
        let category_key = template
            .category
            .clone()
            .unwrap_or_else(|| "未分类".to_string());

        category_map.entry(category_key).or_default().push(json!({
            "id": template.id,
            "user_id": template.user_id,
            "template_key": template.template_key,
            "template_name": template.template_name,
            "template_content": template.template_content,
            "description": template.description,
            "category": template.category,
            "parameters": template.parameters,
            "is_active": template.is_active,
            "is_system_default": false,
            "created_at": template.created_at.and_utc().to_rfc3339(),
            "updated_at": template.updated_at.and_utc().to_rfc3339(),
        }));
    }

    for system in system_templates {
        if user_keys.contains(system.template_key.as_str()) {
            continue;
        }

        let category_key = if system.category.is_empty() {
            "未分类".to_string()
        } else {
            system.category.clone()
        };
        let params_str = serde_json::to_string(&system.parameters).unwrap_or_default();

        category_map.entry(category_key).or_default().push(json!({
            "id": system.template_key,
            "user_id": user_id,
            "template_key": system.template_key,
            "template_name": system.template_name,
            "template_content": system.content,
            "description": system.description,
            "category": system.category,
            "parameters": params_str,
            "is_active": true,
            "is_system_default": true,
            "created_at": now,
            "updated_at": now,
        }));
    }

    let mut result = Vec::new();
    for (category, mut templates) in category_map {
        templates.sort_by(|left, right| {
            left["template_key"]
                .as_str()
                .unwrap_or("")
                .cmp(right["template_key"].as_str().unwrap_or(""))
        });

        result.push(json!({
            "category": category,
            "count": templates.len(),
            "templates": templates,
        }));
    }

    json!(result)
}

fn build_prompt_template_export_payload(
    user_templates: &[prompt_template::Model],
    system_templates: &[SystemTemplate],
    export_time: DateTime<Utc>,
) -> Value {
    let user_keys: BTreeSet<&str> = user_templates
        .iter()
        .map(|template| template.template_key.as_str())
        .collect();

    let mut export_items = Vec::new();
    let mut customized_count = 0u32;
    let mut system_default_count = 0u32;

    for template in user_templates {
        let system_hash = system_templates
            .iter()
            .find(|system| system.template_key == template.template_key)
            .map(|system| system.content_hash.as_str());

        export_items.push(json!({
            "template_key": template.template_key,
            "template_name": template.template_name,
            "template_content": template.template_content,
            "description": template.description,
            "category": template.category,
            "parameters": template.parameters,
            "is_active": template.is_active,
            "is_customized": true,
            "system_content_hash": system_hash,
        }));
        customized_count += 1;
    }

    for system in system_templates {
        if user_keys.contains(system.template_key.as_str()) {
            continue;
        }

        let params_str = serde_json::to_string(&system.parameters).unwrap_or_default();
        export_items.push(json!({
            "template_key": system.template_key,
            "template_name": system.template_name,
            "template_content": system.content,
            "description": system.description,
            "category": system.category,
            "parameters": params_str,
            "is_active": true,
            "is_customized": false,
            "system_content_hash": system.content_hash,
        }));
        system_default_count += 1;
    }

    json!({
        "templates": export_items,
        "export_time": export_time,
        "version": "2.0",
        "statistics": {
            "total": customized_count + system_default_count,
            "customized": customized_count,
            "system_default": system_default_count,
        },
    })
}

fn build_prompt_template_preview_success_payload(
    rendered: String,
    parameters: &HashMap<String, String>,
) -> Value {
    json!({
        "success": true,
        "rendered_content": rendered,
        "parameters_used": parameters.keys().collect::<Vec<_>>(),
    })
}

fn build_prompt_template_preview_error_payload(error: &str) -> Value {
    json!({
        "success": false,
        "error": format!("渲染失败: {error}"),
        "rendered_content": Value::Null,
    })
}

fn build_prompt_template_sync_to_default_payload(
    template_key: &str,
    deleted: bool,
    status: Value,
) -> Value {
    let (action, message) = if deleted {
        ("reset_to_system_default", "已同步到系统默认模板")
    } else {
        ("already_system_default", "当前已是系统默认模板")
    };

    json!({
        "template_key": template_key,
        "action": action,
        "message": message,
        "status": status,
    })
}

fn build_prompt_template_reset_payload(template_key: &str, deleted: bool) -> Value {
    json!({
        "message": if deleted { "已重置为系统默认" } else { "已是系统默认状态" },
        "template_key": template_key,
    })
}

fn build_prompt_template_delete_payload(template_key: &str) -> Value {
    json!({
        "message": "模板已删除",
        "template_key": template_key,
    })
}

fn build_prompt_template_sync_status_response(items: Vec<Value>, managed_only: bool) -> Value {
    json!({
        "total": items.len(),
        "managed_only": managed_only,
        "items": items,
    })
}

fn select_prompt_template_sync_status_keys(managed_only: bool) -> Vec<String> {
    if managed_only {
        PromptTemplateService::managed_keys().to_vec()
    } else {
        PromptTemplateService::all_system_templates()
            .iter()
            .map(|template| template.template_key.clone())
            .collect()
    }
}

async fn load_prompt_template_sync_status_payload(
    db: &DatabaseConnection,
    user_id: &str,
    managed_only: bool,
) -> Result<Value, String> {
    let _ = PromptTemplateService::sync_managed_templates_for_user(db, user_id).await?;

    let template_keys = select_prompt_template_sync_status_keys(managed_only);
    let mut items = Vec::with_capacity(template_keys.len());

    for key in &template_keys {
        let user_template = PromptTemplateService::find_user_template(db, user_id, key).await?;
        items.push(PromptTemplateService::build_sync_status(
            key,
            user_template.as_ref(),
        ));
    }

    Ok(build_prompt_template_sync_status_response(
        items,
        managed_only,
    ))
}

#[cfg(test)]
fn build_prompt_templates_route_owner_contract() -> Value {
    json!({
        "owner": "prompt_templates",
        "rust_owner": "backend-rs/src/api/prompt_templates.rs",
        "routes": {
            "list": PROMPT_TEMPLATES_LIST_CREATE_ROUTE,
            "create_or_update": PROMPT_TEMPLATES_LIST_CREATE_ROUTE,
            "categories": PROMPT_TEMPLATES_CATEGORIES_ROUTE,
            "system_defaults": PROMPT_TEMPLATES_SYSTEM_DEFAULTS_ROUTE,
            "sync_status": PROMPT_TEMPLATES_SYNC_STATUS_ROUTE,
            "sync_to_default": PROMPT_TEMPLATES_SYNC_TO_DEFAULT_ROUTE,
            "detail": PROMPT_TEMPLATES_DETAIL_ROUTE,
            "update": PROMPT_TEMPLATES_DETAIL_ROUTE,
            "delete": PROMPT_TEMPLATES_DETAIL_ROUTE,
            "reset": PROMPT_TEMPLATES_RESET_ROUTE,
            "export": PROMPT_TEMPLATES_EXPORT_ROUTE,
            "import": PROMPT_TEMPLATES_IMPORT_ROUTE,
            "preview": PROMPT_TEMPLATES_PREVIEW_ROUTE
        },
        "methods": {
            "list": ["GET"],
            "create_or_update": ["POST"],
            "categories": ["GET"],
            "system_defaults": ["GET"],
            "sync_status": ["GET"],
            "sync_to_default": ["POST"],
            "detail": ["GET", "PUT", "DELETE"],
            "reset": ["POST"],
            "export": ["POST"],
            "import": ["POST"],
            "preview": ["POST"]
        },
        "service_owners": [
            "backend-rs/src/services/prompt_template_service.rs",
            "backend-rs/src/models/prompt_template.rs"
        ],
        "readiness_probes": [
            "prompt-templates-list-auth-guard-rust",
            "prompt-templates-system-defaults-auth-guard-rust",
            "prompt-templates-system-defaults-business-rust",
            "prompt-templates-create-business-rust",
            "prompt-templates-list-business-rust",
            "prompt-templates-detail-business-rust",
            "prompt-templates-sync-status-business-rust",
            "prompt-templates-export-business-rust",
            "prompt-templates-delete-business-rust",
            "prompt-templates-missing-detail-business-rust"
        ],
        "source_map_files": [
            "backend/app/api/prompt_templates.py",
            "backend/app/models/prompt_template.py",
            "backend/app/schemas/prompt_template.py",
            "backend/app/services/prompt_template_sync_service.py",
            "backend/app/services/prompt_service.py"
        ],
        "owner_profile": {
            "name": "phase5-prompt-templates-business-owner",
            "business_probes": [
                "prompt-templates-system-defaults-business-rust",
                "prompt-templates-create-business-rust",
                "prompt-templates-list-business-rust",
                "prompt-templates-detail-business-rust",
                "prompt-templates-sync-status-business-rust",
                "prompt-templates-export-business-rust",
                "prompt-templates-delete-business-rust",
                "prompt-templates-missing-detail-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "rollback_boundary": {
            "source_map_policy": "keep_python_prompt_templates_route_model_schema_sync_prompt_files_as_source_map_until_explicit_freeze_delete_round",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit source-map freeze/delete/repoint approval"
            ],
            "freeze_reason": "Rust prompt_templates route group has dedicated phase5-prompt-templates-business-owner probes for system defaults, create/list/detail, sync-status, export, delete, and missing-detail behavior; final Python source-map freeze/delete/repoint still requires explicit approval and rollback policy."
        },
        "business_smoke_status": {
            "owner_profile": "phase5-prompt-templates-business-owner",
            "readiness_probe_count": 10,
            "business_probe_count": 8,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Prompt templates route business smoke is covered by phase5-prompt-templates-business-owner; final completion now requires explicit source-map freeze/delete/repoint approval with same-round rollback policy."
    })
}

#[derive(Deserialize, Default)]
struct ListQuery {
    category: Option<String>,
    is_active: Option<bool>,
}

#[derive(Deserialize)]
struct SyncStatusQuery {
    #[serde(default = "default_managed_only")]
    managed_only: bool,
}

fn default_managed_only() -> bool {
    true
}

// GET /prompt-templates
async fn get_all_templates(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Query(params): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _ = PromptTemplateService::sync_managed_templates_for_user(&db, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let (templates, categories) = PromptTemplateService::list_user_templates(
        &db,
        &claims.sub,
        params.category.as_deref(),
        params.is_active,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })?;
    let total = templates.len();

    Ok(Json(build_prompt_template_list_payload(
        templates, total, categories,
    )))
}

// GET /prompt-templates/categories
async fn get_templates_by_category(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _ = PromptTemplateService::sync_managed_templates_for_user(&db, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let (user_templates, _) =
        PromptTemplateService::list_user_templates(&db, &claims.sub, None, None)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": format!("{}", e)})),
                )
            })?;

    Ok(Json(build_prompt_template_categories_payload(
        &claims.sub,
        &user_templates,
        PromptTemplateService::all_system_templates(),
        chrono::Utc::now(),
    )))
}

// GET /prompt-templates/system-defaults
async fn get_system_defaults(
    Extension(_claims): Extension<Claims>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let templates = PromptTemplateService::all_system_templates();
    Ok(Json(build_prompt_template_system_defaults_payload(
        templates,
        templates.len(),
    )))
}

// GET /prompt-templates/sync-status
async fn get_template_sync_status(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Query(params): Query<SyncStatusQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    load_prompt_template_sync_status_payload(&db, &claims.sub, params.managed_only)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

// POST /prompt-templates/{template_key}/sync-to-default
async fn sync_template_to_default(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(template_key): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _info = PromptTemplateService::system_template_info(&template_key).ok_or((
        StatusCode::NOT_FOUND,
        Json(json!({"detail": format!("系统默认模板 {} 不存在", template_key)})),
    ))?;

    let deleted = PromptTemplateService::delete_user_template(&db, &claims.sub, &template_key)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let status = PromptTemplateService::build_sync_status(&template_key, None);

    Ok(Json(build_prompt_template_sync_to_default_payload(
        &template_key,
        deleted,
        status,
    )))
}

// GET /prompt-templates/{template_key}
async fn get_template(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(template_key): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _ = PromptTemplateService::sync_managed_templates_for_user(&db, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let tmpl = PromptTemplateService::find_user_template(&db, &claims.sub, &template_key)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": format!("模板 {} 不存在", template_key)})),
        ))?;

    Ok(Json(json!(tmpl)))
}

// POST /prompt-templates (create or update)
async fn create_or_update_template(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<PromptTemplateUpsertRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_prompt_template_upsert_payload_from_route_payload(body);

    let tmpl = PromptTemplateService::upsert_template(&db, &claims.sub, &request)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;
    Ok(Json(json!(tmpl)))
}

// PUT /prompt-templates/{template_key}
async fn update_template(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(template_key): Path<String>,
    Json(body): Json<PromptTemplateUpdateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let existing = PromptTemplateService::find_user_template(&db, &claims.sub, &template_key)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": format!("模板 {} 不存在", template_key)})),
        ))?;

    let request = build_prompt_template_update_payload_from_route_payload(body, &existing);

    let tmpl = PromptTemplateService::upsert_template(&db, &claims.sub, &request)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;
    Ok(Json(json!(tmpl)))
}

// DELETE /prompt-templates/{template_key}
async fn delete_template(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(template_key): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let existing = PromptTemplateService::find_user_template(&db, &claims.sub, &template_key)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    if existing.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": format!("模板 {} 不存在", template_key)})),
        ));
    }

    PromptTemplateService::delete_user_template(&db, &claims.sub, &template_key)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    Ok(Json(build_prompt_template_delete_payload(&template_key)))
}

// POST /prompt-templates/{template_key}/reset
async fn reset_to_default(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(template_key): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _info = PromptTemplateService::system_template_info(&template_key).ok_or((
        StatusCode::NOT_FOUND,
        Json(json!({"detail": format!("系统默认模板 {} 不存在", template_key)})),
    ))?;

    let deleted = PromptTemplateService::delete_user_template(&db, &claims.sub, &template_key)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    Ok(Json(build_prompt_template_reset_payload(
        &template_key,
        deleted,
    )))
}

// POST /prompt-templates/export
async fn export_templates(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (user_templates, _) =
        PromptTemplateService::list_user_templates(&db, &claims.sub, None, None)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": format!("{}", e)})),
                )
            })?;

    Ok(Json(build_prompt_template_export_payload(
        &user_templates,
        PromptTemplateService::all_system_templates(),
        chrono::Utc::now(),
    )))
}

// POST /prompt-templates/import
async fn import_templates(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<PromptTemplateImportRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_prompt_template_import_request_from_route_payload(body).map_err(
        |error| match error {
            BuildPromptTemplateImportRequestError::MissingTemplates => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "缺少 templates 字段"})),
            ),
        },
    )?;

    let system_templates = PromptTemplateService::all_system_templates();
    let system_dict: HashMap<&str, &prompt_template_service::SystemTemplate> = system_templates
        .iter()
        .map(|s| (s.template_key.as_str(), s))
        .collect();

    let mut kept_system_default = 0u32;
    let mut created_or_updated = 0u32;
    let mut converted_to_custom = 0u32;
    let mut converted_list = Vec::new();

    for item in request.templates() {
        let template_key = item.template_key().to_string();
        let template_name = item.template_name_value().clone();
        let is_customized = item.is_customized();
        let imported_content = item.imported_content().to_string();

        let existing = PromptTemplateService::find_user_template(&db, &claims.sub, &template_key)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": format!("{}", e)})),
                )
            })?;

        let system = system_dict.get(template_key.as_str());

        if !is_customized {
            if let Some(sys) = system {
                let system_content = sys.content.trim();
                if imported_content == system_content {
                    if existing.is_some() {
                        PromptTemplateService::delete_user_template(
                            &db,
                            &claims.sub,
                            &template_key,
                        )
                        .await
                        .map_err(|e| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"detail": format!("{}", e)})),
                            )
                        })?;
                    }
                    kept_system_default += 1;
                } else {
                    // Content differs from system, convert to custom
                    let merge_data = item.upsert_payload();
                    PromptTemplateService::upsert_template(&db, &claims.sub, &merge_data)
                        .await
                        .map_err(|e| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"detail": format!("{}", e)})),
                            )
                        })?;
                    converted_to_custom += 1;
                    converted_list.push(json!({
                        "template_key": template_key,
                        "template_name": template_name,
                        "reason": "内容与系统默认不一致，已转为自定义",
                    }));
                }
            } else {
                // System doesn't have this template, import as custom
                let merge_data = item.upsert_payload();
                PromptTemplateService::upsert_template(&db, &claims.sub, &merge_data)
                    .await
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"detail": format!("{}", e)})),
                        )
                    })?;
                created_or_updated += 1;
            }
        } else {
            // Marked as customized, direct upsert
            let merge_data = item.upsert_payload();
            PromptTemplateService::upsert_template(&db, &claims.sub, &merge_data)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"detail": format!("{}", e)})),
                    )
                })?;
            created_or_updated += 1;
        }
    }

    Ok(Json(json!({
        "message": "导入成功",
        "statistics": {
            "total": request.templates().len(),
            "kept_system_default": kept_system_default,
            "created_or_updated": created_or_updated,
            "converted_to_custom": converted_to_custom,
        },
        "converted_templates": converted_list,
    })))
}

// POST /prompt-templates/{template_key}/preview
#[derive(Deserialize)]
struct PreviewRequest {
    template_content: String,
    parameters: Option<HashMap<String, String>>,
}

async fn preview_template(
    Extension(_claims): Extension<Claims>,
    Path(_template_key): Path<String>,
    Json(body): Json<PreviewRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let params = body.parameters.unwrap_or_default();
    match PromptTemplateService::format_prompt(&body.template_content, &params) {
        Ok(rendered) => Ok(Json(build_prompt_template_preview_success_payload(
            rendered, &params,
        ))),
        Err(e) => Ok(Json(build_prompt_template_preview_error_payload(
            &e.to_string(),
        ))),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(PROMPT_TEMPLATES_LIST_CREATE_ROUTE, get(get_all_templates))
        .route(
            PROMPT_TEMPLATES_CATEGORIES_ROUTE,
            get(get_templates_by_category),
        )
        .route(
            PROMPT_TEMPLATES_SYSTEM_DEFAULTS_ROUTE,
            get(get_system_defaults),
        )
        .route(
            PROMPT_TEMPLATES_SYNC_STATUS_ROUTE,
            get(get_template_sync_status),
        )
        .route(
            PROMPT_TEMPLATES_SYNC_TO_DEFAULT_ROUTE,
            post(sync_template_to_default),
        )
        .route(PROMPT_TEMPLATES_DETAIL_ROUTE, get(get_template))
        .route(
            PROMPT_TEMPLATES_LIST_CREATE_ROUTE,
            post(create_or_update_template),
        )
        .route(PROMPT_TEMPLATES_DETAIL_ROUTE, put(update_template))
        .route(PROMPT_TEMPLATES_DETAIL_ROUTE, delete(delete_template))
        .route(PROMPT_TEMPLATES_RESET_ROUTE, post(reset_to_default))
        .route(PROMPT_TEMPLATES_EXPORT_ROUTE, post(export_templates))
        .route(PROMPT_TEMPLATES_IMPORT_ROUTE, post(import_templates))
        .route(PROMPT_TEMPLATES_PREVIEW_ROUTE, post(preview_template))
}

#[cfg(test)]
mod tests {
    use super::{
        build_prompt_template_categories_payload, build_prompt_template_delete_payload,
        build_prompt_template_export_payload,
        build_prompt_template_import_request_from_route_payload,
        build_prompt_template_list_payload, build_prompt_template_preview_error_payload,
        build_prompt_template_preview_success_payload, build_prompt_template_reset_payload,
        build_prompt_template_sync_status_response, build_prompt_template_sync_to_default_payload,
        build_prompt_template_system_defaults_payload,
        build_prompt_template_update_payload_from_route_body,
        build_prompt_template_update_payload_from_route_payload,
        build_prompt_template_upsert_payload_from_route_body,
        build_prompt_template_upsert_payload_from_route_payload,
        build_prompt_templates_route_owner_contract, select_prompt_template_sync_status_keys,
        BuildPromptTemplateImportRequestError, PromptTemplateImportRouteRequest,
        PromptTemplateUpdateRouteRequest, PromptTemplateUpsertRouteRequest,
        PROMPT_TEMPLATES_CATEGORIES_ROUTE, PROMPT_TEMPLATES_DETAIL_ROUTE,
        PROMPT_TEMPLATES_EXPORT_ROUTE, PROMPT_TEMPLATES_IMPORT_ROUTE,
        PROMPT_TEMPLATES_LIST_CREATE_ROUTE, PROMPT_TEMPLATES_PREVIEW_ROUTE,
        PROMPT_TEMPLATES_RESET_ROUTE, PROMPT_TEMPLATES_SYNC_STATUS_ROUTE,
        PROMPT_TEMPLATES_SYNC_TO_DEFAULT_ROUTE, PROMPT_TEMPLATES_SYSTEM_DEFAULTS_ROUTE,
    };
    use crate::models::prompt_template;
    use crate::services::prompt_template_service::SystemTemplate;
    use chrono::{DateTime, TimeZone};
    use serde_json::{json, Value};
    use std::collections::HashMap;

    fn sample_prompt_template() -> prompt_template::Model {
        prompt_template::Model {
            id: "template-id".to_string(),
            user_id: "user-1".to_string(),
            template_key: "chapter_generate".to_string(),
            template_name: "章节生成".to_string(),
            template_content: "existing content".to_string(),
            description: Some("existing description".to_string()),
            category: Some("writing".to_string()),
            parameters: Some("[\"chapter_title\"]".to_string()),
            is_active: true,
            is_system_default: false,
            created_at: DateTime::from_timestamp(0, 0)
                .expect("valid time")
                .naive_utc(),
            updated_at: DateTime::from_timestamp(0, 0)
                .expect("valid time")
                .naive_utc(),
        }
    }

    #[test]
    fn should_publish_prompt_templates_route_owner_contract() {
        let contract = build_prompt_templates_route_owner_contract();

        assert_eq!(contract["owner"], "prompt_templates");
        assert_eq!(
            contract["rust_owner"],
            "backend-rs/src/api/prompt_templates.rs"
        );
        assert_eq!(
            contract["routes"]["list"],
            PROMPT_TEMPLATES_LIST_CREATE_ROUTE
        );
        assert_eq!(
            contract["routes"]["create_or_update"],
            PROMPT_TEMPLATES_LIST_CREATE_ROUTE
        );
        assert_eq!(
            contract["routes"]["categories"],
            PROMPT_TEMPLATES_CATEGORIES_ROUTE
        );
        assert_eq!(
            contract["routes"]["system_defaults"],
            PROMPT_TEMPLATES_SYSTEM_DEFAULTS_ROUTE
        );
        assert_eq!(
            contract["routes"]["sync_status"],
            PROMPT_TEMPLATES_SYNC_STATUS_ROUTE
        );
        assert_eq!(
            contract["routes"]["sync_to_default"],
            PROMPT_TEMPLATES_SYNC_TO_DEFAULT_ROUTE
        );
        assert_eq!(contract["routes"]["detail"], PROMPT_TEMPLATES_DETAIL_ROUTE);
        assert_eq!(contract["routes"]["update"], PROMPT_TEMPLATES_DETAIL_ROUTE);
        assert_eq!(contract["routes"]["delete"], PROMPT_TEMPLATES_DETAIL_ROUTE);
        assert_eq!(contract["routes"]["reset"], PROMPT_TEMPLATES_RESET_ROUTE);
        assert_eq!(contract["routes"]["export"], PROMPT_TEMPLATES_EXPORT_ROUTE);
        assert_eq!(contract["routes"]["import"], PROMPT_TEMPLATES_IMPORT_ROUTE);
        assert_eq!(
            contract["routes"]["preview"],
            PROMPT_TEMPLATES_PREVIEW_ROUTE
        );

        assert_eq!(
            contract["methods"]["detail"],
            json!(["GET", "PUT", "DELETE"])
        );
        assert_eq!(contract["methods"]["create_or_update"], json!(["POST"]));
        assert_eq!(
            contract["service_owners"]
                .as_array()
                .expect("service owner list should be present")
                .len(),
            2
        );
        assert_eq!(
            contract["readiness_probes"]
                .as_array()
                .expect("readiness probes should be present")
                .len(),
            10
        );
        assert_eq!(
            contract["readiness_probes"][9],
            "prompt-templates-missing-detail-business-rust"
        );
        assert_eq!(
            contract["source_map_files"]
                .as_array()
                .expect("source map files should be present")
                .len(),
            5
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-prompt-templates-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][5],
            "prompt-templates-export-business-rust"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            10
        );
        assert_eq!(contract["business_smoke_status"]["business_probe_count"], 8);
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("prompt templates migration policy should be present")
            .contains("phase5-prompt-templates-business-owner"));
    }

    #[test]
    fn should_keep_prompt_templates_route_group_paths_stable() {
        assert_eq!(PROMPT_TEMPLATES_LIST_CREATE_ROUTE, "/prompt-templates");
        assert_eq!(
            PROMPT_TEMPLATES_CATEGORIES_ROUTE,
            "/prompt-templates/categories"
        );
        assert_eq!(
            PROMPT_TEMPLATES_SYSTEM_DEFAULTS_ROUTE,
            "/prompt-templates/system-defaults"
        );
        assert_eq!(
            PROMPT_TEMPLATES_SYNC_STATUS_ROUTE,
            "/prompt-templates/sync-status"
        );
        assert_eq!(
            PROMPT_TEMPLATES_SYNC_TO_DEFAULT_ROUTE,
            "/prompt-templates/{template_key}/sync-to-default"
        );
        assert_eq!(
            PROMPT_TEMPLATES_DETAIL_ROUTE,
            "/prompt-templates/{template_key}"
        );
        assert_eq!(
            PROMPT_TEMPLATES_RESET_ROUTE,
            "/prompt-templates/{template_key}/reset"
        );
        assert_eq!(PROMPT_TEMPLATES_EXPORT_ROUTE, "/prompt-templates/export");
        assert_eq!(PROMPT_TEMPLATES_IMPORT_ROUTE, "/prompt-templates/import");
        assert_eq!(
            PROMPT_TEMPLATES_PREVIEW_ROUTE,
            "/prompt-templates/{template_key}/preview"
        );
    }

    #[test]
    fn build_prompt_template_upsert_payload_from_route_body_keeps_payload_shape() {
        let body = json!({
            "template_key": "chapter_generate",
            "template_name": "章节生成",
            "template_content": "new content",
            "description": null,
            "category": "writing",
            "parameters": {
                "vars": ["chapter_title"]
            },
            "is_active": false
        });

        let payload = build_prompt_template_upsert_payload_from_route_body(&body);

        assert_eq!(payload, body);
    }

    #[test]
    fn build_prompt_template_upsert_payload_from_route_payload_keeps_payload_shape() {
        let payload = build_prompt_template_upsert_payload_from_route_payload(
            PromptTemplateUpsertRouteRequest {
                body: json!({
                    "template_key": "chapter_generate",
                    "template_name": "章节生成",
                    "template_content": "new content",
                    "description": null,
                    "category": "writing",
                    "parameters": {
                        "vars": ["chapter_title"]
                    },
                    "is_active": false
                }),
            },
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["template_content"], "new content");
    }

    #[test]
    fn build_prompt_template_update_payload_from_route_body_keeps_existing_fields_when_missing() {
        let existing = sample_prompt_template();

        let payload = build_prompt_template_update_payload_from_route_body(
            &json!({
                "template_content": "updated content"
            }),
            &existing,
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["template_name"], "章节生成");
        assert_eq!(payload["template_content"], "updated content");
        assert_eq!(payload["description"], "existing description");
        assert_eq!(payload["category"], "writing");
        assert_eq!(payload["parameters"], "[\"chapter_title\"]");
        assert_eq!(payload["is_active"], true);
    }

    #[test]
    fn build_prompt_template_update_payload_from_route_body_prefers_route_values() {
        let existing = sample_prompt_template();

        let payload = build_prompt_template_update_payload_from_route_body(
            &json!({
                "template_key": "chapter_rewrite",
                "template_name": "章节改写",
                "description": null,
                "parameters": {
                    "vars": ["scene"]
                },
                "is_active": false
            }),
            &existing,
        );

        assert_eq!(payload["template_key"], "chapter_rewrite");
        assert_eq!(payload["template_name"], "章节改写");
        assert!(payload["description"].is_null());
        assert_eq!(payload["parameters"], json!({"vars": ["scene"]}));
        assert_eq!(payload["is_active"], false);
    }

    #[test]
    fn build_prompt_template_update_payload_from_route_body_ignores_non_object_body() {
        let existing = sample_prompt_template();

        let payload = build_prompt_template_update_payload_from_route_body(
            &json!("not-an-object"),
            &existing,
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["template_content"], "existing content");
        assert_eq!(payload["description"], "existing description");
    }

    #[test]
    fn build_prompt_template_update_payload_from_route_payload_keeps_existing_fields_when_missing()
    {
        let existing = sample_prompt_template();

        let payload = build_prompt_template_update_payload_from_route_payload(
            PromptTemplateUpdateRouteRequest {
                body: json!({
                    "template_content": "updated content"
                }),
            },
            &existing,
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["template_name"], "章节生成");
        assert_eq!(payload["template_content"], "updated content");
        assert_eq!(payload["description"], "existing description");
    }

    #[test]
    fn build_prompt_template_import_request_from_route_payload_requires_templates_array() {
        let error = build_prompt_template_import_request_from_route_payload(
            PromptTemplateImportRouteRequest { templates: None },
        )
        .expect_err("missing templates should fail");

        assert_eq!(
            error,
            BuildPromptTemplateImportRequestError::MissingTemplates
        );
    }

    #[test]
    fn build_prompt_template_import_request_from_route_payload_projects_import_items() {
        let request = build_prompt_template_import_request_from_route_payload(
            PromptTemplateImportRouteRequest {
                templates: Some(json!([
                    {
                        "template_key": "chapter_generate",
                        "template_name": "章节生成",
                        "template_content": "  imported content  ",
                        "is_customized": true,
                        "category": "writing"
                    }
                ])),
            },
        )
        .expect("templates should be parsed");

        let item = &request.templates()[0];

        assert_eq!(item.template_key(), "chapter_generate");
        assert_eq!(item.template_name_value(), "章节生成");
        assert_eq!(item.imported_content(), "imported content");
        assert!(item.is_customized());
        assert_eq!(
            item.upsert_payload(),
            json!({
                "template_key": "chapter_generate",
                "template_name": "章节生成",
                "template_content": "  imported content  ",
                "is_customized": true,
                "category": "writing"
            })
        );
    }

    #[test]
    fn build_prompt_template_import_request_from_route_payload_keeps_non_object_item_payload() {
        let request = build_prompt_template_import_request_from_route_payload(
            PromptTemplateImportRouteRequest {
                templates: Some(json!(["raw-template"])),
            },
        )
        .expect("templates should be parsed");

        let item = &request.templates()[0];

        assert_eq!(item.template_key(), "");
        assert!(item.template_name_value().is_null());
        assert_eq!(item.imported_content(), "");
        assert!(!item.is_customized());
        assert_eq!(item.upsert_payload(), json!("raw-template"));
    }

    fn category_test_datetime() -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str("2026-05-22T03:00:00", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn category_user_template(
        template_key: &str,
        category: Option<&str>,
        template_name: &str,
    ) -> prompt_template::Model {
        prompt_template::Model {
            id: format!("id-{template_key}"),
            user_id: "user-1".to_string(),
            template_key: template_key.to_string(),
            template_name: template_name.to_string(),
            template_content: format!("content-{template_key}"),
            description: Some(format!("desc-{template_key}")),
            category: category.map(str::to_string),
            parameters: Some("[\"tone\"]".to_string()),
            is_active: true,
            is_system_default: false,
            created_at: category_test_datetime(),
            updated_at: category_test_datetime(),
        }
    }

    fn category_system_templates() -> Vec<SystemTemplate> {
        vec![
            SystemTemplate {
                template_key: "alpha".to_string(),
                template_name: "Alpha".to_string(),
                category: "分类A".to_string(),
                description: "system alpha".to_string(),
                parameters: vec!["tone".to_string()],
                content: "alpha system".to_string(),
                content_hash: "hash-alpha".to_string(),
            },
            SystemTemplate {
                template_key: "gamma".to_string(),
                template_name: "Gamma".to_string(),
                category: String::new(),
                description: "system gamma".to_string(),
                parameters: vec!["style".to_string()],
                content: "gamma system".to_string(),
                content_hash: "hash-gamma".to_string(),
            },
        ]
    }

    fn export_test_datetime() -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str("2026-05-22T02:15:00", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn export_user_template() -> prompt_template::Model {
        prompt_template::Model {
            id: "template-1".to_string(),
            user_id: "user-1".to_string(),
            template_key: "chapter_generate".to_string(),
            template_name: "章节生成".to_string(),
            template_content: "custom content".to_string(),
            description: Some("自定义描述".to_string()),
            category: Some("生成".to_string()),
            parameters: Some("[\"tone\"]".to_string()),
            is_active: true,
            is_system_default: false,
            created_at: export_test_datetime(),
            updated_at: export_test_datetime(),
        }
    }

    fn export_system_templates() -> Vec<SystemTemplate> {
        vec![
            SystemTemplate {
                template_key: "chapter_generate".to_string(),
                template_name: "章节生成".to_string(),
                category: "生成".to_string(),
                description: "默认描述".to_string(),
                parameters: vec!["tone".to_string()],
                content: "system content".to_string(),
                content_hash: "sys-hash-1".to_string(),
            },
            SystemTemplate {
                template_key: "chapter_rewrite".to_string(),
                template_name: "章节重写".to_string(),
                category: "改写".to_string(),
                description: "默认改写".to_string(),
                parameters: vec!["style".to_string()],
                content: "rewrite content".to_string(),
                content_hash: "sys-hash-2".to_string(),
            },
        ]
    }

    #[test]
    fn build_prompt_template_list_payload_keeps_templates_total_and_categories() {
        let payload = build_prompt_template_list_payload(
            vec![json!({"template_key": "alpha"})],
            1,
            vec!["分类A".to_string(), "分类B".to_string()],
        );

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["templates"][0]["template_key"], "alpha");
        assert_eq!(payload["categories"][0], "分类A");
        assert_eq!(payload["categories"][1], "分类B");
    }

    #[test]
    fn build_prompt_template_system_defaults_payload_keeps_templates_and_total() {
        let payload = build_prompt_template_system_defaults_payload(
            vec![
                json!({"template_key": "alpha"}),
                json!({"template_key": "beta"}),
            ],
            2,
        );

        assert_eq!(payload["total"], 2);
        assert_eq!(payload["templates"][0]["template_key"], "alpha");
        assert_eq!(payload["templates"][1]["template_key"], "beta");
    }

    #[test]
    fn build_prompt_template_categories_payload_groups_and_sorts_templates() {
        let payload = build_prompt_template_categories_payload(
            "user-1",
            &[
                category_user_template("beta", Some("分类A"), "Beta"),
                category_user_template("delta", None, "Delta"),
            ],
            &category_system_templates(),
            chrono::Utc
                .with_ymd_and_hms(2026, 5, 22, 3, 0, 0)
                .single()
                .expect("datetime should be valid"),
        );

        let groups = payload.as_array().expect("groups should be an array");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0]["category"], "分类A");
        assert_eq!(groups[0]["count"], 2);
        assert_eq!(groups[0]["templates"][0]["template_key"], "alpha");
        assert_eq!(groups[0]["templates"][0]["is_system_default"], true);
        assert_eq!(groups[0]["templates"][1]["template_key"], "beta");
        assert_eq!(groups[0]["templates"][1]["is_system_default"], false);

        assert_eq!(groups[1]["category"], "未分类");
        assert_eq!(groups[1]["count"], 2);
        assert_eq!(groups[1]["templates"][0]["template_key"], "delta");
        assert_eq!(groups[1]["templates"][0]["category"], Value::Null);
        assert_eq!(groups[1]["templates"][1]["template_key"], "gamma");
        assert_eq!(groups[1]["templates"][1]["user_id"], "user-1");
        assert_eq!(groups[1]["templates"][1]["category"], "");
        assert_eq!(groups[1]["templates"][1]["parameters"], "[\"style\"]");
    }

    #[test]
    fn build_prompt_template_categories_payload_skips_system_defaults_overridden_by_user() {
        let payload = build_prompt_template_categories_payload(
            "user-1",
            &[category_user_template(
                "alpha",
                Some("分类A"),
                "Alpha Custom",
            )],
            &category_system_templates(),
            chrono::Utc
                .with_ymd_and_hms(2026, 5, 22, 3, 0, 0)
                .single()
                .expect("datetime should be valid"),
        );

        let groups = payload.as_array().expect("groups should be an array");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0]["category"], "分类A");
        assert_eq!(groups[0]["count"], 1);
        assert_eq!(groups[0]["templates"][0]["template_key"], "alpha");
        assert_eq!(groups[0]["templates"][0]["is_system_default"], false);
    }

    #[test]
    fn build_prompt_template_export_payload_keeps_custom_and_system_items() {
        let payload = build_prompt_template_export_payload(
            &[export_user_template()],
            &export_system_templates(),
            chrono::Utc
                .with_ymd_and_hms(2026, 5, 22, 2, 15, 0)
                .single()
                .expect("datetime should be valid"),
        );

        assert_eq!(payload["version"], "2.0");
        assert_eq!(payload["statistics"]["total"], 2);
        assert_eq!(payload["statistics"]["customized"], 1);
        assert_eq!(payload["statistics"]["system_default"], 1);

        let items = payload["templates"]
            .as_array()
            .expect("templates should be an array");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["template_key"], "chapter_generate");
        assert_eq!(items[0]["is_customized"], true);
        assert_eq!(items[0]["system_content_hash"], "sys-hash-1");
        assert_eq!(items[1]["template_key"], "chapter_rewrite");
        assert_eq!(items[1]["is_customized"], false);
        assert_eq!(items[1]["parameters"], "[\"style\"]");
    }

    #[test]
    fn build_prompt_template_export_payload_omits_duplicate_system_default_items() {
        let mut second_user_template = export_user_template();
        second_user_template.template_key = "chapter_rewrite".to_string();
        second_user_template.template_name = "章节重写-用户版".to_string();

        let payload = build_prompt_template_export_payload(
            &[export_user_template(), second_user_template],
            &export_system_templates(),
            chrono::Utc
                .with_ymd_and_hms(2026, 5, 22, 2, 15, 0)
                .single()
                .expect("datetime should be valid"),
        );

        let items = payload["templates"]
            .as_array()
            .expect("templates should be an array");
        assert_eq!(items.len(), 2);
        assert_eq!(payload["statistics"]["customized"], 2);
        assert_eq!(payload["statistics"]["system_default"], 0);
    }

    #[test]
    fn build_prompt_template_preview_success_payload_keeps_rendered_content_and_parameters() {
        let mut parameters = HashMap::new();
        parameters.insert("tone".to_string(), "warm".to_string());

        let payload = build_prompt_template_preview_success_payload(
            "rendered result".to_string(),
            &parameters,
        );

        assert_eq!(payload["success"], true);
        assert_eq!(payload["rendered_content"], "rendered result");
        let parameters_used = payload["parameters_used"]
            .as_array()
            .expect("parameters_used should be an array");
        assert_eq!(parameters_used.len(), 1);
        assert_eq!(parameters_used[0], "tone");
    }

    #[test]
    fn build_prompt_template_preview_error_payload_keeps_compat_error_shape() {
        let payload = build_prompt_template_preview_error_payload("missing variable");

        assert_eq!(payload["success"], false);
        assert_eq!(payload["error"], "渲染失败: missing variable");
        assert!(payload["rendered_content"].is_null());
    }

    #[test]
    fn build_prompt_template_sync_to_default_payload_keeps_deleted_contract() {
        let payload = build_prompt_template_sync_to_default_payload(
            "chapter_generate",
            true,
            json!({"sync_state": "system_default"}),
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["action"], "reset_to_system_default");
        assert_eq!(payload["message"], "已同步到系统默认模板");
        assert_eq!(payload["status"]["sync_state"], "system_default");
    }

    #[test]
    fn build_prompt_template_sync_to_default_payload_keeps_existing_default_contract() {
        let payload = build_prompt_template_sync_to_default_payload(
            "chapter_generate",
            false,
            json!({"sync_state": "system_default"}),
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["action"], "already_system_default");
        assert_eq!(payload["message"], "当前已是系统默认模板");
        assert_eq!(payload["status"]["sync_state"], "system_default");
    }

    #[test]
    fn build_prompt_template_reset_payload_keeps_reset_message_variants() {
        let deleted_payload = build_prompt_template_reset_payload("chapter_generate", true);
        let unchanged_payload = build_prompt_template_reset_payload("chapter_generate", false);

        assert_eq!(deleted_payload["template_key"], "chapter_generate");
        assert_eq!(deleted_payload["message"], "已重置为系统默认");
        assert_eq!(unchanged_payload["template_key"], "chapter_generate");
        assert_eq!(unchanged_payload["message"], "已是系统默认状态");
    }

    #[test]
    fn build_prompt_template_delete_payload_keeps_delete_success_shape() {
        let payload = build_prompt_template_delete_payload("chapter_generate");

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["message"], "模板已删除");
    }

    #[test]
    fn build_prompt_template_sync_status_response_keeps_existing_shell() {
        let payload = build_prompt_template_sync_status_response(
            vec![json!({
                "template_key": "chapter_generate",
                "sync_status": "system_default",
            })],
            true,
        );

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["managed_only"], true);
        assert_eq!(payload["items"][0]["template_key"], "chapter_generate");
        assert_eq!(payload["items"][0]["sync_status"], "system_default");
    }

    #[test]
    fn select_prompt_template_sync_status_keys_uses_managed_filter_when_enabled() {
        let managed_keys = select_prompt_template_sync_status_keys(true);
        let all_keys = select_prompt_template_sync_status_keys(false);

        assert!(!managed_keys.is_empty());
        assert!(all_keys.len() >= managed_keys.len());
        assert!(managed_keys.iter().all(|key| all_keys.contains(key)));
    }
}
