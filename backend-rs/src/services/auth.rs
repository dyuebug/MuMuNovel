use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

    pub async fn login_local(
        &self,
        db: &DatabaseConnection,
        username: &str,
        password: &str,
    ) -> Result<Option<(user::Model, String)>, Box<dyn std::error::Error + Send + Sync>> {
        let pwd = user_password::Entity::find_by_id(username)
            .one(db)
            .await?
            .or_else(|| {
                // also try finding by username column
                None // Will be handled by fallback
            });

        let pwd = match pwd {
            Some(p) => p,
            None => {
                // Try finding by username column
                let u = user_entity::Entity::find()
                    .filter(user::Column::Username.eq(username))
                    .one(db)
                    .await?;
                let uid = match u {
                    Some(ref u) => u.user_id.clone(),
                    None => {
                        // Try linuxdo_id
                        let u = user_entity::Entity::find()
                            .filter(user::Column::LinuxdoId.eq(username))
                            .one(db)
                            .await?;
                        match u {
                            Some(ref u) => u.user_id.clone(),
                            None => return Ok(None),
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

        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("password hash failed: {}", e))?
            .to_string();

        let now = Utc::now();

        let u = user::ActiveModel {
            user_id: Set(user_id.clone()),
            username: Set(username.to_string()),
            display_name: Set(display_name.to_string()),
            avatar_url: Set(None),
            trust_level: Set(0),
            is_admin: Set(false),
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
