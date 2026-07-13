use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::models::user;
use crate::models::user as user_entity;
use crate::models::user_password;
use crate::services::password_hash_service::{
    hash_password, is_legacy_sha256, verify_password, PasswordHashError,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub is_admin: bool,
    pub exp: usize,
    pub iat: usize,
}

pub struct AuthService {
    jwt_secret: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalPasswordDecision {
    Verified,
    InvalidCredentials,
}

fn decide_local_password(
    password: &str,
    password_verifier: &str,
) -> Result<LocalPasswordDecision, PasswordHashError> {
    verify_password(password, password_verifier).map(|verified| {
        if verified {
            LocalPasswordDecision::Verified
        } else {
            LocalPasswordDecision::InvalidCredentials
        }
    })
}

impl AuthService {
    pub fn new(jwt_secret: &str) -> Self {
        Self {
            jwt_secret: jwt_secret.to_string(),
        }
    }

    pub fn create_token(&self, user: &user::Model) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now();
        let claims = Claims {
            sub: user.user_id.clone(),
            username: user.username.clone(),
            is_admin: user.is_admin,
            iat: now.timestamp() as usize,
            exp: (now.timestamp() + 86400 * 7) as usize,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(data.claims)
    }

    async fn upgrade_legacy_password_hash(
        db: &DatabaseConnection,
        pwd: &user_password::Model,
        password: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !is_legacy_sha256(&pwd.password_hash) {
            return Ok(());
        }

        let mut active: user_password::ActiveModel = pwd.clone().into();
        active.password_hash = Set(hash_password(password)?);
        active.updated_at = Set(Utc::now());
        active.update(db).await?;
        Ok(())
    }

    /// Auto-create local admin from .env config (mirrors Python backend logic)
    async fn ensure_local_admin(
        db: &DatabaseConnection,
        cfg: &AppConfig,
        username: &str,
        password: &str,
    ) -> Result<Option<user::Model>, Box<dyn std::error::Error + Send + Sync>> {
        if !cfg.local_auth_enabled
            || cfg.local_auth_username.is_empty()
            || cfg.local_auth_password.is_empty()
        {
            return Ok(None);
        }
        if username != cfg.local_auth_username || password != cfg.local_auth_password {
            return Ok(None);
        }

        // Generate deterministic user_id from username (same as Python: local_{md5[:16]})
        let digest = md5::compute(username.as_bytes());
        let user_id = format!("local_{:x}", digest)
            .chars()
            .take(22)
            .collect::<String>();

        // Check if admin already exists
        if user_entity::Entity::find_by_id(&user_id)
            .one(db)
            .await?
            .is_some()
        {
            return Ok(None); // Already created, normal login flow will handle it
        }

        let now = Utc::now();
        let display_name = if cfg.local_auth_display_name.is_empty() {
            username.to_string()
        } else {
            cfg.local_auth_display_name.clone()
        };

        // Create admin user (is_admin=true for local users, same as Python)
        let u = user::ActiveModel {
            user_id: Set(user_id.clone()),
            username: Set(username.to_string()),
            display_name: Set(display_name),
            avatar_url: Set(None),
            trust_level: Set(9),
            is_admin: Set(true),
            linuxdo_id: Set(user_id.clone()),
            created_at: Set(now),
            last_login: Set(now),
        };
        u.insert(db).await?;

        // Set initial password
        let hash = hash_password(password)?;
        let pwd = user_password::ActiveModel {
            user_id: Set(user_id.clone()),
            username: Set(username.to_string()),
            password_hash: Set(hash),
            has_custom_password: Set(false), // .env default, not custom
            created_at: Set(now),
            updated_at: Set(now),
        };
        pwd.insert(db).await?;

        let user = user_entity::Entity::find_by_id(&user_id)
            .one(db)
            .await?
            .ok_or("inserted admin user not found")?;

        tracing::info!(
            "Local admin user '{}' auto-created from .env config",
            username
        );
        Ok(Some(user))
    }

