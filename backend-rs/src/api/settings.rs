use std::time::Instant;

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use reqwest::Client;
use reqwest::Url;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::ai::config::AIConfig;
use crate::ai::service::AIService;
use crate::ai::types::{ToolChoice, ToolDef, ToolFunction};
use crate::models::settings;
use crate::services::auth::Claims;
use crate::services::settings_service::{
    build_create_settings_preset_request_from_route_payload,
    build_settings_update_request_from_route_body,
    build_update_settings_preset_request_from_route_payload, normalize_openai_compatible_base_url,
    CreateSettingsPresetRequest, EffectiveSettingsOverrides, SettingsService,
    UpdateSettingsPresetRequest, SETTINGS_DELETE_MISSING_DETAIL, SETTINGS_UPDATE_MISSING_DETAIL,
};

const SETTINGS_ROUTE: &str = "/settings";
const SETTINGS_API_KEY_ROUTE: &str = "/settings/api-key";
const SETTINGS_MODELS_ROUTE: &str = "/settings/models";
const SETTINGS_TEST_ROUTE: &str = "/settings/test";
const SETTINGS_FETCH_MODELS_ROUTE: &str = "/settings/fetch-models";
const SETTINGS_TEST_WEB_RESEARCH_ROUTE: &str = "/settings/test-web-research";
const SETTINGS_CHECK_FUNCTION_CALLING_ROUTE: &str = "/settings/check-function-calling";
const SETTINGS_PRESETS_ROUTE: &str = "/settings/presets";
const SETTINGS_PRESETS_FROM_CURRENT_ROUTE: &str = "/settings/presets/from-current";
const SETTINGS_PRESET_DETAIL_ROUTE: &str = "/settings/presets/{preset_id}";
const SETTINGS_PRESET_ACTIVATE_ROUTE: &str = "/settings/presets/{preset_id}/activate";
const SETTINGS_PRESET_TEST_ROUTE: &str = "/settings/presets/{preset_id}/test";

#[cfg(test)]
fn build_settings_route_owner_contract() -> Value {
    json!({
        "owner": "settings",
        "rust_owner": "backend-rs/src/api/settings.rs",
        "route_prefix": "/api",
        "routes": {
            "settings": SETTINGS_ROUTE,
            "api_key": SETTINGS_API_KEY_ROUTE,
            "models": SETTINGS_MODELS_ROUTE,
            "test": SETTINGS_TEST_ROUTE,
            "fetch_models": SETTINGS_FETCH_MODELS_ROUTE,
            "test_web_research": SETTINGS_TEST_WEB_RESEARCH_ROUTE,
            "check_function_calling": SETTINGS_CHECK_FUNCTION_CALLING_ROUTE,
            "presets": SETTINGS_PRESETS_ROUTE,
            "presets_from_current": SETTINGS_PRESETS_FROM_CURRENT_ROUTE,
            "preset_detail": SETTINGS_PRESET_DETAIL_ROUTE,
            "preset_activate": SETTINGS_PRESET_ACTIVATE_ROUTE,
            "preset_test": SETTINGS_PRESET_TEST_ROUTE
        },
        "method_contract": {
            "settings": ["GET", "POST", "PUT", "DELETE"],
            "api_key": ["GET"],
            "models": ["GET"],
            "test": ["POST"],
            "fetch_models": ["POST"],
            "test_web_research": ["POST"],
            "check_function_calling": ["POST"],
            "presets": ["GET", "POST"],
            "presets_from_current": ["POST"],
            "preset_detail": ["PUT", "DELETE"],
            "preset_activate": ["POST"],
            "preset_test": ["POST"]
        },
        "service_handoffs": {
            "settings_crud_owner": "backend-rs/src/services/settings_service.rs",
            "api_key_payload_owner": "backend-rs/src/api/settings.rs",
            "models_payload_owner": "backend-rs/src/api/settings.rs",
            "preset_query_owner": "backend-rs/src/api/settings.rs",
            "runtime_config_owner": "backend-rs/src/services/settings_service.rs"
        },
        "readiness_probes": [
            "settings-auth-guard-rust",
            "settings-api-key-auth-guard-rust",
            "settings-presets-auth-guard-rust",
            "settings-presets-create-auth-guard-rust",
            "settings-presets-from-current-auth-guard-rust",
            "settings-presets-update-auth-guard-rust",
            "settings-presets-delete-auth-guard-rust",
            "settings-presets-activate-auth-guard-rust",
            "settings-presets-test-auth-guard-rust",
            "settings-models-auth-guard-rust",
            "settings-fetch-models-auth-guard-rust",
            "settings-test-auth-guard-rust",
            "settings-check-function-calling-auth-guard-rust",
            "settings-get-business-rust",
            "settings-presets-get-business-rust",
            "settings-test-business-rust",
            "settings-check-function-calling-business-rust",
            "settings-presets-create-business-rust",
            "settings-presets-update-business-rust",
            "settings-presets-test-business-rust",
            "settings-presets-activate-business-rust",
            "settings-get-after-preset-activate-business-rust",
            "settings-deactivate-active-preset-business-rust",
            "settings-presets-list-after-deactivate-business-rust",
            "settings-presets-delete-business-rust",
            "settings-presets-from-current-business-rust",
            "settings-presets-delete-current-business-rust"
        ],
        "source_map_files": [],
        "owner_profile": {
            "name": "phase5-settings-business-owner",
            "business_probes": [
                "settings-get-business-rust",
                "settings-presets-get-business-rust",
                "settings-test-business-rust",
                "settings-check-function-calling-business-rust",
                "settings-presets-create-business-rust",
                "settings-presets-update-business-rust",
                "settings-presets-test-business-rust",
                "settings-presets-activate-business-rust",
                "settings-get-after-preset-activate-business-rust",
                "settings-deactivate-active-preset-business-rust",
                "settings-presets-list-after-deactivate-business-rust",
                "settings-presets-delete-business-rust",
                "settings-presets-from-current-business-rust",
                "settings-presets-delete-current-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "rollback_boundary": {
            "source_map_policy": "settings_route_source_map_deleted_remaining_python_closeout_is_settings_model_only",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_bootstrap_status": "settings_route_runtime_registration_deleted_no_python_route_shell_remains",
            "python_route_files_status": "settings_route_source_map_deleted_no_direct_route_group_python_source_maps_remain",
            "source_map_freeze_status": "physical_closeout_completed",
            "source_map_physical_closeout_action": "delete_completed",
            "python_fallback_removal_ready": true,
            "remaining_blockers": [
                "surviving settings.py model source-map still needs its own separate physical closeout review"
            ],
            "freeze_reason": "Rust settings route group has dedicated phase5-settings-business-owner probes for settings CRUD, presets, API test, function-calling check, test-web-research, and preset lifecycle; the Python settings route shell, bootstrap rollback registration, detached schema shell, and old runtime-store facade have been removed from the active production route boundary, and the surviving settings.py model now sits outside the direct route-group boundary.",
            "rollback_files": []
        },
        "business_smoke_status": {
            "owner_profile": "phase5-settings-business-owner",
            "business_probe_count": 14,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "settings route source-map shell deleted; remaining Python closeout work is limited to backend/migrator_app/models/settings.py",
        "migration_policy": "Settings route business smoke is covered by phase5-settings-business-owner; the Python settings route shell, its explicit bootstrap rollback registration, the detached schema shell, and the old runtime-store facade have been physically deleted, and the remaining Python closeout work is limited to backend/migrator_app/models/settings.py."
    })
}

#[derive(Deserialize)]
struct ModelsQuery {
    api_key: Option<String>,
    api_base_url: Option<String>,
    provider: Option<String>,
}

#[derive(Deserialize)]
struct TestConnectionRequest {
    api_key: Option<String>,
    api_base_url: Option<String>,
    provider: Option<String>,
    llm_model: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    api_backup_urls: Option<Vec<String>>,
    fallback_strategy: Option<String>,
}

#[derive(Deserialize)]
struct FetchModelsRequest {
    api_key: Option<String>,
    api_base_url: Option<String>,
    provider: Option<String>,
    models_url: Option<String>,
}

#[derive(Debug, PartialEq)]
struct TestPresetConnectionRequest {
    api_key: Option<String>,
    api_base_url: Option<String>,
    provider: Option<String>,
    llm_model: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    api_backup_urls: Option<Vec<String>>,
    fallback_strategy: Option<String>,
}

fn build_test_preset_connection_request(config: &Value) -> TestPresetConnectionRequest {
    TestPresetConnectionRequest {
        api_key: config
            .get("api_key")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        api_base_url: config
            .get("api_base_url")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        provider: config
            .get("api_provider")
            .or_else(|| config.get("provider"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        llm_model: config
            .get("llm_model")
            .or_else(|| config.get("model"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        temperature: config.get("temperature").and_then(Value::as_f64),
        max_tokens: config
            .get("max_tokens")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        api_backup_urls: config.get("api_backup_urls").and_then(|value| {
            value.as_array().map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .collect()
            })
        }),
        fallback_strategy: config
            .get("fallback_strategy")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
}

#[derive(Deserialize)]
struct TestWebResearchRequest {
    provider: String,
    exa_api_key: Option<String>,
    exa_base_url: Option<String>,
    grok_api_key: Option<String>,
    grok_base_url: Option<String>,
    grok_model: Option<String>,
    grok_search_enabled: Option<bool>,
    query: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
struct SettingsUpdateRouteRequest {
    #[serde(default)]
    api_provider: Option<Value>,
    #[serde(default)]
    clear_api_key: Option<Value>,
    #[serde(default)]
    api_key: Option<Value>,
    #[serde(default)]
    api_base_url: Option<Value>,
    #[serde(default)]
    api_backup_urls: Option<Value>,
    #[serde(default)]
    provider_type: Option<Value>,
    #[serde(default)]
    fallback_strategy: Option<Value>,
    #[serde(default)]
    azure_api_version: Option<Value>,
    #[serde(default)]
    llm_model: Option<Value>,
    #[serde(default)]
    temperature: Option<Value>,
    #[serde(default)]
    max_tokens: Option<Value>,
    #[serde(default)]
    system_prompt: Option<Value>,
    #[serde(default)]
    preferences: Option<Value>,
    #[serde(default)]
    web_research_enabled: Option<Value>,
    #[serde(default)]
    web_research_exa_enabled: Option<Value>,
    #[serde(default)]
    web_research_grok_enabled: Option<Value>,
    #[serde(default)]
    web_research_exa_api_key: Option<Value>,
    #[serde(default)]
    web_research_exa_base_url: Option<Value>,
    #[serde(default)]
    web_research_grok_api_key: Option<Value>,
    #[serde(default)]
    web_research_grok_base_url: Option<Value>,
    #[serde(default)]
    web_research_grok_model: Option<Value>,
    #[serde(default)]
    web_research_grok_search_enabled: Option<Value>,
}

impl SettingsUpdateRouteRequest {
    fn into_body(self) -> Value {
        let mut body = Map::new();

        insert_present_field(&mut body, "api_provider", self.api_provider);
        insert_present_field(&mut body, "clear_api_key", self.clear_api_key);
        insert_present_field(&mut body, "api_key", self.api_key);
        insert_present_field(&mut body, "api_base_url", self.api_base_url);
        insert_present_field(&mut body, "api_backup_urls", self.api_backup_urls);
        insert_present_field(&mut body, "provider_type", self.provider_type);
        insert_present_field(&mut body, "fallback_strategy", self.fallback_strategy);
        insert_present_field(&mut body, "azure_api_version", self.azure_api_version);
        insert_present_field(&mut body, "llm_model", self.llm_model);
        insert_present_field(&mut body, "temperature", self.temperature);
        insert_present_field(&mut body, "max_tokens", self.max_tokens);
        insert_present_field(&mut body, "system_prompt", self.system_prompt);
        insert_present_field(&mut body, "preferences", self.preferences);
        insert_present_field(&mut body, "web_research_enabled", self.web_research_enabled);
        insert_present_field(
            &mut body,
            "web_research_exa_enabled",
            self.web_research_exa_enabled,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_enabled",
            self.web_research_grok_enabled,
        );
        insert_present_field(
            &mut body,
            "web_research_exa_api_key",
            self.web_research_exa_api_key,
        );
        insert_present_field(
            &mut body,
            "web_research_exa_base_url",
            self.web_research_exa_base_url,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_api_key",
            self.web_research_grok_api_key,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_base_url",
            self.web_research_grok_base_url,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_model",
            self.web_research_grok_model,
        );
        insert_present_field(
            &mut body,
            "web_research_grok_search_enabled",
            self.web_research_grok_search_enabled,
        );

        Value::Object(body)
    }
}

fn insert_present_field(body: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        body.insert(key.to_string(), value);
    }
}

fn build_settings_update_request_from_typed_route_payload(
    body: SettingsUpdateRouteRequest,
) -> crate::services::settings_service::SettingsUpdateRequest {
    build_settings_update_request_from_route_body(&body.into_body())
}

#[derive(Deserialize, Default, Clone, Debug)]
struct CreatePresetFromCurrentRouteQuery {
    name: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
struct CreatePresetFromCurrentRouteBody {
    name: Option<String>,
    description: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct CreatePresetFromCurrentRequest {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
struct CreateSettingsPresetRouteRequest {
    #[serde(default)]
    name: Option<Value>,
    #[serde(default)]
    description: Option<Value>,
    #[serde(default)]
    config: Option<Value>,
}

impl CreateSettingsPresetRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "config": self.config,
        })
    }
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
struct UpdateSettingsPresetRouteRequest {
    #[serde(default)]
    name: Option<Value>,
    #[serde(default)]
    description: Option<Value>,
    #[serde(default)]
    config: Option<Value>,
}

impl UpdateSettingsPresetRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "config": self.config,
        })
    }
}

fn build_create_preset_from_current_request(
    query: CreatePresetFromCurrentRouteQuery,
    body: Option<CreatePresetFromCurrentRouteBody>,
) -> CreatePresetFromCurrentRequest {
    let body_name = body
        .as_ref()
        .and_then(|payload| payload.name.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string);
    let body_description = body.and_then(|payload| payload.description);

    let name = query
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or(body_name)
        .unwrap_or_else(|| "My Preset".to_string());
    let description = query
        .description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or(body_description);

    CreatePresetFromCurrentRequest { name, description }
}

fn build_create_settings_preset_request_from_typed_route_payload(
    body: CreateSettingsPresetRouteRequest,
) -> CreateSettingsPresetRequest {
    build_create_settings_preset_request_from_route_payload(&body.into_body())
}

fn build_update_settings_preset_request_from_typed_route_payload(
    body: UpdateSettingsPresetRouteRequest,
) -> UpdateSettingsPresetRequest {
    build_update_settings_preset_request_from_route_payload(&body.into_body())
}

fn build_stored_api_key_payload(api_key: Option<&str>) -> Value {
    let trimmed = api_key.unwrap_or_default().trim();
    json!({
        "api_key": trimmed,
        "has_api_key": !trimmed.is_empty(),
    })
}

fn build_available_models_payload(provider: &str, models: Vec<Value>) -> Value {
    let count = models.len();
    json!({
        "provider": provider,
        "models": models,
        "count": count,
    })
}

fn build_available_models_fallback_payload(
    provider: &str,
    fallback_models: Vec<Value>,
    error: &str,
) -> Value {
    let count = fallback_models.len();
    json!({
        "provider": provider,
        "models": fallback_models,
        "count": count,
        "message": format!("Model list fallback applied: {}", error),
        "fallback_applied": true,
    })
}

fn normalize_fetch_models_payload(models: Vec<Value>) -> Vec<Value> {
    models
        .into_iter()
        .filter_map(|item| {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                let trimmed = id.trim();
                if !trimmed.is_empty() {
                    return Some(json!({
                        "id": trimmed,
                        "owned_by": item
                            .get("owned_by")
                            .and_then(Value::as_str)
                            .or_else(|| item.get("description").and_then(Value::as_str))
                    }));
                }
            }

            let value = item
                .get("value")
                .and_then(Value::as_str)
                .or_else(|| item.get("name").and_then(Value::as_str))
                .or_else(|| item.get("label").and_then(Value::as_str))
                .map(str::trim)
                .filter(|text| !text.is_empty())?;

            Some(json!({
                "id": value,
                "owned_by": item
                    .get("owned_by")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("description").and_then(Value::as_str))
                    .or_else(|| item.get("label").and_then(Value::as_str))
            }))
        })
        .collect()
}

