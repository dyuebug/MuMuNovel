use axum::{extract::Extension, http::StatusCode, response::Json, routing::get, Router};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::chapter_candidate_route_gateway_smoke_service::{
    run_chapter_candidate_route_gateway_smoke_suite, ChapterCandidateRouteGatewaySmokeResult,
};
use crate::services::chapter_single_generation_active_gateway_smoke_service::{
    run_chapter_single_generation_active_gateway_smoke_suite,
    ChapterSingleGenerationActiveGatewaySmokeResult,
};

const CHAPTER_CANDIDATE_ROUTE_GATEWAY_SMOKE_ROUTE: &str =
    "/health/chapter-candidate-route-gateway-smoke";
const CHAPTER_SINGLE_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE: &str =
    "/health/chapter-single-generation-active-gateway-smoke";

async fn health_check() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn liveness_check() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn readiness_check(db: Option<Extension<DatabaseConnection>>) -> (StatusCode, Json<Value>) {
    let db_healthy = match db {
        Some(Extension(ref conn)) => conn.ping().await.is_ok(),
        None => false,
    };

    let database_status = json!({
        "healthy": db_healthy,
        "message": if db_healthy { "connected" } else { "unavailable" },
    });

    let startup_ready = true;
    let is_ready = startup_ready && db_healthy;

    let body = json!({
        "status": if is_ready { "ready" } else { "not_ready" },
        "checks": {
            "startup": {"ready": startup_ready},
            "database": database_status,
        },
    });

    let code = if is_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (code, Json(body))
}

async fn db_session_stats(db: Option<Extension<DatabaseConnection>>) -> Json<Value> {
    let healthy = match db {
        Some(Extension(ref conn)) => conn.ping().await.is_ok(),
        None => false,
    };

    Json(json!({
        "status": "ok",
        "session_stats": {
            "active": 0,
            "idle": 0,
            "total": 0,
        },
        "warning": if healthy { Value::Null } else { json!("database unavailable") },
    }))
}

async fn chapter_candidate_route_gateway_smoke() -> (StatusCode, Json<Value>) {
    let smoke_output = run_chapter_candidate_route_gateway_smoke_suite().await;

    match smoke_output {
        Ok(results) => {
            let probes = results.iter().map(smoke_result_payload).collect::<Vec<_>>();
            (
                StatusCode::OK,
                Json(json!({
                    "status": "ok",
                    "owner": "rust",
                    "route_group": "chapters",
                    "probe_count": probes.len(),
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "probes": probes,
                })),
            )
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "status": "failed",
                "owner": "rust",
                "route_group": "chapters",
                "error": error,
            })),
        ),
    }
}

async fn chapter_single_generation_active_gateway_smoke() -> (StatusCode, Json<Value>) {
    let smoke_output = run_chapter_single_generation_active_gateway_smoke_suite().await;

    match smoke_output {
        Ok(results) => {
            let probes = results
                .iter()
                .map(active_gateway_smoke_result_payload)
                .collect::<Vec<_>>();
            (
                StatusCode::OK,
                Json(json!({
                    "status": "ok",
                    "owner": "rust",
                    "route_group": "chapter_single_generation",
                    "probe_count": probes.len(),
                    "rollback_boundary": "legacy_single_generation_direct_ai",
                    "probes": probes,
                })),
            )
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "status": "failed",
                "owner": "rust",
                "route_group": "chapter_single_generation",
                "error": error,
            })),
        ),
    }
}

fn smoke_result_payload(result: &ChapterCandidateRouteGatewaySmokeResult) -> Value {
    json!({
        "name": &result.name,
        "owner": &result.owner,
        "route_group": &result.route_group,
        "ok": result.ok,
        "execution_path": &result.execution_path,
        "fallback_applied": result.fallback_applied,
        "reason": &result.reason,
        "rollback_boundary": &result.rollback_boundary,
        "rust_error": &result.rust_error,
        "result": &result.result,
        "runtime_state": &result.runtime_state,
    })
}

fn active_gateway_smoke_result_payload(
    result: &ChapterSingleGenerationActiveGatewaySmokeResult,
) -> Value {
    json!({
        "name": &result.name,
        "owner": &result.owner,
        "route_group": &result.route_group,
        "ok": result.ok,
        "execution_path": &result.execution_path,
        "fallback_applied": result.fallback_applied,
        "reason": &result.reason,
        "rollback_boundary": &result.rollback_boundary,
        "rust_error": &result.rust_error,
        "content": &result.content,
        "result": &result.result,
        "runtime_state": &result.runtime_state,
    })
}

pub fn routes() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/livez", get(liveness_check))
        .route("/readyz", get(readiness_check))
        .route("/health/db-sessions", get(db_session_stats))
        .route(
            CHAPTER_CANDIDATE_ROUTE_GATEWAY_SMOKE_ROUTE,
            get(chapter_candidate_route_gateway_smoke),
        )
        .route(
            CHAPTER_SINGLE_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE,
            get(chapter_single_generation_active_gateway_smoke),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        chapter_candidate_route_gateway_smoke, chapter_single_generation_active_gateway_smoke,
        CHAPTER_CANDIDATE_ROUTE_GATEWAY_SMOKE_ROUTE,
        CHAPTER_SINGLE_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE,
    };
    use axum::http::StatusCode;

    #[test]
    fn should_keep_chapter_candidate_route_gateway_smoke_route_public_path() {
        assert_eq!(
            CHAPTER_CANDIDATE_ROUTE_GATEWAY_SMOKE_ROUTE,
            "/health/chapter-candidate-route-gateway-smoke"
        );
        assert_eq!(
            CHAPTER_SINGLE_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE,
            "/health/chapter-single-generation-active-gateway-smoke"
        );
    }

    #[tokio::test]
    async fn should_expose_chapter_candidate_route_gateway_smoke_payload() {
        let (status, axum::Json(body)) = chapter_candidate_route_gateway_smoke().await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["owner"], "rust");
        assert_eq!(body["route_group"], "chapters");
        assert_eq!(body["probe_count"], 2);
        assert_eq!(
            body["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            body["probes"][0]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(body["probes"][0]["result"]["gateway_consumed"], true);
        assert_eq!(body["probes"][1]["execution_path"], "python_fallback");
        assert_eq!(body["probes"][1]["fallback_applied"], true);
    }

    #[tokio::test]
    async fn should_expose_chapter_single_generation_active_gateway_smoke_payload() {
        let (status, axum::Json(body)) = chapter_single_generation_active_gateway_smoke().await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["owner"], "rust");
        assert_eq!(body["route_group"], "chapter_single_generation");
        assert_eq!(body["probe_count"], 2);
        assert_eq!(
            body["rollback_boundary"],
            "legacy_single_generation_direct_ai"
        );
        assert_eq!(
            body["probes"][0]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(body["probes"][0]["result"]["gateway_consumed"], true);
        assert_eq!(body["probes"][0]["content"], "Rust 候选章节正文。");
        assert_eq!(body["probes"][1]["execution_path"], "python_fallback");
        assert_eq!(body["probes"][1]["fallback_applied"], true);
        assert_eq!(body["probes"][1]["content"], "直接生成回退章节正文。");
    }
}
