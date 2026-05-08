use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};

use crate::models::{user, user_password};
use crate::services::auth::Claims;

fn user_to_value(u: &user::Model) -> Value {
    json!({
        "user_id": u.user_id,
        "username": u.username,
        "display_name": u.display_name,
        "avatar_url": u.avatar_url,
        "trust_level": u.trust_level,
        "is_admin": u.is_admin,
        "is_active": u.trust_level != -1,
        "linuxdo_id": u.linuxdo_id,
        "created_at": u.created_at.to_rfc3339(),
        "last_login": u.last_login.to_rfc3339(),
    })
}

fn api_error(status: StatusCode, detail: impl Into<String>) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "detail": detail.into() })))
}

fn check_admin(claims: &Claims) -> Result<(), (StatusCode, Json<Value>)> {
    if claims.is_admin {
        Ok(())
    } else {
        Err(api_error(StatusCode::FORBIDDEN, "需要管理员权限"))
    }
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| format!("password hash failed: {err}"))
}

async fn admin_count(db: &DatabaseConnection) -> Result<usize, (StatusCode, Json<Value>)> {
    let admins = user::Entity::find()
        .filter(user::Column::IsAdmin.eq(true))
        .all(db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(admins.len())
}

async fn find_user(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<user::Model, (StatusCode, Json<Value>)> {
    user::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "用户不存在"))
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
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let user_id = body
        .get("user_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "缺少 user_id"))?;
    let is_admin = body
        .get("is_admin")
        .and_then(|value| value.as_bool())
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "缺少 is_admin"))?;

    if user_id == claims.sub && !is_admin {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "不能撤销自己的管理员权限",
        ));
    }

    let target = find_user(&db, user_id).await?;
    if target.is_admin && !is_admin && admin_count(&db).await? <= 1 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "无法撤销管理员权限，至少需要保留一个管理员",
        ));
    }

    let mut active: user::ActiveModel = target.into();
    active.is_admin = Set(is_admin);
    active.last_login = Set(Utc::now());
    active
        .update(&db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let action = if is_admin { "授予" } else { "撤销" };
    Ok(Json(json!({
        "message": format!("已{action}管理员权限"),
        "user_id": user_id,
        "is_admin": is_admin,
    })))
}

async fn delete_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    if user_id == claims.sub {
        return Err(api_error(StatusCode::BAD_REQUEST, "不能删除自己的账号"));
    }

    let target = find_user(&db, &user_id).await?;
    if target.is_admin {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "无法删除该用户（用户不存在或为管理员）",
        ));
    }

    user_password::Entity::delete_by_id(&user_id)
        .exec(&db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    user::Entity::delete_by_id(&user_id)
        .exec(&db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(Json(json!({
        "message": "用户已删除",
        "user_id": user_id,
    })))
}

async fn reset_user_password(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let user_id = body
        .get("user_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "缺少 user_id"))?;

    if user_id == claims.sub {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "不能重置自己的密码，请使用修改密码功能",
        ));
    }

    let target = find_user(&db, user_id).await?;
    let default_password = format!("{}@666", target.username);
    let new_password = body
        .get("new_password")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(&default_password);
    let password_hash = hash_password(new_password)
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err))?;
    let now = Utc::now();
    let has_custom_password = body
        .get("new_password")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .is_some();

    match user_password::Entity::find_by_id(user_id)
        .one(&db)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    {
        Some(password) => {
            let mut active: user_password::ActiveModel = password.into();
            active.password_hash = Set(password_hash);
            active.has_custom_password = Set(has_custom_password);
            active.updated_at = Set(now);
            active
                .update(&db)
                .await
                .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
        }
        None => {
            let password = user_password::ActiveModel {
                user_id: Set(user_id.to_string()),
                username: Set(target.username.clone()),
                password_hash: Set(password_hash),
                has_custom_password: Set(has_custom_password),
                created_at: Set(now),
                updated_at: Set(now),
            };
            password
                .insert(&db)
                .await
                .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
        }
    }

    let mut response = json!({
        "message": "密码重置成功",
        "user_id": user_id,
        "username": target.username,
    });

    if !has_custom_password {
        response["default_password"] = json!(new_password);
        response["message"] = json!(format!("密码已重置为默认密码: {new_password}"));
    }

    Ok(Json(response))
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
