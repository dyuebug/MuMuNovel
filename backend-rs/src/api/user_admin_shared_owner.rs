use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::{http::StatusCode, response::Json};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::{user, user_password};
use crate::services::auth::Claims;

pub type UserAdminApiError = (StatusCode, Json<Value>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeleteUserMode {
    RejectAdminTarget,
    AllowAdminTargetIfNotLastAdmin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordResetMode {
    UseDefaultWhenMissing,
    UseDefaultWhenMissingOrEmpty,
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordResetOutcome {
    pub user_id: String,
    pub username: String,
    pub actual_password: String,
    pub used_default_password: bool,
}

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

fn database_error(error: impl ToString) -> UserAdminApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
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

pub fn build_user_reset_password_request(
    body: &Value,
) -> Result<UserResetPasswordRequest, UserAdminApiError> {
    let user_id = body
        .get("user_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "缺少 user_id"))?;

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
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    let now = Utc::now();

    match user_password::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(database_error)?
    {
        Some(password) => {
            let mut active: user_password::ActiveModel = password.into();
            active.password_hash = sea_orm::Set(password_hash);
            active.has_custom_password = sea_orm::Set(has_custom_password);
            active.updated_at = sea_orm::Set(now);
            active.update(db).await.map_err(database_error)?;
        }
        None => {
            let password = user_password::ActiveModel {
                user_id: sea_orm::Set(user_id.to_string()),
                username: sea_orm::Set(target.username.clone()),
                password_hash: sea_orm::Set(password_hash),
                has_custom_password: sea_orm::Set(has_custom_password),
                created_at: sea_orm::Set(now),
                updated_at: sea_orm::Set(now),
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

fn build_delete_user_payload() -> Value {
    json!({
        "success": true,
        "message": "用户已删除",
    })
}

fn build_delete_user_with_user_id_payload(target_user_id: &str) -> Value {
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
        return Err(api_error(StatusCode::BAD_REQUEST, "不能删除自己的账号"));
    }

    let target = find_user(db, target_user_id).await?;
    match mode {
        DeleteUserMode::RejectAdminTarget if target.is_admin => {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "无法删除该用户（用户不存在或为管理员）",
            ));
        }
        DeleteUserMode::AllowAdminTargetIfNotLastAdmin if target.is_admin => {
            if admin_count(db).await? <= 1 {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "不能删除最后一个管理员账号",
                ));
            }
        }
        _ => {}
    }

    user_password::Entity::delete_by_id(target_user_id)
        .exec(db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    user::Entity::delete_by_id(target_user_id)
        .exec(db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

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

pub async fn set_admin_payload(
    db: &DatabaseConnection,
    actor_user_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<Value, UserAdminApiError> {
    if user_id == actor_user_id && !is_admin {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "不能撤销自己的管理员权限",
        ));
    }

    let target = find_user(db, user_id).await?;
    if target.is_admin && !is_admin && admin_count(db).await? <= 1 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "无法撤销管理员权限，至少需要保留一个管理员",
        ));
    }

    let mut active: crate::models::user::ActiveModel = target.into();
    active.is_admin = sea_orm::Set(is_admin);
    active.last_login = sea_orm::Set(chrono::Utc::now());
    active
        .update(db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let action = if is_admin { "授予" } else { "撤销" };
    Ok(json!({
        "message": format!("已{action}管理员权限"),
        "user_id": user_id,
        "is_admin": is_admin,
    }))
}

#[cfg(test)]
fn build_user_admin_shared_owner_contract() -> Value {
    json!({
        "owner": "user_admin_shared_owner",
        "rust_owner": "backend-rs/src/api/user_admin_shared_owner.rs",
        "scope": "shared_users_admin_guard_password_reset_delete_set_admin_payload_owner",
        "consuming_route_owners": [
            "backend-rs/src/api/users.rs",
            "backend-rs/src/api/admin.rs"
        ],
        "python_source_map": [
            "backend/migrator_app/models/user.py"
        ],
        "shared_behavior_contract": {
            "admin_guard": "check_admin preserves 403 detail for non-admin callers",
            "user_projection": "user_to_value preserves user/admin payload fields and active flag projection",
            "password_hashing": "hash_password keeps argon2 salted password hashing",
            "password_defaults": "default_password_for_username keeps username@666 default",
            "user_reset_password": "User reset requires user_id and treats missing or empty new_password as default password",
            "admin_reset_password": "Admin reset targets path userId and treats missing password as default while preserving empty string as custom password",
            "delete_standard_user": "Standard users delete blocks self-delete and rejects admin targets",
            "delete_admin_user": "Admin users delete blocks self-delete and protects the last admin account",
            "set_admin": "set_admin_payload blocks self-demotion and protects the last admin account"
        },
        "readiness_evidence": {
            "users_route_group": [
                "users-set-admin-auth-guard-rust",
                "users-reset-password-auth-guard-rust",
                "users-set-admin-grant-business-rust",
                "users-set-admin-revoke-business-rust",
                "users-reset-password-business-rust",
                "users-delete-business-rust"
            ],
            "admin_route_group": [
                "admin-users-delete-auth-guard-rust",
                "admin-users-reset-password-auth-guard-rust",
                "admin-users-toggle-status-auth-guard-rust",
                "admin-users-delete-business-rust",
                "admin-users-reset-password-business-rust",
                "admin-users-toggle-status-business-rust"
            ]
        },
        "owner_profile": {
            "name": "user-admin-shared-owner",
            "profile_kind": "shared_route_owner_support",
            "covered_route_owner_profiles": [
                "phase5-users-business-owner",
                "phase5-admin-business-owner"
            ],
            "business_probes": [
                "users-set-admin-grant-business-rust",
                "users-set-admin-revoke-business-rust",
                "users-reset-password-business-rust",
                "users-delete-business-rust",
                "admin-users-delete-business-rust",
                "admin-users-reset-password-business-rust",
                "admin-users-toggle-status-business-rust"
            ],
            "route_readiness_probes": [
                "users-set-admin-auth-guard-rust",
                "users-reset-password-auth-guard-rust",
                "admin-users-delete-auth-guard-rust",
                "admin-users-toggle-status-auth-guard-rust",
                "admin-users-reset-password-auth-guard-rust"
            ],
            "python_fallback_probe_count": 0,
            "manifest_profile": null
        },
        "business_smoke_status": {
            "owner_profile": "user-admin-shared-owner",
            "covered_route_owner_profiles": [
                "phase5-users-business-owner",
                "phase5-admin-business-owner"
            ],
            "readiness_probe_count": 12,
            "business_probe_count": 7,
            "auth_guard_probe_count": 5,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_shared_rust_owner_profiles"
        },
        "next_cutover_gate": "explicit user model source-map freeze/delete/repoint approval across users/admin with same-round rollback policy",
        "migration_policy": "User/admin shared business smoke is covered by phase5-users-business-owner and phase5-admin-business-owner; the Python users/admin route shells and old runtime-store facade have been physically deleted, and final completion now requires explicit user model source-map freeze/delete/repoint approval across both route groups with same-round rollback policy.",
        "validation_boundary": [
            "cargo test api::user_admin_shared_owner",
            "cargo test api::users",
            "cargo test api::admin",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-users-business-owner --route-group users",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile route-groups --route-group admin",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "users_admin_shared_owner_source_map_deleted_remaining_user_model_only",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "freeze_reason": "Rust shared owner now owns admin guard, user projection, password reset, delete, and set-admin behavior consumed by users/admin routes; the Python users/admin route shells and old runtime-store facade have been physically deleted, and the remaining shared source map is now limited to the user model definition that must be reviewed across both route groups in the same round.",
            "rollback_files": []
        }
    })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use crate::models::user;
    use crate::services::auth::Claims;

    use super::{
        api_error, build_admin_reset_password_payload, build_admin_reset_password_request,
        build_admin_reset_password_request_from_route_payload, build_delete_user_payload,
        build_delete_user_with_user_id_payload, build_user_admin_shared_owner_contract,
        build_user_reset_password_payload, build_user_reset_password_request,
        build_user_reset_password_request_from_route_payload, check_admin,
        default_password_for_username, hash_password, resolve_password_reset_value, user_to_value,
        AdminResetPasswordRouteRequest, DeleteUserMode, PasswordResetMode, PasswordResetOutcome,
        UserResetPasswordRouteRequest,
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

    #[test]
    fn should_publish_user_admin_shared_owner_contract() {
        let contract = build_user_admin_shared_owner_contract();

        assert_eq!(contract["owner"], "user_admin_shared_owner");
        assert_eq!(
            contract["rust_owner"],
            "backend-rs/src/api/user_admin_shared_owner.rs"
        );
        assert!(contract["consuming_route_owners"]
            .as_array()
            .expect("consuming route owners")
            .iter()
            .any(|owner| owner == "backend-rs/src/api/users.rs"));
        assert!(contract["consuming_route_owners"]
            .as_array()
            .expect("consuming route owners")
            .iter()
            .any(|owner| owner == "backend-rs/src/api/admin.rs"));
        assert_eq!(contract["owner_profile"]["name"], "user-admin-shared-owner");
        assert_eq!(
            contract["owner_profile"]["covered_route_owner_profiles"][0],
            "phase5-users-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["covered_route_owner_profiles"][1],
            "phase5-admin-business-owner"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert!(contract["readiness_evidence"]["users_route_group"]
            .as_array()
            .expect("users evidence")
            .iter()
            .any(|probe| probe == "users-reset-password-business-rust"));
        assert!(contract["readiness_evidence"]["admin_route_group"]
            .as_array()
            .expect("admin evidence")
            .iter()
            .any(|probe| probe == "admin-users-reset-password-auth-guard-rust"));
        assert!(contract["readiness_evidence"]["admin_route_group"]
            .as_array()
            .expect("admin evidence")
            .iter()
            .any(|probe| probe == "admin-users-reset-password-business-rust"));
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes")
            .iter()
            .any(|probe| probe == "admin-users-delete-business-rust"));
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_shared_rust_owner_profiles"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            12
        );
        assert_eq!(contract["business_smoke_status"]["business_probe_count"], 7);
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            5
        );
        assert_eq!(contract["business_smoke_status"]["fixture_probe_count"], 0);
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit user model source-map freeze/delete/repoint approval across users/admin with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("phase5-admin-business-owner"));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"],
            json!([])
        );
        assert_eq!(contract["rollback_boundary"]["rollback_files"], json!([]));
    }
}
