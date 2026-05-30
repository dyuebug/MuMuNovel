use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::user_admin_route_service::{
    api_error, default_password_for_username, hash_password, user_to_value, UserAdminApiError,
};
use crate::models::user;
use crate::models::user_password;

#[derive(Debug, PartialEq, Eq)]
pub struct CreateUserRequest {
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_admin: bool,
    pub trust_level: i32,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct CreateUserRouteRequest {
    #[serde(default)]
    pub username: Option<Value>,
    #[serde(default)]
    pub display_name: Option<Value>,
    #[serde(default)]
    pub avatar_url: Option<Value>,
    #[serde(default)]
    pub is_admin: Option<Value>,
    #[serde(default)]
    pub trust_level: Option<Value>,
    #[serde(default)]
    pub password: Option<Value>,
}

impl CreateUserRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "username": self.username,
            "display_name": self.display_name,
            "avatar_url": self.avatar_url,
            "is_admin": self.is_admin,
            "trust_level": self.trust_level,
            "password": self.password,
        })
    }
}

pub fn build_create_user_request(body: &Value) -> Result<CreateUserRequest, UserAdminApiError> {
    let username = body
        .get("username")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| api_error(axum::http::StatusCode::BAD_REQUEST, "缺少 username"))?;

    let display_name = body
        .get("display_name")
        .and_then(|value| value.as_str())
        .unwrap_or(username);

    Ok(CreateUserRequest {
        username: username.to_string(),
        display_name: display_name.to_string(),
        avatar_url: body
            .get("avatar_url")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        is_admin: body
            .get("is_admin")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        trust_level: body
            .get("trust_level")
            .and_then(|value| value.as_i64())
            .unwrap_or(0) as i32,
        password: body
            .get("password")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    })
}

pub fn build_create_user_request_from_route_payload(
    body: CreateUserRouteRequest,
) -> Result<CreateUserRequest, UserAdminApiError> {
    build_create_user_request(&body.into_body())
}

pub fn build_create_user_payload(
    created: Option<&user::Model>,
    default_password: Option<&str>,
) -> Value {
    json!({
        "success": true,
        "message": "用户创建成功",
        "user": created.map(user_to_value),
        "default_password": default_password,
    })
}

pub async fn create_user_payload(
    db: &DatabaseConnection,
    request: &CreateUserRequest,
) -> Result<Value, UserAdminApiError> {
    let existing = user::Entity::find()
        .filter(user::Column::Username.eq(&request.username))
        .one(db)
        .await
        .map_err(|error| {
            api_error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            )
        })?;

    if existing.is_some() {
        return Err(api_error(axum::http::StatusCode::CONFLICT, "用户名已存在"));
    }

    let user_id = format!(
        "admin_created_{}",
        Uuid::new_v4().to_string().replace('-', "")[..16].to_string()
    );
    let now = Utc::now();

    let user_model = user::ActiveModel {
        user_id: Set(user_id.clone()),
        username: Set(request.username.clone()),
        display_name: Set(request.display_name.clone()),
        avatar_url: Set(request.avatar_url.clone()),
        trust_level: Set(request.trust_level),
        is_admin: Set(request.is_admin),
        linuxdo_id: Set(user_id.clone()),
        created_at: Set(now),
        last_login: Set(now),
    };
    user_model.insert(db).await.map_err(|error| {
        api_error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
        )
    })?;

    let has_custom_password = request.password.is_some();
    let default_password = default_password_for_username(&request.username);
    let actual_password = request.password.as_deref().unwrap_or(&default_password);
    let password_hash = hash_password(actual_password)
        .map_err(|error| api_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, error))?;

    let password_model = user_password::ActiveModel {
        user_id: Set(user_id.clone()),
        username: Set(request.username.clone()),
        password_hash: Set(password_hash),
        has_custom_password: Set(has_custom_password),
        created_at: Set(now),
        updated_at: Set(now),
    };
    password_model.insert(db).await.map_err(|error| {
        api_error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
        )
    })?;

    let created = user::Entity::find_by_id(&user_id)
        .one(db)
        .await
        .map_err(|error| {
            api_error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            )
        })?;

    let response_default = if has_custom_password {
        None
    } else {
        Some(default_password)
    };

    Ok(build_create_user_payload(
        created.as_ref(),
        response_default.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use crate::models::user;

    use super::{
        build_create_user_payload, build_create_user_request,
        build_create_user_request_from_route_payload, CreateUserRouteRequest,
    };

    fn user_model() -> user::Model {
        user::Model {
            user_id: "user-1".to_string(),
            username: "tester".to_string(),
            display_name: "Tester".to_string(),
            avatar_url: Some("https://example.com/avatar.png".to_string()),
            trust_level: 1,
            is_admin: true,
            linuxdo_id: "linuxdo-1".to_string(),
            created_at: Utc
                .with_ymd_and_hms(2026, 5, 22, 5, 10, 0)
                .single()
                .expect("datetime should be valid"),
            last_login: Utc
                .with_ymd_and_hms(2026, 5, 22, 5, 10, 0)
                .single()
                .expect("datetime should be valid"),
        }
    }

    #[test]
    fn build_create_user_request_keeps_defaults_and_optional_fields() {
        let request = build_create_user_request(&json!({
            "username": "alice",
            "avatar_url": "https://example.com/avatar.png"
        }))
        .expect("request should build");

        assert_eq!(request.username, "alice");
        assert_eq!(request.display_name, "alice");
        assert_eq!(
            request.avatar_url,
            Some("https://example.com/avatar.png".to_string())
        );
        assert_eq!(request.is_admin, false);
        assert_eq!(request.trust_level, 0);
        assert_eq!(request.password, None);
    }

    #[test]
    fn build_create_user_request_rejects_missing_username() {
        let error = build_create_user_request(&json!({"display_name": "Alice"}))
            .expect_err("missing username should fail");

        assert_eq!(error.0, axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(error.1 .0["detail"], "缺少 username");
    }

    #[test]
    fn build_create_user_request_from_route_payload_keeps_existing_contract() {
        let request = build_create_user_request_from_route_payload(CreateUserRouteRequest {
            username: Some(json!("alice")),
            display_name: None,
            avatar_url: Some(json!("https://example.com/avatar.png")),
            is_admin: None,
            trust_level: None,
            password: None,
        })
        .expect("request should build");

        assert_eq!(request.username, "alice");
        assert_eq!(request.display_name, "alice");
        assert_eq!(
            request.avatar_url,
            Some("https://example.com/avatar.png".to_string())
        );
        assert_eq!(request.is_admin, false);
        assert_eq!(request.trust_level, 0);
        assert_eq!(request.password, None);
    }

    #[test]
    fn build_create_user_payload_keeps_success_shell() {
        let created = user_model();
        let payload = build_create_user_payload(Some(&created), Some("alice@666"));

        assert_eq!(payload["success"], true);
        assert_eq!(payload["message"], "用户创建成功");
        assert_eq!(payload["user"]["user_id"], "user-1");
        assert_eq!(payload["default_password"], "alice@666");

        let without_default = build_create_user_payload(Some(&created), None);
        assert!(without_default["default_password"].is_null());
    }

    #[test]
    fn create_user_payload_error_shape_keeps_conflict_contract() {
        let error = axum::http::StatusCode::CONFLICT;

        assert_eq!(error, axum::http::StatusCode::CONFLICT);
    }
}
