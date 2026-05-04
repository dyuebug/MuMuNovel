use std::collections::HashMap;

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::prompt_template_service::{self, PromptTemplateService};

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

    let (templates, categories) =
        PromptTemplateService::list_user_templates(&db, &claims.sub, params.category.as_deref(), params.is_active)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": format!("{}", e)})),
                )
            })?;

    Ok(Json(json!({
        "templates": templates,
        "total": templates.len(),
        "categories": categories,
    })))
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

    let user_keys: std::collections::BTreeSet<String> = user_templates
        .iter()
        .map(|t| t.template_key.clone())
        .collect();

    let system_templates = PromptTemplateService::all_system_templates();

    let mut category_map: HashMap<String, Vec<Value>> = HashMap::new();
    let now = chrono::Utc::now();

    // Add user templates
    for t in &user_templates {
        let cat = t.category.clone().unwrap_or_else(|| "未分类".to_string());
        let tmpl = json!({
            "id": t.id,
            "user_id": t.user_id,
            "template_key": t.template_key,
            "template_name": t.template_name,
            "template_content": t.template_content,
            "description": t.description,
            "category": t.category,
            "parameters": t.parameters,
            "is_active": t.is_active,
            "is_system_default": false,
            "created_at": t.created_at,
            "updated_at": t.updated_at,
        });
        category_map.entry(cat).or_default().push(tmpl);
    }

    // Add system defaults not customized by user
    for sys in system_templates {
        if user_keys.contains(&sys.template_key) {
            continue;
        }
        let cat = if sys.category.is_empty() {
            "未分类".to_string()
        } else {
            sys.category.clone()
        };
        let params_str = serde_json::to_string(&sys.parameters).unwrap_or_default();
        let tmpl = json!({
            "id": sys.template_key,
            "user_id": claims.sub,
            "template_key": sys.template_key,
            "template_name": sys.template_name,
            "template_content": sys.content,
            "description": sys.description,
            "category": sys.category,
            "parameters": params_str,
            "is_active": true,
            "is_system_default": true,
            "created_at": now,
            "updated_at": now,
        });
        category_map.entry(cat).or_default().push(tmpl);
    }

    let mut result: Vec<Value> = category_map
        .into_iter()
        .map(|(category, mut templates)| {
            templates.sort_by(|a, b| {
                a["template_key"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["template_key"].as_str().unwrap_or(""))
            });
            json!({
                "category": category,
                "count": templates.len(),
                "templates": templates,
            })
        })
        .collect();
    result.sort_by(|a, b| {
        a["category"]
            .as_str()
            .unwrap_or("")
            .cmp(b["category"].as_str().unwrap_or(""))
    });

    Ok(Json(json!(result)))
}

// GET /prompt-templates/system-defaults
async fn get_system_defaults(
    Extension(_claims): Extension<Claims>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let templates = PromptTemplateService::all_system_templates();
    Ok(Json(json!({
        "templates": templates,
        "total": templates.len(),
    })))
}

// GET /prompt-templates/sync-status
async fn get_template_sync_status(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Query(params): Query<SyncStatusQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _ = PromptTemplateService::sync_managed_templates_for_user(&db, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let template_keys: Vec<String> = if params.managed_only {
        PromptTemplateService::managed_keys().to_vec()
    } else {
        PromptTemplateService::all_system_templates()
            .iter()
            .map(|t| t.template_key.clone())
            .collect()
    };

    let mut items = Vec::new();
    for key in &template_keys {
        let user_tmpl = PromptTemplateService::find_user_template(&db, &claims.sub, key)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": format!("{}", e)})),
                )
            })?;
        let status = PromptTemplateService::build_sync_status(key, user_tmpl.as_ref());
        items.push(status);
    }

    Ok(Json(json!({
        "total": items.len(),
        "managed_only": params.managed_only,
        "items": items,
    })))
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

    let (action, message) = if deleted {
        ("reset_to_system_default", "已同步到系统默认模板")
    } else {
        ("already_system_default", "当前已是系统默认模板")
    };

    let status = PromptTemplateService::build_sync_status(&template_key, None);

    Ok(Json(json!({
        "template_key": template_key,
        "action": action,
        "message": message,
        "status": status,
    })))
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
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tmpl = PromptTemplateService::upsert_template(&db, &claims.sub, &body)
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
    Json(body): Json<Value>,
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

    // Merge body into existing, then upsert
    let mut merged = json!(existing);
    if let Value::Object(ref mut map) = merged {
        if let Value::Object(body_map) = &body {
            for (k, v) in body_map {
                map.insert(k.clone(), v.clone());
            }
        }
    }

    let tmpl = PromptTemplateService::upsert_template(&db, &claims.sub, &merged)
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

    Ok(Json(json!({"message": "模板已删除", "template_key": template_key})))
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

    Ok(Json(json!({
        "message": if deleted { "已重置为系统默认" } else { "已是系统默认状态" },
        "template_key": template_key,
    })))
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

    let system_templates = PromptTemplateService::all_system_templates();
    let user_keys: std::collections::BTreeSet<String> = user_templates
        .iter()
        .map(|t| t.template_key.clone())
        .collect();

    let mut export_items = Vec::new();
    let mut customized_count = 0u32;
    let mut system_default_count = 0u32;

    for t in &user_templates {
        let sys = system_templates
            .iter()
            .find(|s| s.template_key == t.template_key);
        let system_hash = sys.map(|s| s.content_hash.as_str());
        export_items.push(json!({
            "template_key": t.template_key,
            "template_name": t.template_name,
            "template_content": t.template_content,
            "description": t.description,
            "category": t.category,
            "parameters": t.parameters,
            "is_active": t.is_active,
            "is_customized": true,
            "system_content_hash": system_hash,
        }));
        customized_count += 1;
    }

    for sys in system_templates {
        if user_keys.contains(&sys.template_key) {
            continue;
        }
        let params_str = serde_json::to_string(&sys.parameters).unwrap_or_default();
        export_items.push(json!({
            "template_key": sys.template_key,
            "template_name": sys.template_name,
            "template_content": sys.content,
            "description": sys.description,
            "category": sys.category,
            "parameters": params_str,
            "is_active": true,
            "is_customized": false,
            "system_content_hash": sys.content_hash,
        }));
        system_default_count += 1;
    }

    Ok(Json(json!({
        "templates": export_items,
        "export_time": chrono::Utc::now(),
        "version": "2.0",
        "statistics": {
            "total": customized_count + system_default_count,
            "customized": customized_count,
            "system_default": system_default_count,
        },
    })))
}