fn build_fetch_models_success_payload(models: Vec<Value>) -> Value {
    let model_count = models.len();
    json!({
        "success": true,
        "models": normalize_fetch_models_payload(models),
        "message": format!("Fetched {} models", model_count)
    })
}

fn build_fetch_models_fallback_payload(fallback_models: Vec<Value>, error: &str) -> Value {
    json!({
        "success": true,
        "models": fallback_models,
        "message": format!("Model list fallback applied: {}", error)
    })
}

fn build_fetch_models_failure_payload(error: &str, error_type: &str) -> Value {
    json!({
        "success": false,
        "models": [],
        "message": "Failed to fetch models",
        "error": error,
        "error_type": error_type
    })
}

#[derive(Debug, PartialEq, Eq)]
enum FindPresetConfigError {
    PresetNotFound,
}

fn find_preset_config(
    preferences: Option<&str>,
    preset_id: &str,
) -> Result<Value, FindPresetConfigError> {
    let (presets, _version) = crate::services::settings_service::get_api_presets(preferences);

    presets
        .into_iter()
        .find(|preset| preset.get("id").and_then(Value::as_str) == Some(preset_id))
        .map(|preset| preset.get("config").cloned().unwrap_or_else(|| json!({})))
        .ok_or(FindPresetConfigError::PresetNotFound)
}

async fn get_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::get_or_create(&db, &claims.sub).await {
        Ok(settings) => Ok(Json(settings)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn get_stored_api_key(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match load_settings_model(&db, &claims.sub).await {
        Ok(Some(model)) => Ok(Json(build_stored_api_key_payload(Some(&model.api_key)))),
        Ok(None) => Ok(Json(build_stored_api_key_payload(None))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn create_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<SettingsUpdateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_settings_update_request_from_typed_route_payload(body);

    match SettingsService::create_or_update(&db, &claims.sub, &request).await {
        Ok(settings) => Ok(Json(settings)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn update_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<SettingsUpdateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_settings_update_request_from_typed_route_payload(body);

    match SettingsService::update_existing(&db, &claims.sub, &request).await {
        Ok(settings) => Ok(Json(settings)),
        Err(e) => {
            let detail = format!("{}", e);
            if detail.contains(SETTINGS_UPDATE_MISSING_DETAIL) {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({"detail": SETTINGS_UPDATE_MISSING_DETAIL})),
                ))
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": detail})),
                ))
            }
        }
    }
}

async fn delete_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::delete_existing(&db, &claims.sub).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            let detail = format!("{}", e);
            if detail.contains(SETTINGS_DELETE_MISSING_DETAIL) {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({"detail": SETTINGS_DELETE_MISSING_DETAIL})),
                ))
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": detail})),
                ))
            }
        }
    }
}

async fn get_available_models(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Query(query): Query<ModelsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let effective = SettingsService::resolve_effective_runtime_settings(
        &db,
        &claims.sub,
        EffectiveSettingsOverrides {
            provider: query.provider.clone(),
            api_key: query.api_key.clone(),
            api_base_url: query.api_base_url.clone(),
            ..Default::default()
        },
    )
    .await?;

    let lookup_base_url = query
        .api_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(effective.base_url.as_str());

    match fetch_available_models_for_provider(
        &effective.provider,
        &effective.api_key,
        lookup_base_url,
    )
    .await
    {
        Ok(models) => {
            let message = available_models_success_message(&effective.provider, &models);
            let mut payload = build_available_models_payload(&effective.provider, models);
            if let Some(message) = message {
                if let Some(object) = payload.as_object_mut() {
                    object.insert("message".to_string(), Value::String(message.to_string()));
                }
            }
            Ok(Json(payload))
        }
        Err(error) => {
            let fallback = curated_model_options(&effective.provider);
            if !fallback.is_empty() {
                Ok(Json(build_available_models_fallback_payload(
                    &effective.provider,
                    fallback,
                    &error,
                )))
            } else {
                let openai_fallback = curated_model_options("openai");
                Ok(Json(build_available_models_fallback_payload(
                    &effective.provider,
                    openai_fallback,
                    &error,
                )))
            }
        }
    }
}

async fn test_api_connection(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<TestConnectionRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let temperature = body.temperature.unwrap_or(0.7);
    let max_tokens = body.max_tokens.unwrap_or(2000);
    let probe_max_tokens = max_tokens.clamp(1, 64);
    let api_backup_urls = normalize_probe_backup_urls(body.api_backup_urls.as_deref());
    let fallback_strategy = normalize_probe_fallback_strategy(body.fallback_strategy.as_deref());
    let effective = SettingsService::resolve_effective_runtime_settings(
        &db,
        &claims.sub,
        EffectiveSettingsOverrides {
            provider: body.provider.clone(),
            api_key: body.api_key.clone(),
            api_base_url: body.api_base_url.clone(),
            model: body.llm_model.clone(),
            temperature: None,
            max_tokens: None,
        },
    )
    .await?;

    let started = Instant::now();
    let (prefer_normalized_v1_candidate, read_timeout_secs, transport_max_retries) =
        build_probe_transport_config(&effective.provider, &effective.base_url);
    let effective_backup_urls = if fallback_strategy == "auto" {
        api_backup_urls.clone()
    } else {
        Vec::new()
    };
    let service = AIService::new(AIConfig {
        provider: effective.provider.clone(),
        api_key: effective.api_key.clone(),
        base_url: effective.base_url.clone(),
        backup_urls: effective_backup_urls,
        model: effective.model.clone(),
        temperature,
        max_tokens: probe_max_tokens,
        prefer_normalized_v1_candidate,
        read_timeout_secs,
        transport_max_retries: Some(transport_max_retries),
        ..Default::default()
    });

    match service
        .generate_text_detailed(
            "Please reply with the single word OK.",
            Some("You are an API connectivity probe."),
            None,
        )
        .await
    {
        Ok(response) => {
            let transport_diagnostics = response.transport_diagnostics.clone();
            let mut details = Map::new();
            details.insert("api_available".to_string(), json!(true));
            details.insert("model_accessible".to_string(), json!(true));
            details.insert(
                "response_valid".to_string(),
                json!(!response.content.trim().is_empty()),
            );
            details.insert("temperature".to_string(), json!(temperature));
            details.insert("max_tokens".to_string(), json!(max_tokens));
            details.insert("probe_max_tokens".to_string(), json!(probe_max_tokens));

            Ok(Json(json!({
                "success": true,
                "message": "API 连接测试成功",
                "response_time_ms": started.elapsed().as_millis(),
                "provider": effective.provider,
                "model": effective.model,
                "response_preview": response.content.chars().take(100).collect::<String>(),
                "details": build_probe_details(
                    &effective.base_url,
                    Some(api_backup_urls.as_slice()),
                    Some(fallback_strategy.as_str()),
                    transport_diagnostics,
                    Some(details),
                ),
            })))
        }
        Err(error) => {
            let error_message = error.message;
            let transport_diagnostics = error.transport_diagnostics;
            let error_type = classify_error_type(&error_message);
            let http_status_code = error
                .status_code
                .or_else(|| extract_http_status_code(&error_message));
            Ok(Json(json!({
                "success": false,
                "message": if error_type == "TimeoutError" { "API 请求超时" } else { "API 测试失败" },
                "response_time_ms": started.elapsed().as_millis(),
                "provider": effective.provider,
                "model": effective.model,
                "error": error_message,
                "error_type": error_type,
                "suggestions": build_api_probe_failure_suggestions(
                    &error_message,
                    &effective.base_url,
                    Some(api_backup_urls.as_slice()),
                    Some(fallback_strategy.as_str()),
                    http_status_code,
                ),
                "details": build_probe_details(
                    &effective.base_url,
                    Some(api_backup_urls.as_slice()),
                    Some(fallback_strategy.as_str()),
                    transport_diagnostics,
                    http_status_code.map(|status_code| {
                        let mut details = Map::new();
                        details.insert("http_status_code".to_string(), json!(status_code));
                        details
                    }),
                ),
            })))
        }
    }
}

async fn fetch_models_endpoint(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<FetchModelsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let effective = SettingsService::resolve_effective_runtime_settings(
        &db,
        &claims.sub,
        EffectiveSettingsOverrides {
            provider: body.provider.clone(),
            api_key: body.api_key.clone(),
            api_base_url: body.api_base_url.clone(),
            ..Default::default()
        },
    )
    .await?;

    match fetch_provider_models(
        &effective.provider,
        &effective.api_key,
        &effective.base_url,
        body.models_url.as_deref(),
    )
    .await
    {
        Ok(models) => Ok(Json(build_fetch_models_success_payload(models))),
        Err(error) => {
            let fallback = curated_fetch_models(&effective.provider);
            if !fallback.is_empty() {
                Ok(Json(build_fetch_models_fallback_payload(fallback, &error)))
            } else {
                Ok(Json(build_fetch_models_failure_payload(
                    &error,
                    classify_error_type(&error),
                )))
            }
        }
    }
}

async fn test_web_research_connection(
    Json(body): Json<TestWebResearchRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let provider = body.provider.trim().to_lowercase();
    let query = body.query.unwrap_or_else(|| "hello".to_string());
    let client = Client::new();
    let started = Instant::now();

    match provider.as_str() {
        "exa" => {
            let api_key = body.exa_api_key.unwrap_or_default();
            let base_url = normalize_exa_base_url(
                body.exa_base_url.as_deref().unwrap_or("https://api.exa.ai"),
            );
            let response = client
                .post(format!("{}/search", base_url))
                .header("x-api-key", api_key)
                .json(&json!({ "query": query, "numResults": 1 }))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    Ok(Json(json!({
                        "success": status.is_success(),
                        "provider": provider,
                        "message": if status.is_success() { "Exa connection succeeded" } else { "Exa connection failed" },
                        "response_preview": text.chars().take(200).collect::<String>(),
                        "result_count": if status.is_success() { 1 } else { 0 },
                        "search_status": if status.is_success() { "success_with_sources" } else { "failed" },
                        "error": if status.is_success() { Value::Null } else { Value::String(text.chars().take(300).collect()) },
                        "error_type": if status.is_success() { Value::Null } else { Value::String(format!("HTTP {}", status.as_u16())) }
                    })))
                }
                Err(error) => Ok(Json(json!({
                    "success": false,
                    "provider": provider,
                    "message": "Exa connection failed",
                    "response_preview": "",
                    "result_count": 0,
                    "search_status": "failed",
                    "error": error.to_string(),
                    "error_type": classify_error_type(&error.to_string()),
                    "suggestions": generic_suggestions("web_research")
                }))),
            }
        }
        "grok" => {
            let grok_key = body.grok_api_key.unwrap_or_default();
            let grok_base_url = normalize_openai_compatible_base_url(
                body.grok_base_url
                    .as_deref()
                    .unwrap_or("https://api.x.ai/v1"),
            );
            let grok_model = body
                .grok_model
                .unwrap_or_else(|| "grok-4.1-fast".to_string());
            let service = AIService::new(AIConfig {
                provider: "openai".to_string(),
                api_key: grok_key,
                base_url: grok_base_url,
                model: grok_model.clone(),
                temperature: 0.1,
                max_tokens: 256,
                ..Default::default()
            });
            match service
                .generate_text(
                    &format!("Answer briefly: {}", query),
                    Some("You are a web research connectivity probe."),
                    None,
                )
                .await
            {
                Ok(response) => Ok(Json(json!({
                    "success": true,
                    "provider": provider,
                    "message": "Grok connection succeeded",
                    "response_preview": response.content.chars().take(200).collect::<String>(),
                    "result_count": 1,
                    "source_count": 0,
                    "search_status": "success_without_sources",
                    "status_note": if body.grok_search_enabled.unwrap_or(false) { "search_enabled_requested" } else { "chat_probe_only" },
                    "response_time_ms": started.elapsed().as_millis()
                }))),
                Err(error) => Ok(Json(json!({
                    "success": false,
                    "provider": provider,
                    "message": "Grok connection failed",
                    "response_preview": "",
                    "result_count": 0,
                    "search_status": "failed",
                    "error": error,
                    "error_type": classify_error_type(&error),
                    "suggestions": generic_suggestions("web_research")
                }))),
            }
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "Unsupported web research provider"})),
        )),
    }
}

