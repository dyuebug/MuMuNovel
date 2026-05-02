pub mod ai;
pub mod api;
pub mod config;
pub mod db;
pub mod middleware;
pub mod models;
pub mod services;
pub mod tasks;

use std::net::SocketAddr;

use sea_orm::{ConnectionTrait, Schema};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::models::{career, chapter, character, foreshadow, organization, outline, project, project_default_style, relationship, settings, user, user_password, writing_style};

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
        let stmt = schema.create_table_from_entity(user::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create users table");
        let stmt = schema.create_table_from_entity(user_password::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create user_passwords table");
        let stmt = schema.create_table_from_entity(project::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create projects table");
        let stmt = schema.create_table_from_entity(outline::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create outlines table");
        let stmt = schema.create_table_from_entity(character::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create characters table");
        let stmt = schema.create_table_from_entity(career::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create careers table");
        let stmt = schema.create_table_from_entity(organization::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create organizations table");
        let stmt = schema.create_table_from_entity(relationship::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create character_relationships table");
        let stmt = schema.create_table_from_entity(chapter::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create chapters table");
        let stmt = schema.create_table_from_entity(settings::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create settings table");
        let stmt = schema.create_table_from_entity(writing_style::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create writing_styles table");
        let stmt = schema.create_table_from_entity(project_default_style::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create project_default_styles table");
        let stmt = schema.create_table_from_entity(foreshadow::Entity);
        db.execute(builder.build(&stmt)).await
            .expect("failed to create foreshadows table");
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
            // Run cleanup every 60 seconds
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            registry.prune_old_tasks().await;
        }
    });
}
