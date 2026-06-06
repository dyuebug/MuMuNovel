use std::env;
use std::error::Error;
use std::fmt;
use uuid::Uuid;

use tracing::{info, warn};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppRuntimeMode {
    Development,
    NonDevelopment,
}

impl AppRuntimeMode {
    fn from_debug(debug: bool) -> Self {
        if debug {
            Self::Development
        } else {
            Self::NonDevelopment
        }
    }

    pub fn is_development(self) -> bool {
        matches!(self, Self::Development)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::NonDevelopment => "non-development",
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    MissingJwtSecret { mode: AppRuntimeMode },
    MissingDatabaseUrl { mode: AppRuntimeMode },
    StartupSchemaSyncNotAllowed { mode: AppRuntimeMode },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingJwtSecret { mode } => write!(
                f,
                "JWT_SECRET is required when runtime mode is {}",
                mode.as_str()
            ),
            Self::MissingDatabaseUrl { mode } => write!(
                f,
                "DATABASE_URL is required when runtime mode is {}",
                mode.as_str()
            ),
            Self::StartupSchemaSyncNotAllowed { mode } => write!(
                f,
                "ENABLE_STARTUP_SCHEMA_SYNC is not allowed when runtime mode is {}. Use the explicit migration step instead.",
                mode.as_str()
            ),
        }
    }
}

impl Error for ConfigError {}

#[derive(Clone)]
pub struct AppConfig {
    pub app_host: String,
    pub app_port: u16,
    pub app_name: String,
    pub app_version: String,
    pub database_url: String,
    pub database_pool_size: u32,
    pub enable_startup_schema_sync: bool,
    pub log_level: String,
    pub debug: bool,
    pub runtime_mode: AppRuntimeMode,
    pub cors_origins: String,
    pub jwt_secret: String,
    pub static_dir: String,
    pub local_auth_enabled: bool,
    pub local_auth_username: String,
    pub local_auth_password: String,
    pub local_auth_display_name: String,
    pub linuxdo_client_id: String,
    pub linuxdo_client_secret: String,
    pub linuxdo_redirect_uri: String,
    pub frontend_url: String,
    pub session_expire_minutes: u32,
    pub session_refresh_threshold_minutes: u32,
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_or_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(default)
}

fn env_or_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn resolve_jwt_secret(mode: AppRuntimeMode, secret: String) -> Result<String, ConfigError> {
    let secret = secret.trim().to_string();
    if !secret.is_empty() {
        return Ok(secret);
    }

    if mode.is_development() {
        let generated = Uuid::new_v4().to_string().replace('-', "");
        warn!(
            "JWT_SECRET not set in development mode; generated an ephemeral secret for local use"
        );
        Ok(generated)
    } else {
        Err(ConfigError::MissingJwtSecret { mode })
    }
}

fn resolve_database_url(mode: AppRuntimeMode, database_url: String) -> Result<String, ConfigError> {
    let database_url = database_url.trim().to_string();
    if !database_url.is_empty() {
        return Ok(database_url);
    }

    if mode.is_development() {
        warn!(
            "DATABASE_URL not set in development mode; falling back to sqlite::memory: for local bootstrap"
        );
        Ok("sqlite::memory:".to_string())
    } else {
        Err(ConfigError::MissingDatabaseUrl { mode })
    }
}

fn resolve_startup_schema_sync(mode: AppRuntimeMode, enabled: bool) -> Result<bool, ConfigError> {
    if !enabled {
        return Ok(false);
    }

    if mode.is_development() {
        warn!(
            "ENABLE_STARTUP_SCHEMA_SYNC is enabled in development mode, but startup schema sync is disabled for strangler deployments. Ignoring the flag and expecting an explicit migration step."
        );
        Ok(false)
    } else {
        Err(ConfigError::StartupSchemaSyncNotAllowed { mode })
    }
}