async fn check_function_calling(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<TestConnectionRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let api_backup_urls = normalize_probe_backup_urls(body.api_backup_urls.as_deref());
    let fallback_strategy = normalize_probe_fallback_strategy(body.fallback_strategy.as_deref());
    let effective = SettingsService::resolve_effective_runtime_settings(
        &db,
        &claims.sub,
        EffectiveSettingsOverrides {
            provider: body.provider.clone(),
            api_key: body.api_key.clone(),
            api_base_url: body.api_base_url.clone(),
            model: body.llm_model.clone(),
            temperature: Some(0.1),
            max_tokens: Some(512),
        },
    )
    .await?;

    let probe_max_tokens = effective.max_tokens.clamp(1, 64);
    let started = Instant::now();
    let (prefer_normalized_v1_candidate, read_timeout_secs, transport_max_retries) =
        build_probe_transport_config(&effective.provider, &effective.base_url);
    let effective_backup_urls = if fallback_strategy == "auto" {
        api_backup_urls.clone()
    } else {
        Vec::new()
    };
    let tools = vec![ToolDef {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "get_weather".to_string(),
            description: "获取指定城市的当前天气信息".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "city": {
                        "type": "string",
                        "description": "城市名称，例如：北京、上海、深圳"
                    },
                    "unit": {
                        "type": "string",
                        "enum": ["celsius", "fahrenheit"],
                        "description": "温度单位"
                    }
                },
                "required": ["city"]
            }),
        },
    }];

    let service = AIService::new(AIConfig {
        provider: effective.provider.clone(),
        api_key: effective.api_key.clone(),
        base_url: effective.base_url.clone(),
        backup_urls: effective_backup_urls,
        model: effective.model.clone(),
        temperature: 0.3,
        max_tokens: probe_max_tokens,
        prefer_normalized_v1_candidate,
        read_timeout_secs,
        transport_max_retries: Some(transport_max_retries),
        ..Default::default()
    });

    match service
        .generate_text_with_tool_choice_detailed(
            "Do not explain or answer directly. Call the get_weather tool immediately for city=Beijing and unit=celsius.",
            None,
            Some(&tools),
            Some(&ToolChoice::Required),
        )
        .await
    {
        Ok(response) => {
            let transport_diagnostics = response.transport_diagnostics.clone();
            let finish_reason = response.finish_reason.clone();
            let response_preview = response.content.chars().take(200).collect::<String>();
            let tool_calls = response.tool_calls;
            let supported = tool_calls
                .as_ref()
                .map(|calls| !calls.is_empty())
                .unwrap_or(false);
            let message = if supported {
                "✅ 支持 Function Calling"
            } else {
                "❌ 不支持 Function Calling"
            };
            let response_type = if supported { "tool_calls" } else { "text" };
            let mut details = Map::new();
            details.insert("finish_reason".to_string(), json!(finish_reason));
            details.insert("has_tool_calls".to_string(), json!(supported));
            details.insert(
                "tool_call_count".to_string(),
                json!(tool_calls.as_ref().map(|calls| calls.len()).unwrap_or(0)),
            );
            details.insert("test_tool".to_string(), json!("get_weather"));
            details.insert("response_type".to_string(), json!(response_type));
            let mut payload = json!({
                "success": true,
                "supported": supported,
                "message": message,
                "response_time_ms": started.elapsed().as_millis(),
                "provider": effective.provider,
                "model": effective.model,
                "details": build_probe_details(
                    &effective.base_url,
                    Some(api_backup_urls.as_slice()),
                    Some(fallback_strategy.as_str()),
                    transport_diagnostics,
                    Some(details),
                )
            });

            if let Some(tool_calls) = tool_calls {
                if let Some(object) = payload.as_object_mut() {
                    object.insert("tool_calls".to_string(), Value::Array(tool_calls.into_iter().map(|call| serde_json::to_value(call).unwrap_or(Value::Null)).collect()));
                    object.insert(
                        "suggestions".to_string(),
                        json!([
                            "✅ 该模型支持 Function Calling，可以正常使用 MCP 插件",
                            "建议：启用需要的 MCP 插件以扩展 AI 能力",
                            "提示：测试成功检测到工具调用，模型能够正确解析和使用外部工具"
                        ]),
                    );
                }
            } else if let Some(object) = payload.as_object_mut() {
                object.insert(
                    "response_preview".to_string(),
                    Value::String(response_preview),
                );
                object.insert(
                    "suggestions".to_string(),
                    json!([
                        "❌ 该模型不支持 Function Calling，无法使用 MCP 插件功能",
                        "建议：更换支持工具调用的模型",
                        "推荐模型：GPT-4 系列、GPT-4-turbo、Claude 3 Opus/Sonnet、Gemini 1.5 Pro 等",
                        "说明：模型返回了文本回复而非工具调用，表明不支持该功能"
                    ]),
                );
            }

            Ok(Json(payload))
        }
        Err(error) => {
            let error_message = error.message;
            let transport_diagnostics = error.transport_diagnostics;
            let error_type = classify_error_type(&error_message);
            let http_status_code = error
                .status_code
                .or_else(|| extract_http_status_code(&error_message));
            let failure_message = if error_type == "TimeoutError" {
                "检测超时".to_string()
            } else if let Some(status_code) = http_status_code {
                match status_code {
                    500..=599 => format!("上游服务暂时不可用（HTTP {status_code}）"),
                    429 => "请求过于频繁，暂时无法确认模型能力".to_string(),
                    401 => "认证失败，暂时无法确认模型能力".to_string(),
                    404 => "接口地址或模型不可用，暂时无法确认模型能力".to_string(),
                    _ => "检测失败，暂时无法确认模型能力".to_string(),
                }
            } else {
                "Function Calling 检测失败，暂时无法确认模型能力".to_string()
            };
            Ok(Json(json!({
            "success": false,
            "supported": Value::Null,
            "message": failure_message,
            "response_time_ms": started.elapsed().as_millis(),
            "provider": effective.provider,
            "model": effective.model,
            "error": error_message,
            "error_type": error_type,
            "suggestions": build_api_probe_failure_suggestions(
                &error_message,
                &effective.base_url,
                Some(api_backup_urls.as_slice()),
                Some(fallback_strategy.as_str()),
                http_status_code,
            ),
            "details": build_probe_details(
                &effective.base_url,
                Some(api_backup_urls.as_slice()),
                Some(fallback_strategy.as_str()),
                transport_diagnostics,
                http_status_code.map(|status_code| {
                    let mut details = Map::new();
                    details.insert("http_status_code".to_string(), json!(status_code));
                    details
                }),
            )
        })))
        }
    }
}

async fn get_presets(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::list_presets(&db, &claims.sub).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn create_preset(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<CreateSettingsPresetRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_create_settings_preset_request_from_typed_route_payload(body);
    match SettingsService::create_preset(&db, &claims.sub, &request).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn update_preset(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(preset_id): Path<String>,
    Json(body): Json<UpdateSettingsPresetRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_update_settings_preset_request_from_typed_route_payload(body);
    match SettingsService::update_preset(&db, &claims.sub, &preset_id, &request).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            let detail = format!("{}", e);
            if detail.contains("not found") {
                Err((StatusCode::NOT_FOUND, Json(json!({"detail": "预设不存在"}))))
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": detail})),
                ))
            }
        }
    }
}

async fn delete_preset(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(preset_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::delete_preset(&db, &claims.sub, &preset_id).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            let detail = format!("{}", e);
            if detail.contains("无法删除激活中的预设") {
                Err((StatusCode::BAD_REQUEST, Json(json!({"detail": detail}))))
            } else if detail.contains("not found") {
                Err((StatusCode::NOT_FOUND, Json(json!({"detail": "预设不存在"}))))
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": detail})),
                ))
            }
        }
    }
}

async fn activate_preset(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(preset_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::activate_preset(&db, &claims.sub, &preset_id).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            let detail = format!("{}", e);
            if detail.contains("not found") {
                Err((StatusCode::NOT_FOUND, Json(json!({"detail": "预设不存在"}))))
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": detail})),
                ))
            }
        }
    }
}

async fn test_preset(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(preset_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let settings_payload = SettingsService::get_or_create(&db, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", error)})),
            )
        })?;

    let settings_preferences = settings_payload
        .get("preferences")
        .and_then(|value| value.as_str());

    let config =
        find_preset_config(settings_preferences, &preset_id).map_err(|error| match error {
            FindPresetConfigError::PresetNotFound => {
                (StatusCode::NOT_FOUND, Json(json!({"detail": "预设不存在"})))
            }
        })?;
    let request = build_test_preset_connection_request(&config);

    test_api_connection(
        Extension(claims),
        Extension(db),
        Json(TestConnectionRequest {
            api_key: request.api_key,
            api_base_url: request.api_base_url,
            provider: request.provider,
            llm_model: request.llm_model,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            api_backup_urls: request.api_backup_urls,
            fallback_strategy: request.fallback_strategy,
        }),
    )
    .await
}

async fn create_preset_from_current(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Query(query): Query<CreatePresetFromCurrentRouteQuery>,
    body: Option<Json<CreatePresetFromCurrentRouteBody>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_create_preset_from_current_request(query, body.map(|Json(value)| value));
    match SettingsService::create_preset_from_current(
        &db,
        &claims.sub,
        &request.name,
        request.description.as_deref(),
    )
    .await
    {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn load_settings_model(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<Option<settings::Model>, String> {
    settings::Entity::find()
        .filter(settings::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(|e| format!("{}", e))
}

fn normalize_exa_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://api.exa.ai".to_string();
    }
    trimmed.to_string()
}

const AZURE_MODELS_EMPTY_MESSAGE: &str =
    "Azure OpenAI 无法自动获取模型列表，请手动填写部署名称到模型字段";
const DEFAULT_PROBE_READ_TIMEOUT_SECONDS: f64 = 10.0;

fn is_openai_compatible_provider(provider: &str) -> bool {
    matches!(
        provider,
        "openai" | "openai_responses" | "azure" | "newapi" | "custom" | "sub2api"
    )
}

fn build_openai_compatible_model_candidate_urls(base_url: &str) -> Vec<String> {
    let normalized = base_url.trim().trim_end_matches('/');
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();

    if normalized.ends_with("/v1") {
        candidates.push(format!("{normalized}/models"));
        let root_base = normalized.trim_end_matches("/v1").trim_end_matches('/');
        if !root_base.is_empty() {
            candidates.push(format!("{root_base}/models"));
        }
    } else {
        candidates.push(format!("{normalized}/models"));
        candidates.push(format!("{normalized}/v1/models"));
    }

    let mut unique_candidates = Vec::new();
    for candidate in candidates {
        if !unique_candidates.contains(&candidate) {
            unique_candidates.push(candidate);
        }
    }

    unique_candidates
}

fn build_provider_model_header_pairs(provider: &str, api_key: &str) -> Vec<(&'static str, String)> {
    match provider {
        "azure" => vec![
            ("api-key", api_key.to_string()),
            ("Content-Type", "application/json".to_string()),
        ],
        "anthropic" => vec![
            ("x-api-key", api_key.to_string()),
            ("anthropic-version", "2023-06-01".to_string()),
        ],
        _ => vec![
            ("Authorization", format!("Bearer {api_key}")),
            ("Content-Type", "application/json".to_string()),
        ],
    }
}

fn parse_openai_compatible_available_models(payload: &Value) -> Vec<Value> {
    let raw_models = payload
        .get("data")
        .or_else(|| payload.get("models"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    raw_models
        .into_iter()
        .filter_map(|model| match model {
            Value::String(model_id) => {
                let trimmed = model_id.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(json!({
                        "value": trimmed,
                        "label": trimmed,
                        "description": "",
                    }))
                }
            }
            Value::Object(object) => {
                let model_id = object
                    .get("id")
                    .and_then(Value::as_str)
                    .or_else(|| object.get("name").and_then(Value::as_str))
                    .map(|value| value.trim().trim_start_matches("models/").to_string())
                    .filter(|value| !value.is_empty())?;

                let description = object
                    .get("description")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| {
                        object
                            .get("display_name")
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                    })
                    .map(|value| value.to_string())
                    .or_else(|| {
                        object.get("created").and_then(|created| {
                            if let Some(text) = created.as_str() {
                                Some(format!("Created: {text}"))
                            } else if let Some(value) = created.as_i64() {
                                Some(format!("Created: {value}"))
                            } else if let Some(value) = created.as_u64() {
                                Some(format!("Created: {value}"))
                            } else {
                                created.as_f64().map(|value| format!("Created: {value}"))
                            }
                        })
                    })
                    .unwrap_or_default();

                Some(json!({
                    "value": model_id,
                    "label": model_id,
                    "description": description,
                }))
            }
            _ => None,
        })
        .collect()
}

fn parse_anthropic_available_models(payload: &Value) -> Vec<Value> {
    payload
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|model| {
            let model_id = model.get("id").and_then(Value::as_str)?.trim();
            if model_id.is_empty() {
                return None;
            }

            Some(json!({
                "value": model_id,
                "label": model_id,
                "description": model
                    .get("display_name")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            }))
        })
        .collect()
}

fn parse_gemini_available_models(payload: &Value) -> Vec<Value> {
    payload
        .get("models")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|model| {
            let supported_methods = model
                .get("supportedGenerationMethods")
                .and_then(Value::as_array)?;
            if !supported_methods
                .iter()
                .any(|method| method.as_str() == Some("generateContent"))
            {
                return None;
            }

            let raw_name = model.get("name").and_then(Value::as_str)?.trim();
            let model_id = raw_name.trim_start_matches("models/").trim();
            if model_id.is_empty() {
                return None;
            }

            Some(json!({
                "value": model_id,
                "label": model
                    .get("displayName")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(model_id),
                "description": "",
            }))
        })
        .collect()
}

