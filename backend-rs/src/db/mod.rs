pub mod connection;

use sea_orm::DatabaseConnection;
use tracing::warn;

use crate::config::AppConfig;

pub async fn init_pool(cfg: &AppConfig) -> Option<DatabaseConnection> {
    match connection::connect(cfg).await {
        Ok(db) => Some(db),
        Err(e) => {
            warn!("Database connection failed: {}. /readyz will report not_ready.", e);
            None
        }
    }
}
