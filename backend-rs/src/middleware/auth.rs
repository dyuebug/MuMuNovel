use std::{
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tower::{Layer, Service};

use crate::services::auth::AuthService;

#[derive(Clone)]
pub struct AuthLayer {
    jwt_secret: String,
}

impl AuthLayer {
    pub fn new(jwt_secret: &str) -> Self {
        Self {
            jwt_secret: jwt_secret.to_string(),
        }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, service: S) -> Self::Service {
        AuthMiddleware {
            service,
            jwt_secret: self.jwt_secret.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AuthMiddleware<S> {
    service: S,
    jwt_secret: String,
}

const EXACT_PUBLIC_PATHS: &[&str] = &[
    "/health",
    "/livez",
    "/readyz",
    "/health/db-sessions",
    "/health/chapter-candidate-route-gateway-smoke",
    "/health/chapter-single-generation-active-gateway-smoke",
    "/health/chapter-batch-generation-active-gateway-smoke",
    "/health/chapter-regeneration-stream-workflow-smoke",
    "/docs",
    "/redoc",
    "/openapi.json",
    "/api/auth/login",
    "/api/auth/local/login",
    "/api/auth/bind/login",
    "/api/auth/linuxdo/url",
    "/api/auth/linuxdo/callback",
    "/api/auth/callback",
    "/api/auth/register",
    "/api/auth/logout",
    "/api/auth/config",
    "/api/changelog",
    "/api/changelog/refresh",
    "/api/characters/validate-import",
    "/api/projects/validate-import",
];

const PUBLIC_PATH_PREFIXES: &[&str] = &["/assets"];

fn is_public(path: &str) -> bool {
    EXACT_PUBLIC_PATHS.contains(&path)
        || PUBLIC_PATH_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix))
}

impl<S, ReqBody> Service<Request<ReqBody>> for AuthMiddleware<S>
where
    S: Service<Request<ReqBody>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future =
        Pin<Box<dyn Send + std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let path = req.uri().path().to_string();

        if is_public(&path) {
            let mut service = self.service.clone();
            return Box::pin(async move { service.call(req).await });
        }

        let token = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .or_else(|| {
                req.headers()
                    .get("Cookie")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|cookies| {
                        cookies.split(';').find_map(|c| {
                            let (k, v) = c.trim().split_once('=')?;
                            if k == "token" {
                                Some(v)
                            } else {
                                None
                            }
                        })
                    })
            })
            .map(|s| s.to_string());

        let mut service = self.service.clone();
        let jwt_secret = self.jwt_secret.clone();

        Box::pin(async move {
            match token {
                Some(t) => {
                    let auth = AuthService::new(&jwt_secret);
                    match auth.verify_token(&t) {
                        Ok(claims) => {
                            let mut req = req;
                            req.extensions_mut().insert(claims);
                            service.call(req).await
                        }
                        Err(_) => {
                            let body = json!({"detail": "Token无效或已过期，请重新登录"});
                            Ok((StatusCode::UNAUTHORIZED, axum::Json(body)).into_response())
                        }
                    }
                }
                None => {
                    let body = json!({"detail": "未登录，请先登录"});
                    Ok((StatusCode::UNAUTHORIZED, axum::Json(body)).into_response())
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::is_public;

    #[test]
    fn exact_public_paths_remain_public() {
        for path in [
            "/health",
            "/readyz",
            "/health/chapter-candidate-route-gateway-smoke",
            "/health/chapter-single-generation-active-gateway-smoke",
            "/health/chapter-batch-generation-active-gateway-smoke",
            "/health/chapter-regeneration-stream-workflow-smoke",
            "/api/auth/login",
            "/api/auth/callback",
            "/api/changelog/refresh",
            "/api/characters/validate-import",
            "/api/projects/validate-import",
        ] {
            assert!(is_public(path), "expected public path: {}", path);
        }
    }

    #[test]
    fn asset_prefix_remains_public() {
        assert!(is_public("/assets/app.js"));
        assert!(is_public("/assets/nested/chunk.css"));
    }

    #[test]
    fn protected_paths_remain_protected() {
        for path in ["/api/projects", "/api/settings", "/api/auth/user", "/"] {
            assert!(!is_public(path), "expected protected path: {}", path);
        }
    }
}
