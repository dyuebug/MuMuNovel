use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::api::user_admin_shared_owner::{
    api_error, build_admin_reset_password_payload,
    build_admin_reset_password_request_from_route_payload, check_admin, delete_admin_user_payload,
    find_user, hash_password, reset_user_password_workflow, user_to_value,
    AdminResetPasswordRouteRequest, PasswordResetMode, UserAdminApiError,
};
use crate::models::{user, user_password};
use crate::services::auth::Claims;

const ADMIN_USERS_ROUTE: &str = "/admin/users";
const ADMIN_USER_DETAIL_ROUTE: &str = "/admin/users/{userId}";
const ADMIN_USER_TOGGLE_STATUS_ROUTE: &str = "/admin/users/{userId}/toggle-status";
const ADMIN_USER_RESET_PASSWORD_ROUTE: &str = "/admin/users/{userId}/reset-password";

#[cfg(test)]
fn build_admin_route_owner_contract() -> Value {
    json!({
        "owner": "admin-users",
        "rust_owner": "backend-rs/src/api/admin.rs",
        "route_prefix": "/api",
        "routes": {
            "list": ADMIN_USERS_ROUTE,
            "create": ADMIN_USERS_ROUTE,
            "detail_update": ADMIN_USER_DETAIL_ROUTE,
            "detail_delete": ADMIN_USER_DETAIL_ROUTE,
            "toggle_status": ADMIN_USER_TOGGLE_STATUS_ROUTE,
            "reset_password": ADMIN_USER_RESET_PASSWORD_ROUTE
        },
        "method_contract": {
            "list": ["GET"],
            "create": ["POST"],
            "detail_update": ["PUT"],
            "detail_delete": ["DELETE"],
            "toggle_status": ["POST"],
            "reset_password": ["POST"]
        },
        "service_handoffs": {
            "route_owner": "backend-rs/src/api/admin.rs",
            "admin_guard_and_shared_user_ops_owner": "backend-rs/src/api/user_admin_shared_owner.rs",
            "password_reset_workflow_owner": "backend-rs/src/api/user_admin_shared_owner.rs"
        },
        "behavior_contract": {
            "route_entrypoints": [
                "list_users",
                "create_user",
                "update_user",
                "delete_user",
                "toggle_user_status",
                "reset_password"
            ],
            "admin_guarded_entrypoints": [
                "list_users",
                "create_user",
                "update_user",
                "delete_user",
                "toggle_user_status",
                "reset_password"
            ],
            "route_local_request_contracts": [
                "CreateUserRouteRequest",
                "UpdateUserRouteRequest",
                "ToggleUserStatusRouteRequest",
                "AdminResetPasswordRouteRequest"
            ],
            "shared_service_consumers": [
                "check_admin",
                "find_user",
                "user_to_value",
                "hash_password",
                "delete_admin_user_payload",
                "reset_user_password_workflow",
                "build_admin_reset_password_payload"
            ],
            "last_admin_protection": "update_user and delete_user block removing the final admin account"
        },
        "readiness_evidence": [
            "admin-users-list-auth-guard-rust",
            "admin-users-create-auth-guard-rust",
            "admin-users-update-auth-guard-rust",
            "admin-users-delete-auth-guard-rust",
            "admin-users-toggle-status-auth-guard-rust",
            "admin-users-reset-password-auth-guard-rust",
            "admin-users-list-business-rust",
            "admin-users-update-business-rust",
            "admin-users-toggle-status-business-rust",
            "admin-users-reset-password-business-rust",
            "admin-users-delete-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-admin-business-owner",
            "business_probes": [
                "admin-users-list-business-rust",
                "admin-users-update-business-rust",
                "admin-users-toggle-status-business-rust",
                "admin-users-reset-password-business-rust",
                "admin-users-delete-business-rust"
            ],
            "route_readiness_probes": [
                "admin-users-list-auth-guard-rust",
                "admin-users-create-auth-guard-rust",
                "admin-users-update-auth-guard-rust",
                "admin-users-delete-auth-guard-rust",
                "admin-users-toggle-status-auth-guard-rust",
                "admin-users-reset-password-auth-guard-rust"
            ],
            "python_fallback_probe_count": 0,
            "manifest_profile": "phase5-admin-business-owner",
            "profile_kind": "logged_in_business_readiness"
        },
        "business_smoke_status": {
            "owner_profile": "phase5-admin-business-owner",
            "owner_profile_probe_count": 6,
            "business_probe_count": 5,
            "fixture_probe_count": 1,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit user model source-map freeze/delete/repoint approval with same-round rollback policy",
        "source_map_files": [
            "backend/migrator_app/models/user.py"
        ],
        "rollback_boundary": {
            "source_map_policy": "admin_users_route_source_map_deleted_remaining_user_model_only",
            "python_route_files_status": "admin_users_route_source_map_deleted_remaining_user_model_only",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "freeze_reason": "Rust admin route owner covers admin users list/update/delete/toggle-status/reset-password logged-in business smoke plus auth-guard probes; the Python users/admin route shells and old runtime-store facade have been physically deleted, and the remaining admin source map is now limited to the shared user model definition.",
            "rollback_files": []
        },
        "migration_policy": "Do not bind admin-users readiness to the /api/users business owner profile; the Python users/admin route shells and old runtime-store facade have been physically deleted, and admin completion still requires explicit user model source-map closeout."
    })
}

#[derive(Debug, PartialEq, Eq)]
struct CreateUserRequest {
    username: String,
    display_name: String,
    avatar_url: Option<String>,
    is_admin: bool,
    trust_level: i32,
    password: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct CreateUserRouteRequest {
    #[serde(default)]
    username: Option<Value>,
    #[serde(default)]
    display_name: Option<Value>,
    #[serde(default)]
    avatar_url: Option<Value>,
    #[serde(default)]
    is_admin: Option<Value>,
    #[serde(default)]
    trust_level: Option<Value>,
    #[serde(default)]
    password: Option<Value>,
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

#[derive(Debug, PartialEq, Eq)]
struct UpdateUserRequest {
    display_name: Option<String>,
    avatar_url_present: bool,
    avatar_url: Option<String>,
    trust_level: Option<i32>,
    is_admin: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct UpdateUserRouteRequest {
    #[serde(default)]
    display_name: Option<Value>,
    #[serde(default)]
    avatar_url: Option<Value>,
    #[serde(default)]
    trust_level: Option<Value>,
    #[serde(default)]
    is_admin: Option<Value>,
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

#[derive(Debug, PartialEq, Eq)]
struct ToggleUserStatusRequest {
    is_active: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct ToggleUserStatusRouteRequest {
    #[serde(default)]
    is_active: Option<Value>,
}

impl ToggleUserStatusRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "is_active": self.is_active,
        })
    }
}

fn default_password_for_username(username: &str) -> String {
    format!("{username}@666")
}

fn build_create_user_request(body: &Value) -> Result<CreateUserRequest, UserAdminApiError> {
    let username = body
        .get("username")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "缺少 username"))?;

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

fn build_create_user_request_from_route_payload(
    body: CreateUserRouteRequest,
) -> Result<CreateUserRequest, UserAdminApiError> {
    build_create_user_request(&body.into_body())
}

fn build_create_user_payload(
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

async fn create_user_payload(
    db: &DatabaseConnection,
    request: &CreateUserRequest,
) -> Result<Value, UserAdminApiError> {
    let existing = user::Entity::find()
        .filter(user::Column::Username.eq(&request.username))
        .one(db)
        .await
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    if existing.is_some() {
        return Err(api_error(StatusCode::CONFLICT, "用户名已存在"));
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
    user_model
        .insert(db)
        .await
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let has_custom_password = request.password.is_some();
    let default_password = default_password_for_username(&request.username);
    let actual_password = request.password.as_deref().unwrap_or(&default_password);
    let password_hash = hash_password(actual_password)
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error))?;

    let password_model = user_password::ActiveModel {
        user_id: Set(user_id.clone()),
        username: Set(request.username.clone()),
        password_hash: Set(password_hash),
        has_custom_password: Set(has_custom_password),
        created_at: Set(now),
        updated_at: Set(now),
    };
    password_model
        .insert(db)
        .await
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let created = user::Entity::find_by_id(&user_id)
        .one(db)
        .await
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

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

fn build_update_user_request(body: &Value) -> UpdateUserRequest {
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

fn build_update_user_request_from_route_payload(body: UpdateUserRouteRequest) -> UpdateUserRequest {
    build_update_user_request(&body.into_body())
}

fn should_block_last_admin_removal(request: &UpdateUserRequest, existing_is_admin: bool) -> bool {
    matches!(request.is_admin, Some(false)) && existing_is_admin
}

fn build_update_user_payload(saved: &user::Model) -> Value {
    json!({
        "success": true,
        "message": "用户信息更新成功",
        "user": user_to_value(saved),
    })
}

async fn update_user_payload(
    db: &DatabaseConnection,
    user_id: &str,
    request: &UpdateUserRequest,
) -> Result<Value, UserAdminApiError> {
    let existing = find_user(db, user_id).await?;

    if should_block_last_admin_removal(request, existing.is_admin)
        && crate::api::user_admin_shared_owner::admin_count(db).await? <= 1
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
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

    let saved = active
        .update(db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(build_update_user_payload(&saved))
}

fn build_toggle_user_status_request(body: &Value) -> ToggleUserStatusRequest {
    ToggleUserStatusRequest {
        is_active: body
            .get("is_active")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    }
}

fn build_toggle_user_status_request_from_route_payload(
    body: ToggleUserStatusRouteRequest,
) -> ToggleUserStatusRequest {
    build_toggle_user_status_request(&body.into_body())
}

fn build_toggle_user_status_payload(is_active: bool) -> Value {
    let status_text = if is_active { "启用" } else { "禁用" };
    json!({
        "success": true,
        "message": format!("用户已{}", status_text),
        "is_active": is_active,
    })
}

async fn toggle_user_status_payload(
    db: &DatabaseConnection,
    actor_user_id: &str,
    target_user_id: &str,
    request: &ToggleUserStatusRequest,
) -> Result<Value, UserAdminApiError> {
    if target_user_id == actor_user_id {
        return Err(api_error(StatusCode::BAD_REQUEST, "不能禁用自己的账号"));
    }

    let existing = find_user(db, target_user_id).await?;
    let mut active: user::ActiveModel = existing.into();
    active.trust_level = Set(if request.is_active { 0 } else { -1 });
    active
        .update(db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(build_toggle_user_status_payload(request.is_active))
}

async fn list_users(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let users = user::Entity::find().all(&db).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })?;

    let users_data: Vec<Value> = users.iter().map(user_to_value).collect();

    Ok(Json(json!({
        "total": users_data.len(),
        "users": users_data,
    })))
}

async fn create_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<CreateUserRouteRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let request = build_create_user_request_from_route_payload(body)?;
    let payload = create_user_payload(&db, &request).await?;
    Ok((StatusCode::CREATED, Json(payload)))
}

async fn update_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
    Json(body): Json<UpdateUserRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let request = build_update_user_request_from_route_payload(body);
    let payload = update_user_payload(&db, &user_id, &request).await?;
    Ok(Json(payload))
}

async fn toggle_user_status(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
    Json(body): Json<ToggleUserStatusRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let request = build_toggle_user_status_request_from_route_payload(body);
    let payload = toggle_user_status_payload(&db, &claims.sub, &user_id, &request).await?;
    Ok(Json(payload))
}

async fn reset_password(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
    Json(body): Json<AdminResetPasswordRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;
    let request = build_admin_reset_password_request_from_route_payload(body);

    let outcome = reset_user_password_workflow(
        &db,
        &user_id,
        request.new_password.as_deref(),
        PasswordResetMode::UseDefaultWhenMissing,
    )
    .await?;

    Ok(Json(build_admin_reset_password_payload(
        &outcome.actual_password,
    )))
}

async fn delete_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;
    let payload = delete_admin_user_payload(&db, &claims.sub, &user_id).await?;
    Ok(Json(payload))
}