fn available_models_success_message(provider: &str, models: &[Value]) -> Option<&'static str> {
    if provider == "azure" && models.is_empty() {
        Some(AZURE_MODELS_EMPTY_MESSAGE)
    } else {
        None
    }
}

async fn send_get_request_with_headers(
    client: &Client,
    url: &str,
    header_pairs: &[(&'static str, String)],
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = client.get(url);
    for (name, value) in header_pairs {
        request = request.header(*name, value);
    }
    request.send().await
}

async fn fetch_available_models_for_provider(
    provider: &str,
    api_key: &str,
    base_url: &str,
) -> Result<Vec<Value>, String> {
    let client = Client::new();

    match provider {
        "anthropic" => {
            let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
            let response = send_get_request_with_headers(
                &client,
                &url,
                &build_provider_model_header_pairs(provider, api_key),
            )
            .await
            .map_err(|error| error.to_string())?;
            let status = response.status();
            let payload: Value = response.json().await.map_err(|error| error.to_string())?;
            if !status.is_success() {
                return Err(format!("HTTP {}: {}", status.as_u16(), payload));
            }
            Ok(parse_anthropic_available_models(&payload))
        }
        "gemini" => {
            let url = format!("{}/models", base_url.trim_end_matches('/'));
            let response = client
                .get(url)
                .query(&[("key", api_key)])
                .send()
                .await
                .map_err(|error| error.to_string())?;
            let status = response.status();
            let payload: Value = response.json().await.map_err(|error| error.to_string())?;
            if !status.is_success() {
                return Err(format!("HTTP {}: {}", status.as_u16(), payload));
            }
            Ok(parse_gemini_available_models(&payload))
        }
        _ if is_openai_compatible_provider(provider) => {
            let candidate_urls = build_openai_compatible_model_candidate_urls(base_url);
            let header_pairs = build_provider_model_header_pairs(provider, api_key);
            let mut last_http_error: Option<String> = None;
            let mut last_network_error: Option<String> = None;

            for (index, url) in candidate_urls.iter().enumerate() {
                let response = send_get_request_with_headers(&client, url, &header_pairs).await;

                match response {
                    Ok(response) => {
                        let status = response.status();
                        let payload: Value =
                            response.json().await.map_err(|error| error.to_string())?;

                        if !status.is_success() {
                            let error = format!("HTTP {}: {}", status.as_u16(), payload);
                            last_http_error = Some(error.clone());

                            if provider == "azure" && matches!(status.as_u16(), 403 | 404) {
                                return Ok(Vec::new());
                            }

                            if status.as_u16() == 404 && index + 1 < candidate_urls.len() {
                                continue;
                            }

                            return Err(error);
                        }

                        let models = parse_openai_compatible_available_models(&payload);
                        if !models.is_empty() {
                            return Ok(models);
                        }

                        if index + 1 < candidate_urls.len() {
                            continue;
                        }

                        if provider == "azure" {
                            return Ok(Vec::new());
                        }
                    }
                    Err(error) => {
                        last_network_error = Some(error.to_string());
                        if (error.is_connect() || error.is_timeout())
                            && index + 1 < candidate_urls.len()
                        {
                            continue;
                        }
                        return Err(error.to_string());
                    }
                }
            }

            if provider == "azure" {
                return Ok(Vec::new());
            }

            if let Some(error) = last_http_error {
                return Err(error);
            }

            if let Some(error) = last_network_error {
                return Err(error);
            }

            Err("未能从 API 获取到可用的模型列表".to_string())
        }
        _ => Err(format!("不支持的提供商: {provider}")),
    }
}

async fn fetch_provider_models(
    provider: &str,
    api_key: &str,
    base_url: &str,
    models_url: Option<&str>,
) -> Result<Vec<Value>, String> {
    let client = Client::new();
    match provider {
        "gemini" => {
            let url = models_url
                .map(|value| value.to_string())
                .unwrap_or_else(|| {
                    format!("{}/models?key={}", base_url.trim_end_matches('/'), api_key)
                });
            let response = client.get(url).send().await.map_err(|e| format!("{}", e))?;
            let status = response.status();
            let value: Value = response.json().await.map_err(|e| format!("{}", e))?;
            if !status.is_success() {
                return Err(format!("HTTP {}: {}", status.as_u16(), value));
            }
            let models = value
                .get("models")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|item| {
                    let raw_name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let id = raw_name.rsplit('/').next().unwrap_or(raw_name).to_string();
                    json!({
                        "value": id,
                        "label": item.get("displayName").and_then(|v| v.as_str()).unwrap_or(raw_name),
                        "description": item.get("description").and_then(|v| v.as_str()).unwrap_or("")
                    })
                })
                .collect::<Vec<_>>();
            Ok(models)
        }
        "anthropic" => Ok(curated_model_options(provider)),
        _ => {
            let url = models_url
                .map(|value| value.to_string())
                .unwrap_or_else(|| format!("{}/models", base_url.trim_end_matches('/')));
            let response = client
                .get(url)
                .bearer_auth(api_key)
                .send()
                .await
                .map_err(|e| format!("{}", e))?;
            let status = response.status();
            let value: Value = response.json().await.map_err(|e| format!("{}", e))?;
            if !status.is_success() {
                return Err(format!("HTTP {}: {}", status.as_u16(), value));
            }
            let models = value
                .get("data")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|item| {
                    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    json!({
                        "value": id,
                        "label": id,
                        "description": item.get("owned_by").and_then(|v| v.as_str()).unwrap_or("")
                    })
                })
                .collect::<Vec<_>>();
            Ok(models)
        }
    }
}

fn curated_model_options(provider: &str) -> Vec<Value> {
    match provider {
        "anthropic" => vec![
            json!({"value":"claude-3-5-sonnet-latest","label":"Claude 3.5 Sonnet","description":"Anthropic"}),
            json!({"value":"claude-3-7-sonnet-latest","label":"Claude 3.7 Sonnet","description":"Anthropic"}),
            json!({"value":"claude-3-5-haiku-latest","label":"Claude 3.5 Haiku","description":"Anthropic"}),
            json!({"value":"claude-3-opus-latest","label":"Claude 3 Opus","description":"Anthropic"}),
        ],
        "gemini" => vec![
            json!({"value":"gemini-2.5-pro","label":"Gemini 2.5 Pro","description":"Google Gemini"}),
            json!({"value":"gemini-2.5-flash","label":"Gemini 2.5 Flash","description":"Google Gemini"}),
            json!({"value":"gemini-2.0-flash","label":"Gemini 2.0 Flash","description":"Google Gemini"}),
            json!({"value":"gemini-1.5-pro","label":"Gemini 1.5 Pro","description":"Google Gemini"}),
        ],
        _ => vec![
            json!({"value":"gpt-4o","label":"gpt-4o","description":"OpenAI-compatible"}),
            json!({"value":"gpt-4o-mini","label":"gpt-4o-mini","description":"OpenAI-compatible"}),
            json!({"value":"gpt-4.1","label":"gpt-4.1","description":"OpenAI-compatible"}),
            json!({"value":"gpt-4.1-mini","label":"gpt-4.1-mini","description":"OpenAI-compatible"}),
            json!({"value":"deepseek-chat","label":"deepseek-chat","description":"OpenAI-compatible"}),
            json!({"value":"deepseek-reasoner","label":"deepseek-reasoner","description":"OpenAI-compatible"}),
        ],
    }
}

fn curated_fetch_models(provider: &str) -> Vec<Value> {
    curated_model_options(provider)
        .into_iter()
        .map(|item| {
            json!({
                "id": item.get("value").and_then(|v| v.as_str()).unwrap_or_default(),
                "owned_by": item.get("description").and_then(|v| v.as_str())
            })
        })
        .collect()
}

fn normalize_probe_backup_urls(backup_urls: Option<&[String]>) -> Vec<String> {
    let mut normalized = Vec::new();

    for item in backup_urls.unwrap_or(&[]) {
        let candidate = item.trim().trim_end_matches('/');
        if candidate.is_empty() || normalized.iter().any(|existing| existing == candidate) {
            continue;
        }
        normalized.push(candidate.to_string());
    }

    normalized
}

fn normalize_probe_fallback_strategy(fallback_strategy: Option<&str>) -> String {
    let normalized = fallback_strategy
        .unwrap_or("auto")
        .trim()
        .to_ascii_lowercase();
    if normalized.is_empty() {
        "auto".to_string()
    } else {
        normalized
    }
}

fn build_probe_endpoint_diagnostics(
    api_base_url: &str,
    backup_urls: Option<&[String]>,
    fallback_strategy: Option<&str>,
) -> Value {
    let normalized_primary = api_base_url.trim().trim_end_matches('/').to_string();
    let normalized_backups = normalize_probe_backup_urls(backup_urls);
    let normalized_strategy = normalize_probe_fallback_strategy(fallback_strategy);
    json!({
        "primary_endpoint": normalized_primary,
        "backup_endpoints": normalized_backups,
        "configured_endpoint_count": (if normalized_primary.is_empty() { 0 } else { 1 }) + normalized_backups.len(),
        "fallback_strategy": normalized_strategy,
        "auto_failover_enabled": normalized_strategy == "auto" && !normalized_backups.is_empty(),
    })
}

fn build_probe_transport_config(provider: &str, api_base_url: &str) -> (bool, Option<f64>, u32) {
    let normalized_provider = provider.trim().to_ascii_lowercase();
    let normalized_base_url = api_base_url.trim().trim_end_matches('/');
    let prefer_normalized_v1_candidate = !normalized_base_url.is_empty()
        && is_openai_compatible_provider(&normalized_provider)
        && !normalized_base_url.ends_with("/v1");
    (
        prefer_normalized_v1_candidate,
        Some(DEFAULT_PROBE_READ_TIMEOUT_SECONDS),
        1,
    )
}

fn build_probe_details(
    api_base_url: &str,
    backup_urls: Option<&[String]>,
    fallback_strategy: Option<&str>,
    transport_diagnostics: Option<Value>,
    extra: Option<Map<String, Value>>,
) -> Value {
    let mut details = extra.unwrap_or_default();
    details.insert(
        "endpoint_diagnostics".to_string(),
        build_probe_endpoint_diagnostics(api_base_url, backup_urls, fallback_strategy),
    );
    if let Some(transport_diagnostics) = transport_diagnostics {
        details.insert("transport_diagnostics".to_string(), transport_diagnostics);
    }
    Value::Object(details)
}

fn is_running_in_docker_environment() -> bool {
    std::path::Path::new("/.dockerenv").exists()
}

fn is_local_gateway_host(hostname: Option<&str>) -> bool {
    matches!(
        hostname,
        Some("127.0.0.1" | "localhost" | "host.docker.internal")
    )
}

fn extract_http_status_code(error: &str) -> Option<u16> {
    error
        .split(|ch: char| !ch.is_ascii_digit())
        .find_map(|part| match part.len() {
            3 => part
                .parse::<u16>()
                .ok()
                .filter(|status| (100..=599).contains(status)),
            _ => None,
        })
}

fn build_api_probe_failure_suggestions(
    error: &str,
    api_base_url: &str,
    backup_urls: Option<&[String]>,
    fallback_strategy: Option<&str>,
    status_code: Option<u16>,
) -> Vec<String> {
    let lowered = error.to_ascii_lowercase();
    let normalized_base_url = api_base_url.trim().trim_end_matches('/');
    let normalized_backups = normalize_probe_backup_urls(backup_urls);
    let normalized_strategy = normalize_probe_fallback_strategy(fallback_strategy);
    let auto_failover_enabled = normalized_strategy == "auto" && !normalized_backups.is_empty();
    let parsed_base_url = Url::parse(normalized_base_url).ok();
    let base_url_hostname = parsed_base_url.as_ref().and_then(|url| url.host_str());
    let is_local_gateway = normalized_base_url.starts_with("http://127.0.0.1")
        || normalized_base_url.starts_with("http://localhost")
        || normalized_base_url.starts_with("https://127.0.0.1")
        || normalized_base_url.starts_with("https://localhost");
    let status_code = status_code.or_else(|| extract_http_status_code(error));

    if lowered.contains("blocked") {
        return vec![
            "The upstream API request was blocked or rejected".to_string(),
            "Check whether the API key has permission for the target model".to_string(),
            "Confirm the API key is bound to the expected proxy or gateway".to_string(),
            "Verify the API base URL and gateway policy are consistent".to_string(),
        ];
    }

    if lowered.contains("unauthorized") || lowered.contains("401") {
        return vec![
            "API key authentication failed".to_string(),
            "Check whether the API key is correct and active".to_string(),
            "Confirm the API key has sufficient permission".to_string(),
        ];
    }

    if lowered.contains("not found") || lowered.contains("404") {
        return vec![
            "The API endpoint or model could not be found".to_string(),
            "Confirm the API base URL is correct".to_string(),
            "Verify the target model exists on the current service".to_string(),
        ];
    }

    if lowered.contains("rate limit") || lowered.contains("429") {
        return vec![
            "The API request hit a rate limit".to_string(),
            "Retry later after the rate limit window resets".to_string(),
            "Consider reducing concurrency or switching to a backup endpoint".to_string(),
        ];
    }

    if lowered.contains("insufficient") || lowered.contains("quota") {
        return vec![
            "The API quota appears to be exhausted".to_string(),
            "Check the account balance or quota usage".to_string(),
            "Confirm the current key is allowed to use this model".to_string(),
        ];
    }

    if matches!(status_code, Some(502 | 503 | 504)) {
        let mut suggestions = if is_local_gateway {
            vec![
                "The local gateway or proxy is reachable, but it failed to forward the model request upstream".to_string(),
                "Check the local gateway logs and verify its upstream provider configuration for /chat/completions or /responses".to_string(),
            ]
        } else {
            vec![
                "The upstream gateway or proxy returned a server error while processing the request".to_string(),
                "Check whether the current API gateway can reach its model provider and whether the target model is healthy".to_string(),
            ]
        };

        if auto_failover_enabled {
            suggestions.push(
                "Retry the request and inspect transport diagnostics to confirm whether failover was attempted"
                    .to_string(),
            );
        } else {
            suggestions.push(
                "Configure at least one backup endpoint and keep fallback strategy as auto if you want automatic failover"
                    .to_string(),
            );
        }
        return suggestions;
    }

    if lowered.contains("non-json")
        || lowered.contains("non json")
        || lowered.contains("doctype html")
    {
        let mut suggestions = vec![
            "The configured Base URL returned an HTML page, not an API JSON response".to_string(),
            "Use the provider's API root instead of its web console or homepage".to_string(),
            "For DeepSeek-compatible Chat Completions, try a documented endpoint such as `https://api.deepseek.com/v1` or the gateway's exact `/v1` API base path".to_string(),
            "If this gateway requires a vendor-specific path, copy the complete API Base URL from the gateway documentation".to_string(),
        ];
        if !normalized_base_url.is_empty() && !normalized_base_url.ends_with("/v1") {
            suggestions.insert(
                1,
                "The current Base URL does not end with `/v1`; configure the exact API base path from the gateway instead of relying on the homepage root".to_string(),
            );
        }
        return suggestions;
    }

    if lowered.contains("timeout")
        || lowered.contains("connecttimeout")
        || lowered.contains("readtimeout")
        || lowered.contains("pooltimeout")
        || lowered.contains("connecterror")
    {
        let mut suggestions = vec![
            "The API endpoint did not respond in time or could not be reached".to_string(),
            "Check the network path, API base URL, and gateway process status".to_string(),
        ];

        if base_url_hostname == Some("host.docker.internal") {
            if is_running_in_docker_environment() {
                suggestions.push(
                    "The current backend appears to run inside Docker; confirm the host machine is exposing the gateway on the configured port".to_string(),
                );
            } else {
                suggestions.push(
                    "`host.docker.internal` usually only works from inside Docker Desktop containers; if this backend runs on the host OS, switch the API base URL to `http://127.0.0.1:<port>` or `http://localhost:<port>`".to_string(),
                );
            }
        } else if is_local_gateway || is_local_gateway_host(base_url_hostname) {
            suggestions.push(
                "If this is a local gateway, verify the gateway process is listening and can answer /chat/completions on the configured port".to_string(),
            );
        }

        if auto_failover_enabled {
            suggestions.push(
                "Retry after checking transport diagnostics to confirm whether backup endpoint failover was attempted"
                    .to_string(),
            );
        } else {
            suggestions.push(
                "Configure at least one backup endpoint and keep fallback strategy as auto if you want automatic failover"
                    .to_string(),
            );
        }
        return suggestions;
    }

    vec![
        "An unknown error occurred during the request".to_string(),
        "Check the network and configuration parameters".to_string(),
        "Review the detailed error message for more clues".to_string(),
    ]
}

