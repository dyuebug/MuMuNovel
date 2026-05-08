use std::path::Path;
use std::sync::Arc;

use axum::{Extension, Router};
use sea_orm::DatabaseConnection;
use tower_http::{
    cors::CorsLayer, normalize_path::NormalizePathLayer, request_id::MakeRequestUuid,
    services::ServeDir, trace::TraceLayer, ServiceBuilderExt as _,
};
use tracing::info;

use crate::config::AppConfig;
use crate::mcp::McpClientManager;
use crate::middleware::auth::AuthLayer;
use crate::services::book_import_service::BookImportService;
use crate::tasks::registry::TaskRegistry;
use crate::tasks::stream::TaskStreamHub;

use super::{
    admin, ai_test, auth, background_tasks, book_import, careers, changelog, chapters, characters,
    foreshadows, health, inspiration, mcp_plugins, organizations, outlines, polish, projects,
    prompt_templates, prompt_workshop, relationships, settings, users, wizard, writing_styles,
};

pub fn build(
    db: Option<DatabaseConnection>,
    cfg: &AppConfig,
    task_registry: TaskRegistry,
) -> Router {
    let cors = if cfg.debug {
        CorsLayer::permissive()
    } else {
        CorsLayer::very_permissive()
    };

    let middleware_stack = tower::ServiceBuilder::new()
        .set_x_request_id(MakeRequestUuid)
        .layer(TraceLayer::new_for_http())
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
    router.layer(NormalizePathLayer::trim_trailing_slash())
}
