use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use uuid::Uuid;

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

fn check_admin(claims: &Claims) -> Result<(), (StatusCode, Json<Value>)> {
    if claims.is_admin {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, Json(json!({"detail": "需要管理员权限"}))))
    }
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("password hash failed: {}", e))
}

async fn list_users(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let users = user::Entity::find().all(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
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
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let username = body.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let display_name = body.get("display_name").and_then(|v| v.as_str()).unwrap_or(username);

    // Check duplicate
    let existing = user::Entity::find()
        .filter(user::Column::Username.eq(username))
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)}))))?;

    if existing.is_some() {
        return Err((StatusCode::CONFLICT, Json(json!({"detail": "用户名已存在"}))));
    }

    let user_id = format!("admin_created_{}", Uuid::new_v4().to_string().replace('-', "")[..16].to_string());
    let now = Utc::now();
    let is_admin = body.get("is_admin").and_then(|v| v.as_bool()).unwrap_or(false);
    let trust_level = body.get("trust_level").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    let u = user::ActiveModel {
        user_id: Set(user_id.clone()),
        username: Set(username.to_string()),
        display_name: Set(display_name.to_string()),
        avatar_url: Set(body.get("avatar_url").and_then(|v| v.as_str()).map(String::from)),
        trust_level: Set(trust_level),
        is_admin: Set(is_admin),
        linuxdo_id: Set(user_id.clone()),
        created_at: Set(now),
        last_login: Set(now),
    };
    u.insert(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?;

    // Set password
    let has_custom_password = body.get("password").is_some();
    let default_pwd = format!("{}@666", username);
    let actual_password = body.get("password").and_then(|v| v.as_str()).unwrap_or(&default_pwd);
    let hash = hash_password(actual_password).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": e})))
    })?;

    let pwd = user_password::ActiveModel {
        user_id: Set(user_id.clone()),
        username: Set(username.to_string()),
        password_hash: Set(hash),
        has_custom_password: Set(has_custom_password),
        created_at: Set(now),
        updated_at: Set(now),
    };
    pwd.insert(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?;

    let created = user::Entity::find_by_id(&user_id).one(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?;
    let response_default = if has_custom_password { None } else { Some(default_pwd) };

    Ok((StatusCode::CREATED, Json(json!({
        "success": true,
        "message": "用户创建成功",
        "user": created.as_ref().map(user_to_value),
        "default_password": response_default,
    }))))
}

async fn update_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let existing = user::Entity::find_by_id(&user_id).one(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?.ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "用户不存在"}))))?;

    // Check last admin removal
    if let Some(false) = body.get("is_admin").and_then(|v| v.as_bool()) {
        if existing.is_admin {
            let admin_count = user::Entity::find()
                .filter(user::Column::IsAdmin.eq(true))
                .all(&db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)}))))?
                .len();
            if admin_count <= 1 {
                return Err((StatusCode::BAD_REQUEST, Json(json!({"detail": "不能取消最后一个管理员的权限"}))));
            }
        }
    }

    let mut active: user::ActiveModel = existing.into();
    if let Some(v) = body.get("display_name").and_then(|v| v.as_str()) { active.display_name = Set(v.to_string()); }
    if body.get("avatar_url").is_some() { active.avatar_url = Set(body.get("avatar_url").and_then(|v| v.as_str()).map(String::from)); }
    if let Some(v) = body.get("trust_level").and_then(|v| v.as_i64()) { active.trust_level = Set(v as i32); }
    if let Some(v) = body.get("is_admin").and_then(|v| v.as_bool()) { active.is_admin = Set(v); }
    active.last_login = Set(Utc::now());

    let saved = active.update(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "用户信息更新成功",
        "user": user_to_value(&saved),
    })))
}

async fn toggle_user_status(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    if user_id == claims.sub {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"detail": "不能禁用自己的账号"}))));
    }

    let existing = user::Entity::find_by_id(&user_id).one(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?.ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "用户不存在"}))))?;

    let is_active = body.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut active: user::ActiveModel = existing.into();
    if is_active {
        active.trust_level = Set(0);
    } else {
        active.trust_level = Set(-1);
    }

    active.update(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?;

    let status_text = if is_active { "启用" } else { "禁用" };
    Ok(Json(json!({
        "success": true,
        "message": format!("用户已{}", status_text),
        "is_active": is_active,
    })))
}

async fn reset_password(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    let target = user::Entity::find_by_id(&user_id).one(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?.ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "用户不存在"}))))?;

    let default_pwd = format!("{}@666", target.username);
    let new_password = body.get("new_password").and_then(|v| v.as_str()).unwrap_or(&default_pwd);
    let hash = hash_password(new_password).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": e})))
    })?;

    let now = Utc::now();
    let existing_pwd = user_password::Entity::find_by_id(&user_id).one(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?;

    match existing_pwd {
        Some(p) => {
            let mut active: user_password::ActiveModel = p.into();
            active.password_hash = Set(hash);
            active.has_custom_password = Set(body.get("new_password").is_some());
            active.updated_at = Set(now);
            active.update(&db).await.map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
            })?;
        }
        None => {
            let pwd = user_password::ActiveModel {
                user_id: Set(user_id.clone()),
                username: Set(target.username.clone()),
                password_hash: Set(hash),
                has_custom_password: Set(body.get("new_password").is_some()),
                created_at: Set(now),
                updated_at: Set(now),
            };
            pwd.insert(&db).await.map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
            })?;
        }
    }

    Ok(Json(json!({
        "success": true,
        "message": "密码重置成功",
        "new_password": new_password,
    })))
}

async fn delete_user(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&claims)?;

    if user_id == claims.sub {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"detail": "不能删除自己的账号"}))));
    }

    let target = user::Entity::find_by_id(&user_id).one(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?.ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "用户不存在"}))))?;

    // Check last admin
    if target.is_admin {
        let admin_count = user::Entity::find()
            .filter(user::Column::IsAdmin.eq(true))
            .all(&db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)}))))?
            .len();
        if admin_count <= 1 {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"detail": "不能删除最后一个管理员账号"}))));
        }
    }

    user_password::Entity::delete_by_id(&user_id).exec(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?;
    user::Entity::delete_by_id(&user_id).exec(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "用户已删除",
    })))
}

pub fn routes() -> Router {
    Router::new()
        .route("/admin/users", get(list_users))
        .route("/admin/users", post(create_user))
        .route("/admin/users/{userId}", put(update_user))
        .route("/admin/users/{userId}", delete(delete_user))
        .route("/admin/users/{userId}/toggle-status", post(toggle_user_status))
        .route("/admin/users/{userId}/reset-password", post(reset_password))
}
