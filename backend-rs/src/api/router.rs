use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use axum::http::HeaderValue;
use axum::{Extension, Router};
use sea_orm::DatabaseConnection;
use tower_http::{
    cors::{AllowHeaders, AllowMethods, CorsLayer},
    normalize_path::NormalizePathLayer,
    request_id::MakeRequestUuid,
    services::ServeDir,
    trace::TraceLayer,
    ServiceBuilderExt as _,
};
use tracing::{info, warn, Level};
use url::Url;

use crate::config::{AppConfig, AppRuntimeMode};
use crate::mcp::McpClientManager;
use crate::middleware::auth::AuthLayer;
use crate::services::book_import_service::BookImportService;
use crate::tasks::registry::TaskRegistry;
use crate::tasks::stream::TaskStreamHub;

use super::{
    admin, ai_test, auth, background_tasks, book_import, careers, changelog, chapters, characters,
    foreshadows, health, inspiration, mcp_plugins, memories, organizations, outlines, polish,
    projects, prompt_templates, prompt_workshop, relationships, settings, users, wizard,
    writing_styles,
};

#[derive(Debug)]
pub enum RouterBuildError {
    EmptyCorsOrigins {
        mode: AppRuntimeMode,
    },
    WildcardCorsOriginsNotAllowed {
        mode: AppRuntimeMode,
    },
    InvalidCorsOrigin {
        origin: String,
        reason: &'static str,
    },
}

impl fmt::Display for RouterBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCorsOrigins { mode } => write!(
                f,
                "CORS_ORIGINS is required when runtime mode is {}",
                mode.as_str()
            ),
            Self::WildcardCorsOriginsNotAllowed { mode } => write!(
                f,
                "CORS_ORIGINS='*' is only allowed in development mode; runtime mode {} requires explicit origins for credentialed requests",
                mode.as_str()
            ),
            Self::InvalidCorsOrigin { origin, reason } => {
                write!(f, "invalid CORS origin '{}': {}", origin, reason)
            }
        }
    }
}

impl Error for RouterBuildError {}

#[derive(Debug)]
enum CorsPolicy {
    DevelopmentPermissive,
    Explicit(Vec<HeaderValue>),
}

fn normalize_origin(origin: &str) -> Result<HeaderValue, RouterBuildError> {
    let parsed = Url::parse(origin).map_err(|_| RouterBuildError::InvalidCorsOrigin {
        origin: origin.to_string(),
        reason: "must be a valid absolute http(s) origin",
    })?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(RouterBuildError::InvalidCorsOrigin {
            origin: origin.to_string(),
            reason: "scheme must be http or https",
        });
    }

    if parsed.host_str().is_none() {
        return Err(RouterBuildError::InvalidCorsOrigin {
            origin: origin.to_string(),
            reason: "host is required",
        });
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(RouterBuildError::InvalidCorsOrigin {
            origin: origin.to_string(),
            reason: "userinfo is not allowed in origins",
        });
    }

    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(RouterBuildError::InvalidCorsOrigin {
            origin: origin.to_string(),
            reason: "query and fragment are not allowed in origins",
        });
    }

    if parsed.path() != "/" {
        return Err(RouterBuildError::InvalidCorsOrigin {
            origin: origin.to_string(),
            reason: "path segments are not allowed in origins",
        });
    }

    HeaderValue::from_str(&parsed.origin().ascii_serialization()).map_err(|_| {
        RouterBuildError::InvalidCorsOrigin {
            origin: origin.to_string(),
            reason: "origin could not be encoded as an HTTP header value",
        }
    })
}

fn resolve_cors_policy(cfg: &AppConfig) -> Result<CorsPolicy, RouterBuildError> {
    let raw = cfg.cors_origins.trim();
    if raw.is_empty() {
        return Err(RouterBuildError::EmptyCorsOrigins {
            mode: cfg.runtime_mode,
        });
    }

    if raw == "*" {
        if cfg.runtime_mode.is_development() {
            warn!(
                "CORS_ORIGINS='*' enabled in development mode; using permissive credentialed CORS for local workflows"
            );
            return Ok(CorsPolicy::DevelopmentPermissive);
        }

        return Err(RouterBuildError::WildcardCorsOriginsNotAllowed {
            mode: cfg.runtime_mode,
        });
    }

    let mut origins = Vec::new();
    for part in raw.split(',') {
        let origin = part.trim();
        if origin.is_empty() {
            return Err(RouterBuildError::InvalidCorsOrigin {
                origin: raw.to_string(),
                reason: "origin list contains an empty entry",
            });
        }
        if origin == "*" {
            return Err(RouterBuildError::InvalidCorsOrigin {
                origin: raw.to_string(),
                reason: "wildcard must be the only CORS_ORIGINS value",
            });
        }

        let header = normalize_origin(origin)?;
        if !origins.contains(&header) {
            origins.push(header);
        }
    }

    if origins.is_empty() {
        return Err(RouterBuildError::EmptyCorsOrigins {
            mode: cfg.runtime_mode,
        });
    }

    Ok(CorsPolicy::Explicit(origins))
}