pub fn load() -> Result<AppConfig, ConfigError> {
    let _ = dotenvy::from_filename("../backend/.env").ok();
    let _ = dotenvy::from_filename(".env").ok();
    let _ = dotenvy::dotenv().ok();

    let debug_enabled = env_or_bool("DEBUG", false);
    let runtime_mode = AppRuntimeMode::from_debug(debug_enabled);
    info!(
        "Config bootstrap mode selected: {} (DEBUG={})",
        runtime_mode.as_str(),
        debug_enabled
    );

    let database_url = resolve_database_url(runtime_mode, env_or("DATABASE_URL", ""))?;
    let jwt_secret = resolve_jwt_secret(runtime_mode, env_or("JWT_SECRET", ""))?;
    let enable_startup_schema_sync = resolve_startup_schema_sync(
        runtime_mode,
        env_or_bool("ENABLE_STARTUP_SCHEMA_SYNC", false),
    )?;

    Ok(AppConfig {
        app_host: env_or("APP_HOST", "127.0.0.1"),
        app_port: env_or("APP_PORT", "8001").parse().unwrap_or(8001),
        app_name: env_or("APP_NAME", "MuMuNovel"),
        app_version: env_or("APP_VERSION", "0.1.0-rs"),
        database_url,
        database_pool_size: env_or_u32("DATABASE_POOL_SIZE", 50),
        enable_startup_schema_sync,
        log_level: env_or("LOG_LEVEL", "info"),
        debug: debug_enabled,
        runtime_mode,
        cors_origins: env_or("CORS_ORIGINS", "*"),
        jwt_secret,
        static_dir: env_or("STATIC_DIR", "../backend/static"),
        local_auth_enabled: env_or_bool("LOCAL_AUTH_ENABLED", true),
        local_auth_username: env_or("LOCAL_AUTH_USERNAME", ""),
        local_auth_password: env_or("LOCAL_AUTH_PASSWORD", ""),
        local_auth_display_name: env_or("LOCAL_AUTH_DISPLAY_NAME", "本地管理员"),
        linuxdo_client_id: env_or("LINUXDO_CLIENT_ID", ""),
        linuxdo_client_secret: env_or("LINUXDO_CLIENT_SECRET", ""),
        linuxdo_redirect_uri: env_or("LINUXDO_REDIRECT_URI", ""),
        frontend_url: env_or("FRONTEND_URL", "http://localhost"),
        session_expire_minutes: env_or_u32("SESSION_EXPIRE_MINUTES", 120),
        session_refresh_threshold_minutes: env_or_u32("SESSION_REFRESH_THRESHOLD_MINUTES", 30),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_database_url, resolve_jwt_secret, resolve_startup_schema_sync, AppRuntimeMode,
        ConfigError,
    };

    #[test]
    fn development_mode_allows_generated_jwt_secret() {
        let secret = resolve_jwt_secret(AppRuntimeMode::Development, String::new())
            .expect("development mode should allow generated secret");

        assert!(!secret.is_empty());
    }

    #[test]
    fn non_development_requires_jwt_secret() {
        let err = resolve_jwt_secret(AppRuntimeMode::NonDevelopment, " ".to_string())
            .expect_err("non-development mode should reject empty jwt secret");

        assert!(matches!(err, ConfigError::MissingJwtSecret { .. }));
    }

    #[test]
    fn development_mode_allows_in_memory_database_fallback() {
        let database_url = resolve_database_url(AppRuntimeMode::Development, String::new())
            .expect("development mode should allow sqlite fallback");

        assert_eq!(database_url, "sqlite::memory:");
    }

    #[test]
    fn non_development_requires_database_url() {
        let err = resolve_database_url(AppRuntimeMode::NonDevelopment, "\n".to_string())
            .expect_err("non-development mode should reject empty database url");

        assert!(matches!(err, ConfigError::MissingDatabaseUrl { .. }));
    }

    #[test]
    fn development_mode_ignores_startup_schema_sync_flag() {
        let enabled = resolve_startup_schema_sync(AppRuntimeMode::Development, true)
            .expect("development mode should not fail hard on startup schema sync flag");

        assert!(!enabled);
    }

    #[test]
    fn non_development_rejects_startup_schema_sync_flag() {
        let err = resolve_startup_schema_sync(AppRuntimeMode::NonDevelopment, true)
            .expect_err("non-development mode should reject startup schema sync flag");

        assert!(matches!(
            err,
            ConfigError::StartupSchemaSyncNotAllowed { .. }
        ));
    }
}
