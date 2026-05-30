use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};

use crate::models::user_password;
use crate::services::user_admin_route_service::{
    api_error, default_password_for_username, find_user, hash_password, UserAdminApiError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordResetMode {
    UseDefaultWhenMissing,
    UseDefaultWhenMissingOrEmpty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordResetOutcome {
    pub user_id: String,
    pub username: String,
    pub actual_password: String,
    pub used_default_password: bool,
}

fn database_error(error: impl ToString) -> UserAdminApiError {
    api_error(
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        error.to_string(),
    )
}

fn resolve_password_reset_value(
    username: &str,
    requested_password: Option<&str>,
    mode: PasswordResetMode,
) -> (String, bool, bool) {
    let default_password = default_password_for_username(username);

    match mode {
        PasswordResetMode::UseDefaultWhenMissing => match requested_password {
            Some(password) => (password.to_string(), true, false),
            None => (default_password, false, true),
        },
        PasswordResetMode::UseDefaultWhenMissingOrEmpty => match requested_password {
            Some(password) if !password.is_empty() => (password.to_string(), true, false),
            _ => (default_password, false, true),
        },
    }
}

pub fn build_user_reset_password_payload(outcome: &PasswordResetOutcome) -> Value {
    let mut response = json!({
        "message": "密码重置成功",
        "user_id": outcome.user_id,
        "username": outcome.username,
    });

    if outcome.used_default_password {
        response["default_password"] = json!(outcome.actual_password);
        response["message"] = json!(format!("密码已重置为默认密码: {}", outcome.actual_password));
    }

    response
}

pub fn build_admin_reset_password_payload(actual_password: &str) -> Value {
    json!({
        "success": true,
        "message": "密码重置成功",
        "new_password": actual_password,
    })
}

pub async fn reset_user_password_workflow(
    db: &DatabaseConnection,
    user_id: &str,
    requested_password: Option<&str>,
    mode: PasswordResetMode,
) -> Result<PasswordResetOutcome, UserAdminApiError> {
    let target = find_user(db, user_id).await?;
    let (actual_password, has_custom_password, used_default_password) =
        resolve_password_reset_value(&target.username, requested_password, mode);
    let password_hash = hash_password(&actual_password)
        .map_err(|error| api_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, error))?;
    let now = Utc::now();

    match user_password::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(database_error)?
    {
        Some(password) => {
            let mut active: user_password::ActiveModel = password.into();
            active.password_hash = Set(password_hash);
            active.has_custom_password = Set(has_custom_password);
            active.updated_at = Set(now);
            active.update(db).await.map_err(database_error)?;
        }
        None => {
            let password = user_password::ActiveModel {
                user_id: Set(user_id.to_string()),
                username: Set(target.username.clone()),
                password_hash: Set(password_hash),
                has_custom_password: Set(has_custom_password),
                created_at: Set(now),
                updated_at: Set(now),
            };
            password.insert(db).await.map_err(database_error)?;
        }
    }

    Ok(PasswordResetOutcome {
        user_id: user_id.to_string(),
        username: target.username,
        actual_password,
        used_default_password,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_admin_reset_password_payload, build_user_reset_password_payload,
        resolve_password_reset_value, PasswordResetMode, PasswordResetOutcome,
    };

    #[test]
    fn resolve_password_reset_value_keeps_user_route_default_on_empty_behavior() {
        let (password, has_custom_password, used_default_password) = resolve_password_reset_value(
            "alice",
            Some(""),
            PasswordResetMode::UseDefaultWhenMissingOrEmpty,
        );

        assert_eq!(password, "alice@666");
        assert!(!has_custom_password);
        assert!(used_default_password);
    }

    #[test]
    fn resolve_password_reset_value_keeps_admin_route_empty_string_behavior() {
        let (password, has_custom_password, used_default_password) = resolve_password_reset_value(
            "alice",
            Some(""),
            PasswordResetMode::UseDefaultWhenMissing,
        );

        assert_eq!(password, "");
        assert!(has_custom_password);
        assert!(!used_default_password);
    }

    #[test]
    fn user_reset_password_payload_keeps_default_password_shape() {
        let payload = build_user_reset_password_payload(&PasswordResetOutcome {
            user_id: "user-1".to_string(),
            username: "alice".to_string(),
            actual_password: "alice@666".to_string(),
            used_default_password: true,
        });

        assert_eq!(payload["user_id"], "user-1");
        assert_eq!(payload["username"], "alice");
        assert_eq!(payload["default_password"], "alice@666");
        assert_eq!(payload["message"], "密码已重置为默认密码: alice@666");
    }

    #[test]
    fn admin_reset_password_payload_keeps_existing_shape() {
        let payload = build_admin_reset_password_payload("custom-password");

        assert_eq!(payload["success"], true);
        assert_eq!(payload["message"], "密码重置成功");
        assert_eq!(payload["new_password"], "custom-password");
    }
}
