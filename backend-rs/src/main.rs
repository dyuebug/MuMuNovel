pub mod ai;
pub mod api;
pub mod config;
pub mod db;
pub mod mcp;
pub mod middleware;
pub mod models;
pub mod services;
pub mod tasks;
pub mod utils;

use std::net::SocketAddr;

use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = config::load();
    let db = db::init_pool(&cfg).await;

    if cfg.enable_startup_schema_sync {
        tracing::warn!(
            "ENABLE_STARTUP_SCHEMA_SYNC is enabled, but startup schema sync has been disabled for strangler deployments. Use the explicit migration step instead."
        );
    }

    // Initialize background task system
    let task_registry = tasks::registry::TaskRegistry::new();
    tasks::persistence::load_from_disk(&task_registry).await;
    tasks::recovery::recover_orphan_tasks(&task_registry).await;
    tasks::persistence::start_periodic_save(task_registry.clone());
    start_periodic_cleanup(task_registry.clone());

    let app = api::router::build(db, &cfg, task_registry);

    let addr: SocketAddr = format!("{}:{}", cfg.app_host, cfg.app_port)
        .parse()
        .expect("invalid bind address");

    info!("MuMuNovel Rust backend starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

fn start_periodic_cleanup(registry: tasks::registry::TaskRegistry) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            registry.prune_old_tasks().await;
        }
    });
}
