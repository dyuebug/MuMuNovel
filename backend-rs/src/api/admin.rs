use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

use crate::models::user;
use crate::services::auth::Claims;
use crate::services::user_admin_create_user_route_service::{
    build_create_user_request_from_route_payload, create_user_payload, CreateUserRouteRequest,
};
use crate::services::user_admin_delete_user_route_service::delete_admin_user_payload;
use crate::services::user_admin_password_reset_workflow_service::{
    build_admin_reset_password_payload, reset_user_password_workflow, PasswordResetMode,
};
use crate::services::user_admin_route_service::{check_admin, user_to_value};
use crate::services::user_admin_toggle_status_route_service::{
    build_toggle_user_status_request_from_route_payload, toggle_user_status_payload,
    ToggleUserStatusRouteRequest,
};
use crate::services::user_admin_update_user_route_service::{
    build_update_user_request_from_route_payload, update_user_payload, UpdateUserRouteRequest,
};
use crate::services::user_password_reset_request_service::{
    build_admin_reset_password_request_from_route_payload, AdminResetPasswordRouteRequest,
};

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
        .route("/admin/users", get(list_users))
        .route("/admin/users", post(create_user))
        .route("/admin/users/{userId}", put(update_user))
        .route("/admin/users/{userId}", delete(delete_user))
        .route(
            "/admin/users/{userId}/toggle-status",
            post(toggle_user_status),
        )
        .route("/admin/users/{userId}/reset-password", post(reset_password))
}