pub fn routes() -> Router {
    Router::new()
        .route(ADMIN_USERS_ROUTE, get(list_users))
        .route(ADMIN_USERS_ROUTE, post(create_user))
        .route(ADMIN_USER_DETAIL_ROUTE, put(update_user))
        .route(ADMIN_USER_DETAIL_ROUTE, delete(delete_user))
        .route(ADMIN_USER_TOGGLE_STATUS_ROUTE, post(toggle_user_status))
        .route(ADMIN_USER_RESET_PASSWORD_ROUTE, post(reset_password))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use crate::models::user;

    use super::{
        build_admin_route_owner_contract, build_create_user_payload, build_create_user_request,
        build_create_user_request_from_route_payload, build_toggle_user_status_payload,
        build_toggle_user_status_request, build_toggle_user_status_request_from_route_payload,
        build_update_user_payload, build_update_user_request,
        build_update_user_request_from_route_payload, should_block_last_admin_removal,
        CreateUserRouteRequest, ToggleUserStatusRouteRequest, UpdateUserRouteRequest,
        ADMIN_USERS_ROUTE, ADMIN_USER_DETAIL_ROUTE, ADMIN_USER_RESET_PASSWORD_ROUTE,
        ADMIN_USER_TOGGLE_STATUS_ROUTE,
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
    fn should_keep_admin_route_paths_stable() {
        assert_eq!(ADMIN_USERS_ROUTE, "/admin/users");
        assert_eq!(ADMIN_USER_DETAIL_ROUTE, "/admin/users/{userId}");
        assert_eq!(
            ADMIN_USER_TOGGLE_STATUS_ROUTE,
            "/admin/users/{userId}/toggle-status"
        );
        assert_eq!(
            ADMIN_USER_RESET_PASSWORD_ROUTE,
            "/admin/users/{userId}/reset-password"
        );
    }

    #[test]
    fn should_publish_admin_route_owner_contract() {
        let contract = build_admin_route_owner_contract();

        assert_eq!(contract["owner"], "admin-users");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/admin.rs");
        assert_eq!(
            contract["service_handoffs"]["route_owner"],
            "backend-rs/src/api/admin.rs"
        );
        assert_eq!(
            contract["service_handoffs"]["admin_guard_and_shared_user_ops_owner"],
            "backend-rs/src/api/user_admin_shared_owner.rs"
        );
        assert_eq!(
            contract["service_handoffs"]["password_reset_workflow_owner"],
            "backend-rs/src/api/user_admin_shared_owner.rs"
        );
        assert!(contract["behavior_contract"]["route_entrypoints"]
            .as_array()
            .expect("route entrypoints")
            .contains(&json!("create_user")));
        assert!(contract["readiness_evidence"]
            .as_array()
            .expect("readiness evidence")
            .contains(&json!("admin-users-delete-auth-guard-rust")));
        assert!(contract["readiness_evidence"]
            .as_array()
            .expect("readiness evidence")
            .contains(&json!("admin-users-delete-business-rust")));
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-admin-business-owner"
        );
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes")
            .contains(&json!("admin-users-reset-password-business-rust")));
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["owner_profile"]["manifest_profile"],
            "phase5-admin-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["profile_kind"],
            "logged_in_business_readiness"
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["owner_profile_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(5)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(1)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit user model source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_route_files_status"],
            "admin_users_route_source_map_deleted_remaining_user_model_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"],
            json!([])
        );
        assert_eq!(contract["rollback_boundary"]["rollback_files"], json!([]));
        assert_eq!(
            contract["migration_policy"],
            "Do not bind admin-users readiness to the /api/users business owner profile; the Python users/admin route shells and old runtime-store facade have been physically deleted, and admin completion still requires explicit user model source-map closeout."
        );
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
        assert!(!request.is_admin);
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
        assert!(!request.is_admin);
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

    #[test]
    fn build_toggle_user_status_request_defaults_to_false() {
        let request = build_toggle_user_status_request(&json!({}));
        assert!(!request.is_active);
    }

    #[test]
    fn build_toggle_user_status_request_reads_boolean_flag() {
        let request = build_toggle_user_status_request(&json!({"is_active": true}));
        assert!(request.is_active);
    }

    #[test]
    fn build_toggle_user_status_request_from_route_payload_keeps_existing_contract() {
        let request =
            build_toggle_user_status_request_from_route_payload(ToggleUserStatusRouteRequest {
                is_active: Some(json!(true)),
            });
        assert!(request.is_active);
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
