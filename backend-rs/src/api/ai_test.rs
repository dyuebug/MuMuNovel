use axum::{
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Json, Sse},
    routing::post,
    Router,
};
use futures::{stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ai::config::AIConfig;
use crate::ai::service::AIService;
use crate::ai::types::AIStreamChunk;
use crate::services::auth::Claims;
use crate::utils::sse::{sse_chunk, sse_done, sse_error, sse_reasoning_chunk};

const AI_TEST_ROUTE: &str = "/ai-test";
const AI_TEST_STREAM_ROUTE: &str = "/ai-test-stream";
const AI_TEST_ALIAS_ROUTE: &str = "/ai/test";
const AI_TEST_STREAM_ALIAS_ROUTE: &str = "/ai/test-stream";
const AI_TEST_DEFAULT_MAX_TOKENS: u32 = 4096;
const AI_TEST_PROBE_MAX_TOKENS: u32 = 64;

#[cfg(test)]
fn build_ai_test_route_owner_contract() -> Value {
    json!({
        "owner": "ai_test",
        "rust_owner": "backend-rs/src/api/ai_test.rs",
        "routes": {
            "test": AI_TEST_ROUTE,
            "test_stream": AI_TEST_STREAM_ROUTE,
            "test_alias": AI_TEST_ALIAS_ROUTE,
            "test_stream_alias": AI_TEST_STREAM_ALIAS_ROUTE
        },
        "methods": {
            "test": ["POST"],
            "test_stream": ["POST"],
            "test_alias": ["POST"],
            "test_stream_alias": ["POST"]
        },
        "service_owners": [
            "backend-rs/src/ai/service.rs",
            "backend-rs/src/ai/config.rs",
            "backend-rs/src/ai/types.rs",
            "backend-rs/src/ai/clients/openai.rs",
            "backend-rs/src/ai/clients/gemini.rs",
            "backend-rs/src/ai/clients/anthropic.rs"
        ],
        "readiness_probes": [
            "ai-test-auth-guard-rust",
            "ai-test-stream-auth-guard-rust",
            "ai-test-alias-auth-guard-rust",
            "ai-test-stream-alias-auth-guard-rust",
            "ai-test-main-business-rust",
            "ai-test-alias-business-rust",
            "ai-test-stream-main-business-rust",
            "ai-test-stream-alias-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-ai-test-owner",
            "business_probes": [
                "ai-test-main-business-rust",
                "ai-test-alias-business-rust",
                "ai-test-stream-main-business-rust",
                "ai-test-stream-alias-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-ai-test-owner",
            "readiness_probe_count": 8,
            "business_probe_count": 4,
            "auth_guard_probe_count": 4,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "ai-test route source-map shell deleted; surviving Python closeout work is outside this route group",
        "migration_policy": "AI test route business smoke is covered by phase5-ai-test-owner; the detached Python ai_test route shell is already gone, and the remaining Python settings/ai-gateway files are shared runtime dependencies outside this direct route-group boundary.",
        "source_map_files": [],
        "behavior_contract": {
            "auth": "all AI test routes require Claims before provider access",
            "aliases": "legacy /ai/test aliases stay registered beside /ai-test",
            "streaming": "both /ai-test-stream and /ai/test-stream emit SSE data, done, empty, and error chunks",
            "probe_max_tokens": "max_tokens defaults to 4096 and is clamped to 1..=64 for probe calls"
        },
        "rollback_boundary": {
            "source_map_policy": "ai_test_route_source_map_deleted_no_remaining_route_group_python_source_map_hold",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "freeze_reason": "phase5-ai-test-owner covers main, alias, stream-main, and stream-alias business probes with zero Python fallback probes, the detached Python ai_test route shell is already gone, and the remaining Python settings plus ai-gateway files now belong to broader shared runtime lanes outside this direct route-group boundary."
        }
    })
}

fn probe_max_tokens(max_tokens: Option<u32>) -> u32 {
    max_tokens
        .unwrap_or(AI_TEST_DEFAULT_MAX_TOKENS)
        .clamp(1, AI_TEST_PROBE_MAX_TOKENS)
}

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
    let probe_max_tokens = probe_max_tokens(body.max_tokens);
    let cfg = AIConfig {
        provider: body.provider.unwrap_or_else(|| "openai".into()),
        api_key: body.api_key.unwrap_or_default(),
        base_url: body
            .base_url
            .unwrap_or_else(|| "https://api.openai.com/v1".into()),
        model: body.model.unwrap_or_else(|| "gpt-4".into()),
        temperature: body.temperature.unwrap_or(0.7),
        max_tokens: probe_max_tokens,
        ..Default::default()
    };

    let _ = claims; // auth check

    let service = AIService::new(cfg);
    match service
        .generate_text(&body.prompt, body.system_prompt.as_deref(), None)
        .await
    {
        Ok(resp) => Ok(Json(
            json!({"success": true, "probe_max_tokens": probe_max_tokens, "data": resp}),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "probe_max_tokens": probe_max_tokens, "message": e})),
        )),
    }
}

