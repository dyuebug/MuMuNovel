use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::user_admin_shared_owner::{
    api_error, build_user_reset_password_payload,
    build_user_reset_password_request_from_route_payload, check_admin,
    delete_standard_user_payload, find_user, reset_user_password_workflow, set_admin_payload,
    user_to_value, PasswordResetMode, UserAdminApiError, UserResetPasswordRouteRequest,
};
use crate::models::user;
use crate::services::auth::Claims;

const USERS_LIST_ROUTE: &str = "/users";
const USERS_CURRENT_ROUTE: &str = "/users/current";
const USERS_SET_ADMIN_ROUTE: &str = "/users/set-admin";
const USERS_RESET_PASSWORD_ROUTE: &str = "/users/reset-password";
const USERS_DETAIL_ROUTE: &str = "/users/{user_id}";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SetAdminRouteRequest {
    user_id: Option<String>,
    is_admin: Option<bool>,
}

#[derive(Debug, PartialEq, Eq)]
struct SetAdminRequest {
    user_id: String,
    is_admin: bool,
}

fn build_set_admin_request(body: &Value) -> Result<SetAdminRequest, UserAdminApiError> {
    let user_id = body
        .get("user_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "缺少 user_id"))?;
    let is_admin = body
        .get("is_admin")
        .and_then(|value| value.as_bool())
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "缺少 is_admin"))?;

    Ok(SetAdminRequest {
        user_id: user_id.to_string(),
        is_admin,
    })
}

fn build_set_admin_request_from_route_payload(
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

#[cfg(test)]
fn build_users_route_owner_contract() -> Value {
    json!({
        "owner": "users",
        "scope": "users_current_list_detail_set_admin_delete_reset_password_route_group",
        "python_source_map": [
            "backend/migrator_app/models/user.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/api/users.rs",
            "backend-rs/src/api/user_admin_shared_owner.rs",
            "deploy/strangler-gateway-probes.json"
        ],
        "route_contract": {
            "list": USERS_LIST_ROUTE,
            "current": USERS_CURRENT_ROUTE,
            "detail": USERS_DETAIL_ROUTE,
            "delete": USERS_DETAIL_ROUTE,
            "set_admin": USERS_SET_ADMIN_ROUTE,
            "reset_password": USERS_RESET_PASSWORD_ROUTE
        },
        "behavior_contract": {
            "route_entrypoints": [
                "list_users",
                "get_current_user",
                "get_user",
                "set_admin",
                "delete_user",
                "reset_user_password"
            ],
            "admin_guarded_entrypoints": [
                "list_users",
                "get_user",
                "set_admin",
                "delete_user",
                "reset_user_password"
            ],
            "service_consumers": [
                "check_admin",
                "find_user",
                "user_to_value",
                "set_admin_payload",
                "delete_standard_user_payload",
                "reset_user_password_workflow",
                "build_user_reset_password_payload"
            ],
            "request_contracts": [
                "SetAdminRouteRequest",
                "UserResetPasswordRouteRequest"
            ],
            "protected_self_reset_policy": "reset_user_password rejects resetting the caller's own password"
        },
        "readiness_evidence": [
            "users-current-auth-guard-rust",
            "users-list-auth-guard-rust",
            "users-set-admin-auth-guard-rust",
            "users-reset-password-auth-guard-rust",
            "users-current-business-rust",
            "users-list-business-rust",
            "users-detail-business-rust",
            "users-set-admin-grant-business-rust",
            "users-set-admin-revoke-business-rust",
            "users-reset-password-business-rust",
            "users-delete-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-users-business-owner",
            "business_probes": [
                "users-current-business-rust",
                "users-list-business-rust",
                "users-detail-business-rust",
                "users-set-admin-grant-business-rust",
                "users-set-admin-revoke-business-rust",
                "users-reset-password-business-rust",
                "users-delete-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-users-business-owner",
            "owner_profile_probe_count": 8,
            "business_probe_count": 7,
            "fixture_probe_count": 1,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit user model source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Users route business smoke is covered by phase5-users-business-owner; the Python users/admin route shells and old runtime-store facade have been physically deleted, and final completion now requires explicit user model source-map freeze/delete/repoint approval with same-round rollback policy.",
        "validation_boundary": [
            "cargo test api::users",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-users-business-owner",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "users_route_source_map_deleted_remaining_user_model_only",
            "python_route_files_status": "users_route_source_map_deleted_remaining_user_model_only",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "freeze_reason": "Rust users route group has dedicated phase5-users-business-owner probes for current/list/detail/admin grant/revoke/reset-password/delete; the Python users/admin route shells and old runtime-store facade have been physically deleted, and the remaining users source map is now limited to the shared user model definition.",
            "rollback_files": []
        }
    })
}

async fn list_users(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let users = user::Entity::find()
        .all(&db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let result: Vec<Value> = users.iter().map(user_to_value).collect();

    Ok(Json(json!(result)))
}

async fn get_current_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let current = find_user(&db, &claims.sub).await?;
    Ok(Json(user_to_value(&current)))
}

async fn get_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let target = find_user(&db, &user_id).await?;
    Ok(Json(user_to_value(&target)))
}

async fn set_admin(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<SetAdminRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let request = build_set_admin_request_from_route_payload(body)?;
    let payload = set_admin_payload(&db, &claims.sub, &request.user_id, request.is_admin).await?;
    Ok(Json(payload))
}

async fn delete_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;
    let payload = delete_standard_user_payload(&db, &claims.sub, &user_id).await?;
    Ok(Json(payload))
}

async fn reset_user_password(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<UserResetPasswordRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let request = build_user_reset_password_request_from_route_payload(body)?;

    if request.user_id == claims.sub {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "不能重置自己的密码，请使用修改密码功能",
        ));
    }

    let outcome = reset_user_password_workflow(
        &db,
        &request.user_id,
        request.new_password.as_deref(),
        PasswordResetMode::UseDefaultWhenMissingOrEmpty,
    )
    .await?;

    Ok(Json(build_user_reset_password_payload(&outcome)))
}

