use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde::Deserialize;
use serde_json::{json, Value};

use super::user_admin_route_service::{admin_count, api_error, find_user, UserAdminApiError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SetAdminRouteRequest {
    pub user_id: Option<String>,
    pub is_admin: Option<bool>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SetAdminRequest {
    pub user_id: String,
    pub is_admin: bool,
}

pub fn build_set_admin_request(body: &Value) -> Result<SetAdminRequest, UserAdminApiError> {
    let user_id = body
        .get("user_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| api_error(axum::http::StatusCode::BAD_REQUEST, "缺少 user_id"))?;
    let is_admin = body
        .get("is_admin")
        .and_then(|value| value.as_bool())
        .ok_or_else(|| api_error(axum::http::StatusCode::BAD_REQUEST, "缺少 is_admin"))?;

    Ok(SetAdminRequest {
        user_id: user_id.to_string(),
        is_admin,
    })
}

pub fn build_set_admin_request_from_route_payload(
    body: SetAdminRouteRequest,
) -> Result<SetAdminRequest, UserAdminApiError> {
    let mut payload = serde_json::Map::new();

    if let Some(user_id) = body.user_id {
        payload.insert("user_id".to_string(), Value::String(user_id));
    }
    if let Some(is_admin) = body.is_admin {
        payload.insert("is_admin".to_string(), Value::Bool(is_admin));
    }

    build_set_admin_request(&Value::Object(payload))
}

pub fn build_set_admin_payload(user_id: &str, is_admin: bool) -> Value {
    let action = if is_admin { "授予" } else { "撤销" };
    json!({
        "message": format!("已{action}管理员权限"),
        "user_id": user_id,
        "is_admin": is_admin,
    })
}

pub async fn set_admin_payload(
    db: &DatabaseConnection,
    actor_user_id: &str,
    request: &SetAdminRequest,
) -> Result<Value, UserAdminApiError> {
    if request.user_id == actor_user_id && !request.is_admin {
        return Err(api_error(
            axum::http::StatusCode::BAD_REQUEST,
            "不能撤销自己的管理员权限",
        ));
    }

    let target = find_user(db, &request.user_id).await?;
    if target.is_admin && !request.is_admin && admin_count(db).await? <= 1 {
        return Err(api_error(
            axum::http::StatusCode::BAD_REQUEST,
            "无法撤销管理员权限，至少需要保留一个管理员",
        ));
    }

    let mut active: crate::models::user::ActiveModel = target.into();
    active.is_admin = Set(request.is_admin);
    active.last_login = Set(Utc::now());
    active.update(db).await.map_err(|err| {
        api_error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
    })?;

    Ok(build_set_admin_payload(&request.user_id, request.is_admin))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use super::{
        build_set_admin_payload, build_set_admin_request,
        build_set_admin_request_from_route_payload, SetAdminRouteRequest,
    };

    #[test]
    fn build_set_admin_request_keeps_existing_required_fields_contract() {
        let request = build_set_admin_request(&json!({
            "user_id": "user-1",
            "is_admin": true
        }))
        .expect("request should build");

        assert_eq!(request.user_id, "user-1");
        assert_eq!(request.is_admin, true);
    }

    #[test]
    fn build_set_admin_request_rejects_missing_fields() {
        let missing_user = build_set_admin_request(&json!({"is_admin": true}))
            .expect_err("missing user_id should fail");
        assert_eq!(missing_user.0, StatusCode::BAD_REQUEST);
        assert_eq!(missing_user.1 .0["detail"], "缺少 user_id");

        let missing_flag = build_set_admin_request(&json!({"user_id": "user-1"}))
            .expect_err("missing is_admin should fail");
        assert_eq!(missing_flag.0, StatusCode::BAD_REQUEST);
        assert_eq!(missing_flag.1 .0["detail"], "缺少 is_admin");
    }

    #[test]
    fn build_set_admin_payload_keeps_success_shell() {
        let grant = build_set_admin_payload("user-1", true);
        assert_eq!(grant["message"], "已授予管理员权限");
        assert_eq!(grant["user_id"], "user-1");
        assert_eq!(grant["is_admin"], true);

        let revoke = build_set_admin_payload("user-1", false);
        assert_eq!(revoke["message"], "已撤销管理员权限");
    }

    #[test]
    fn build_set_admin_request_from_route_payload_keeps_existing_contract() {
        let request = build_set_admin_request_from_route_payload(SetAdminRouteRequest {
            user_id: Some("user-1".to_string()),
            is_admin: Some(true),
        })
        .expect("request should build");

        assert_eq!(request.user_id, "user-1");
        assert!(request.is_admin);
    }

    #[test]
    fn build_set_admin_request_from_route_payload_rejects_missing_fields() {
        let missing_user = build_set_admin_request_from_route_payload(SetAdminRouteRequest {
            user_id: None,
            is_admin: Some(true),
        })
        .expect_err("missing user_id should fail");
        assert_eq!(missing_user.0, StatusCode::BAD_REQUEST);
        assert_eq!(missing_user.1 .0["detail"], "缺少 user_id");

        let missing_flag = build_set_admin_request_from_route_payload(SetAdminRouteRequest {
            user_id: Some("user-1".to_string()),
            is_admin: None,
        })
        .expect_err("missing is_admin should fail");
        assert_eq!(missing_flag.0, StatusCode::BAD_REQUEST);
        assert_eq!(missing_flag.1 .0["detail"], "缺少 is_admin");
    }
}