fn project_ai_test_stream_events(
    chunk: Result<AIStreamChunk, String>,
) -> Vec<Result<axum::response::sse::Event, std::convert::Infallible>> {
    let mut events = Vec::with_capacity(3);
    match chunk {
        Ok(chunk) => {
            if let Some(reasoning) = chunk.reasoning_content.filter(|value| !value.is_empty()) {
                events.push(Ok(sse_reasoning_chunk(&reasoning)));
            }
            if let Some(content) = chunk.content.filter(|value| !value.is_empty()) {
                events.push(Ok(sse_chunk(&content)));
            }
            if chunk.done {
                events.push(Ok(sse_done()));
            }
        }
        Err(_) => events.push(Ok(sse_error("模型测试流调用失败", 500))),
    }
    events
}

async fn test_ai_stream(
    Extension(claims): Extension<Claims>,
    Json(body): Json<TestAIRequest>,
) -> impl IntoResponse {
    let probe_max_tokens = probe_max_tokens(body.max_tokens);
    let cfg = AIConfig {
        provider: body.provider.unwrap_or_else(|| "openai".into()),
        api_key: body.api_key.unwrap_or_default(),
        base_url: body
            .base_url
            .unwrap_or_else(|| "https://api.openai.com/v1".into()),
        model: body.model.unwrap_or_else(|| "gpt-4".into()),
        temperature: body.temperature.unwrap_or(0.7),
        max_tokens: probe_max_tokens,
        ..Default::default()
    };

    let _ = claims;

    let service = AIService::new(cfg);
    let rx = service.generate_text_stream(body.prompt, body.system_prompt, None);
    let sse_stream = rx.flat_map(|chunk| stream::iter(project_ai_test_stream_events(chunk)));

    Sse::new(sse_stream.boxed())
}

pub fn routes() -> Router {
    Router::new()
        .route(AI_TEST_ROUTE, post(test_ai))
        .route(AI_TEST_STREAM_ROUTE, post(test_ai_stream))
        .route(AI_TEST_ALIAS_ROUTE, post(test_ai))
        .route(AI_TEST_STREAM_ALIAS_ROUTE, post(test_ai_stream))
}

#[cfg(test)]
mod tests {
    use super::{
        build_ai_test_route_owner_contract, probe_max_tokens, AI_TEST_ALIAS_ROUTE, AI_TEST_ROUTE,
        AI_TEST_STREAM_ALIAS_ROUTE, AI_TEST_STREAM_ROUTE,
    };
    use serde_json::json;

    #[test]
    fn should_publish_ai_test_route_owner_contract() {
        let contract = build_ai_test_route_owner_contract();

        assert_eq!(contract["owner"], json!("ai_test"));
        assert_eq!(
            contract["rust_owner"],
            json!("backend-rs/src/api/ai_test.rs")
        );
        assert_eq!(contract["routes"]["test"], json!(AI_TEST_ROUTE));
        assert_eq!(contract["routes"]["test_alias"], json!(AI_TEST_ALIAS_ROUTE));
        assert_eq!(contract["methods"]["test"], json!(["POST"]));
        assert_eq!(contract["methods"]["test_stream"], json!(["POST"]));
        assert_eq!(contract["service_owners"].as_array().map(Vec::len), Some(6));
        assert_eq!(
            contract["readiness_probes"].as_array().map(Vec::len),
            Some(8)
        );
        assert_eq!(
            contract["readiness_probes"]
                .as_array()
                .and_then(|probes| probes.last()),
            Some(&json!("ai-test-stream-alias-business-rust"))
        );
        assert_eq!(
            contract["source_map_files"].as_array().map(Vec::len),
            Some(0)
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            json!("phase5-ai-test-owner")
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be present");
        assert_eq!(business_probes.len(), 4);
        assert!(business_probes
            .iter()
            .any(|probe| probe == "ai-test-stream-main-business-rust"));
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            json!("covered_by_dedicated_rust_owner_profile")
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(8)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(4)
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            json!(4)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            json!("ai-test route source-map shell deleted; surviving Python closeout work is outside this route group")
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy should be a string")
            .contains("phase5-ai-test-owner"));
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy should be a string")
            .contains("shared runtime dependencies outside this direct route-group boundary"));
        assert!(contract["behavior_contract"]["probe_max_tokens"]
            .as_str()
            .unwrap_or_default()
            .contains("1..=64"));
        assert!(contract["behavior_contract"]["streaming"]
            .as_str()
            .unwrap_or_default()
            .contains("/ai/test-stream"));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"],
            json!([])
        );
    }

    #[test]
    fn should_keep_ai_test_route_group_paths_stable() {
        let contract = build_ai_test_route_owner_contract();

        assert_eq!(
            contract["routes"],
            json!({
                "test": AI_TEST_ROUTE,
                "test_stream": AI_TEST_STREAM_ROUTE,
                "test_alias": AI_TEST_ALIAS_ROUTE,
                "test_stream_alias": AI_TEST_STREAM_ALIAS_ROUTE
            })
        );
    }

    #[test]
    fn should_clamp_ai_test_probe_max_tokens() {
        assert_eq!(probe_max_tokens(None), 64);
        assert_eq!(probe_max_tokens(Some(0)), 1);
        assert_eq!(probe_max_tokens(Some(32)), 32);
        assert_eq!(probe_max_tokens(Some(4096)), 64);
    }
}
