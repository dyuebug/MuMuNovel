#![recursion_limit = "256"]

mod ai;
mod api;
mod config;
mod db;
mod mcp;
mod middleware;
mod models;
mod services;
mod tasks;
mod utils;

use std::net::SocketAddr;
use std::process::exit;

use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::error!("Startup configuration error: {}", err);
            exit(1);
        }
    };
    cfg.log_startup_contract();
    let db = db::init_pool(&cfg).await;

    // Initialize background task system
    let task_registry = tasks::registry::TaskRegistry::new();
    tasks::persistence::load_from_disk(&task_registry).await;
    tasks::recovery::recover_orphan_tasks(&task_registry).await;
    tasks::persistence::start_periodic_save(task_registry.clone());
    start_periodic_cleanup(task_registry.clone());

    let app = match api::router::build(db, &cfg, task_registry) {
        Ok(app) => app,
        Err(err) => {
            tracing::error!("Router build error: {}", err);
            exit(1);
        }
    };

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
