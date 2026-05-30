use serde::Deserialize;
use serde_json::Value;

use super::user_admin_route_service::{api_error, UserAdminApiError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UserResetPasswordRouteRequest {
    pub user_id: Option<String>,
    pub new_password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AdminResetPasswordRouteRequest {
    pub new_password: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UserResetPasswordRequest {
    pub user_id: String,
    pub new_password: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AdminResetPasswordRequest {
    pub new_password: Option<String>,
}

pub fn build_user_reset_password_request(
    body: &Value,
) -> Result<UserResetPasswordRequest, UserAdminApiError> {
    let user_id = body
        .get("user_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| api_error(axum::http::StatusCode::BAD_REQUEST, "缺少 user_id"))?;

    Ok(UserResetPasswordRequest {
        user_id: user_id.to_string(),
        new_password: body
            .get("new_password")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    })
}

pub fn build_user_reset_password_request_from_route_payload(
    body: UserResetPasswordRouteRequest,
) -> Result<UserResetPasswordRequest, UserAdminApiError> {
    let mut payload = serde_json::Map::new();

    if let Some(user_id) = body.user_id {
        payload.insert("user_id".to_string(), Value::String(user_id));
    }
    if let Some(new_password) = body.new_password {
        payload.insert("new_password".to_string(), Value::String(new_password));
    }

    build_user_reset_password_request(&Value::Object(payload))
}

pub fn build_admin_reset_password_request(body: &Value) -> AdminResetPasswordRequest {
    AdminResetPasswordRequest {
        new_password: body
            .get("new_password")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    }
}

pub fn build_admin_reset_password_request_from_route_payload(
    body: AdminResetPasswordRouteRequest,
) -> AdminResetPasswordRequest {
    let mut payload = serde_json::Map::new();

    if let Some(new_password) = body.new_password {
        payload.insert("new_password".to_string(), Value::String(new_password));
    }

    build_admin_reset_password_request(&Value::Object(payload))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use super::{
        build_admin_reset_password_request, build_admin_reset_password_request_from_route_payload,
        build_user_reset_password_request, build_user_reset_password_request_from_route_payload,
        AdminResetPasswordRouteRequest, UserResetPasswordRouteRequest,
    };

    #[test]
    fn build_user_reset_password_request_keeps_existing_contract() {
        let request = build_user_reset_password_request(&json!({
            "user_id": "user-1",
            "new_password": "custom-pass"
        }))
        .expect("request should build");

        assert_eq!(request.user_id, "user-1");
        assert_eq!(request.new_password, Some("custom-pass".to_string()));
    }

    #[test]
    fn build_user_reset_password_request_rejects_missing_user_id() {
        let error = build_user_reset_password_request(&json!({"new_password": "custom-pass"}))
            .expect_err("missing user_id should fail");

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert_eq!(error.1 .0["detail"], "缺少 user_id");
    }

    #[test]
    fn build_admin_reset_password_request_keeps_missing_password_as_none() {
        let request = build_admin_reset_password_request(&json!({}));

        assert_eq!(request.new_password, None);
    }

    #[test]
    fn build_admin_reset_password_request_keeps_empty_string_behavior() {
        let request = build_admin_reset_password_request(&json!({
            "new_password": ""
        }));

        assert_eq!(request.new_password, Some(String::new()));
    }

    #[test]
    fn build_admin_reset_password_request_keeps_existing_contract() {
        let request = build_admin_reset_password_request(&json!({
            "new_password": "custom-pass"
        }));

        assert_eq!(request.new_password, Some("custom-pass".to_string()));
    }

    #[test]
    fn build_admin_reset_password_request_from_route_payload_keeps_existing_contract() {
        let request =
            build_admin_reset_password_request_from_route_payload(AdminResetPasswordRouteRequest {
                new_password: Some("custom-pass".to_string()),
            });

        assert_eq!(request.new_password, Some("custom-pass".to_string()));
    }

    #[test]
    fn build_user_reset_password_request_from_route_payload_keeps_existing_contract() {
        let request =
            build_user_reset_password_request_from_route_payload(UserResetPasswordRouteRequest {
                user_id: Some("user-1".to_string()),
                new_password: Some("custom-pass".to_string()),
            })
            .expect("request should build");

        assert_eq!(request.user_id, "user-1");
        assert_eq!(request.new_password, Some("custom-pass".to_string()));
    }

    #[test]
    fn build_user_reset_password_request_from_route_payload_rejects_missing_user_id() {
        let error =
            build_user_reset_password_request_from_route_payload(UserResetPasswordRouteRequest {
                user_id: None,
                new_password: Some("custom-pass".to_string()),
            })
            .expect_err("missing user_id should fail");

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert_eq!(error.1 .0["detail"], "缺少 user_id");
    }
}
