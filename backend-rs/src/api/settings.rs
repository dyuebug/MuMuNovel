use std::time::Instant;

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use reqwest::Client;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ai::config::AIConfig;
use crate::ai::service::AIService;
use crate::ai::types::{ToolDef, ToolFunction};
use crate::models::settings;
use crate::services::auth::Claims;
use crate::services::settings_service::SettingsService;

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
}

#[derive(Deserialize)]
struct FetchModelsRequest {
    api_key: Option<String>,
    api_base_url: Option<String>,
    provider: Option<String>,
    models_url: Option<String>,
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

#[derive(Deserialize, Default)]
struct CreatePresetFromCurrentQuery {
    name: Option<String>,
    description: Option<String>,
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
        Ok(Some(model)) => {
            let api_key = model.api_key.trim().to_string();
            Ok(Json(json!({
                "api_key": api_key,
                "has_api_key": !model.api_key.trim().is_empty()
            })))
        }
        Ok(None) => Ok(Json(json!({ "api_key": "", "has_api_key": false }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn create_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::update(&db, &claims.sub, &body).await {
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
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::update(&db, &claims.sub, &body).await {
        Ok(settings) => Ok(Json(settings)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn delete_settings(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::delete(&db, &claims.sub).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

async fn get_available_models(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Query(query): Query<ModelsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let effective = resolve_effective_settings(
        &db,
        &claims.sub,
        query.provider.as_deref(),
        query.api_key.clone(),
        query.api_base_url.clone(),
        None,
        None,
        None,
    )
    .await?;

    match fetch_provider_models(
        &effective.provider,
        &effective.api_key,
        &effective.base_url,
        None,
    )
    .await
    {
        Ok(models) => Ok(Json(json!({
            "provider": effective.provider,
            "models": models,
            "count": models.len()
        }))),
        Err(error) => {
            let fallback = curated_model_options(&effective.provider);
            if !fallback.is_empty() {
                Ok(Json(json!({
                    "provider": effective.provider,
                    "models": fallback,
                    "count": fallback.len(),
                    "message": format!("Model list fallback applied: {}", error),
                    "fallback_applied": true
                })))
            } else {
                let openai_fallback = curated_model_options("openai");
                Ok(Json(json!({
                    "provider": effective.provider,
                    "models": openai_fallback,
                    "count": openai_fallback.len(),
                    "message": format!("Model list fallback applied: {}", error),
                    "fallback_applied": true
                })))
            }
        }
    }
}

async fn test_api_connection(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<TestConnectionRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let effective = resolve_effective_settings(
        &db,
        &claims.sub,
        body.provider.as_deref(),
        body.api_key.clone(),
        body.api_base_url.clone(),
        body.llm_model.clone(),
        body.temperature,
        body.max_tokens,
    )
    .await?;

    let probe_max_tokens = effective.max_tokens.clamp(1, 64);
    let started = Instant::now();
    let service = AIService::new(AIConfig {
        provider: effective.provider.clone(),
        api_key: effective.api_key.clone(),
        base_url: effective.base_url.clone(),
        model: effective.model.clone(),
        temperature: effective.temperature,
        max_tokens: probe_max_tokens,
        ..Default::default()
    });

    match service
        .generate_text(
            "Please reply with the single word OK.",
            Some("You are an API connectivity probe."),
            None,
        )
        .await
    {
        Ok(response) => Ok(Json(json!({
            "success": true,
            "message": "API connection succeeded",
            "response_time_ms": started.elapsed().as_millis(),
            "provider": effective.provider,
            "model": effective.model,
            "probe_max_tokens": probe_max_tokens,
            "response_preview": response.content.chars().take(200).collect::<String>()
        }))),
        Err(error) => Ok(Json(json!({
            "success": false,
            "message": "API test failed",
            "response_time_ms": started.elapsed().as_millis(),
            "provider": effective.provider,
            "model": effective.model,
            "probe_max_tokens": probe_max_tokens,
            "error": error,
            "error_type": classify_error_type(&error),
            "suggestions": generic_suggestions("api_test")
        }))),
    }
}

async fn fetch_models_endpoint(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<FetchModelsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let effective = resolve_effective_settings(
        &db,
        &claims.sub,
        body.provider.as_deref(),
        body.api_key.clone(),
        body.api_base_url.clone(),
        None,
        None,
        None,
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
        Ok(models) => {
            let model_count = models.len();
            Ok(Json(json!({
            "success": true,
            "models": normalize_fetch_models_payload(models),
            "message": format!("Fetched {} models", model_count)
        })))
        }
        Err(error) => {
            let fallback = curated_fetch_models(&effective.provider);
            if !fallback.is_empty() {
                Ok(Json(json!({
                    "success": true,
                    "models": fallback,
                    "message": format!("Model list fallback applied: {}", error)
                })))
            } else {
                Ok(Json(json!({
                    "success": false,
                    "models": [],
                    "message": "Failed to fetch models",
                    "error": error,
                    "error_type": classify_error_type(&error)
                })))
            }
        }
    }
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
    let effective = resolve_effective_settings(
        &db,
        &claims.sub,
        body.provider.as_deref(),
        body.api_key.clone(),
        body.api_base_url.clone(),
        body.llm_model.clone(),
        Some(0.1),
        Some(512),
    )
    .await?;

    let started = Instant::now();
    let tools = vec![ToolDef {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "ping_tool".to_string(),
            description: "Return a test ping result.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            }),
        },
    }];

    let service = AIService::new(AIConfig {
        provider: effective.provider.clone(),
        api_key: effective.api_key.clone(),
        base_url: effective.base_url.clone(),
        model: effective.model.clone(),
        temperature: 0.1,
        max_tokens: 512,
        ..Default::default()
    });

    match service
        .generate_text(
            "Call the ping_tool function with message='ok'.",
            Some("You must call the provided function when available."),
            Some(&tools),
        )
        .await
    {
        Ok(response) => {
            let supported = response
                .tool_calls
                .as_ref()
                .map(|calls| !calls.is_empty())
                .unwrap_or(false);
            Ok(Json(json!({
                "success": supported,
                "supported": supported,
                "message": if supported { "Function calling is supported" } else { "Model did not return tool calls" },
                "response_time_ms": started.elapsed().as_millis(),
                "provider": effective.provider,
                "model": effective.model,
                "tool_calls": response.tool_calls,
                "response_preview": response.content.chars().take(200).collect::<String>()
            })))
        }
        Err(error) => Ok(Json(json!({
            "success": false,
            "supported": Value::Null,
            "message": "Unable to confirm function-calling support",
            "response_time_ms": started.elapsed().as_millis(),
            "provider": effective.provider,
            "model": effective.model,
            "error": error,
            "error_type": classify_error_type(&error),
            "suggestions": generic_suggestions("function_calling")
        }))),
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
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match SettingsService::create_preset(&db, &claims.sub, &body).await {
        Ok(result) => Ok((StatusCode::CREATED, Json(result))),
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
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match SettingsService::update_preset(&db, &claims.sub, &preset_id, &body).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            let detail = format!("{}", e);
            if detail.contains("not found") {
                Err((StatusCode::NOT_FOUND, Json(json!({"detail": detail}))))
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
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
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
                Err((StatusCode::NOT_FOUND, Json(json!({"detail": detail}))))
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
    let settings = load_settings_model(&db, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Settings not found"})),
        ))?;

    let preferences: Value = settings
        .preferences
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_else(|| json!({}));
    let presets = preferences
        .get("api_presets")
        .and_then(|value| value.get("presets"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let preset = presets
        .into_iter()
        .find(|preset| preset.get("id").and_then(Value::as_str) == Some(preset_id.as_str()))
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Preset not found"})),
        ))?;

    let config = preset.get("config").cloned().unwrap_or_else(|| json!({}));
    let request = TestConnectionRequest {
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
    };

    test_api_connection(Extension(claims), Extension(db), Json(request)).await
}

async fn create_preset_from_current(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Query(query): Query<CreatePresetFromCurrentQuery>,
    body: Option<Json<Value>>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let body = body.map(|Json(value)| value).unwrap_or_else(|| json!({}));
    let name = query
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            body.get("name")
                .and_then(|v| v.as_str())
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or("My Preset");
    let description = query
        .description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| body.get("description").and_then(|v| v.as_str()));
    match SettingsService::create_preset_from_current(&db, &claims.sub, name, description).await {
        Ok(result) => Ok((StatusCode::CREATED, Json(result))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )),
    }
}

#[derive(Clone)]
struct EffectiveSettings {
    provider: String,
    api_key: String,
    base_url: String,
    model: String,
    temperature: f64,
    max_tokens: u32,
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

async fn resolve_effective_settings(
    db: &DatabaseConnection,
    user_id: &str,
    provider: Option<&str>,
    api_key: Option<String>,
    api_base_url: Option<String>,
    model: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
) -> Result<EffectiveSettings, (StatusCode, Json<Value>)> {
    let stored = load_settings_model(db, user_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )
    })?;

    let stored_provider = stored
        .as_ref()
        .map(|s| s.provider_type.clone())
        .unwrap_or_else(|| "openai".to_string());
    let effective_provider = provider
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or(stored_provider);

    let stored_key = stored
        .as_ref()
        .map(|s| s.api_key.trim().to_string())
        .unwrap_or_default();
    let incoming_key = api_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let effective_key = incoming_key.unwrap_or(stored_key);
    if effective_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "API key is required"})),
        ));
    }

