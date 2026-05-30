use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::{http::StatusCode, response::Json};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{json, Value};

use crate::models::user;
use crate::services::auth::Claims;

pub type UserAdminApiError = (StatusCode, Json<Value>);

pub fn api_error(status: StatusCode, detail: impl Into<String>) -> UserAdminApiError {
    (status, Json(json!({ "detail": detail.into() })))
}

pub fn check_admin(claims: &Claims) -> Result<(), UserAdminApiError> {
    if claims.is_admin {
        Ok(())
    } else {
        Err(api_error(StatusCode::FORBIDDEN, "需要管理员权限"))
    }
}

pub fn user_to_value(model: &user::Model) -> Value {
    json!({
        "user_id": model.user_id,
        "username": model.username,
        "display_name": model.display_name,
        "avatar_url": model.avatar_url,
        "trust_level": model.trust_level,
        "is_admin": model.is_admin,
        "is_active": model.trust_level != -1,
        "linuxdo_id": model.linuxdo_id,
        "created_at": model.created_at.to_rfc3339(),
        "last_login": model.last_login.to_rfc3339(),
    })
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| format!("password hash failed: {err}"))
}

pub fn default_password_for_username(username: &str) -> String {
    format!("{username}@666")
}

pub async fn admin_count(db: &DatabaseConnection) -> Result<usize, UserAdminApiError> {
    let admins = user::Entity::find()
        .filter(user::Column::IsAdmin.eq(true))
        .all(db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(admins.len())
}

pub async fn find_user(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<user::Model, UserAdminApiError> {
    user::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "用户不存在"))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use chrono::{TimeZone, Utc};

    use crate::models::user;
    use crate::services::auth::Claims;

    use super::{
        api_error, check_admin, default_password_for_username, hash_password, user_to_value,
    };

    fn claims(is_admin: bool) -> Claims {
        Claims {
            sub: "user-1".to_string(),
            username: "tester".to_string(),
            is_admin,
            exp: 1,
            iat: 1,
        }
    }

    fn user_model() -> user::Model {
        user::Model {
            user_id: "user-1".to_string(),
            username: "tester".to_string(),
            display_name: "Tester".to_string(),
            avatar_url: Some("https://example.com/avatar.png".to_string()),
            trust_level: 0,
            is_admin: true,
            linuxdo_id: "linuxdo-1".to_string(),
            created_at: Utc
                .with_ymd_and_hms(2026, 5, 22, 1, 30, 0)
                .single()
                .expect("datetime should be valid"),
            last_login: Utc
                .with_ymd_and_hms(2026, 5, 22, 1, 45, 0)
                .single()
                .expect("datetime should be valid"),
        }
    }

    #[test]
    fn check_admin_preserves_forbidden_contract() {
        assert!(check_admin(&claims(true)).is_ok());

        let error = check_admin(&claims(false)).expect_err("non-admin should fail");
        assert_eq!(error.0, StatusCode::FORBIDDEN);
        assert_eq!(error.1 .0["detail"], "需要管理员权限");
    }

    #[test]
    fn user_to_value_keeps_existing_payload_shape() {
        let payload = user_to_value(&user_model());

        assert_eq!(payload["user_id"], "user-1");
        assert_eq!(payload["username"], "tester");
        assert_eq!(payload["display_name"], "Tester");
        assert_eq!(payload["is_admin"], true);
        assert_eq!(payload["is_active"], true);
        assert_eq!(payload["linuxdo_id"], "linuxdo-1");
    }

    #[test]
    fn default_password_and_hash_password_keep_existing_semantics() {
        let default_password = default_password_for_username("alice");
        assert_eq!(default_password, "alice@666");

        let hashed = hash_password(&default_password).expect("hashing should succeed");
        assert!(!hashed.is_empty());
        assert_ne!(hashed, default_password);
    }

    #[test]
    fn api_error_keeps_detail_shape() {
        let error = api_error(StatusCode::BAD_REQUEST, "缺少 user_id");
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert_eq!(error.1 .0["detail"], "缺少 user_id");
    }
}
