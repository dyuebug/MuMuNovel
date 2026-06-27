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

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use tracing::info;
use tracing_subscriber::EnvFilter;
use url::Url;
use uuid::Uuid;

const MIGRATION_NOOP_EXECUTOR_SMOKE_COMMAND: &str = "migration-noop-executor-smoke";
const MIGRATION_NEEDED_EXECUTOR_SMOKE_COMMAND: &str = "migration-needed-executor-smoke";
const MIGRATION_EXECUTOR_COMMAND: &str = "migration-executor";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let command = std::env::args().nth(1);
    if matches!(
        command.as_deref(),
        Some(
            MIGRATION_NOOP_EXECUTOR_SMOKE_COMMAND
                | MIGRATION_NEEDED_EXECUTOR_SMOKE_COMMAND
                | MIGRATION_EXECUTOR_COMMAND
        )
    ) {
        if command.as_deref() == Some(MIGRATION_EXECUTOR_COMMAND) {
            exit(run_migration_executor_command().await);
        }
        if command.as_deref() == Some(MIGRATION_NEEDED_EXECUTOR_SMOKE_COMMAND) {
            exit(run_migration_needed_executor_smoke_command().await);
        }
        exit(run_migration_noop_executor_smoke_command().await);
    }

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

async fn run_migration_noop_executor_smoke_command() -> i32 {
    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::error!("Migration executor smoke configuration error: {}", err);
            return 1;
        }
    };

    let db = match db::connection::connect(&cfg).await {
        Ok(db) => db,
        Err(err) => {
            tracing::error!(
                "Migration executor smoke database connection error: {}",
                err
            );
            return 1;
        }
    };

    let report = services::schema_migration_metadata_service::run_rust_migration_executor_shell(
        &db,
        cfg.rust_migration_noop_executor_smoke_enabled,
    )
    .await;

    match serde_json::to_string_pretty(&report.to_json()) {
        Ok(payload) => println!("{payload}"),
        Err(err) => tracing::error!(
            "Failed to serialize migration executor smoke report: {}",
            err
        ),
    }

    report.exit_code
}

async fn run_migration_needed_executor_smoke_command() -> i32 {
    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::error!("Migration-needed smoke configuration error: {}", err);
            return 1;
        }
    };

    let base_db = match db::connection::connect(&cfg).await {
        Ok(db) => db,
        Err(err) => {
            tracing::error!(
                "Migration-needed smoke base database connection error: {}",
                err
            );
            return 1;
        }
    };

    let smoke_schema = format!("rust_migration_smoke_{}", Uuid::new_v4().simple());
    if let Err(err) = create_postgres_smoke_schema(&base_db, &smoke_schema).await {
        tracing::error!(
            "Migration-needed smoke schema creation error for {}: {}",
            smoke_schema,
            err
        );
        return 1;
    }

    let mut scoped_cfg = cfg.clone();
    scoped_cfg.database_url = match scoped_database_url(&cfg.database_url, &smoke_schema) {
        Ok(url) => url,
        Err(err) => {
            tracing::error!("Migration-needed smoke URL error: {}", err);
            let _ = drop_postgres_smoke_schema(&base_db, &smoke_schema).await;
            return 1;
        }
    };
    scoped_cfg.database_pool_size = 1;

    let scoped_db = match db::connection::connect(&scoped_cfg).await {
        Ok(db) => db,
        Err(err) => {
            tracing::error!(
                "Migration-needed smoke scoped database connection error: {}",
                err
            );
            let _ = drop_postgres_smoke_schema(&base_db, &smoke_schema).await;
            return 1;
        }
    };

    let report =
        services::schema_migration_metadata_service::run_rust_migration_executor_shell_with_tail_gate(
            &scoped_db,
            true,
            true,
        )
        .await;

    match serde_json::to_string_pretty(&report.to_json()) {
        Ok(payload) => println!("{payload}"),
        Err(err) => tracing::error!(
            "Failed to serialize migration-needed executor smoke report: {}",
            err
        ),
    }

    if let Err(err) = drop_postgres_smoke_schema(&base_db, &smoke_schema).await {
        tracing::error!(
            "Migration-needed smoke schema cleanup error for {}: {}",
            smoke_schema,
            err
        );
    }

    report.exit_code
}

async fn run_migration_executor_command() -> i32 {
    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::error!("Migration executor configuration error: {}", err);
            return 1;
        }
    };

    let db = match db::connection::connect(&cfg).await {
        Ok(db) => db,
        Err(err) => {
            tracing::error!("Migration executor database connection error: {}", err);
            return 1;
        }
    };

    let report =
        services::schema_migration_metadata_service::run_rust_migration_executor_shell_with_tail_gate(
            &db,
            true,
            true,
        )
        .await;

    match serde_json::to_string_pretty(&report.to_json()) {
        Ok(payload) => println!("{payload}"),
        Err(err) => tracing::error!("Failed to serialize migration executor report: {}", err),
    }

    report.exit_code
}

async fn create_postgres_smoke_schema(
    db: &sea_orm::DatabaseConnection,
    schema_name: &str,
) -> Result<(), sea_orm::DbErr> {
    let sql = format!(r#"CREATE SCHEMA "{}""#, schema_name);
    db.execute(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .map(|_| ())
}

async fn drop_postgres_smoke_schema(
    db: &sea_orm::DatabaseConnection,
    schema_name: &str,
) -> Result<(), sea_orm::DbErr> {
    let sql = format!(r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#, schema_name);
    db.execute(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .map(|_| ())
}

fn scoped_database_url(base_url: &str, schema_name: &str) -> Result<String, String> {
    let mut url = Url::parse(base_url).map_err(|err| err.to_string())?;
    if url.scheme() != "postgres" && url.scheme() != "postgresql" {
        return Err(format!(
            "migration-needed smoke requires a PostgreSQL URL, got {}",
            url.scheme()
        ));
    }

    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema_name}"));
    Ok(url.to_string())
}