    pub async fn login_local(
        &self,
        db: &DatabaseConnection,
        cfg: &AppConfig,
        username: &str,
        password: &str,
    ) -> Result<Option<(user::Model, String)>, Box<dyn std::error::Error + Send + Sync>> {
        // 1. Try finding user by username in user_passwords table (primary key = user_id)
        let pwd = user_password::Entity::find_by_id(username).one(db).await?;

        let pwd = match pwd {
            Some(p) => p,
            None => {
                // 2. Try finding by username column in users table
                let u = user_entity::Entity::find()
                    .filter(user::Column::Username.eq(username))
                    .one(db)
                    .await?;
                let uid = match u {
                    Some(ref u) => u.user_id.clone(),
                    None => {
                        // 3. Try linuxdo_id
                        let u = user_entity::Entity::find()
                            .filter(user::Column::LinuxdoId.eq(username))
                            .one(db)
                            .await?;
                        match u {
                            Some(ref u) => u.user_id.clone(),
                            None => {
                                // 4. Fallback: auto-create local admin from .env (mirrors Python)
                                if let Some(admin) =
                                    Self::ensure_local_admin(db, cfg, username, password).await?
                                {
                                    let token = self.create_token(&admin)?;
                                    return Ok(Some((admin, token)));
                                }
                                return Ok(None);
                            }
                        }
                    }
                };

                match user_password::Entity::find_by_id(&uid).one(db).await? {
                    Some(p) => p,
                    None => return Ok(None),
                }
            }
        };

        match decide_local_password(password, &pwd.password_hash) {
            Ok(LocalPasswordDecision::Verified) => {
                Self::upgrade_legacy_password_hash(db, &pwd, password).await?;
                let user = user_entity::Entity::find_by_id(&pwd.user_id)
                    .one(db)
                    .await?
                    .ok_or("user not found")?;
                let token = self.create_token(&user)?;
                Ok(Some((user, token)))
            }
            Ok(LocalPasswordDecision::InvalidCredentials) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn register_local(
        &self,
        db: &DatabaseConnection,
        username: &str,
        password: &str,
        display_name: &str,
    ) -> Result<user::Model, Box<dyn std::error::Error + Send + Sync>> {
        let user_id = format!("local_{}", Uuid::new_v4());

        let hash = hash_password(password)?;

        let now = Utc::now();

        // First registered local user becomes admin if no admin exists yet
        let admin_count = user_entity::Entity::find()
            .filter(user::Column::IsAdmin.eq(true))
            .count(db)
            .await?;
        let is_admin = admin_count == 0;

        let u = user::ActiveModel {
            user_id: Set(user_id.clone()),
            username: Set(username.to_string()),
            display_name: Set(display_name.to_string()),
            avatar_url: Set(None),
            trust_level: Set(0),
            is_admin: Set(is_admin),
            linuxdo_id: Set(user_id.clone()),
            created_at: Set(now),
            last_login: Set(now),
        };
        u.insert(db).await?;

        let pwd = user_password::ActiveModel {
            user_id: Set(user_id.clone()),
            username: Set(username.to_string()),
            password_hash: Set(hash),
            has_custom_password: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        };
        pwd.insert(db).await?;

        let user = user_entity::Entity::find_by_id(&user_id)
            .one(db)
            .await?
            .ok_or("inserted user not found")?;

        Ok(user)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
        Schema, Set,
    };
    use sha2::{Digest, Sha256};

    use super::{decide_local_password, AuthService, LocalPasswordDecision};
    use crate::config::{AppConfig, AppRuntimeMode};
    use crate::models::{user, user_password};
    use crate::services::password_hash_service::{
        hash_password, is_legacy_sha256, verify_password, PasswordHashError,
    };

    async fn setup_auth_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect auth sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(user::Entity)))
            .await
            .expect("create users table");
        db.execute(builder.build(&schema.create_table_from_entity(user_password::Entity)))
            .await
            .expect("create user_passwords table");
        db
    }

