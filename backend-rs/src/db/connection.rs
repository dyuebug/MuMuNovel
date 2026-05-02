use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use tracing::info;

use crate::config::AppConfig;

pub async fn connect(cfg: &AppConfig) -> Result<DatabaseConnection, Box<dyn std::error::Error + Send + Sync>> {
    let url = if cfg.database_url.is_empty() {
        "sqlite::memory:".to_string()
    } else {
        cfg.database_url.clone()
    };

    let mut opt = ConnectOptions::new(url);
    opt.sqlx_logging(false)
        .max_connections(cfg.database_pool_size)
        .sqlx_logging_level(log::LevelFilter::Info);

    info!("Connecting to database...");

    let db = Database::connect(opt).await?;
    Ok(db)
}
