use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde::Deserialize;
use serde_json::{json, Value};

use super::user_admin_route_service::{api_error, find_user, UserAdminApiError};

#[derive(Debug, PartialEq, Eq)]
pub struct ToggleUserStatusRequest {
    pub is_active: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ToggleUserStatusRouteRequest {
    #[serde(default)]
    pub is_active: Option<Value>,
}

impl ToggleUserStatusRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "is_active": self.is_active,
        })
    }
}

pub fn build_toggle_user_status_request(body: &Value) -> ToggleUserStatusRequest {
    ToggleUserStatusRequest {
        is_active: body
            .get("is_active")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    }
}

pub fn build_toggle_user_status_request_from_route_payload(
    body: ToggleUserStatusRouteRequest,
) -> ToggleUserStatusRequest {
    build_toggle_user_status_request(&body.into_body())
}

pub fn build_toggle_user_status_payload(is_active: bool) -> Value {
    let status_text = if is_active { "启用" } else { "禁用" };
    json!({
        "success": true,
        "message": format!("用户已{}", status_text),
        "is_active": is_active,
    })
}

pub async fn toggle_user_status_payload(
    db: &DatabaseConnection,
    actor_user_id: &str,
    target_user_id: &str,
    request: &ToggleUserStatusRequest,
) -> Result<Value, UserAdminApiError> {
    if target_user_id == actor_user_id {
        return Err(api_error(
            axum::http::StatusCode::BAD_REQUEST,
            "不能禁用自己的账号",
        ));
    }

    let existing = find_user(db, target_user_id).await?;
    let mut active: crate::models::user::ActiveModel = existing.into();
    active.trust_level = Set(if request.is_active { 0 } else { -1 });
    active.update(db).await.map_err(|err| {
        api_error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
    })?;

    Ok(build_toggle_user_status_payload(request.is_active))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_toggle_user_status_payload, build_toggle_user_status_request,
        build_toggle_user_status_request_from_route_payload, ToggleUserStatusRouteRequest,
    };

    #[test]
    fn build_toggle_user_status_request_defaults_to_false() {
        let request = build_toggle_user_status_request(&json!({}));
        assert_eq!(request.is_active, false);
    }

    #[test]
    fn build_toggle_user_status_request_reads_boolean_flag() {
        let request = build_toggle_user_status_request(&json!({"is_active": true}));
        assert_eq!(request.is_active, true);
    }

    #[test]
    fn build_toggle_user_status_request_from_route_payload_keeps_existing_contract() {
        let request =
            build_toggle_user_status_request_from_route_payload(ToggleUserStatusRouteRequest {
                is_active: Some(json!(true)),
            });
        assert_eq!(request.is_active, true);
    }

    #[test]
    fn build_toggle_user_status_payload_keeps_success_shell() {
        let enabled = build_toggle_user_status_payload(true);
        assert_eq!(enabled["success"], true);
        assert_eq!(enabled["message"], "用户已启用");
        assert_eq!(enabled["is_active"], true);

        let disabled = build_toggle_user_status_payload(false);
        assert_eq!(disabled["message"], "用户已禁用");
        assert_eq!(disabled["is_active"], false);
    }
}
