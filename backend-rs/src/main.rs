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

use sea_orm::{ConnectionTrait, Schema};
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

    if let Some(ref db) = db {
        let builder = db.get_database_backend();
        let schema = Schema::new(builder);

        macro_rules! create_table_if_not_exists {
            ($db:expr, $builder:expr, $schema:expr, $entity:ty) => {
                let mut stmt = $schema.create_table_from_entity(<$entity>::default());
                stmt.if_not_exists();
                if let Err(e) = $db.execute($builder.build(&stmt)).await {
                    tracing::warn!("create table skipped: {e}");
                }
            };
        }

        create_table_if_not_exists!(db, builder, schema, models::user::Entity);
        create_table_if_not_exists!(db, builder, schema, models::user_password::Entity);
        create_table_if_not_exists!(db, builder, schema, models::project::Entity);
        create_table_if_not_exists!(db, builder, schema, models::outline::Entity);
        create_table_if_not_exists!(db, builder, schema, models::character::Entity);
        create_table_if_not_exists!(db, builder, schema, models::career::Entity);
        create_table_if_not_exists!(db, builder, schema, models::organization::Entity);
        create_table_if_not_exists!(db, builder, schema, models::relationship::Entity);
        create_table_if_not_exists!(db, builder, schema, models::chapter::Entity);
        create_table_if_not_exists!(db, builder, schema, models::settings::Entity);
        create_table_if_not_exists!(db, builder, schema, models::writing_style::Entity);
        create_table_if_not_exists!(db, builder, schema, models::project_default_style::Entity);
        create_table_if_not_exists!(db, builder, schema, models::foreshadow::Entity);
        create_table_if_not_exists!(db, builder, schema, models::mcp_plugin::Entity);
        create_table_if_not_exists!(db, builder, schema, models::prompt_template::Entity);
        create_table_if_not_exists!(db, builder, schema, models::prompt_submission::Entity);
        create_table_if_not_exists!(db, builder, schema, models::prompt_workshop_item::Entity);
        create_table_if_not_exists!(db, builder, schema, models::prompt_workshop_like::Entity);

        info!("Database schema synced");
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