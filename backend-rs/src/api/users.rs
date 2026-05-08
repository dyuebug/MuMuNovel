use axum::{extract::Extension, http::StatusCode, response::Json, routing::get, Router};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

use crate::models::user;
use crate::services::auth::Claims;

async fn list_users(
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let users = user::Entity::find().all(&db).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })?;

    let result: Vec<Value> = users
        .iter()
        .map(|u| {
            json!({
                "user_id": u.user_id,
                "username": u.username,
                "display_name": u.display_name,
                "avatar_url": u.avatar_url,
                "trust_level": u.trust_level,
                "is_admin": u.is_admin,
            })
        })
        .collect();

    Ok(Json(json!(result)))
}

async fn get_current_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let u = user::Entity::find_by_id(&claims.sub)
        .one(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    match u {
        Some(user) => Ok(Json(json!({
            "user_id": user.user_id,
            "username": user.username,
            "display_name": user.display_name,
            "avatar_url": user.avatar_url,
            "trust_level": user.trust_level,
            "is_admin": user.is_admin,
            "linuxdo_id": user.linuxdo_id,
            "created_at": user.created_at.to_rfc3339(),
            "last_login": user.last_login.to_rfc3339(),
        }))),
        None => Err((StatusCode::NOT_FOUND, Json(json!({"detail": "用户不存在"})))),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/users", get(list_users))
        .route("/users/current", get(get_current_user))
}
