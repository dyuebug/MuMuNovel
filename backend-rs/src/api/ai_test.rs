use axum::{
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Json, Sse},
    routing::post,
    Router,
};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ai::config::AIConfig;
use crate::ai::service::AIService;
use crate::ai::types::AIStreamChunk;
use crate::services::auth::Claims;

#[derive(Deserialize)]
#[allow(dead_code)]
struct TestAIRequest {
    prompt: String,
    system_prompt: Option<String>,
    provider: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    stream: Option<bool>,
}

async fn test_ai(
    Extension(claims): Extension<Claims>,
    Json(body): Json<TestAIRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let cfg = AIConfig {
        provider: body.provider.unwrap_or_else(|| "openai".into()),
        api_key: body.api_key.unwrap_or_default(),
        base_url: body.base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
        model: body.model.unwrap_or_else(|| "gpt-4".into()),
        temperature: body.temperature.unwrap_or(0.7),
        max_tokens: body.max_tokens.unwrap_or(4096),
        ..Default::default()
    };

    let _ = claims; // auth check

    let service = AIService::new(cfg);
    match service
        .generate_text(&body.prompt, body.system_prompt.as_deref(), None)
        .await
    {
        Ok(resp) => Ok(Json(json!({"success": true, "data": resp}))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn test_ai_stream(
    Extension(claims): Extension<Claims>,
    Json(body): Json<TestAIRequest>,
) -> impl IntoResponse {
    let cfg = AIConfig {
        provider: body.provider.unwrap_or_else(|| "openai".into()),
        api_key: body.api_key.unwrap_or_default(),
        base_url: body.base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
        model: body.model.unwrap_or_else(|| "gpt-4".into()),
        temperature: body.temperature.unwrap_or(0.7),
        max_tokens: body.max_tokens.unwrap_or(4096),
        ..Default::default()
    };

    let _ = claims;

    let service = AIService::new(cfg);
    let rx = service.generate_text_stream(body.prompt, body.system_prompt, None);

    let sse_stream = rx.map(|chunk| {
        let event = match chunk {
            Ok(AIStreamChunk { content: Some(text), .. }) => {
                axum::response::sse::Event::default().data(text)
            }
            Ok(AIStreamChunk { done: true, finish_reason, .. }) => {
                let reason = finish_reason.unwrap_or_else(|| "stop".into());
                axum::response::sse::Event::default().data(format!("[DONE] {}", reason))
            }
            Ok(_) => axum::response::sse::Event::default().data(""),
            Err(e) => axum::response::sse::Event::default().data(format!("[ERROR] {}", e)),
        };
        Ok::<_, std::convert::Infallible>(event)
    });

    Sse::new(sse_stream.boxed())
}

pub fn routes() -> Router {
    Router::new()
        .route("/ai/test", post(test_ai))
        .route("/ai/test-stream", post(test_ai_stream))
}