    fn test_config() -> AppConfig {
        AppConfig {
            app_host: "127.0.0.1".to_string(),
            app_port: 8001,
            app_name: "MuMuNovel".to_string(),
            app_version: "0.1.0-rs".to_string(),
            database_url: "sqlite::memory:".to_string(),
            database_pool_size: 1,
            enable_startup_schema_sync: false,
            log_level: "info".to_string(),
            debug: false,
            runtime_mode: AppRuntimeMode::Development,
            cors_origins: "http://localhost".to_string(),
            jwt_secret: "test-jwt-secret".to_string(),
            static_dir: "../backend/static".to_string(),
            local_auth_enabled: false,
            local_auth_username: String::new(),
            local_auth_password: String::new(),
            local_auth_display_name: "本地管理员".to_string(),
            linuxdo_client_id: String::new(),
            linuxdo_client_secret: String::new(),
            linuxdo_redirect_uri: String::new(),
            frontend_url: "http://localhost".to_string(),
            session_expire_minutes: 120,
            session_refresh_threshold_minutes: 30,
            chapter_candidate_rust_executor_enabled: true,
            chapter_candidate_rust_executor_fallback_on_error: false,
            chapter_candidate_rust_executor_disabled_reason: String::new(),
            chapter_candidate_rust_executor_rollback_boundary: String::new(),
            rust_migration_noop_executor_smoke_enabled: false,
        }
    }

    async fn seed_user_with_verifier(
        db: &DatabaseConnection,
        password_verifier: &str,
    ) -> chrono::DateTime<Utc> {
        let user_id = "legacy-user-id".to_string();
        let username = "legacy-user".to_string();
        let created_at = Utc::now() - Duration::days(1);
        user::ActiveModel {
            user_id: Set(user_id.clone()),
            username: Set(username.clone()),
            display_name: Set("Legacy User".to_string()),
            avatar_url: Set(None),
            trust_level: Set(0),
            is_admin: Set(false),
            linuxdo_id: Set(user_id.clone()),
            created_at: Set(created_at),
            last_login: Set(created_at),
        }
        .insert(db)
        .await
        .expect("insert password test user");

        user_password::ActiveModel {
            user_id: Set(user_id),
            username: Set(username),
            password_hash: Set(password_verifier.to_string()),
            has_custom_password: Set(true),
            created_at: Set(created_at),
            updated_at: Set(created_at),
        }
        .insert(db)
        .await
        .expect("insert password test verifier");

        created_at
    }

    async fn seed_legacy_user(db: &DatabaseConnection) -> (String, chrono::DateTime<Utc>) {
        let legacy_hash = hex::encode(Sha256::digest(b"admin123"));
        let created_at = seed_user_with_verifier(db, &legacy_hash).await;
        (legacy_hash, created_at)
    }

    #[test]
    fn local_password_decision_accepts_correct_password() {
        let verifier = hash_password("admin123").expect("Argon2 hash should succeed");

        let decision = decide_local_password("admin123", &verifier)
            .expect("valid verifier should produce an authentication decision");

        assert_eq!(decision, LocalPasswordDecision::Verified);
    }

    #[test]
    fn local_password_decision_maps_wrong_password_to_invalid_credentials() {
        let verifier = hash_password("admin123").expect("Argon2 hash should succeed");

        let decision = decide_local_password("wrong-password", &verifier)
            .expect("wrong password should remain a normal authentication decision");

        assert_eq!(decision, LocalPasswordDecision::InvalidCredentials);
    }

    #[test]
    fn local_password_decision_propagates_invalid_verifier() {
        let error = decide_local_password("admin123", "not-a-valid-password-verifier")
            .expect_err("corrupted verifier must not be disguised as invalid credentials");

        assert!(matches!(error, PasswordHashError::InvalidVerifier(_)));
    }

