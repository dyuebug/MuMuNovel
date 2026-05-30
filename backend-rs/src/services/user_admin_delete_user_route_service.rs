use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

use super::user_admin_route_service::{admin_count, api_error, find_user, UserAdminApiError};
use crate::models::{user, user_password};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeleteUserMode {
    RejectAdminTarget,
    AllowAdminTargetIfNotLastAdmin,
}

pub fn build_delete_user_payload() -> Value {
    json!({
        "success": true,
        "message": "用户已删除",
    })
}

pub fn build_delete_user_with_user_id_payload(target_user_id: &str) -> Value {
    json!({
        "message": "用户已删除",
        "user_id": target_user_id,
    })
}

async fn delete_user_with_mode(
    db: &DatabaseConnection,
    actor_user_id: &str,
    target_user_id: &str,
    mode: DeleteUserMode,
) -> Result<(), UserAdminApiError> {
    if target_user_id == actor_user_id {
        return Err(api_error(
            axum::http::StatusCode::BAD_REQUEST,
            "不能删除自己的账号",
        ));
    }

    let target = find_user(db, target_user_id).await?;
    match mode {
        DeleteUserMode::RejectAdminTarget if target.is_admin => {
            return Err(api_error(
                axum::http::StatusCode::BAD_REQUEST,
                "无法删除该用户（用户不存在或为管理员）",
            ));
        }
        DeleteUserMode::AllowAdminTargetIfNotLastAdmin if target.is_admin => {
            if admin_count(db).await? <= 1 {
                return Err(api_error(
                    axum::http::StatusCode::BAD_REQUEST,
                    "不能删除最后一个管理员账号",
                ));
            }
        }
        _ => {}
    }

    user_password::Entity::delete_by_id(target_user_id)
        .exec(db)
        .await
        .map_err(|err| {
            api_error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            )
        })?;
    user::Entity::delete_by_id(target_user_id)
        .exec(db)
        .await
        .map_err(|err| {
            api_error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            )
        })?;

    Ok(())
}

pub async fn delete_admin_user_payload(
    db: &DatabaseConnection,
    actor_user_id: &str,
    target_user_id: &str,
) -> Result<Value, UserAdminApiError> {
    delete_user_with_mode(
        db,
        actor_user_id,
        target_user_id,
        DeleteUserMode::AllowAdminTargetIfNotLastAdmin,
    )
    .await?;

    Ok(build_delete_user_payload())
}

pub async fn delete_standard_user_payload(
    db: &DatabaseConnection,
    actor_user_id: &str,
    target_user_id: &str,
) -> Result<Value, UserAdminApiError> {
    delete_user_with_mode(
        db,
        actor_user_id,
        target_user_id,
        DeleteUserMode::RejectAdminTarget,
    )
    .await?;

    Ok(build_delete_user_with_user_id_payload(target_user_id))
}

#[cfg(test)]
mod tests {
    use super::{
        build_delete_user_payload, build_delete_user_with_user_id_payload, DeleteUserMode,
    };

    #[test]
    fn build_delete_user_payload_keeps_success_shell() {
        let payload = build_delete_user_payload();

        assert_eq!(payload["success"], true);
        assert_eq!(payload["message"], "用户已删除");
    }

    #[test]
    fn build_delete_user_with_user_id_payload_keeps_legacy_user_response_shape() {
        let payload = build_delete_user_with_user_id_payload("user-123");

        assert_eq!(payload["message"], "用户已删除");
        assert_eq!(payload["user_id"], "user-123");
        assert!(payload.get("success").is_none());
    }

    #[test]
    fn delete_user_mode_variants_remain_distinct() {
        assert_ne!(
            DeleteUserMode::RejectAdminTarget,
            DeleteUserMode::AllowAdminTargetIfNotLastAdmin
        );
    }
}
