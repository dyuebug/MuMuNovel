use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub app_host: String,
    pub app_port: u16,
    pub app_name: String,
    pub app_version: String,
    pub database_url: String,
    pub database_pool_size: u32,
    pub log_level: String,
    pub debug: bool,
    pub cors_origins: String,
    pub jwt_secret: String,
    pub static_dir: String,
    pub local_auth_enabled: bool,
    pub linuxdo_client_id: String,
    pub linuxdo_client_secret: String,
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

pub fn load() -> AppConfig {
    let _ = dotenvy::from_filename("../backend/.env").ok();
    let _ = dotenvy::from_filename(".env").ok();
    let _ = dotenvy::dotenv().ok();

    AppConfig {
        app_host: env_or("APP_HOST", "127.0.0.1"),
        app_port: env_or("APP_PORT", "8001").parse().unwrap_or(8001),
        app_name: env_or("APP_NAME", "MuMuNovel"),
        app_version: env_or("APP_VERSION", "0.1.0-rs"),
        database_url: env_or("DATABASE_URL", ""),
        database_pool_size: env_or_u32("DATABASE_POOL_SIZE", 50),
        log_level: env_or("LOG_LEVEL", "info"),
        debug: env_or_bool("DEBUG", false),
        cors_origins: env_or("CORS_ORIGINS", "*"),
        jwt_secret: env_or("JWT_SECRET", ""),
        static_dir: env_or("STATIC_DIR", "../backend/static"),
        local_auth_enabled: env_or_bool("LOCAL_AUTH_ENABLED", true),
        linuxdo_client_id: env_or("LINUXDO_CLIENT_ID", ""),
        linuxdo_client_secret: env_or("LINUXDO_CLIENT_SECRET", ""),
    }
}
