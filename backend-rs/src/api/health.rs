use axum::{extract::Extension, http::StatusCode, response::Json, routing::get, Router};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

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

pub fn routes() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/livez", get(liveness_check))
        .route("/readyz", get(readiness_check))
        .route("/health/db-sessions", get(db_session_stats))
}