    let stored_base = stored
        .as_ref()
        .map(|s| s.api_base_url.trim().to_string())
        .unwrap_or_default();
    let raw_base = api_base_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(stored_base);
    let effective_base = resolve_provider_base_url(&effective_provider, &raw_base);

    let stored_model = stored
        .as_ref()
        .map(|s| s.llm_model.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_model_for_provider(&effective_provider));
    let effective_model = model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(stored_model);

    Ok(EffectiveSettings {
        provider: effective_provider.clone(),
        api_key: effective_key,
        base_url: effective_base,
        model: effective_model,
        temperature: temperature.unwrap_or(stored.as_ref().map(|s| s.temperature).unwrap_or(0.7)),
        max_tokens: max_tokens.unwrap_or(
            stored
                .as_ref()
                .map(|s| s.max_tokens as u32)
                .unwrap_or(32000),
        ),
    })
}

fn resolve_provider_base_url(provider: &str, raw_base_url: &str) -> String {
    match provider {
        "gemini" => normalize_gemini_base_url(raw_base_url),
        "anthropic" => normalize_anthropic_base_url(raw_base_url),
        _ => normalize_openai_compatible_base_url(raw_base_url),
    }
}

fn normalize_openai_compatible_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://api.openai.com/v1".to_string();
    }
    if let Ok(mut url) = reqwest::Url::parse(trimmed) {
        let path = url.path().trim_matches('/');
        if path.is_empty() {
            url.set_path("/v1");
            return url.to_string().trim_end_matches('/').to_string();
        }
    }
    trimmed.to_string()
}