fn classify_error_type(error: &str) -> &'static str {
    let lowered = error.to_lowercase();
    if lowered.contains("401")
        || lowered.contains("403")
        || lowered.contains("unauthorized")
        || lowered.contains("forbidden")
    {
        "AuthenticationError"
    } else if lowered.contains("429") || lowered.contains("rate limit") {
        "HTTPStatusError"
    } else if lowered.contains("502")
        || lowered.contains("503")
        || lowered.contains("504")
        || lowered.contains("bad gateway")
    {
        "HTTPStatusError"
    } else if lowered.contains("timeout") {
        "TimeoutError"
    } else if lowered.contains("404") {
        "EndpointNotFound"
    } else if lowered.contains("http ") {
        "HTTPStatusError"
    } else if lowered.contains("connection")
        || lowered.contains("network")
        || lowered.contains("dns")
    {
        "NetworkError"
    } else {
        "RuntimeError"
    }
}

fn generic_suggestions(kind: &str) -> Vec<&'static str> {
    match kind {
        "api_test" => vec![
            "检查 API Key、Base URL 和模型配置是否正确",
            "确认网络连通性以及当前端点是否可访问",
            "如果配置了备用端点，请同时检查主端点和备用端点状态",
        ],
        "function_calling" => vec![
            "Check whether the selected model supports tool/function calling",
            "Verify the API base URL and provider type",
            "Try a different model if the provider blocks tool calling",
        ],
        "web_research" => vec![
            "Check the API key and provider endpoint",
            "Verify the network path and outbound access",
            "Review the detailed error message for provider-specific clues",
        ],
        _ => vec![
            "Check the network and configuration parameters",
            "Review the detailed error message for more clues",
            "Verify the API base URL, key, and selected model",
        ],
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(SETTINGS_ROUTE, get(get_settings))
        .route(SETTINGS_ROUTE, post(create_settings))
        .route(SETTINGS_ROUTE, put(update_settings))
        .route(SETTINGS_ROUTE, delete(delete_settings))
        .route(SETTINGS_API_KEY_ROUTE, get(get_stored_api_key))
        .route(SETTINGS_MODELS_ROUTE, get(get_available_models))
        .route(SETTINGS_TEST_ROUTE, post(test_api_connection))
        .route(SETTINGS_FETCH_MODELS_ROUTE, post(fetch_models_endpoint))
        .route(
            SETTINGS_TEST_WEB_RESEARCH_ROUTE,
            post(test_web_research_connection),
        )
        .route(
            SETTINGS_CHECK_FUNCTION_CALLING_ROUTE,
            post(check_function_calling),
        )
        .route(SETTINGS_PRESETS_ROUTE, get(get_presets))
        .route(SETTINGS_PRESETS_ROUTE, post(create_preset))
        .route(
            SETTINGS_PRESETS_FROM_CURRENT_ROUTE,
            post(create_preset_from_current),
        )
        .route(SETTINGS_PRESET_DETAIL_ROUTE, put(update_preset))
        .route(SETTINGS_PRESET_DETAIL_ROUTE, delete(delete_preset))
        .route(SETTINGS_PRESET_ACTIVATE_ROUTE, post(activate_preset))
        .route(SETTINGS_PRESET_TEST_ROUTE, post(test_preset))
}