// POST /prompt-templates/import
async fn import_templates(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let templates = body["templates"].as_array().ok_or((
        StatusCode::BAD_REQUEST,
        Json(json!({"detail": "缺少 templates 字段"})),
    ))?;

    let system_templates = PromptTemplateService::all_system_templates();
    let system_dict: HashMap<&str, &prompt_template_service::SystemTemplate> = system_templates
        .iter()
        .map(|s| (s.template_key.as_str(), s))
        .collect();

    let mut kept_system_default = 0u32;
    let mut created_or_updated = 0u32;
    let mut converted_to_custom = 0u32;
    let mut converted_list = Vec::new();

    for item in templates {
        let template_key = item["template_key"].as_str().unwrap_or("");
        let is_customized = item["is_customized"].as_bool().unwrap_or(false);
        let imported_content = item["template_content"].as_str().unwrap_or("").trim().to_string();

        let existing =
            PromptTemplateService::find_user_template(&db, &claims.sub, template_key)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"detail": format!("{}", e)})),
                    )
                })?;

        let system = system_dict.get(template_key);

        if !is_customized {
            if let Some(sys) = system {
                let system_content = sys.content.trim();
                if imported_content == system_content {
                    if existing.is_some() {
                        PromptTemplateService::delete_user_template(&db, &claims.sub, template_key)
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
                    let mut merge_data = item.clone();
                    if let Value::Object(ref mut map) = merge_data {
                        map.insert("template_key".to_string(), json!(template_key));
                    }
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
                        "template_name": item["template_name"],
                        "reason": "内容与系统默认不一致，已转为自定义",
                    }));
                }
            } else {
                // System doesn't have this template, import as custom
                let mut merge_data = item.clone();
                if let Value::Object(ref mut map) = merge_data {
                    map.insert("template_key".to_string(), json!(template_key));
                }
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
            let mut merge_data = item.clone();
            if let Value::Object(ref mut map) = merge_data {
                map.insert("template_key".to_string(), json!(template_key));
            }
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
            "total": templates.len(),
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
        Ok(rendered) => Ok(Json(json!({
            "success": true,
            "rendered_content": rendered,
            "parameters_used": params.keys().collect::<Vec<_>>(),
        }))),
        Err(e) => Ok(Json(json!({
            "success": false,
            "error": format!("渲染失败: {}", e),
            "rendered_content": null,
        }))),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/prompt-templates", get(get_all_templates))
        .route("/prompt-templates/categories", get(get_templates_by_category))
        .route("/prompt-templates/system-defaults", get(get_system_defaults))
        .route("/prompt-templates/sync-status", get(get_template_sync_status))
        .route(
            "/prompt-templates/{template_key}/sync-to-default",
            post(sync_template_to_default),
        )
        .route("/prompt-templates/{template_key}", get(get_template))
        .route("/prompt-templates", post(create_or_update_template))
        .route("/prompt-templates/{template_key}", put(update_template))
        .route("/prompt-templates/{template_key}", delete(delete_template))
        .route(
            "/prompt-templates/{template_key}/reset",
            post(reset_to_default),
        )
        .route("/prompt-templates/export", post(export_templates))
        .route("/prompt-templates/import", post(import_templates))
        .route(
            "/prompt-templates/{template_key}/preview",
            post(preview_template),
        )
}