pub fn routes() -> Router {
    Router::new()
        .route(USERS_LIST_ROUTE, get(list_users))
        .route(USERS_CURRENT_ROUTE, get(get_current_user))
        .route(USERS_SET_ADMIN_ROUTE, post(set_admin))
        .route(USERS_RESET_PASSWORD_ROUTE, post(reset_user_password))
        .route(USERS_DETAIL_ROUTE, get(get_user))
        .route(USERS_DETAIL_ROUTE, delete(delete_user))
}

#[cfg(test)]
mod tests {
    use super::{
        build_set_admin_request, build_set_admin_request_from_route_payload,
        build_users_route_owner_contract, SetAdminRouteRequest, USERS_CURRENT_ROUTE,
        USERS_DETAIL_ROUTE, USERS_LIST_ROUTE, USERS_RESET_PASSWORD_ROUTE, USERS_SET_ADMIN_ROUTE,
    };
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn should_publish_users_route_owner_contract() {
        let contract = build_users_route_owner_contract();

        assert_eq!(contract["owner"], "users");
        assert_eq!(
            contract["scope"],
            "users_current_list_detail_set_admin_delete_reset_password_route_group"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/migrator_app/models/user.py"
        );
        assert_eq!(contract["rust_owner_map"][0], "backend-rs/src/api/users.rs");
        assert_eq!(contract["route_contract"]["list"], USERS_LIST_ROUTE);
        assert_eq!(contract["route_contract"]["current"], USERS_CURRENT_ROUTE);
        assert_eq!(contract["route_contract"]["detail"], USERS_DETAIL_ROUTE);
        assert_eq!(
            contract["route_contract"]["set_admin"],
            USERS_SET_ADMIN_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["reset_password"],
            USERS_RESET_PASSWORD_ROUTE
        );
        assert_eq!(
            contract["behavior_contract"]["route_entrypoints"][5],
            "reset_user_password"
        );
        assert_eq!(
            contract["behavior_contract"]["admin_guarded_entrypoints"][4],
            "reset_user_password"
        );
        assert_eq!(
            contract["readiness_evidence"][10],
            "users-delete-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-users-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][3],
            "users-set-admin-grant-business-rust"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["owner_profile_probe_count"],
            json!(8)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(7)
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
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy should be a string")
            .contains("phase5-users-business-owner"));
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
            contract["rollback_boundary"]["python_route_files_status"],
            "users_route_source_map_deleted_remaining_user_model_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"],
            json!([])
        );
        assert_eq!(contract["rollback_boundary"]["rollback_files"], json!([]));
    }

    #[test]
    fn should_keep_users_route_group_paths_stable() {
        assert_eq!(USERS_LIST_ROUTE, "/users");
        assert_eq!(USERS_CURRENT_ROUTE, "/users/current");
        assert_eq!(USERS_SET_ADMIN_ROUTE, "/users/set-admin");
        assert_eq!(USERS_RESET_PASSWORD_ROUTE, "/users/reset-password");
        assert_eq!(USERS_DETAIL_ROUTE, "/users/{user_id}");
    }

    #[test]
    fn build_set_admin_request_keeps_existing_required_fields_contract() {
        let request = build_set_admin_request(&json!({
            "user_id": "user-1",
            "is_admin": true
        }))
        .expect("request should build");

        assert_eq!(request.user_id, "user-1");
        assert!(request.is_admin);
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