fn normalize_anthropic_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://api.anthropic.com".to_string();
    }
    trimmed.to_string()
}

fn normalize_gemini_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://generativelanguage.googleapis.com/v1beta".to_string();
    }
    trimmed.to_string()
}

fn normalize_exa_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://api.exa.ai".to_string();
    }
    trimmed.to_string()
}

fn default_model_for_provider(provider: &str) -> String {
    match provider {
        "anthropic" => "claude-3-5-sonnet-latest".to_string(),
        "gemini" => "gemini-2.5-flash".to_string(),
        _ => "gpt-4o-mini".to_string(),
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

fn classify_error_type(error: &str) -> &'static str {
    let lowered = error.to_lowercase();
    if lowered.contains("401")
        || lowered.contains("403")
        || lowered.contains("unauthorized")
        || lowered.contains("forbidden")
    {
        "AuthenticationError"
    } else if lowered.contains("timeout") {
        "TimeoutError"
    } else if lowered.contains("404") {
        "EndpointNotFound"
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
        .route("/settings", get(get_settings))
        .route("/settings", post(create_settings))
        .route("/settings", put(update_settings))
        .route("/settings", delete(delete_settings))
        .route("/settings/api-key", get(get_stored_api_key))
        .route("/settings/models", get(get_available_models))
        .route("/settings/test", post(test_api_connection))
        .route("/settings/fetch-models", post(fetch_models_endpoint))
        .route(
            "/settings/test-web-research",
            post(test_web_research_connection),
        )
        .route(
            "/settings/check-function-calling",
            post(check_function_calling),
        )
        .route("/settings/presets", get(get_presets))
        .route("/settings/presets", post(create_preset))
        .route(
            "/settings/presets/from-current",
            post(create_preset_from_current),
        )
        .route("/settings/presets/{preset_id}", put(update_preset))
        .route("/settings/presets/{preset_id}", delete(delete_preset))
        .route(
            "/settings/presets/{preset_id}/activate",
            post(activate_preset),
        )
        .route("/settings/presets/{preset_id}/test", post(test_preset))
}
