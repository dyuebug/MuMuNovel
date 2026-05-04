use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::models::user;
use crate::models::user as user_entity;
use crate::models::user_password;

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

    fn hash_password(password: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| format!("password hash failed: {}", e).into())
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
        let user_id = format!("local_{:x}", digest).chars().take(22).collect::<String>();

        // Check if admin already exists
        if user_entity::Entity::find_by_id(&user_id).one(db).await?.is_some() {
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
        let hash = Self::hash_password(password)?;
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

        tracing::info!("Local admin user '{}' auto-created from .env config", username);
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
        let pwd = user_password::Entity::find_by_id(username)
            .one(db)
            .await?;

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

        let parsed = PasswordHash::new(&pwd.password_hash)
            .map_err(|e| format!("invalid password hash: {}", e))?;
        let valid = Argon2::default().verify_password(password.as_bytes(), &parsed);

        match valid {
            Ok(_) => {
                let user = user_entity::Entity::find_by_id(&pwd.user_id)
                    .one(db)
                    .await?
                    .ok_or("user not found")?;
                let token = self.create_token(&user)?;
                Ok(Some((user, token)))
            }
            Err(_) => Ok(None),
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

        let hash = Self::hash_password(password)?;

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
