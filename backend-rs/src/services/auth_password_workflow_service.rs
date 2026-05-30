use axum::http::StatusCode;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};

use crate::models::{user, user_password};
use crate::services::user_admin_route_service::{
    api_error, default_password_for_username, find_user, hash_password, UserAdminApiError,
};

fn database_error(error: impl ToString) -> UserAdminApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn build_password_status_payload(
    has_password: bool,
    has_custom_password: bool,
    username: Option<String>,
) -> Value {
    let default_password = if has_password && !has_custom_password {
        username
            .as_ref()
            .map(|name| default_password_for_username(name))
    } else {
        None
    };

    json!({
        "has_password": has_password,
        "has_custom_password": has_custom_password,
        "username": username,
        "default_password": default_password,
    })
}

fn build_password_write_success_payload(message: &str) -> Value {
    json!({
        "success": true,
        "message": message,
    })
}

pub async fn load_password_status_workflow(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<Value, UserAdminApiError> {
    let password = user_password::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(database_error)?;

    match password {
        Some(password) => {
            let user = user::Entity::find_by_id(user_id)
                .one(db)
                .await
                .map_err(database_error)?;

            Ok(build_password_status_payload(
                true,
                password.has_custom_password,
                user.map(|value| value.username),
            ))
        }
        None => {
            let user = user::Entity::find_by_id(user_id)
                .one(db)
                .await
                .map_err(database_error)?;

            Ok(build_password_status_payload(
                false,
                false,
                user.map(|value| value.username),
            ))
        }
    }
}

pub async fn set_password_workflow(
    db: &DatabaseConnection,
    user_id: &str,
    password: &str,
) -> Result<Value, UserAdminApiError> {
    let hashed_password = hash_password(password)
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    let now = Utc::now();
    let existing = user_password::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(database_error)?;

    match existing {
        Some(password_model) => {
            let mut active: user_password::ActiveModel = password_model.into();
            active.password_hash = Set(hashed_password);
            active.has_custom_password = Set(true);
            active.updated_at = Set(now);
            active.update(db).await.map_err(database_error)?;
        }
        None => {
            let user = find_user(db, user_id).await?;
            let password = user_password::ActiveModel {
                user_id: Set(user_id.to_string()),
                username: Set(user.username.clone()),
                password_hash: Set(hashed_password),
                has_custom_password: Set(true),
                created_at: Set(now),
                updated_at: Set(now),
            };
            password.insert(db).await.map_err(database_error)?;
        }
    }

    Ok(build_password_write_success_payload("密码设置成功"))
}

pub async fn initialize_password_workflow(
    db: &DatabaseConnection,
    user_id: &str,
    password: &str,
) -> Result<Value, UserAdminApiError> {
    let existing = user_password::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(database_error)?;

    if existing.is_some() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "密码已存在，请使用密码设置接口",
        ));
    }

    let hashed_password = hash_password(password)
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    let user = find_user(db, user_id).await?;
    let now = Utc::now();
    let password = user_password::ActiveModel {
        user_id: Set(user_id.to_string()),
        username: Set(user.username.clone()),
        password_hash: Set(hashed_password),
        has_custom_password: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
    };
    password.insert(db).await.map_err(database_error)?;

    Ok(build_password_write_success_payload("密码初始化成功"))
}

#[cfg(test)]
mod tests {
    use super::{build_password_status_payload, build_password_write_success_payload};

    #[test]
    fn password_status_payload_keeps_default_password_for_non_custom_password() {
        let payload = build_password_status_payload(true, false, Some("alice".to_string()));

        assert_eq!(payload["has_password"], true);
        assert_eq!(payload["has_custom_password"], false);
        assert_eq!(payload["username"], "alice");
        assert_eq!(payload["default_password"], "alice@666");
    }

    #[test]
    fn password_status_payload_keeps_null_default_when_password_missing() {
        let payload = build_password_status_payload(false, false, Some("alice".to_string()));

        assert_eq!(payload["has_password"], false);
        assert_eq!(payload["has_custom_password"], false);
        assert_eq!(payload["username"], "alice");
        assert!(payload["default_password"].is_null());
    }

    #[test]
    fn password_status_payload_keeps_null_default_when_username_missing() {
        let payload = build_password_status_payload(true, false, None);

        assert_eq!(payload["has_password"], true);
        assert_eq!(payload["has_custom_password"], false);
        assert!(payload["username"].is_null());
        assert!(payload["default_password"].is_null());
    }

    #[test]
    fn password_write_success_payload_keeps_existing_shape() {
        let payload = build_password_write_success_payload("密码设置成功");

        assert_eq!(payload["success"], true);
        assert_eq!(payload["message"], "密码设置成功");
    }
}
