use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

use crate::models::user;
use crate::services::auth::Claims;
use crate::services::user_admin_delete_user_route_service::delete_standard_user_payload;
use crate::services::user_admin_password_reset_workflow_service::{
    build_user_reset_password_payload, reset_user_password_workflow, PasswordResetMode,
};
use crate::services::user_admin_route_service::{api_error, check_admin, find_user, user_to_value};
use crate::services::user_admin_set_admin_route_service::{
    build_set_admin_request_from_route_payload, set_admin_payload, SetAdminRouteRequest,
};
use crate::services::user_password_reset_request_service::{
    build_user_reset_password_request_from_route_payload, UserResetPasswordRouteRequest,
};

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
    let payload = set_admin_payload(&db, &claims.sub, &request).await?;
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
        .route("/users", get(list_users))
        .route("/users/current", get(get_current_user))
        .route("/users/set-admin", post(set_admin))
        .route("/users/reset-password", post(reset_user_password))
        .route("/users/{user_id}", get(get_user))
        .route("/users/{user_id}", delete(delete_user))
}