#[cfg(test)]
mod tests {
    use axum::{
        extract::Query,
        http::{HeaderMap, StatusCode},
        routing::{get, post},
        Extension, Json, Router,
    };
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema};
    use serde_json::{json, Value};
    use tokio::net::TcpListener;

    use super::{
        available_models_success_message, build_api_probe_failure_suggestions,
        build_available_models_fallback_payload, build_available_models_payload,
        build_create_preset_from_current_request,
        build_create_settings_preset_request_from_typed_route_payload,
        build_fetch_models_failure_payload, build_fetch_models_fallback_payload,
        build_fetch_models_success_payload, build_openai_compatible_model_candidate_urls,
        build_probe_endpoint_diagnostics, build_probe_transport_config,
        build_provider_model_header_pairs, build_settings_route_owner_contract,
        build_stored_api_key_payload, build_test_preset_connection_request,
        build_update_settings_preset_request_from_typed_route_payload, check_function_calling,
        delete_settings, find_preset_config, get_available_models, normalize_fetch_models_payload,
        parse_anthropic_available_models, parse_gemini_available_models,
        parse_openai_compatible_available_models, test_api_connection, update_settings,
        CreatePresetFromCurrentRouteBody, CreatePresetFromCurrentRouteQuery,
        CreateSettingsPresetRouteRequest, FindPresetConfigError, ModelsQuery,
        SettingsUpdateRouteRequest, TestConnectionRequest, UpdateSettingsPresetRouteRequest,
        SETTINGS_API_KEY_ROUTE, SETTINGS_CHECK_FUNCTION_CALLING_ROUTE, SETTINGS_FETCH_MODELS_ROUTE,
        SETTINGS_MODELS_ROUTE, SETTINGS_PRESETS_FROM_CURRENT_ROUTE, SETTINGS_PRESETS_ROUTE,
        SETTINGS_PRESET_ACTIVATE_ROUTE, SETTINGS_PRESET_DETAIL_ROUTE, SETTINGS_PRESET_TEST_ROUTE,
        SETTINGS_ROUTE, SETTINGS_TEST_ROUTE, SETTINGS_TEST_WEB_RESEARCH_ROUTE,
    };
    use crate::services::auth::Claims;

    async fn setup_settings_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(
            builder.build(&schema.create_table_from_entity(crate::models::settings::Entity)),
        )
        .await
        .expect("create settings table");
        db
    }

    fn test_claims() -> Claims {
        Claims {
            sub: "user-1".to_string(),
            username: "tester".to_string(),
            is_admin: false,
            exp: 0,
            iat: 0,
        }
    }

    async fn spawn_models_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let address = listener.local_addr().expect("listener address");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });
        (format!("http://{}", address), handle)
    }

    async fn spawn_chat_completion_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let address = listener.local_addr().expect("listener address");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });
        (format!("http://{}/v1", address), handle)
    }

    #[test]
    fn should_publish_settings_route_owner_contract() {
        let contract = build_settings_route_owner_contract();

        assert_eq!(contract["owner"], "settings");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/settings.rs");
        assert_eq!(contract["routes"]["settings"], SETTINGS_ROUTE);
        assert_eq!(contract["routes"]["api_key"], SETTINGS_API_KEY_ROUTE);
        assert_eq!(contract["routes"]["models"], SETTINGS_MODELS_ROUTE);
        assert_eq!(contract["routes"]["test"], SETTINGS_TEST_ROUTE);
        assert_eq!(
            contract["routes"]["fetch_models"],
            SETTINGS_FETCH_MODELS_ROUTE
        );
        assert_eq!(
            contract["routes"]["check_function_calling"],
            SETTINGS_CHECK_FUNCTION_CALLING_ROUTE
        );
        assert_eq!(contract["routes"]["presets"], SETTINGS_PRESETS_ROUTE);
        assert_eq!(
            contract["routes"]["preset_detail"],
            SETTINGS_PRESET_DETAIL_ROUTE
        );
        assert!(contract["service_handoffs"]
            .get("preset_request_owner")
            .is_none());
        assert!(contract["service_handoffs"]
            .get("test_preset_request_owner")
            .is_none());
        assert_eq!(
            contract["service_handoffs"]["api_key_payload_owner"],
            "backend-rs/src/api/settings.rs"
        );
        assert_eq!(
            contract["service_handoffs"]["models_payload_owner"],
            "backend-rs/src/api/settings.rs"
        );
        assert_eq!(
            contract["service_handoffs"]["preset_query_owner"],
            "backend-rs/src/api/settings.rs"
        );
        assert_eq!(contract["readiness_probes"].as_array().unwrap().len(), 27);
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 0);
        assert!(contract["source_map_files"].get(0).is_none());
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-settings-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][13],
            "settings-presets-delete-current-business-rust"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_bootstrap_status"],
            "settings_route_runtime_registration_deleted_no_python_route_shell_remains"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_route_files_status"],
            "settings_route_source_map_deleted_no_direct_route_group_python_source_maps_remain"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_status"],
            "physical_closeout_completed"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_physical_closeout_action"],
            "delete_completed"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"][0],
            "surviving settings.py model source-map still needs its own separate physical closeout review"
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            14
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "settings route source-map shell deleted; remaining Python closeout work is limited to backend/migrator_app/models/settings.py"
        );
        assert_eq!(
            contract["migration_policy"],
            "Settings route business smoke is covered by phase5-settings-business-owner; the Python settings route shell, its explicit bootstrap rollback registration, the detached schema shell, and the old runtime-store facade have been physically deleted, and the remaining Python closeout work is limited to backend/migrator_app/models/settings.py."
        );
        assert_eq!(contract["rollback_boundary"]["rollback_files"], json!([]));
    }

    #[test]
    fn build_stored_api_key_payload_trims_and_marks_present_key() {
        let payload = build_stored_api_key_payload(Some("  secret-key  "));

        assert_eq!(payload["api_key"], "secret-key");
        assert_eq!(payload["has_api_key"], true);
    }

    #[test]
    fn build_stored_api_key_payload_handles_empty_and_missing_key() {
        let empty = build_stored_api_key_payload(Some("   "));
        assert_eq!(empty["api_key"], "");
        assert_eq!(empty["has_api_key"], false);

        let missing = build_stored_api_key_payload(None);
        assert_eq!(missing["api_key"], "");
        assert_eq!(missing["has_api_key"], false);
    }

    #[test]
    fn build_available_models_payload_keeps_provider_models_and_count() {
        let payload = build_available_models_payload(
            "openai",
            vec![json!({"value": "gpt-4o", "label": "gpt-4o", "description": "OpenAI-compatible"})],
        );

        assert_eq!(payload["provider"], "openai");
        assert_eq!(payload["count"], 1);
        assert_eq!(payload["models"][0]["value"], "gpt-4o");
    }

    #[test]
    fn build_available_models_fallback_payload_keeps_existing_shell() {
        let payload = build_available_models_fallback_payload(
            "gemini",
            vec![json!({"value": "gemini-2.5-pro"})],
            "timeout",
        );

        assert_eq!(payload["provider"], "gemini");
        assert_eq!(payload["count"], 1);
        assert_eq!(payload["fallback_applied"], true);
        assert_eq!(payload["message"], "Model list fallback applied: timeout");
    }

    #[test]
    fn normalize_fetch_models_payload_accepts_id_and_value_shapes() {
        let payload = normalize_fetch_models_payload(vec![
            json!({"id": "gpt-4o", "owned_by": "openai"}),
            json!({"value": "claude-3-5-sonnet-latest", "description": "Anthropic"}),
        ]);

        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0]["id"], "gpt-4o");
        assert_eq!(payload[0]["owned_by"], "openai");
        assert_eq!(payload[1]["id"], "claude-3-5-sonnet-latest");
        assert_eq!(payload[1]["owned_by"], "Anthropic");
    }

    #[test]
    fn build_fetch_models_payloads_keep_success_and_failure_contracts() {
        let success = build_fetch_models_success_payload(vec![json!({"id": "gpt-4o"})]);
        assert_eq!(success["success"], true);
        assert_eq!(success["models"][0]["id"], "gpt-4o");

        let fallback =
            build_fetch_models_fallback_payload(vec![json!({"id": "gpt-4o-mini"})], "network");
        assert_eq!(fallback["success"], true);
        assert_eq!(fallback["message"], "Model list fallback applied: network");

        let failure = build_fetch_models_failure_payload("failed", "NetworkError");
        assert_eq!(failure["success"], false);
        assert_eq!(failure["models"], json!([]));
        assert_eq!(failure["error_type"], "NetworkError");
    }

    #[test]
    fn find_preset_config_returns_matching_config() {
        let config = find_preset_config(
            Some(
                &json!({
                    "api_presets": {
                        "version": "1.0",
                        "presets": [
                            {
                                "id": "preset_1",
                                "config": {
                                    "api_provider": "openai",
                                    "llm_model": "gpt-4o"
                                }
                            }
                        ]
                    }
                })
                .to_string(),
            ),
            "preset_1",
        )
        .expect("preset config should exist");

        assert_eq!(config["api_provider"], "openai");
        assert_eq!(config["llm_model"], "gpt-4o");
    }

    #[test]
    fn find_preset_config_defaults_to_empty_object_when_config_missing() {
        let config = find_preset_config(
            Some(
                &json!({
                    "api_presets": {
                        "presets": [
                            { "id": "preset_1" }
                        ]
                    }
                })
                .to_string(),
            ),
            "preset_1",
        )
        .expect("preset without config should still resolve");

        assert_eq!(config, json!({}));
    }

    #[test]
    fn find_preset_config_rejects_unknown_preset() {
        let error =
            find_preset_config(Some("{}"), "missing").expect_err("missing preset should fail");

        assert_eq!(error, FindPresetConfigError::PresetNotFound);
    }

    #[test]
    fn should_keep_settings_route_group_paths_stable() {
        assert_eq!(SETTINGS_ROUTE, "/settings");
        assert_eq!(SETTINGS_API_KEY_ROUTE, "/settings/api-key");
        assert_eq!(SETTINGS_MODELS_ROUTE, "/settings/models");
        assert_eq!(SETTINGS_TEST_ROUTE, "/settings/test");
        assert_eq!(SETTINGS_FETCH_MODELS_ROUTE, "/settings/fetch-models");
        assert_eq!(
            SETTINGS_TEST_WEB_RESEARCH_ROUTE,
            "/settings/test-web-research"
        );
        assert_eq!(
            SETTINGS_CHECK_FUNCTION_CALLING_ROUTE,
            "/settings/check-function-calling"
        );
        assert_eq!(SETTINGS_PRESETS_ROUTE, "/settings/presets");
        assert_eq!(
            SETTINGS_PRESETS_FROM_CURRENT_ROUTE,
            "/settings/presets/from-current"
        );
        assert_eq!(
            SETTINGS_PRESET_DETAIL_ROUTE,
            "/settings/presets/{preset_id}"
        );
        assert_eq!(
            SETTINGS_PRESET_ACTIVATE_ROUTE,
            "/settings/presets/{preset_id}/activate"
        );
        assert_eq!(
            SETTINGS_PRESET_TEST_ROUTE,
            "/settings/presets/{preset_id}/test"
        );
    }

    #[test]
    fn build_test_preset_connection_request_prefers_existing_provider_keys() {
        let request = build_test_preset_connection_request(&json!({
            "api_key": "sk-test",
            "api_base_url": "https://api.example.com/v1",
            "api_provider": "openai",
            "provider": "gemini",
            "llm_model": "gpt-4o",
            "model": "gemini-2.5-pro",
            "temperature": 0.7,
            "max_tokens": 1024,
            "api_backup_urls": ["https://backup-1.example.com/v1", 1, "https://backup-2.example.com/v1"],
            "fallback_strategy": "manual"
        }));

        assert_eq!(request.api_key.as_deref(), Some("sk-test"));
        assert_eq!(
            request.api_base_url.as_deref(),
            Some("https://api.example.com/v1")
        );
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.llm_model.as_deref(), Some("gpt-4o"));
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(1024));
        assert_eq!(
            request.api_backup_urls,
            Some(vec![
                "https://backup-1.example.com/v1".to_string(),
                "https://backup-2.example.com/v1".to_string()
            ])
        );
        assert_eq!(request.fallback_strategy.as_deref(), Some("manual"));
    }

    #[test]
    fn build_test_preset_connection_request_falls_back_to_legacy_keys() {
        let request = build_test_preset_connection_request(&json!({
            "provider": "anthropic",
            "model": "claude-3-5-sonnet-latest",
            "temperature": 0.125
        }));

        assert_eq!(request.provider.as_deref(), Some("anthropic"));
        assert_eq!(
            request.llm_model.as_deref(),
            Some("claude-3-5-sonnet-latest")
        );
        assert_eq!(request.temperature, Some(0.125));
        assert_eq!(request.max_tokens, None);
        assert_eq!(request.api_backup_urls, None);
        assert_eq!(request.fallback_strategy, None);
    }

    #[test]
    fn build_test_preset_connection_request_ignores_invalid_max_tokens() {
        let request = build_test_preset_connection_request(&json!({
            "max_tokens": 5000000000u64,
            "temperature": "bad"
        }));

        assert_eq!(request.max_tokens, None);
        assert_eq!(request.temperature, None);
    }

    #[test]
    fn build_create_preset_from_current_request_prefers_non_empty_query_values() {
        let request = build_create_preset_from_current_request(
            CreatePresetFromCurrentRouteQuery {
                name: Some("Query Preset".to_string()),
                description: Some("Query Description".to_string()),
            },
            Some(CreatePresetFromCurrentRouteBody {
                name: Some("Body Preset".to_string()),
                description: Some("Body Description".to_string()),
            }),
        );

        assert_eq!(request.name, "Query Preset");
        assert_eq!(request.description.as_deref(), Some("Query Description"));
    }

    #[test]
    fn build_create_preset_from_current_request_falls_back_to_body_and_default_name() {
        let from_body = build_create_preset_from_current_request(
            CreatePresetFromCurrentRouteQuery::default(),
            Some(CreatePresetFromCurrentRouteBody {
                name: Some("Body Preset".to_string()),
                description: Some("Body Description".to_string()),
            }),
        );
        assert_eq!(from_body.name, "Body Preset");
        assert_eq!(from_body.description.as_deref(), Some("Body Description"));

        let defaulted = build_create_preset_from_current_request(
            CreatePresetFromCurrentRouteQuery::default(),
            Some(CreatePresetFromCurrentRouteBody {
                name: Some("   ".to_string()),
                description: None,
            }),
        );
        assert_eq!(defaulted.name, "My Preset");
        assert_eq!(defaulted.description, None);
    }

    #[test]
    fn build_create_preset_from_current_request_treats_blank_query_as_missing() {
        let request = build_create_preset_from_current_request(
            CreatePresetFromCurrentRouteQuery {
                name: Some("   ".to_string()),
                description: Some("  ".to_string()),
            },
            Some(CreatePresetFromCurrentRouteBody {
                name: Some("Body Preset".to_string()),
                description: Some("".to_string()),
            }),
        );

        assert_eq!(request.name, "Body Preset");
        assert_eq!(request.description.as_deref(), Some(""));
    }

    #[test]
    fn build_create_settings_preset_request_from_typed_route_payload_keeps_existing_shape() {
        let request = build_create_settings_preset_request_from_typed_route_payload(
            CreateSettingsPresetRouteRequest {
                name: Some(json!("Preset A")),
                description: Some(json!("desc")),
                config: Some(json!({
                    "provider": "openai"
                })),
            },
        );

        assert_eq!(request.name(), "Preset A");
        assert_eq!(request.description(), Some(&json!("desc")));
        assert_eq!(request.config()["provider"], "openai");
    }

    #[test]
    fn build_update_settings_preset_request_from_typed_route_payload_tracks_optional_fields() {
        let request = build_update_settings_preset_request_from_typed_route_payload(
            UpdateSettingsPresetRouteRequest {
                name: Some(json!("Renamed")),
                description: Some(json!(null)),
                config: Some(json!({
                    "model": "gpt-4o"
                })),
            },
        );

        assert_eq!(request.name(), Some("Renamed"));
        assert!(request.has_description());
        assert_eq!(request.description(), Some(&json!(null)));
        assert_eq!(
            request.config().expect("config should exist")["model"],
            "gpt-4o"
        );
    }

    #[tokio::test]
    async fn update_settings_returns_404_when_missing() {
        let db = setup_settings_db().await;
        let request = SettingsUpdateRouteRequest {
            llm_model: Some(serde_json::json!("gpt-4.1")),
            ..Default::default()
        };

        let error = update_settings(Extension(test_claims()), Extension(db), Json(request))
            .await
            .expect_err("missing settings should map to 404");

        assert_eq!(error.0, StatusCode::NOT_FOUND);
        assert_eq!(
            error.1 .0["detail"],
            Value::String("设置不存在，请先创建设置".to_string())
        );
    }

    #[tokio::test]
    async fn delete_settings_returns_404_when_missing() {
        let db = setup_settings_db().await;

        let error = delete_settings(Extension(test_claims()), Extension(db))
            .await
            .expect_err("missing settings delete should map to 404");

        assert_eq!(error.0, StatusCode::NOT_FOUND);
        assert_eq!(
            error.1 .0["detail"],
            Value::String("设置不存在".to_string())
        );
    }

    #[test]
    fn openai_compatible_model_candidates_follow_python_fallback_order() {
        assert_eq!(
            build_openai_compatible_model_candidate_urls("https://provider.example.com"),
            vec![
                "https://provider.example.com/models".to_string(),
                "https://provider.example.com/v1/models".to_string(),
            ]
        );
        assert_eq!(
            build_openai_compatible_model_candidate_urls("https://provider.example.com/v1"),
            vec![
                "https://provider.example.com/v1/models".to_string(),
                "https://provider.example.com/models".to_string(),
            ]
        );
    }

    #[test]
    fn provider_model_headers_match_provider_contracts() {
        let openai_headers = build_provider_model_header_pairs("openai", "sk-test");
        assert_eq!(
            openai_headers[0],
            ("Authorization", "Bearer sk-test".to_string())
        );

        let azure_headers = build_provider_model_header_pairs("azure", "azure-key");
        assert_eq!(azure_headers[0], ("api-key", "azure-key".to_string()));
        assert!(!azure_headers
            .iter()
            .any(|(name, _)| *name == "Authorization"));

        let anthropic_headers = build_provider_model_header_pairs("anthropic", "ak-test");
        assert_eq!(anthropic_headers[0], ("x-api-key", "ak-test".to_string()));
        assert_eq!(
            anthropic_headers[1],
            ("anthropic-version", "2023-06-01".to_string())
        );
    }

    #[test]
    fn openai_compatible_model_parser_matches_python_shapes() {
        let models = parse_openai_compatible_available_models(&json!({
            "data": [
                {"id": "gpt-4.1-mini", "created": 123},
                {"name": "models/custom-model", "display_name": "Custom Model"},
                "deepseek-chat"
            ]
        }));

        assert_eq!(models.len(), 3);
        assert_eq!(models[0]["value"], "gpt-4.1-mini");
        assert_eq!(models[0]["description"], "Created: 123");
        assert_eq!(models[1]["value"], "custom-model");
        assert_eq!(models[1]["description"], "Custom Model");
        assert_eq!(models[2]["value"], "deepseek-chat");
    }

    #[test]
    fn anthropic_and_gemini_model_parsers_match_python_contracts() {
        let anthropic_models = parse_anthropic_available_models(&json!({
            "data": [
                {"id": "claude-3-5-sonnet", "display_name": "Claude 3.5 Sonnet"}
            ]
        }));
        assert_eq!(anthropic_models[0]["value"], "claude-3-5-sonnet");
        assert_eq!(anthropic_models[0]["description"], "Claude 3.5 Sonnet");

        let gemini_models = parse_gemini_available_models(&json!({
            "models": [
                {
                    "name": "models/gemini-2.0-pro",
                    "displayName": "Gemini 2.0 Pro",
                    "supportedGenerationMethods": ["generateContent"]
                },
                {
                    "name": "models/embedding-001",
                    "displayName": "Embedding",
                    "supportedGenerationMethods": ["embedContent"]
                }
            ]
        }));
        assert_eq!(gemini_models.len(), 1);
        assert_eq!(gemini_models[0]["value"], "gemini-2.0-pro");
        assert_eq!(gemini_models[0]["label"], "Gemini 2.0 Pro");
    }

    #[test]
    fn azure_empty_models_return_friendly_message_only_for_azure() {
        assert_eq!(
            available_models_success_message("azure", &[]),
            Some("Azure OpenAI 无法自动获取模型列表，请手动填写部署名称到模型字段")
        );
        assert_eq!(available_models_success_message("openai", &[]), None);
        assert_eq!(
            available_models_success_message("azure", &[json!({"value": "gpt-4.1"})]),
            None
        );
    }

    #[test]
    fn probe_endpoint_diagnostics_match_python_minimal_contract() {
        let diagnostics = build_probe_endpoint_diagnostics("https://api.openai.com/v1", None, None);
        assert_eq!(
            diagnostics["primary_endpoint"],
            Value::String("https://api.openai.com/v1".to_string())
        );
        assert_eq!(diagnostics["backup_endpoints"], json!([]));
        assert_eq!(diagnostics["configured_endpoint_count"], json!(1));
        assert_eq!(diagnostics["fallback_strategy"], json!("auto"));
        assert_eq!(diagnostics["auto_failover_enabled"], json!(false));
    }

    #[test]
    fn probe_endpoint_diagnostics_include_backup_urls_and_manual_strategy() {
        let backup_urls = vec![
            "https://backup-1.example.com/v1/".to_string(),
            " https://backup-2.example.com/v1 ".to_string(),
            "https://backup-1.example.com/v1".to_string(),
        ];
        let diagnostics = build_probe_endpoint_diagnostics(
            "https://api.openai.com/v1/",
            Some(backup_urls.as_slice()),
            Some("manual"),
        );

        assert_eq!(
            diagnostics["primary_endpoint"],
            json!("https://api.openai.com/v1")
        );
        assert_eq!(
            diagnostics["backup_endpoints"],
            json!([
                "https://backup-1.example.com/v1",
                "https://backup-2.example.com/v1"
            ])
        );
        assert_eq!(diagnostics["configured_endpoint_count"], json!(3));
        assert_eq!(diagnostics["fallback_strategy"], json!("manual"));
        assert_eq!(diagnostics["auto_failover_enabled"], json!(false));
    }

    #[test]
    fn probe_transport_config_prefers_normalized_v1_for_openai_compatible_root_base_url() {
        let (prefer_normalized_v1_candidate, read_timeout_secs, transport_max_retries) =
            build_probe_transport_config("custom", "https://gateway.example.com");
        assert!(prefer_normalized_v1_candidate);
        assert_eq!(read_timeout_secs, Some(10.0));
        assert_eq!(transport_max_retries, 1);

        let (prefer_normalized_v1_candidate, _, _) =
            build_probe_transport_config("custom", "https://gateway.example.com/v1");
        assert!(!prefer_normalized_v1_candidate);
    }

    #[tokio::test]
    async fn get_available_models_falls_back_from_root_to_v1_for_openai_compatible_provider() {
        let db = setup_settings_db().await;

        let app = Router::new()
            .route(
                "/models",
                get(|| async { (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))) }),
            )
            .route(
                "/v1/models",
                get(|| async { Json(json!({"data": [{"id": "gpt-5.3-codex"}]})) }),
            );
        let (base_url, handle) = spawn_models_server(app).await;

        let response = get_available_models(
            Extension(test_claims()),
            Extension(db),
            Query(ModelsQuery {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(base_url),
                provider: Some("sub2api".to_string()),
            }),
        )
        .await
        .expect("models route should succeed");

        handle.abort();

        assert_eq!(response.0["provider"], "sub2api");
        assert_eq!(response.0["count"], 1);
        assert_eq!(response.0["models"][0]["value"], "gpt-5.3-codex");
    }

    #[tokio::test]
    async fn get_available_models_returns_friendly_empty_message_for_azure_404() {
        let db = setup_settings_db().await;
        let captured_headers = std::sync::Arc::new(std::sync::Mutex::new(Vec::<HeaderMap>::new()));
        let captured_headers_clone = captured_headers.clone();

        let app = Router::new().route(
            "/models",
            get(move |headers: HeaderMap| {
                let captured_headers = captured_headers_clone.clone();
                async move {
                    captured_headers
                        .lock()
                        .expect("lock captured headers")
                        .push(headers);
                    (StatusCode::NOT_FOUND, Json(json!({"error": "not found"})))
                }
            }),
        );
        let (base_url, handle) = spawn_models_server(app).await;

        let response = get_available_models(
            Extension(test_claims()),
            Extension(db),
            Query(ModelsQuery {
                api_key: Some("azure-key".to_string()),
                api_base_url: Some(base_url),
                provider: Some("azure".to_string()),
            }),
        )
        .await
        .expect("azure empty models should be friendly success");

        handle.abort();

        let captured_headers = captured_headers.lock().expect("lock captured headers");
        let first_headers = captured_headers.first().expect("captured azure request");
        assert_eq!(
            first_headers
                .get("api-key")
                .and_then(|value| value.to_str().ok()),
            Some("azure-key")
        );
        assert!(first_headers.get("authorization").is_none());
        assert_eq!(response.0["provider"], "azure");
        assert_eq!(response.0["count"], 0);
        assert_eq!(
            response.0["message"],
            "Azure OpenAI 无法自动获取模型列表，请手动填写部署名称到模型字段"
        );
    }

    #[tokio::test]
    async fn check_function_calling_returns_supported_true_when_tool_calls_present() {
        let db = setup_settings_db().await;

        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({
                    "choices": [{
                        "message": {
                            "content": "",
                            "tool_calls": [{
                                "id": "call_001",
                                "type": "function",
                                "function": {
                                    "name": "get_weather",
                                    "arguments": "{\"city\":\"Beijing\"}"
                                }
                            }]
                        },
                        "finish_reason": "tool_calls"
                    }]
                }))
            }),
        );
        let (base_url, handle) = spawn_chat_completion_server(app).await;
        let expected_base_url = base_url.clone();

        let response = check_function_calling(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(base_url),
                provider: Some("openai".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: None,
                max_tokens: None,
                api_backup_urls: None,
                fallback_strategy: None,
            }),
        )
        .await
        .expect("function calling probe should succeed");

        handle.abort();

        assert_eq!(response.0["success"], json!(true));
        assert_eq!(response.0["supported"], json!(true));
        assert_eq!(response.0["details"]["has_tool_calls"], json!(true));
        assert_eq!(response.0["details"]["tool_call_count"], json!(1));
        assert_eq!(response.0["details"]["response_type"], json!("tool_calls"));
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["primary_endpoint"],
            json!(expected_base_url)
        );
    }

    #[tokio::test]
    async fn check_function_calling_keeps_success_true_when_model_returns_plain_text() {
        let db = setup_settings_db().await;

        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({
                    "choices": [{
                        "message": {
                            "content": "plain text response"
                        },
                        "finish_reason": "stop"
                    }]
                }))
            }),
        );
        let (base_url, handle) = spawn_chat_completion_server(app).await;

        let response = check_function_calling(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(base_url),
                provider: Some("openai".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: None,
                max_tokens: None,
                api_backup_urls: None,
                fallback_strategy: None,
            }),
        )
        .await
        .expect("function calling probe should still return a success shell");

        handle.abort();

        assert_eq!(response.0["success"], json!(true));
        assert_eq!(response.0["supported"], json!(false));
        assert_eq!(response.0["details"]["has_tool_calls"], json!(false));
        assert_eq!(response.0["details"]["tool_call_count"], json!(0));
        assert_eq!(response.0["details"]["response_type"], json!("text"));
        assert_eq!(response.0["response_preview"], json!("plain text response"));
    }

    #[tokio::test]
    async fn check_function_calling_gateway_failure_returns_python_aligned_guidance() {
        let db = setup_settings_db().await;

        let gateway_app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error": "bad gateway"})),
                )
            }),
        );

        let (gateway_base_url, gateway_handle) = spawn_chat_completion_server(gateway_app).await;

        let response = check_function_calling(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(gateway_base_url.clone()),
                provider: Some("openai_responses".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: None,
                max_tokens: None,
                api_backup_urls: None,
                fallback_strategy: Some("auto".to_string()),
            }),
        )
        .await
        .expect("gateway error should still return failure shell");

        gateway_handle.abort();

        assert_eq!(response.0["success"], json!(false));
        assert_eq!(response.0["supported"], Value::Null);
        assert_eq!(response.0["error_type"], json!("HTTPStatusError"));
        assert_eq!(
            response.0["message"],
            json!("上游服务暂时不可用（HTTP 502）")
        );
        assert_eq!(response.0["details"]["http_status_code"], json!(502));
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["primary_endpoint"],
            json!(gateway_base_url)
        );
        let suggestions = response.0["suggestions"]
            .as_array()
            .expect("suggestions should be an array");
        assert!(suggestions.iter().any(|item| {
            item.as_str()
                .map(|value| {
                    value
                        .to_ascii_lowercase()
                        .contains("local gateway or proxy")
                })
                .unwrap_or(false)
        }));
        assert!(suggestions.iter().any(|item| {
            item.as_str()
                .map(|value| value.to_ascii_lowercase().contains("backup endpoint"))
                .unwrap_or(false)
        }));
    }

    #[tokio::test]
    async fn check_function_calling_uses_gemini_owner_path_for_tool_calls() {
        let db = setup_settings_db().await;

        let app = Router::new().route(
            "/models/gemini-2.5-flash:generateContent",
            post(|| async {
                Json(json!({
                    "candidates": [{
                        "content": {
                            "parts": [{
                                "functionCall": {
                                    "name": "get_weather",
                                    "args": { "city": "Beijing", "unit": "celsius" }
                                }
                            }]
                        },
                        "finishReason": "STOP"
                    }]
                }))
            }),
        );
        let (base_url, handle) = spawn_models_server(app).await;

        let response = check_function_calling(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("gk-test".to_string()),
                api_base_url: Some(base_url.clone()),
                provider: Some("gemini".to_string()),
                llm_model: Some("gemini-2.5-flash".to_string()),
                temperature: None,
                max_tokens: None,
                api_backup_urls: None,
                fallback_strategy: None,
            }),
        )
        .await
        .expect("gemini function calling probe should succeed");

        handle.abort();

        assert_eq!(response.0["success"], json!(true));
        assert_eq!(response.0["supported"], json!(true));
        assert_eq!(response.0["details"]["has_tool_calls"], json!(true));
        assert_eq!(response.0["details"]["tool_call_count"], json!(1));
        assert_eq!(response.0["details"]["response_type"], json!("tool_calls"));
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["primary_endpoint"],
            json!(base_url)
        );
        assert_eq!(
            response.0["tool_calls"][0]["function"]["name"],
            json!("get_weather")
        );
    }

    #[tokio::test]
    async fn test_api_connection_uses_gemini_owner_path_for_success_shell() {
        let db = setup_settings_db().await;

        let app = Router::new().route(
            "/models/gemini-2.5-flash:generateContent",
            post(|| async {
                Json(json!({
                    "candidates": [{
                        "content": {
                            "parts": [{
                                "text": "OK from gemini"
                            }]
                        },
                        "finishReason": "STOP"
                    }]
                }))
            }),
        );
        let (base_url, handle) = spawn_models_server(app).await;

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("gk-test".to_string()),
                api_base_url: Some(base_url.clone()),
                provider: Some("gemini".to_string()),
                llm_model: Some("gemini-2.5-flash".to_string()),
                temperature: Some(0.2),
                max_tokens: Some(256),
                api_backup_urls: None,
                fallback_strategy: Some("manual".to_string()),
            }),
        )
        .await
        .expect("gemini API probe should succeed");

        handle.abort();

        assert_eq!(response.0["success"], json!(true));
        assert_eq!(response.0["message"], json!("API 连接测试成功"));
        assert_eq!(response.0["provider"], json!("gemini"));
        assert_eq!(response.0["model"], json!("gemini-2.5-flash"));
        assert_eq!(response.0["response_preview"], json!("OK from gemini"));
        assert_eq!(response.0["details"]["api_available"], json!(true));
        assert_eq!(response.0["details"]["model_accessible"], json!(true));
        assert_eq!(response.0["details"]["response_valid"], json!(true));
        assert_eq!(response.0["details"]["temperature"], json!(0.2));
        assert_eq!(response.0["details"]["max_tokens"], json!(256));
        assert_eq!(response.0["details"]["probe_max_tokens"], json!(64));
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["primary_endpoint"],
            json!(base_url)
        );
        assert!(response.0["details"].get("transport_diagnostics").is_none());
    }

    #[tokio::test]
    async fn test_api_connection_returns_python_style_details_shell() {
        let db = setup_settings_db().await;

        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({
                    "choices": [{
                        "message": {
                            "content": "TEST_OK from probe response"
                        },
                        "finish_reason": "stop"
                    }]
                }))
            }),
        );
        let (base_url, handle) = spawn_chat_completion_server(app).await;

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(base_url),
                provider: Some("openai".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: Some(0.9),
                max_tokens: Some(512),
                api_backup_urls: Some(vec![
                    "https://backup-1.example.com/v1/".to_string(),
                    "https://backup-2.example.com/v1".to_string(),
                ]),
                fallback_strategy: Some("manual".to_string()),
            }),
        )
        .await
        .expect("api test probe should succeed");

        handle.abort();

        assert_eq!(response.0["success"], json!(true));
        assert_eq!(response.0["message"], json!("API 连接测试成功"));
        assert_eq!(response.0["provider"], json!("openai"));
        assert_eq!(response.0["model"], json!("gpt-4.1-mini"));
        assert_eq!(
            response.0["response_preview"],
            json!("TEST_OK from probe response")
        );
        assert_eq!(response.0["details"]["api_available"], json!(true));
        assert_eq!(response.0["details"]["model_accessible"], json!(true));
        assert_eq!(response.0["details"]["response_valid"], json!(true));
        assert_eq!(response.0["details"]["temperature"], json!(0.9));
        assert_eq!(response.0["details"]["max_tokens"], json!(512));
        assert_eq!(response.0["details"]["probe_max_tokens"], json!(64));
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["backup_endpoints"],
            json!([
                "https://backup-1.example.com/v1",
                "https://backup-2.example.com/v1"
            ])
        );
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["fallback_strategy"],
            json!("manual")
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["total_attempts"],
            json!(1)
        );
    }

    #[tokio::test]
    async fn test_api_connection_prefers_normalized_v1_candidate_for_root_base_url() {
        let db = setup_settings_db().await;

        let app = Router::new()
            .route(
                "/chat/completions",
                post(|| async {
                    (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "root endpoint not found"})),
                    )
                }),
            )
            .route(
                "/v1/chat/completions",
                post(|| async {
                    Json(json!({
                        "choices": [{
                            "message": {
                                "content": "TEST_OK from normalized candidate"
                            },
                            "finish_reason": "stop"
                        }]
                    }))
                }),
            );
        let (base_url, handle) = spawn_models_server(app).await;

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(base_url.clone()),
                provider: Some("custom".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: Some(0.4),
                max_tokens: Some(128),
                api_backup_urls: None,
                fallback_strategy: None,
            }),
        )
        .await
        .expect("api test should succeed via normalized /v1 candidate");

        handle.abort();

        assert_eq!(response.0["success"], json!(true));
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["total_attempts"],
            json!(1)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["effective_base_url"],
            json!(format!("{}/v1", base_url))
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["result"],
            json!("success")
        );
    }

    #[tokio::test]
    async fn check_function_calling_prefers_normalized_v1_candidate_for_root_base_url() {
        let db = setup_settings_db().await;

        let app = Router::new()
            .route(
                "/chat/completions",
                post(|| async {
                    (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "root endpoint not found"})),
                    )
                }),
            )
            .route(
                "/v1/chat/completions",
                post(|| async {
                    Json(json!({
                        "choices": [{
                            "message": {
                                "content": "",
                                "tool_calls": [{
                                    "id": "call_001",
                                    "type": "function",
                                    "function": {
                                        "name": "get_weather",
                                        "arguments": "{\"city\":\"Beijing\"}"
                                    }
                                }]
                            },
                            "finish_reason": "tool_calls"
                        }]
                    }))
                }),
            );
        let (base_url, handle) = spawn_models_server(app).await;

        let response = check_function_calling(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(base_url.clone()),
                provider: Some("custom".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: None,
                max_tokens: None,
                api_backup_urls: None,
                fallback_strategy: None,
            }),
        )
        .await
        .expect("function calling probe should succeed via normalized /v1 candidate");

        handle.abort();

        assert_eq!(response.0["success"], json!(true));
        assert_eq!(response.0["supported"], json!(true));
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["total_attempts"],
            json!(1)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["effective_base_url"],
            json!(format!("{}/v1", base_url))
        );
    }

    #[tokio::test]
    async fn test_api_connection_openai_responses_root_base_url_does_not_fallback_to_root_candidate(
    ) {
        let db = setup_settings_db().await;

        let app = Router::new()
            .route(
                "/v1/chat/completions",
                post(|| async {
                    (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "normalized endpoint not found"})),
                    )
                }),
            )
            .route(
                "/chat/completions",
                post(|| async {
                    Json(json!({
                        "choices": [{
                            "message": {
                                "content": "TEST_OK from root candidate"
                            },
                            "finish_reason": "stop"
                        }]
                    }))
                }),
            );
        let (base_url, handle) = spawn_models_server(app).await;

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(base_url.clone()),
                provider: Some("openai_responses".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: Some(0.4),
                max_tokens: Some(128),
                api_backup_urls: None,
                fallback_strategy: Some("auto".to_string()),
            }),
        )
        .await
        .expect("openai_responses probe should keep the Python /v1-only candidate contract");

        handle.abort();

        assert_eq!(response.0["success"], json!(false));
        assert_eq!(response.0["error_type"], json!("EndpointNotFound"));
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["total_attempts"],
            json!(1)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["effective_base_url"],
            json!(format!("{}/v1", base_url))
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["base_url"],
            json!(format!("{}/v1", base_url))
        );
    }

    #[tokio::test]
    async fn check_function_calling_sub2api_root_base_url_does_not_fallback_to_root_candidate() {
        let db = setup_settings_db().await;

        let app = Router::new()
            .route(
                "/v1/chat/completions",
                post(|| async {
                    (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "normalized endpoint not found"})),
                    )
                }),
            )
            .route(
                "/chat/completions",
                post(|| async {
                    Json(json!({
                        "choices": [{
                            "message": {
                                "content": "",
                                "tool_calls": [{
                                    "id": "call_001",
                                    "type": "function",
                                    "function": {
                                        "name": "get_weather",
                                        "arguments": "{\"city\":\"Beijing\"}"
                                    }
                                }]
                            },
                            "finish_reason": "tool_calls"
                        }]
                    }))
                }),
            );
        let (base_url, handle) = spawn_models_server(app).await;

        let response = check_function_calling(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(base_url.clone()),
                provider: Some("sub2api".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: None,
                max_tokens: None,
                api_backup_urls: None,
                fallback_strategy: Some("auto".to_string()),
            }),
        )
        .await
        .expect(
            "sub2api function-calling probe should keep the Python /v1-only candidate contract",
        );

        handle.abort();

        assert_eq!(response.0["success"], json!(false));
        assert_eq!(response.0["supported"], Value::Null);
        assert_eq!(response.0["error_type"], json!("EndpointNotFound"));
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["total_attempts"],
            json!(1)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["effective_base_url"],
            json!(format!("{}/v1", base_url))
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["base_url"],
            json!(format!("{}/v1", base_url))
        );
    }

    #[tokio::test]
    async fn test_api_connection_https_local_gateway_falls_back_to_http_candidate() {
        let db = setup_settings_db().await;

        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({
                    "choices": [{
                        "message": {
                            "content": "TEST_OK from http local gateway candidate"
                        },
                        "finish_reason": "stop"
                    }]
                }))
            }),
        );
        let (http_base_url, handle) = spawn_chat_completion_server(app).await;
        let https_base_url = http_base_url.replacen("http://", "https://", 1);

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(https_base_url.clone()),
                provider: Some("openai".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: Some(0.4),
                max_tokens: Some(128),
                api_backup_urls: None,
                fallback_strategy: Some("auto".to_string()),
            }),
        )
        .await
        .expect("https local gateway probe should fall back to the http candidate");

        handle.abort();

        assert_eq!(response.0["success"], json!(true));
        assert_eq!(
            response.0["response_preview"],
            json!("TEST_OK from http local gateway candidate")
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["effective_base_url"],
            json!(http_base_url)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["total_attempts"],
            json!(2)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["result"],
            json!("network_error")
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][1]["result"],
            json!("success")
        );
    }

    #[tokio::test]
    async fn test_api_connection_auto_fallback_uses_backup_endpoint() {
        let db = setup_settings_db().await;

        let primary_app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "primary failed"})),
                )
            }),
        );
        let backup_app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({
                    "choices": [{
                        "message": {
                            "content": "TEST_OK from backup"
                        },
                        "finish_reason": "stop"
                    }]
                }))
            }),
        );

        let (primary_base_url, primary_handle) = spawn_chat_completion_server(primary_app).await;
        let (backup_base_url, backup_handle) = spawn_chat_completion_server(backup_app).await;

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(primary_base_url),
                provider: Some("openai".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: Some(0.4),
                max_tokens: Some(128),
                api_backup_urls: Some(vec![backup_base_url.clone()]),
                fallback_strategy: Some("auto".to_string()),
            }),
        )
        .await
        .expect("api test should succeed via backup endpoint");

        primary_handle.abort();
        backup_handle.abort();

        assert_eq!(response.0["success"], json!(true));
        assert_eq!(response.0["response_preview"], json!("TEST_OK from backup"));
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["effective_base_url"],
            json!(backup_base_url)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["backup_endpoint_used"],
            json!(true)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["failover_count"],
            json!(1)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["will_failover"],
            json!(true)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][1]["endpoint_role"],
            json!("backup")
        );
    }

    #[tokio::test]
    async fn test_api_connection_v1_404_does_not_fallback_to_root_or_backup() {
        let db = setup_settings_db().await;

        let primary_app = Router::new()
            .route(
                "/v1/chat/completions",
                post(|| async {
                    (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "normalized endpoint not found"})),
                    )
                }),
            )
            .route(
                "/chat/completions",
                post(|| async {
                    Json(json!({
                        "choices": [{
                            "message": {
                                "content": "TEST_OK from root candidate"
                            },
                            "finish_reason": "stop"
                        }]
                    }))
                }),
            );
        let backup_app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({
                    "choices": [{
                        "message": {
                            "content": "TEST_OK from backup"
                        },
                        "finish_reason": "stop"
                    }]
                }))
            }),
        );

        let (primary_base_url, primary_handle) = spawn_chat_completion_server(primary_app).await;
        let (backup_base_url, backup_handle) = spawn_chat_completion_server(backup_app).await;

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(primary_base_url.clone()),
                provider: Some("custom".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: Some(0.4),
                max_tokens: Some(128),
                api_backup_urls: Some(vec![backup_base_url.clone()]),
                fallback_strategy: Some("auto".to_string()),
            }),
        )
        .await
        .expect("v1 404 should still return a failure shell");

        primary_handle.abort();
        backup_handle.abort();

        assert_eq!(response.0["success"], json!(false));
        assert_eq!(response.0["error_type"], json!("EndpointNotFound"));
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["effective_base_url"],
            json!(primary_base_url)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["backup_endpoint_used"],
            json!(false)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["failover_count"],
            json!(0)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["status_code"],
            json!(404)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["will_failover"],
            json!(false)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["total_attempts"],
            json!(1)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["endpoint_role"],
            json!("primary")
        );
    }

    #[tokio::test]
    async fn test_api_connection_manual_fallback_does_not_use_backup_endpoint() {
        let db = setup_settings_db().await;

        let primary_app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "primary failed"})),
                )
            }),
        );
        let backup_app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({
                    "choices": [{
                        "message": {
                            "content": "TEST_OK from backup"
                        },
                        "finish_reason": "stop"
                    }]
                }))
            }),
        );

        let (primary_base_url, primary_handle) = spawn_chat_completion_server(primary_app).await;
        let (backup_base_url, backup_handle) = spawn_chat_completion_server(backup_app).await;

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(primary_base_url),
                provider: Some("openai".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: Some(0.4),
                max_tokens: Some(128),
                api_backup_urls: Some(vec![backup_base_url]),
                fallback_strategy: Some("manual".to_string()),
            }),
        )
        .await
        .expect("manual fallback should still return failure shell");

        primary_handle.abort();
        backup_handle.abort();

        assert_eq!(response.0["success"], json!(false));
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["fallback_strategy"],
            json!("manual")
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["total_attempts"],
            json!(1)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["backup_endpoint_used"],
            json!(false)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["endpoint_role"],
            json!("primary")
        );
    }

    #[tokio::test]
    async fn test_api_connection_auto_fallback_failure_keeps_transport_diagnostics() {
        let db = setup_settings_db().await;

        let primary_app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "primary failed"})),
                )
            }),
        );
        let backup_app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "backup failed"})),
                )
            }),
        );

        let (primary_base_url, primary_handle) = spawn_chat_completion_server(primary_app).await;
        let (backup_base_url, backup_handle) = spawn_chat_completion_server(backup_app).await;

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(primary_base_url),
                provider: Some("openai".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: Some(0.4),
                max_tokens: Some(128),
                api_backup_urls: Some(vec![backup_base_url]),
                fallback_strategy: Some("auto".to_string()),
            }),
        )
        .await
        .expect("auto fallback failure should still return failure shell");

        primary_handle.abort();
        backup_handle.abort();

        assert_eq!(response.0["success"], json!(false));
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["total_attempts"],
            json!(2)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["backup_endpoint_used"],
            json!(true)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["summary"]["failover_count"],
            json!(1)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["status_code"],
            json!(500)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][0]["will_failover"],
            json!(true)
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][1]["endpoint_role"],
            json!("backup")
        );
        assert_eq!(
            response.0["details"]["transport_diagnostics"]["attempts"][1]["status_code"],
            json!(500)
        );
    }

    #[tokio::test]
    async fn test_api_connection_gateway_failure_returns_python_aligned_guidance() {
        let db = setup_settings_db().await;

        let gateway_app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error": "bad gateway"})),
                )
            }),
        );

        let (gateway_base_url, gateway_handle) = spawn_chat_completion_server(gateway_app).await;

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("sk-test".to_string()),
                api_base_url: Some(gateway_base_url.clone()),
                provider: Some("openai_responses".to_string()),
                llm_model: Some("gpt-4.1-mini".to_string()),
                temperature: Some(0.4),
                max_tokens: Some(128),
                api_backup_urls: None,
                fallback_strategy: Some("auto".to_string()),
            }),
        )
        .await
        .expect("gateway error should still return failure shell");

        gateway_handle.abort();

        assert_eq!(response.0["success"], json!(false));
        assert_eq!(response.0["error_type"], json!("HTTPStatusError"));
        assert_eq!(response.0["details"]["http_status_code"], json!(502));
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["primary_endpoint"],
            json!(gateway_base_url)
        );
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["auto_failover_enabled"],
            json!(false)
        );
        let suggestions = response.0["suggestions"]
            .as_array()
            .expect("suggestions should be an array");
        assert!(suggestions.iter().any(|item| {
            item.as_str()
                .map(|value| {
                    value
                        .to_ascii_lowercase()
                        .contains("local gateway or proxy")
                })
                .unwrap_or(false)
        }));
        assert!(suggestions.iter().any(|item| {
            item.as_str()
                .map(|value| value.to_ascii_lowercase().contains("backup endpoint"))
                .unwrap_or(false)
        }));
    }

    #[tokio::test]
    async fn test_api_connection_anthropic_gateway_failure_keeps_structured_status_code() {
        let db = setup_settings_db().await;

        let gateway_app = Router::new().route(
            "/v1/messages",
            post(|| async {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error": {"message": "bad gateway"}})),
                )
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let address = listener.local_addr().expect("listener address");
        let gateway_handle = tokio::spawn(async move {
            axum::serve(listener, gateway_app)
                .await
                .expect("serve test app");
        });
        let gateway_base_url = format!("http://{address}/v1");

        let response = test_api_connection(
            Extension(test_claims()),
            Extension(db),
            Json(TestConnectionRequest {
                api_key: Some("ak-test".to_string()),
                api_base_url: Some(gateway_base_url.clone()),
                provider: Some("anthropic".to_string()),
                llm_model: Some("claude-3-5-sonnet-latest".to_string()),
                temperature: Some(0.4),
                max_tokens: Some(128),
                api_backup_urls: None,
                fallback_strategy: Some("auto".to_string()),
            }),
        )
        .await
        .expect("gateway error should still return failure shell");

        gateway_handle.abort();

        assert_eq!(response.0["success"], json!(false));
        assert_eq!(response.0["error_type"], json!("HTTPStatusError"));
        assert_eq!(response.0["details"]["http_status_code"], json!(502));
        assert_eq!(
            response.0["details"]["endpoint_diagnostics"]["primary_endpoint"],
            json!(gateway_base_url)
        );
        assert!(response.0["error"]
            .as_str()
            .map(|value| value.contains("Anthropic HTTP 502"))
            .unwrap_or(false));
    }

    #[test]
    fn build_api_probe_failure_suggestions_prefers_structured_status_code_over_message_parsing() {
        let suggestions = build_api_probe_failure_suggestions(
            "gateway failed without numeric status in message",
            "http://127.0.0.1:8317/v1",
            None,
            Some("auto"),
            Some(502),
        );

        assert!(suggestions
            .iter()
            .any(|item| { item.contains("local gateway or proxy") }));
        assert!(suggestions
            .iter()
            .any(|item| { item.to_ascii_lowercase().contains("backup endpoint") }));
    }

    #[test]
    fn host_docker_timeout_guidance_matches_python_contract() {
        let suggestions = build_api_probe_failure_suggestions(
            "ReadTimeout upstream timeout for http://host.docker.internal:8317/v1/chat/completions",
            "http://host.docker.internal:8317/v1",
            None,
            Some("auto"),
            None,
        );

        assert!(suggestions
            .iter()
            .any(|item: &String| item.contains("host.docker.internal")));
        assert!(suggestions
            .iter()
            .any(|item: &String| item.contains("127.0.0.1")));
        assert!(suggestions
            .iter()
            .any(|item: &String| { item.to_ascii_lowercase().contains("backup endpoint") }));
    }
}