    #[tokio::test]
    async fn successful_legacy_login_upgrades_password_verifier_to_argon2() {
        let db = setup_auth_db().await;
        let (legacy_hash, original_updated_at) = seed_legacy_user(&db).await;
        let config = test_config();
        let auth = AuthService::new(&config.jwt_secret);

        let result = auth
            .login_local(&db, &config, "legacy-user", "admin123")
            .await
            .expect("legacy login should succeed")
            .expect("legacy credentials should authenticate");

        assert_eq!(result.0.user_id, "legacy-user-id");
        assert!(!result.1.is_empty());
        let stored = user_password::Entity::find_by_id("legacy-user-id")
            .one(&db)
            .await
            .expect("load upgraded password")
            .expect("upgraded password row should exist");
        assert_ne!(stored.password_hash, legacy_hash);
        assert!(!is_legacy_sha256(&stored.password_hash));
        assert!(stored.password_hash.starts_with("$argon2id$"));
        assert!(stored.password_hash.len() > 64);
        assert!(stored.updated_at > original_updated_at);
        assert!(verify_password("admin123", &stored.password_hash)
            .expect("upgraded Argon2 verifier should remain valid"));
    }

    #[tokio::test]
    async fn wrong_legacy_password_does_not_upgrade_or_modify_verifier() {
        let db = setup_auth_db().await;
        let (legacy_hash, original_updated_at) = seed_legacy_user(&db).await;
        let config = test_config();
        let auth = AuthService::new(&config.jwt_secret);

        let result = auth
            .login_local(&db, &config, "legacy-user", "wrong-password")
            .await
            .expect("wrong legacy password should be a normal authentication result");

        assert!(result.is_none());
        let stored = user_password::Entity::find_by_id("legacy-user-id")
            .one(&db)
            .await
            .expect("load unchanged password")
            .expect("password row should remain present");
        assert_eq!(stored.password_hash, legacy_hash);
        assert_eq!(stored.updated_at, original_updated_at);
    }

    #[tokio::test]
    async fn successful_argon2_login_does_not_rehash_or_modify_verifier() {
        let db = setup_auth_db().await;
        let verifier = hash_password("admin123").expect("Argon2 hash should succeed");
        let original_updated_at = seed_user_with_verifier(&db, &verifier).await;
        let config = test_config();
        let auth = AuthService::new(&config.jwt_secret);

        let result = auth
            .login_local(&db, &config, "legacy-user", "admin123")
            .await
            .expect("canonical Argon2 login should succeed")
            .expect("canonical Argon2 credentials should authenticate");

        assert_eq!(result.0.user_id, "legacy-user-id");
        let stored = user_password::Entity::find_by_id("legacy-user-id")
            .one(&db)
            .await
            .expect("load canonical password")
            .expect("canonical password row should remain present");
        assert_eq!(stored.password_hash, verifier);
        assert_eq!(stored.updated_at, original_updated_at);
    }

    #[tokio::test]
    async fn corrupted_verifier_login_returns_error_without_modifying_database() {
        let db = setup_auth_db().await;
        let corrupted = "not-a-valid-password-verifier";
        let original_updated_at = seed_user_with_verifier(&db, corrupted).await;
        let config = test_config();
        let auth = AuthService::new(&config.jwt_secret);

        let error = auth
            .login_local(&db, &config, "legacy-user", "admin123")
            .await
            .expect_err("corrupted verifier must propagate an authentication error");

        assert!(error.to_string().starts_with("invalid password hash: "));
        let stored = user_password::Entity::find_by_id("legacy-user-id")
            .one(&db)
            .await
            .expect("load corrupted password")
            .expect("corrupted password row should remain present");
        assert_eq!(stored.password_hash, corrupted);
        assert_eq!(stored.updated_at, original_updated_at);
    }
}