fn build_cors_layer(cfg: &AppConfig) -> Result<CorsLayer, RouterBuildError> {
    match resolve_cors_policy(cfg)? {
        CorsPolicy::DevelopmentPermissive => Ok(CorsLayer::very_permissive()),
        CorsPolicy::Explicit(origins) => {
            info!(
                "CORS allowlist configured with {} explicit origin(s) in {} mode",
                origins.len(),
                cfg.runtime_mode.as_str()
            );
            Ok(CorsLayer::new()
                .allow_credentials(true)
                .allow_headers(AllowHeaders::mirror_request())
                .allow_methods(AllowMethods::mirror_request())
                .allow_origin(origins))
        }
    }
}

pub fn build(
    db: Option<DatabaseConnection>,
    cfg: &AppConfig,
    task_registry: TaskRegistry,
) -> Result<Router, RouterBuildError> {
    let cors = build_cors_layer(cfg)?;

    let middleware_stack = tower::ServiceBuilder::new()
        .set_x_request_id(MakeRequestUuid)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    tracing::span!(
                        Level::INFO,
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                    )
                })
                .on_response(
                    |response: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     _span: &tracing::Span| {
                        tracing::info!(
                            status = %response.status(),
                            latency_ms = latency.as_millis(),
                            "request completed"
                        );
                    },
                ),
        )
        .layer(cors)
        .layer(AuthLayer::new(&cfg.jwt_secret));

    let task_stream_hub = TaskStreamHub::new();
    let book_import_service = Arc::new(BookImportService::new());

    let api_routes = Router::new()
        .merge(auth::routes())
        .merge(users::routes())
        .merge(projects::routes())
        .merge(outlines::routes())
        .merge(characters::routes())
        .merge(careers::routes())
        .merge(organizations::routes())
        .merge(relationships::routes())
        .merge(chapters::routes())
        .merge(settings::routes())
        .merge(writing_styles::routes())
        .merge(foreshadows::routes())
        .merge(admin::routes())
        .merge(changelog::routes())
        .merge(ai_test::routes())
        .merge(background_tasks::routes())
        .merge(prompt_templates::routes())
        .merge(prompt_workshop::routes())
        .merge(mcp_plugins::routes())
        .merge(memories::routes())
        .merge(book_import::routes())
        .merge(polish::routes())
        .merge(inspiration::routes())
        .merge(wizard::routes())
        .layer(Extension(task_registry))
        .layer(Extension(task_stream_hub))
        .layer(Extension(book_import_service))
        .layer(Extension(Arc::new(McpClientManager::new())));

    let mut router = Router::new()
        .merge(health::routes())
        .nest("/api", api_routes)
        .layer(Extension(cfg.clone()))
        .layer(middleware_stack);

    // Static file serving + SPA fallback (matching Python backend behavior)
    let static_dir = Path::new(&cfg.static_dir);
    if static_dir.exists() {
        let assets_dir = static_dir.join("assets");
        let index_path = static_dir.join("index.html");

        if assets_dir.exists() {
            router = router.nest_service("/assets", ServeDir::new(assets_dir));
        }

        if index_path.exists() {
            let index_html = std::fs::read_to_string(&index_path).unwrap_or_default();
            let static_dir_clone = static_dir.to_path_buf();
            router = router.fallback_service(tower::service_fn(
                move |req: axum::http::Request<axum::body::Body>| {
                    let path = req.uri().path().trim_start_matches('/').to_string();
                    let static_dir = static_dir_clone.clone();
                    let index_html = index_html.clone();
                    async move {
                        // API paths that don't match a route should return 404, not SPA HTML
                        if path.starts_with("api/") {
                            return Ok::<_, std::convert::Infallible>(
                                axum::response::Response::builder()
                                    .status(404)
                                    .header("content-type", "application/json")
                                    .body(axum::body::Body::from(r#"{"detail":"Not Found"}"#))
                                    .unwrap(),
                            );
                        }

                        // Try to serve the exact file if it exists
                        let file_path = static_dir.join(path);
                        if file_path.exists() && file_path.is_file() {
                            let content_type = mime_guess::from_path(&file_path)
                                .first_or_octet_stream()
                                .to_string();
                            match tokio::fs::read(&file_path).await {
                                Ok(data) => {
                                    return Ok::<_, std::convert::Infallible>(
                                        axum::response::Response::builder()
                                            .status(200)
                                            .header("content-type", content_type)
                                            .body(axum::body::Body::from(data))
                                            .unwrap(),
                                    );
                                }
                                Err(_) => {}
                            }
                        }
                        // SPA fallback: serve index.html
                        Ok::<_, std::convert::Infallible>(
                            axum::response::Response::builder()
                                .status(200)
                                .header("content-type", "text/html; charset=utf-8")
                                .body(axum::body::Body::from(index_html))
                                .unwrap(),
                        )
                    }
                },
            ));
            info!("Static file serving enabled from {}", cfg.static_dir);
        }
    } else {
        info!("Static dir {} not found, running API-only", cfg.static_dir);
    }

    if let Some(db) = db {
        router = router.layer(Extension(db));
    }

    // Strip trailing slashes so /api/settings/ matches /api/settings
    Ok(router.layer(NormalizePathLayer::trim_trailing_slash()))
}

#[cfg(test)]
mod tests {
    use super::{resolve_cors_policy, CorsPolicy, RouterBuildError};
    use crate::config::{AppConfig, AppRuntimeMode};

    fn test_config(mode: AppRuntimeMode, cors_origins: &str) -> AppConfig {
        AppConfig {
            app_host: "127.0.0.1".to_string(),
            app_port: 8001,
            app_name: "MuMuNovel".to_string(),
            app_version: "0.1.0-rs".to_string(),
            database_url: "sqlite::memory:".to_string(),
            database_pool_size: 50,
            enable_startup_schema_sync: false,
            log_level: "info".to_string(),
            debug: mode.is_development(),
            runtime_mode: mode,
            cors_origins: cors_origins.to_string(),
            jwt_secret: "secret".to_string(),
            static_dir: "../backend/static".to_string(),
            local_auth_enabled: true,
            local_auth_username: String::new(),
            local_auth_password: String::new(),
            local_auth_display_name: "本地管理员".to_string(),
            linuxdo_client_id: String::new(),
            linuxdo_client_secret: String::new(),
            linuxdo_redirect_uri: String::new(),
            frontend_url: "http://localhost".to_string(),
            session_expire_minutes: 120,
            session_refresh_threshold_minutes: 30,
        }
    }

    #[test]
    fn development_mode_allows_wildcard_cors() {
        let cfg = test_config(AppRuntimeMode::Development, "*");

        let policy = resolve_cors_policy(&cfg).expect("development wildcard should be allowed");

        assert!(matches!(policy, CorsPolicy::DevelopmentPermissive));
    }

    #[test]
    fn non_development_rejects_wildcard_cors() {
        let cfg = test_config(AppRuntimeMode::NonDevelopment, "*");

        let err =
            resolve_cors_policy(&cfg).expect_err("non-development wildcard should be rejected");

        assert!(matches!(
            err,
            RouterBuildError::WildcardCorsOriginsNotAllowed { .. }
        ));
    }

    #[test]
    fn explicit_origins_are_normalized_and_deduplicated() {
        let cfg = test_config(
            AppRuntimeMode::NonDevelopment,
            "http://localhost:3000/, http://localhost:3000, https://example.com",
        );

        let policy = resolve_cors_policy(&cfg).expect("explicit origins should parse");

        match policy {
            CorsPolicy::Explicit(origins) => {
                let origins: Vec<_> = origins
                    .into_iter()
                    .map(|value| value.to_str().unwrap().to_string())
                    .collect();
                assert_eq!(
                    origins,
                    vec![
                        "http://localhost:3000".to_string(),
                        "https://example.com".to_string()
                    ]
                );
            }
            CorsPolicy::DevelopmentPermissive => {
                panic!("explicit non-development origins should not become permissive")
            }
        }
    }

    #[test]
    fn explicit_non_development_origins_build_usable_cors_layer() {
        let cfg = test_config(
            AppRuntimeMode::NonDevelopment,
            "http://localhost:8005, http://127.0.0.1:8005",
        );

        let result = std::panic::catch_unwind(|| super::build_cors_layer(&cfg));

        assert!(result.is_ok(), "explicit CORS config should not panic");
        assert!(result.unwrap().is_ok(), "explicit CORS config should build");
    }

    #[test]
    fn non_development_rejects_origin_with_path_segments() {
        let cfg = test_config(AppRuntimeMode::NonDevelopment, "https://example.com/app");

        let err = resolve_cors_policy(&cfg)
            .expect_err("origin path segments should be rejected for CORS");

        assert!(matches!(err, RouterBuildError::InvalidCorsOrigin { .. }));
    }
}
