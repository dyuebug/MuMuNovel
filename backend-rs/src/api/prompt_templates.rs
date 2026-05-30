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
use crate::services::prompt_template_category_payload_adapter_service::build_prompt_template_categories_payload;
use crate::services::prompt_template_export_payload_adapter_service::build_prompt_template_export_payload;
use crate::services::prompt_template_list_payload_adapter_service::{
    build_prompt_template_list_payload, build_prompt_template_system_defaults_payload,
};
use crate::services::prompt_template_preview_payload_adapter_service::{
    build_prompt_template_preview_error_payload, build_prompt_template_preview_success_payload,
};
use crate::services::prompt_template_request_service::{
    build_prompt_template_import_request_from_route_payload,
    build_prompt_template_update_payload_from_route_payload,
    build_prompt_template_upsert_payload_from_route_payload, BuildPromptTemplateImportRequestError,
    PromptTemplateImportRouteRequest, PromptTemplateUpdateRouteRequest,
    PromptTemplateUpsertRouteRequest,
};
use crate::services::prompt_template_reset_payload_adapter_service::{
    build_prompt_template_delete_payload, build_prompt_template_reset_payload,
    build_prompt_template_sync_to_default_payload,
};
use crate::services::prompt_template_service::{self, PromptTemplateService};
use crate::services::prompt_template_sync_status_query_service::load_prompt_template_sync_status_payload;

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
        .route("/prompt-templates", get(get_all_templates))
        .route(
            "/prompt-templates/categories",
            get(get_templates_by_category),
        )
        .route(
            "/prompt-templates/system-defaults",
            get(get_system_defaults),
        )
        .route(
            "/prompt-templates/sync-status",
            get(get_template_sync_status),
        )
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
