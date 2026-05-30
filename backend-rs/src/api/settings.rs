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
use crate::services::settings_api_key_payload_adapter_service::build_stored_api_key_payload;
use crate::services::settings_models_payload_adapter_service::{
    build_available_models_fallback_payload, build_available_models_payload,
    build_fetch_models_failure_payload, build_fetch_models_fallback_payload,
    build_fetch_models_success_payload,
};
use crate::services::settings_preset_query_service::{find_preset_config, FindPresetConfigError};
use crate::services::settings_preset_request_service::{
    build_create_preset_from_current_request,
    build_create_settings_preset_request_from_typed_route_payload,
    build_update_settings_preset_request_from_typed_route_payload,
    CreatePresetFromCurrentRouteBody, CreatePresetFromCurrentRouteQuery,
    CreateSettingsPresetRouteRequest, UpdateSettingsPresetRouteRequest,
};
use crate::services::settings_runtime_config_service::{
    normalize_openai_compatible_base_url, resolve_effective_runtime_settings,
    EffectiveSettingsOverrides,
};
use crate::services::settings_service::SettingsService;
use crate::services::settings_test_preset_request_service::build_test_preset_connection_request;
use crate::services::settings_update_request_service::{
    build_settings_update_request_from_typed_route_payload, SettingsUpdateRouteRequest,
};

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

    match SettingsService::update(&db, &claims.sub, &request).await {
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

    match SettingsService::update(&db, &claims.sub, &request).await {
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
    let effective = resolve_effective_runtime_settings(
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

    match fetch_provider_models(
        &effective.provider,
        &effective.api_key,
        &effective.base_url,
        None,
    )
    .await
    {
        Ok(models) => Ok(Json(build_available_models_payload(
            &effective.provider,
            models,
        ))),
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
    let effective = resolve_effective_runtime_settings(
        &db,
        &claims.sub,
        EffectiveSettingsOverrides {
            provider: body.provider.clone(),
            api_key: body.api_key.clone(),
            api_base_url: body.api_base_url.clone(),
            model: body.llm_model.clone(),
            temperature: body.temperature,
            max_tokens: body.max_tokens,
        },
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
    let effective = resolve_effective_runtime_settings(
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
    let effective = resolve_effective_runtime_settings(
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
    Json(body): Json<CreateSettingsPresetRouteRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let request = build_create_settings_preset_request_from_typed_route_payload(body);
    match SettingsService::create_preset(&db, &claims.sub, &request).await {
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
    Json(body): Json<UpdateSettingsPresetRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_update_settings_preset_request_from_typed_route_payload(body);
    match SettingsService::update_preset(&db, &claims.sub, &preset_id, &request).await {
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

    let config = find_preset_config(settings.preferences.as_deref(), &preset_id).map_err(
        |error| match error {
            FindPresetConfigError::PresetNotFound => (
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "Preset not found"})),
            ),
        },
    )?;
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
        }),
    )
    .await
}

async fn create_preset_from_current(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Query(query): Query<CreatePresetFromCurrentRouteQuery>,
    body: Option<Json<CreatePresetFromCurrentRouteBody>>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let request = build_create_preset_from_current_request(query, body.map(|Json(value)| value));
    match SettingsService::create_preset_from_current(
        &db,
        &claims.sub,
        &request.name,
        request.description.as_deref(),
    )
    .await
    {
        Ok(result) => Ok((StatusCode::CREATED, Json(result))),
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
