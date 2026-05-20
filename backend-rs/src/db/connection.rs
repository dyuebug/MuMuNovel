use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use tracing::info;

use crate::config::AppConfig;

fn database_driver_label(url: &str) -> &'static str {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        "postgres"
    } else if url.starts_with("sqlite:") {
        "sqlite"
    } else {
        "unknown"
    }
}

pub async fn connect(
    cfg: &AppConfig,
) -> Result<DatabaseConnection, Box<dyn std::error::Error + Send + Sync>> {
    let mut opt = ConnectOptions::new(cfg.database_url.clone());
    opt.sqlx_logging(false)
        .max_connections(cfg.database_pool_size)
        .sqlx_logging_level(log::LevelFilter::Info);

    info!(
        "Connecting to database (driver={}, mode={})...",
        database_driver_label(&cfg.database_url),
        cfg.runtime_mode.as_str()
    );

    let db = Database::connect(opt).await?;
    Ok(db)
}
