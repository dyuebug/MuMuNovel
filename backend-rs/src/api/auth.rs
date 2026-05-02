use axum::{
    extract::Extension,
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::AppConfig;
use crate::models::{user, user_password};
use crate::services::auth::{AuthService, Claims};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::Utc;

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

fn set_cookie(response: &mut Response, name: &str, value: &str) {
    let cookie = format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800",
        name, value
    );
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.parse().unwrap());
}

async fn login(
    Extension(db): Extension<DatabaseConnection>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let auth = AuthService::new(&cfg.jwt_secret);

    match auth.login_local(&db, &body.username, &body.password).await {
        Ok(Some((user, token))) => {
            let body = json!({
                "success": true,
                "message": "登录成功",
                "user": {
                    "user_id": user.user_id,
                    "username": user.username,
                    "display_name": user.display_name,
                    "avatar_url": user.avatar_url,
                    "trust_level": user.trust_level,
                    "is_admin": user.is_admin,
                    "linuxdo_id": user.linuxdo_id,
                },
                "token": token,
            });

            let mut response = (StatusCode::OK, Json(body)).into_response();
            set_cookie(&mut response, "token", &token);
            Ok(response)
        }
        Ok(None) => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"success": false, "message": "用户名或密码错误"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("{}", e)})),
        )),
    }
}

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
    display_name: Option<String>,
}

async fn register(
    Extension(db): Extension<DatabaseConnection>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let auth = AuthService::new(&cfg.jwt_secret);
    let display_name = body.display_name.unwrap_or_else(|| body.username.clone());

    match auth
        .register_local(&db, &body.username, &body.password, &display_name)
        .await
    {
        Ok(user) => Ok((
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "message": "注册成功",
                "user": {
                    "user_id": user.user_id,
                    "username": user.username,
                    "display_name": user.display_name,
                },
            })),
        )),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": format!("{}", e)})),
        )),
    }
}

async fn logout() -> impl IntoResponse {
    let mut response =
        (StatusCode::OK, Json(json!({"success": true, "message": "已登出"}))).into_response();
    set_cookie(&mut response, "token", "");
    response
}

async fn get_auth_config(Extension(cfg): Extension<AppConfig>) -> Json<Value> {
    Json(json!({
        "local_auth_enabled": cfg.local_auth_enabled,
        "linuxdo_enabled": !cfg.linuxdo_client_id.is_empty() && !cfg.linuxdo_client_secret.is_empty(),
    }))
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
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "用户不存在"})),
        )),
    }
}

async fn get_password_status(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let pwd = user_password::Entity::find_by_id(&claims.sub)
        .one(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    match pwd {
        Some(p) => {
            let user = user::Entity::find_by_id(&claims.sub)
                .one(&db)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"detail": format!("{}", e)})),
                    )
                })?;

            let username = user.as_ref().map(|u| u.username.clone());
            let default_password = if !p.has_custom_password {
                username.as_ref().map(|name| format!("{}@666", name))
            } else {
                None
            };

            Ok(Json(json!({
                "has_password": true,
                "has_custom_password": p.has_custom_password,
                "username": username,
                "default_password": default_password,
            })))
        }
        None => {
            let user = user::Entity::find_by_id(&claims.sub)
                .one(&db)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"detail": format!("{}", e)})),
                    )
                })?;

            Ok(Json(json!({
                "has_password": false,
                "has_custom_password": false,
                "username": user.map(|u| u.username),
                "default_password": null,
            })))
        }
    }
}

// local/login is the actual path the frontend calls
async fn local_login(
    Extension(db): Extension<DatabaseConnection>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    login(Extension(db), Extension(cfg), Json(body)).await
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("password hash failed: {}", e))
}

#[derive(Deserialize)]
struct SetPasswordRequest {
    password: String,
}

async fn set_password(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<SetPasswordRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let hash = hash_password(&body.password).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": e})))
    })?;

    let now = Utc::now();
    let existing = user_password::Entity::find_by_id(&claims.sub)
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)}))))?;

    match existing {
        Some(p) => {
            let mut active: user_password::ActiveModel = p.into();
            active.password_hash = Set(hash);
            active.has_custom_password = Set(true);
            active.updated_at = Set(now);
            active.update(&db).await.map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
            })?;
        }
        None => {
            let user = user::Entity::find_by_id(&claims.sub)
                .one(&db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)}))))?
                .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "用户不存在"}))))?;

            let pwd = user_password::ActiveModel {
                user_id: Set(claims.sub.clone()),
                username: Set(user.username.clone()),
                password_hash: Set(hash),
                has_custom_password: Set(true),
                created_at: Set(now),
                updated_at: Set(now),
            };
            pwd.insert(&db).await.map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
            })?;
        }
    }

    Ok(Json(json!({"success": true, "message": "密码设置成功"})))
}

async fn initialize_password(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<SetPasswordRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Same logic as set_password but for initialization
    let hash = hash_password(&body.password).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": e})))
    })?;

    let now = Utc::now();
    let existing = user_password::Entity::find_by_id(&claims.sub)
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)}))))?;

    if existing.is_some() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"detail": "密码已存在，请使用密码设置接口"}))));
    }

    let user = user::Entity::find_by_id(&claims.sub)
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)}))))?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "用户不存在"}))))?;

    let pwd = user_password::ActiveModel {
        user_id: Set(claims.sub.clone()),
        username: Set(user.username.clone()),
        password_hash: Set(hash),
        has_custom_password: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
    };
    pwd.insert(&db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": format!("{}", e)})))
    })?;

    Ok(Json(json!({"success": true, "message": "密码初始化成功"})))
}

pub fn routes() -> Router {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/local/login", post(local_login))
        .route("/auth/register", post(register))
        .route("/auth/logout", post(logout))
        .route("/auth/config", get(get_auth_config))
        .route("/auth/user", get(get_current_user))
        .route("/auth/password/status", get(get_password_status))
        .route("/auth/password/set", post(set_password))
        .route("/auth/password/initialize", post(initialize_password))
}
