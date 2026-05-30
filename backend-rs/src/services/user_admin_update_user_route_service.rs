use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde::Deserialize;
use serde_json::{json, Value};

use super::user_admin_route_service::{
    admin_count, api_error, find_user, user_to_value, UserAdminApiError,
};
use crate::models::user;

#[derive(Debug, PartialEq, Eq)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub avatar_url_present: bool,
    pub avatar_url: Option<String>,
    pub trust_level: Option<i32>,
    pub is_admin: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct UpdateUserRouteRequest {
    #[serde(default)]
    pub display_name: Option<Value>,
    #[serde(default)]
    pub avatar_url: Option<Value>,
    #[serde(default)]
    pub trust_level: Option<Value>,
    #[serde(default)]
    pub is_admin: Option<Value>,
}

impl UpdateUserRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "display_name": self.display_name,
            "avatar_url": self.avatar_url,
            "trust_level": self.trust_level,
            "is_admin": self.is_admin,
        })
    }
}

pub fn build_update_user_request(body: &Value) -> UpdateUserRequest {
    UpdateUserRequest {
        display_name: body
            .get("display_name")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        avatar_url_present: body.get("avatar_url").is_some(),
        avatar_url: body
            .get("avatar_url")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        trust_level: body
            .get("trust_level")
            .and_then(|value| value.as_i64())
            .map(|value| value as i32),
        is_admin: body.get("is_admin").and_then(|value| value.as_bool()),
    }
}

pub fn build_update_user_request_from_route_payload(
    body: UpdateUserRouteRequest,
) -> UpdateUserRequest {
    build_update_user_request(&body.into_body())
}

pub fn should_block_last_admin_removal(
    request: &UpdateUserRequest,
    existing_is_admin: bool,
) -> bool {
    matches!(request.is_admin, Some(false)) && existing_is_admin
}

pub fn build_update_user_payload(saved: &user::Model) -> Value {
    json!({
        "success": true,
        "message": "用户信息更新成功",
        "user": user_to_value(saved),
    })
}

pub async fn update_user_payload(
    db: &DatabaseConnection,
    user_id: &str,
    request: &UpdateUserRequest,
) -> Result<Value, UserAdminApiError> {
    let existing = find_user(db, user_id).await?;

    if should_block_last_admin_removal(request, existing.is_admin) && admin_count(db).await? <= 1 {
        return Err(api_error(
            axum::http::StatusCode::BAD_REQUEST,
            "不能取消最后一个管理员的权限",
        ));
    }

    let mut active: user::ActiveModel = existing.into();
    if let Some(display_name) = &request.display_name {
        active.display_name = Set(display_name.clone());
    }
    if request.avatar_url_present {
        active.avatar_url = Set(request.avatar_url.clone());
    }
    if let Some(trust_level) = request.trust_level {
        active.trust_level = Set(trust_level);
    }
    if let Some(is_admin) = request.is_admin {
        active.is_admin = Set(is_admin);
    }
    active.last_login = Set(Utc::now());

    let saved = active.update(db).await.map_err(|err| {
        api_error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
    })?;

    Ok(build_update_user_payload(&saved))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use crate::models::user;

    use super::{
        build_update_user_payload, build_update_user_request,
        build_update_user_request_from_route_payload, should_block_last_admin_removal,
        UpdateUserRouteRequest,
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
                .with_ymd_and_hms(2026, 5, 22, 5, 30, 0)
                .single()
                .expect("datetime should be valid"),
            last_login: Utc
                .with_ymd_and_hms(2026, 5, 22, 5, 31, 0)
                .single()
                .expect("datetime should be valid"),
        }
    }

    #[test]
    fn build_update_user_request_reads_optional_fields_and_presence() {
        let request = build_update_user_request(&json!({
            "display_name": "Alice",
            "avatar_url": null,
            "trust_level": 3,
            "is_admin": false
        }));

        assert_eq!(request.display_name, Some("Alice".to_string()));
        assert!(request.avatar_url_present);
        assert_eq!(request.avatar_url, None);
        assert_eq!(request.trust_level, Some(3));
        assert_eq!(request.is_admin, Some(false));
    }

    #[test]
    fn should_block_last_admin_removal_only_when_revoke_is_requested() {
        let revoke_request = build_update_user_request(&json!({"is_admin": false}));
        assert!(should_block_last_admin_removal(&revoke_request, true));
        assert!(!should_block_last_admin_removal(&revoke_request, false));

        let unchanged_request = build_update_user_request(&json!({}));
        assert!(!should_block_last_admin_removal(&unchanged_request, true));
    }

    #[test]
    fn build_update_user_request_from_route_payload_keeps_existing_contract() {
        let request = build_update_user_request_from_route_payload(UpdateUserRouteRequest {
            display_name: Some(json!("Alice")),
            avatar_url: Some(json!(null)),
            trust_level: Some(json!(3)),
            is_admin: Some(json!(false)),
        });

        assert_eq!(request.display_name, Some("Alice".to_string()));
        assert!(request.avatar_url_present);
        assert_eq!(request.avatar_url, None);
        assert_eq!(request.trust_level, Some(3));
        assert_eq!(request.is_admin, Some(false));
    }

    #[test]
    fn build_update_user_payload_keeps_success_shell() {
        let payload = build_update_user_payload(&user_model());

        assert_eq!(payload["success"], true);
        assert_eq!(payload["message"], "用户信息更新成功");
        assert_eq!(payload["user"]["user_id"], "user-1");
    }
}
