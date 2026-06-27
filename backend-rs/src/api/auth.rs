use axum::{
    extract::{Extension, Query},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

use crate::api::user_admin_shared_owner::{
    api_error, default_password_for_username, find_user, hash_password, UserAdminApiError,
};
use crate::config::AppConfig;
use crate::models::{user, user_password};
use crate::services::auth::{AuthService, Claims};

const OAUTH_STATE_TTL: Duration = Duration::from_secs(300);
const OAUTH_STATE_COOKIE: &str = "oauth_states";
const DEFAULT_COOKIE_MAX_AGE: i64 = 604800;
const COOKIE_PATH: &str = "/";
const COOKIE_SAME_SITE: &str = "Lax";
const AUTH_LOGIN_ROUTE: &str = "/auth/login";
const AUTH_LOCAL_LOGIN_ROUTE: &str = "/auth/local/login";
const AUTH_BIND_LOGIN_ROUTE: &str = "/auth/bind/login";
const AUTH_REFRESH_ROUTE: &str = "/auth/refresh";
const AUTH_LINUXDO_URL_ROUTE: &str = "/auth/linuxdo/url";
const AUTH_LINUXDO_CALLBACK_ROUTE: &str = "/auth/linuxdo/callback";
const AUTH_CALLBACK_ROUTE: &str = "/auth/callback";
const AUTH_REGISTER_ROUTE: &str = "/auth/register";
const AUTH_LOGOUT_ROUTE: &str = "/auth/logout";
const AUTH_CONFIG_ROUTE: &str = "/auth/config";
const AUTH_USER_ROUTE: &str = "/auth/user";
const AUTH_PASSWORD_STATUS_ROUTE: &str = "/auth/password/status";
const AUTH_PASSWORD_SET_ROUTE: &str = "/auth/password/set";
const AUTH_PASSWORD_INITIALIZE_ROUTE: &str = "/auth/password/initialize";

fn password_database_error(error: impl ToString) -> UserAdminApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn build_password_status_payload(
    has_password: bool,
    has_custom_password: bool,
    username: Option<String>,
) -> Value {
    let default_password = if has_password && !has_custom_password {
        username
            .as_ref()
            .map(|name| default_password_for_username(name))
    } else {
        None
    };

    json!({
        "has_password": has_password,
        "has_custom_password": has_custom_password,
        "username": username,
        "default_password": default_password,
    })
}

fn build_password_write_success_payload(message: &str) -> Value {
    json!({
        "success": true,
        "message": message,
    })
}

async fn load_password_status_workflow(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<Value, UserAdminApiError> {
    let password = user_password::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(password_database_error)?;

    match password {
        Some(password) => {
            let user = user::Entity::find_by_id(user_id)
                .one(db)
                .await
                .map_err(password_database_error)?;

            Ok(build_password_status_payload(
                true,
                password.has_custom_password,
                user.map(|value| value.username),
            ))
        }
        None => {
            let user = user::Entity::find_by_id(user_id)
                .one(db)
                .await
                .map_err(password_database_error)?;

            Ok(build_password_status_payload(
                false,
                false,
                user.map(|value| value.username),
            ))
        }
    }
}

async fn set_password_workflow(
    db: &DatabaseConnection,
    user_id: &str,
    password: &str,
) -> Result<Value, UserAdminApiError> {
    let hashed_password = hash_password(password)
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    let now = chrono::Utc::now();
    let existing = user_password::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(password_database_error)?;

    match existing {
        Some(password_model) => {
            let mut active: user_password::ActiveModel = password_model.into();
            active.password_hash = Set(hashed_password);
            active.has_custom_password = Set(true);
            active.updated_at = Set(now);
            active.update(db).await.map_err(password_database_error)?;
        }
        None => {
            let user = find_user(db, user_id).await?;
            let password = user_password::ActiveModel {
                user_id: Set(user_id.to_string()),
                username: Set(user.username.clone()),
                password_hash: Set(hashed_password),
                has_custom_password: Set(true),
                created_at: Set(now),
                updated_at: Set(now),
            };
            password.insert(db).await.map_err(password_database_error)?;
        }
    }

    Ok(build_password_write_success_payload("密码设置成功"))
}

async fn initialize_password_workflow(
    db: &DatabaseConnection,
    user_id: &str,
    password: &str,
) -> Result<Value, UserAdminApiError> {
    let existing = user_password::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(password_database_error)?;

    if existing.is_some() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "密码已存在，请使用密码设置接口",
        ));
    }

    let hashed_password = hash_password(password)
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    let user = find_user(db, user_id).await?;
    let now = chrono::Utc::now();
    let password = user_password::ActiveModel {
        user_id: Set(user_id.to_string()),
        username: Set(user.username.clone()),
        password_hash: Set(hashed_password),
        has_custom_password: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
    };
    password.insert(db).await.map_err(password_database_error)?;

    Ok(build_password_write_success_payload("密码初始化成功"))
}

#[cfg(test)]
fn build_auth_route_owner_contract() -> Value {
    json!({
        "owner": "auth",
        "scope": "auth_login_oauth_session_password_route_group",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/api/auth.rs",
            "backend-rs/src/middleware/auth.rs",
            "backend-rs/src/services/auth.rs",
            "backend-rs/src/api/user_admin_shared_owner.rs",
            "backend-rs/src/models/user.rs",
            "backend-rs/src/models/user_password.rs",
            "deploy/strangler-gateway-probes.json"
        ],
        "route_contract": {
            "login": AUTH_LOGIN_ROUTE,
            "local_login": AUTH_LOCAL_LOGIN_ROUTE,
            "bind_login": AUTH_BIND_LOGIN_ROUTE,
            "refresh": AUTH_REFRESH_ROUTE,
            "linuxdo_url": AUTH_LINUXDO_URL_ROUTE,
            "linuxdo_callback": AUTH_LINUXDO_CALLBACK_ROUTE,
            "callback": AUTH_CALLBACK_ROUTE,
            "register": AUTH_REGISTER_ROUTE,
            "logout": AUTH_LOGOUT_ROUTE,
            "config": AUTH_CONFIG_ROUTE,
            "user": AUTH_USER_ROUTE,
            "password_status": AUTH_PASSWORD_STATUS_ROUTE,
            "password_set": AUTH_PASSWORD_SET_ROUTE,
            "password_initialize": AUTH_PASSWORD_INITIALIZE_ROUTE
        },
        "behavior_contract": {
            "route_entrypoints": [
                "login",
                "local_login",
                "bind_login",
                "refresh_session",
                "get_linuxdo_auth_url",
                "get_linuxdo_callback",
                "register",
                "logout",
                "get_auth_config",
                "get_current_user",
                "get_password_status",
                "set_password",
                "initialize_password"
            ],
            "session_cookie_contract": [
                "token",
                "user_id",
                "session_expire_at",
                "oauth_states",
                "first_login"
            ],
            "service_consumers": [
                "AuthService::login_local",
                "AuthService::register_local",
                "AuthService::create_token",
                "load_password_status_workflow",
                "set_password_workflow",
                "initialize_password_workflow"
            ],
            "oauth_state_contract": {
                "cookie": OAUTH_STATE_COOKIE,
                "ttl_seconds": OAUTH_STATE_TTL.as_secs(),
                "max_retained_states": 8,
                "signature_shape": "sha256(jwt_secret:nonce:issued_at)"
            }
        },
        "readiness_evidence": [
            "auth-config-public-rust",
            "auth-logout-public-rust",
            "auth-linuxdo-url-misconfig-rust",
            "auth-user-auth-guard-rust",
            "auth-password-status-auth-guard-rust",
            "auth-password-set-auth-guard-rust",
            "auth-password-initialize-auth-guard-rust",
            "auth-refresh-auth-guard-rust",
            "auth-callback-missing-code-rust",
            "auth-local-login-invalid-credentials-rust",
            "auth-bind-login-invalid-credentials-rust",
            "auth-register-business-rust",
            "auth-local-login-business-rust",
            "auth-bind-login-business-rust",
            "auth-current-user-business-rust",
            "auth-password-status-business-rust",
            "auth-password-set-business-rust",
            "auth-password-status-after-set-business-rust",
            "auth-password-initialize-existing-business-rust",
            "auth-refresh-business-rust",
            "auth-logout-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-auth-business-owner",
            "business_probes": [
                "auth-config-public-rust",
                "auth-logout-public-rust",
                "auth-linuxdo-url-misconfig-rust",
                "auth-callback-missing-code-rust",
                "auth-local-login-invalid-credentials-rust",
                "auth-bind-login-invalid-credentials-rust",
                "auth-register-business-rust",
                "auth-local-login-business-rust",
                "auth-bind-login-business-rust",
                "auth-current-user-business-rust",
                "auth-password-status-business-rust",
                "auth-password-set-business-rust",
                "auth-password-status-after-set-business-rust",
                "auth-password-initialize-existing-business-rust",
                "auth-refresh-business-rust",
                "auth-logout-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "validation_boundary": [
            "cargo test api::auth",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-auth-business-owner",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "auth_route_group_python_source_map_surface_empty_after_default_fastapi_auth_middleware_closeout",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_route_files_status": "python_auth_route_shells_and_default_fastapi_auth_middleware_deleted",
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "retired_manifest_fallbacks": [
                "auth-logout-public-python-fallback",
                "auth-user-auth-guard-python-fallback",
                "auth-password-status-auth-guard-python-fallback",
                "auth-password-set-auth-guard-python-fallback",
                "auth-password-initialize-auth-guard-python-fallback",
                "auth-refresh-auth-guard-python-fallback",
                "auth-callback-missing-code-python-fallback",
                "auth-local-login-invalid-credentials-python-fallback",
                "auth-bind-login-invalid-credentials-python-fallback"
            ],
            "freeze_reason": "Rust auth route group has dedicated phase5-auth-business-owner probes for config, logout, LinuxDo URL misconfiguration, current user, password status/set/initialize, refresh, callback missing-code, and invalid local/bind login. The Python auth route shell, detached OAuth service shell, and default FastAPI local cookie auth middleware have now been physically deleted, so the auth route-group Python source map surface is empty."
        },
        "business_smoke_status": {
            "owner_profile": "phase5-auth-business-owner",
            "readiness_probe_count": 21,
            "business_probe_count": 16,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "auth route-group python source-map surface empty; any future rollback must be an explicit source restoration decision outside the default FastAPI runtime path",
        "migration_policy": "Auth route business smoke is covered by phase5-auth-business-owner; the Python auth route shell, detached OAuth service shell, and default FastAPI local cookie auth middleware have been physically deleted, so auth has entered Python-exit completed state for the route-group source map surface."
    })
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct AuthQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

struct CookieSpec<'a> {
    name: &'a str,
    value: &'a str,
    max_age: i64,
    http_only: bool,
}

fn auth_redirect_uri(cfg: &AppConfig) -> String {
    if cfg.linuxdo_redirect_uri.trim().is_empty() {
        format!(
            "{}/api/auth/callback",
            cfg.frontend_url.trim_end_matches('/')
        )
    } else {
        cfg.linuxdo_redirect_uri.trim().to_string()
    }
}

fn linuxdo_authorize_url(cfg: &AppConfig, state: &str) -> String {
    let mut url = Url::parse("https://connect.linux.do/oauth2/authorize").unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", &cfg.linuxdo_client_id)
        .append_pair("redirect_uri", &auth_redirect_uri(cfg))
        .append_pair("response_type", "code")
        .append_pair("scope", "read")
        .append_pair("state", state);
    url.to_string()
}

fn linuxdo_token_url() -> &'static str {
    "https://connect.linux.do/oauth2/token"
}

fn linuxdo_userinfo_url() -> &'static str {
    "https://connect.linux.do/api/user"
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sign_oauth_state(secret: &str, nonce: &str, issued_at: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b":");
    hasher.update(nonce.as_bytes());
    hasher.update(b":");
    hasher.update(issued_at.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn generate_state(cfg: &AppConfig) -> String {
    let nonce = uuid::Uuid::new_v4().to_string().replace('-', "");
    let issued_at = unix_timestamp_secs();
    let signature = sign_oauth_state(&cfg.jwt_secret, &nonce, issued_at);
    format!("{nonce}.{issued_at}.{signature}")
}

fn extract_cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|part| {
        let mut segments = part.trim().splitn(2, '=');
        let key = segments.next()?.trim();
        let value = segments.next()?.trim();
        (key == name).then(|| value.to_string())
    })
}

fn read_oauth_states_from_headers(headers: &HeaderMap) -> Vec<String> {
    extract_cookie_value(headers, OAUTH_STATE_COOKIE)
        .map(|value| {
            value
                .split(':')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_oauth_state(state: &str) -> Option<(&str, u64, &str)> {
    let mut parts = state.splitn(3, '.');
    let nonce = parts.next()?;
    let issued_at = parts.next()?.parse::<u64>().ok()?;
    let signature = parts.next()?;
    if nonce.is_empty() || signature.is_empty() {
        return None;
    }
    Some((nonce, issued_at, signature))
}

fn is_oauth_state_valid(cfg: &AppConfig, state: &str) -> bool {
    let Some((nonce, issued_at, signature)) = parse_oauth_state(state) else {
        return false;
    };
    let expected_signature = sign_oauth_state(&cfg.jwt_secret, nonce, issued_at);
    if expected_signature != signature {
        return false;
    }
    let now = unix_timestamp_secs();
    let ttl_secs = OAUTH_STATE_TTL.as_secs();
    now >= issued_at && now.saturating_sub(issued_at) <= ttl_secs
}

fn retain_valid_oauth_states(cfg: &AppConfig, states: Vec<String>) -> Vec<String> {
    states
        .into_iter()
        .filter(|state| is_oauth_state_valid(cfg, state))
        .collect()
}

fn write_oauth_states_cookie(response: &mut Response, states: &[String]) {
    if states.is_empty() {
        clear_cookie(response, OAUTH_STATE_COOKIE);
        return;
    }

    let value = states.join(":");
    set_cookie_with_max_age(
        response,
        OAUTH_STATE_COOKIE,
        &value,
        OAUTH_STATE_TTL.as_secs() as i64,
    );
}

async fn create_or_update_linuxdo_user(
    db: &DatabaseConnection,
    user_id: String,
    username: String,
    display_name: String,
    avatar_url: Option<String>,
    trust_level: i32,
) -> Result<user::Model, String> {
    let existing = user::Entity::find_by_id(&user_id)
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    let now = chrono::Utc::now();

    if let Some(existing) = existing {
        let mut active: user::ActiveModel = existing.into();
        active.username = Set(username);
        active.display_name = Set(display_name);
        active.avatar_url = Set(avatar_url);
        active.trust_level = Set(trust_level);
        active.last_login = Set(now);
        active.update(db).await.map_err(|e| e.to_string())?;
    } else {
        let user_model = user::ActiveModel {
            user_id: Set(user_id.clone()),
            username: Set(username),
            display_name: Set(display_name),
            avatar_url: Set(avatar_url),
            trust_level: Set(trust_level),
            is_admin: Set(trust_level >= 9),
            linuxdo_id: Set(user_id.clone()),
            created_at: Set(now),
            last_login: Set(now),
        };
        user_model.insert(db).await.map_err(|e| e.to_string())?;
    }

    user::Entity::find_by_id(&user_id)
        .one(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "user not found".to_string())
}

fn set_cookie(response: &mut Response, name: &str, value: &str) {
    set_cookie_with_max_age(response, name, value, DEFAULT_COOKIE_MAX_AGE);
}

fn render_cookie(spec: &CookieSpec<'_>) -> String {
    let mut parts = vec![
        format!("{}={}", spec.name, spec.value),
        format!("Path={}", COOKIE_PATH),
    ];
    if spec.http_only {
        parts.push("HttpOnly".to_string());
    }
    parts.push(format!("SameSite={}", COOKIE_SAME_SITE));
    parts.push(format!("Max-Age={}", spec.max_age));
    parts.join("; ")
}

fn append_cookie(response: &mut Response, spec: CookieSpec<'_>) {
    response
        .headers_mut()
        .append(header::SET_COOKIE, render_cookie(&spec).parse().unwrap());
}

fn set_cookie_with_max_age(response: &mut Response, name: &str, value: &str, max_age: i64) {
    append_cookie(
        response,
        CookieSpec {
            name,
            value,
            max_age,
            http_only: true,
        },
    );
}

fn set_cookie_non_httponly(response: &mut Response, name: &str, value: &str, max_age: i64) {
    append_cookie(
        response,
        CookieSpec {
            name,
            value,
            max_age,
            http_only: false,
        },
    );
}

fn clear_cookie(response: &mut Response, name: &str) {
    append_cookie(
        response,
        CookieSpec {
            name,
            value: "",
            max_age: 0,
            http_only: true,
        },
    );
}

async fn login(
    Extension(db): Extension<DatabaseConnection>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let auth = AuthService::new(&cfg.jwt_secret);

    match auth
        .login_local(&db, &cfg, &body.username, &body.password)
        .await
    {
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
            // user_id cookie 仍保留给前端与兼容会话链路使用
            set_cookie_with_max_age(&mut response, "user_id", &user.user_id, 7200);
            // session_expire_at 供前端 sessionManager 判断会话过期
            let expire_at = chrono::Utc::now().timestamp() + 7200;
            set_cookie_non_httponly(
                &mut response,
                "session_expire_at",
                &expire_at.to_string(),
                7200,
            );
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
    let mut response = (
        StatusCode::OK,
        Json(json!({"success": true, "message": "已登出"})),
    )
        .into_response();
    clear_cookie(&mut response, "token");
    clear_cookie(&mut response, "user_id");
    clear_cookie(&mut response, "session_expire_at");
    response
}

async fn get_auth_config(Extension(cfg): Extension<AppConfig>) -> Json<Value> {
    Json(json!({
        "local_auth_enabled": cfg.local_auth_enabled,
        "linuxdo_enabled": !cfg.linuxdo_client_id.is_empty() && !cfg.linuxdo_client_secret.is_empty(),
    }))
}

async fn get_linuxdo_auth_url(
    Extension(cfg): Extension<AppConfig>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<Value>)> {
    if cfg.linuxdo_client_id.is_empty() || cfg.linuxdo_client_secret.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "LinuxDO OAuth 未配置"})),
        ));
    }

    let state = generate_state(&cfg);
    let mut states = retain_valid_oauth_states(&cfg, read_oauth_states_from_headers(&headers));
    if !states.iter().any(|item| item == &state) {
        states.push(state.clone());
    }
    if states.len() > 8 {
        let drain_count = states.len() - 8;
        states.drain(0..drain_count);
    }

    let mut response = Json(json!({
        "auth_url": linuxdo_authorize_url(&cfg, &state),
        "state": state,
    }))
    .into_response();
    write_oauth_states_cookie(&mut response, &states);
    Ok(response)
}

async fn get_linuxdo_callback(
    Extension(db): Extension<DatabaseConnection>,
    Extension(cfg): Extension<AppConfig>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<Value>)> {
    if let Some(error) = query.error {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "detail": format!("授权失败: {}", error)
            })),
        ));
    }
    let Some(code) = query.code else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "缺少 code 参数"})),
        ));
    };
    let Some(state) = query.state else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "缺少 state 参数"})),
        ));
    };
    let mut states = retain_valid_oauth_states(&cfg, read_oauth_states_from_headers(&headers));
    if !is_oauth_state_valid(&cfg, &state) || !states.iter().any(|item| item == &state) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "无效的 state 参数"})),
        ));
    }
    states.retain(|item| item != &state);

    let token_client = reqwest::Client::new();
    let redirect_uri = auth_redirect_uri(&cfg);
    let token_resp = token_client
        .post(linuxdo_token_url())
        .form(&HashMap::from([
            ("client_id".to_string(), cfg.linuxdo_client_id.clone()),
            (
                "client_secret".to_string(),
                cfg.linuxdo_client_secret.clone(),
            ),
            ("code".to_string(), code.clone()),
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("redirect_uri".to_string(), redirect_uri),
        ]))
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"detail": e.to_string()})),
            )
        })?;
    if !token_resp.status().is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({"detail": format!("获取访问令牌失败: {}", token_resp.status())})),
        ));
    }
    let token_json: Value = token_resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"detail": format!("解析访问令牌响应失败: {}", e)})),
        )
    })?;
    let access_token = token_json
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if access_token.is_empty() {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({"detail": "获取访问令牌失败"})),
        ));
    }

    let user_resp = token_client
        .get(linuxdo_userinfo_url())
        .bearer_auth(access_token)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8")
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(json!({"detail": e.to_string()}))))?;
    if !user_resp.status().is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({"detail": format!("获取用户信息失败: {}", user_resp.status())})),
        ));
    }
    let user_json: Value = user_resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"detail": format!("解析用户信息失败: {}", e)})),
        )
    })?;

    let linuxdo_id = user_json
        .get("id")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .unwrap_or_else(|| {
            user_json
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        });
    let username = user_json
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let display_name = user_json
        .get("name")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .unwrap_or(&username)
        .to_string();
    let avatar_url = user_json
        .get("avatar_url")
        .and_then(Value::as_str)
        .map(str::to_string);
    let trust_level = user_json
        .get("trust_level")
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;

    let user = create_or_update_linuxdo_user(
        &db,
        linuxdo_id.clone(),
        username.clone(),
        display_name,
        avatar_url,
        trust_level,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )
    })?;
    let is_first_login = user_password::Entity::find_by_id(&user.user_id)
        .one(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .is_none();

    let auth = AuthService::new(&cfg.jwt_secret);
    let token = auth.create_token(&user).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e.to_string()})),
        )
    })?;

    let redirect_to = format!("{}/auth/callback", cfg.frontend_url.trim_end_matches('/'));
    let mut response = axum::response::Redirect::to(&redirect_to).into_response();
    let max_age = (cfg.session_expire_minutes as i64) * 60;
    write_oauth_states_cookie(&mut response, &states);
    set_cookie(&mut response, "token", &token);
    set_cookie_with_max_age(&mut response, "user_id", &user.user_id, max_age);
    let expire_at = chrono::Utc::now().timestamp() + max_age;
    set_cookie_non_httponly(
        &mut response,
        "session_expire_at",
        &expire_at.to_string(),
        max_age,
    );
    if is_first_login {
        set_cookie_non_httponly(&mut response, "first_login", "true", 300);
    }
    Ok(response)
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

async fn get_password_status(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    load_password_status_workflow(&db, &claims.sub)
        .await
        .map(Json)
}

// local/login is the actual path the frontend calls
async fn local_login(
    Extension(db): Extension<DatabaseConnection>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    login(Extension(db), Extension(cfg), Json(body)).await
}

async fn bind_login(
    Extension(db): Extension<DatabaseConnection>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    login(Extension(db), Extension(cfg), Json(body)).await
}

async fn refresh_session(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(cfg): Extension<AppConfig>,
) -> Response {
    let max_age = 7200;
    let expire_at = chrono::Utc::now().timestamp() + max_age;
    let mut response = Json(json!({
        "message": "会话刷新成功",
        "expire_at": expire_at,
        "remaining_minutes": max_age / 60,
    }))
    .into_response();

    if let Ok(Some(user)) = user::Entity::find_by_id(&claims.sub).one(&db).await {
        let auth = AuthService::new(&cfg.jwt_secret);
        if let Ok(token) = auth.create_token(&user) {
            set_cookie(&mut response, "token", &token);
        }
    }

    set_cookie_with_max_age(&mut response, "user_id", &claims.sub, max_age);
    set_cookie_non_httponly(
        &mut response,
        "session_expire_at",
        &expire_at.to_string(),
        max_age,
    );
    response
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
    set_password_workflow(&db, &claims.sub, &body.password)
        .await
        .map(Json)
}

async fn initialize_password(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<SetPasswordRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    initialize_password_workflow(&db, &claims.sub, &body.password)
        .await
        .map(Json)
}

pub fn routes() -> Router {
    Router::new()
        .route(AUTH_LOGIN_ROUTE, post(login))
        .route(AUTH_LOCAL_LOGIN_ROUTE, post(local_login))
        .route(AUTH_BIND_LOGIN_ROUTE, post(bind_login))
        .route(AUTH_REFRESH_ROUTE, post(refresh_session))
        .route(AUTH_LINUXDO_URL_ROUTE, get(get_linuxdo_auth_url))
        .route(AUTH_LINUXDO_CALLBACK_ROUTE, get(get_linuxdo_callback))
        .route(AUTH_CALLBACK_ROUTE, get(get_linuxdo_callback))
        .route(AUTH_REGISTER_ROUTE, post(register))
        .route(AUTH_LOGOUT_ROUTE, post(logout))
        .route(AUTH_CONFIG_ROUTE, get(get_auth_config))
        .route(AUTH_USER_ROUTE, get(get_current_user))
        .route(AUTH_PASSWORD_STATUS_ROUTE, get(get_password_status))
        .route(AUTH_PASSWORD_SET_ROUTE, post(set_password))
        .route(AUTH_PASSWORD_INITIALIZE_ROUTE, post(initialize_password))
}

#[cfg(test)]
mod tests {
    use super::{
        build_auth_route_owner_contract, build_password_status_payload,
        build_password_write_success_payload, render_cookie, CookieSpec, AUTH_BIND_LOGIN_ROUTE,
        AUTH_CALLBACK_ROUTE, AUTH_CONFIG_ROUTE, AUTH_LINUXDO_CALLBACK_ROUTE,
        AUTH_LINUXDO_URL_ROUTE, AUTH_LOCAL_LOGIN_ROUTE, AUTH_LOGIN_ROUTE, AUTH_LOGOUT_ROUTE,
        AUTH_PASSWORD_INITIALIZE_ROUTE, AUTH_PASSWORD_SET_ROUTE, AUTH_PASSWORD_STATUS_ROUTE,
        AUTH_REFRESH_ROUTE, AUTH_REGISTER_ROUTE, AUTH_USER_ROUTE, COOKIE_PATH, COOKIE_SAME_SITE,
    };
    use serde_json::json;

    #[test]
    fn render_cookie_includes_shared_attributes_for_http_only_cookie() {
        let rendered = render_cookie(&CookieSpec {
            name: "token",
            value: "abc",
            max_age: 7200,
            http_only: true,
        });

        assert_eq!(
            rendered,
            format!(
                "token=abc; Path={}; HttpOnly; SameSite={}; Max-Age=7200",
                COOKIE_PATH, COOKIE_SAME_SITE
            )
        );
    }

    #[test]
    fn render_cookie_omits_http_only_for_frontend_visible_cookie() {
        let rendered = render_cookie(&CookieSpec {
            name: "session_expire_at",
            value: "123",
            max_age: 7200,
            http_only: false,
        });

        assert_eq!(
            rendered,
            format!(
                "session_expire_at=123; Path={}; SameSite={}; Max-Age=7200",
                COOKIE_PATH, COOKIE_SAME_SITE
            )
        );
    }

    #[test]
    fn render_cookie_preserves_clear_cookie_shape() {
        let rendered = render_cookie(&CookieSpec {
            name: "token",
            value: "",
            max_age: 0,
            http_only: true,
        });

        assert_eq!(
            rendered,
            format!(
                "token=; Path={}; HttpOnly; SameSite={}; Max-Age=0",
                COOKIE_PATH, COOKIE_SAME_SITE
            )
        );
    }

    #[test]
    fn should_publish_auth_route_owner_contract() {
        let contract = build_auth_route_owner_contract();

        assert_eq!(contract["owner"], "auth");
        assert_eq!(
            contract["scope"],
            "auth_login_oauth_session_password_route_group"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(contract["rust_owner_map"][0], "backend-rs/src/api/auth.rs");
        assert_eq!(contract["route_contract"]["login"], AUTH_LOGIN_ROUTE);
        assert_eq!(
            contract["route_contract"]["local_login"],
            AUTH_LOCAL_LOGIN_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["linuxdo_callback"],
            AUTH_LINUXDO_CALLBACK_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["password_initialize"],
            AUTH_PASSWORD_INITIALIZE_ROUTE
        );
        assert_eq!(
            contract["behavior_contract"]["route_entrypoints"][12],
            "initialize_password"
        );
        assert_eq!(
            contract["readiness_evidence"][10],
            "auth-bind-login-invalid-credentials-rust"
        );
        assert_eq!(contract["readiness_evidence"].as_array().unwrap().len(), 21);
        assert_eq!(
            contract["readiness_evidence"][20],
            "auth-logout-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-auth-business-owner"
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("auth business probes should be present");
        assert_eq!(business_probes.len(), 16);
        assert_eq!(
            contract["owner_profile"]["business_probes"][13],
            "auth-password-initialize-existing-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(21)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(16)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "auth route-group python source-map surface empty; any future rollback must be an explicit source restoration decision outside the default FastAPI runtime path"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("auth migration policy should be present")
            .contains("phase5-auth-business-owner"));
    }

    #[test]
    fn should_keep_auth_route_group_paths_stable() {
        assert_eq!(AUTH_LOGIN_ROUTE, "/auth/login");
        assert_eq!(AUTH_LOCAL_LOGIN_ROUTE, "/auth/local/login");
        assert_eq!(AUTH_BIND_LOGIN_ROUTE, "/auth/bind/login");
        assert_eq!(AUTH_REFRESH_ROUTE, "/auth/refresh");
        assert_eq!(AUTH_LINUXDO_URL_ROUTE, "/auth/linuxdo/url");
        assert_eq!(AUTH_LINUXDO_CALLBACK_ROUTE, "/auth/linuxdo/callback");
        assert_eq!(AUTH_CALLBACK_ROUTE, "/auth/callback");
        assert_eq!(AUTH_REGISTER_ROUTE, "/auth/register");
        assert_eq!(AUTH_LOGOUT_ROUTE, "/auth/logout");
        assert_eq!(AUTH_CONFIG_ROUTE, "/auth/config");
        assert_eq!(AUTH_USER_ROUTE, "/auth/user");
        assert_eq!(AUTH_PASSWORD_STATUS_ROUTE, "/auth/password/status");
        assert_eq!(AUTH_PASSWORD_SET_ROUTE, "/auth/password/set");
        assert_eq!(AUTH_PASSWORD_INITIALIZE_ROUTE, "/auth/password/initialize");
    }

    #[test]
    fn password_status_payload_keeps_default_password_for_non_custom_password() {
        let payload = build_password_status_payload(true, false, Some("alice".to_string()));

        assert_eq!(payload["has_password"], true);
        assert_eq!(payload["has_custom_password"], false);
        assert_eq!(payload["username"], "alice");
        assert_eq!(payload["default_password"], "alice@666");
    }

    #[test]
    fn password_status_payload_keeps_null_default_when_password_missing() {
        let payload = build_password_status_payload(false, false, Some("alice".to_string()));

        assert_eq!(payload["has_password"], false);
        assert_eq!(payload["has_custom_password"], false);
        assert_eq!(payload["username"], "alice");
        assert!(payload["default_password"].is_null());
    }

    #[test]
    fn password_status_payload_keeps_null_default_when_username_missing() {
        let payload = build_password_status_payload(true, false, None);

        assert_eq!(payload["has_password"], true);
        assert_eq!(payload["has_custom_password"], false);
        assert!(payload["username"].is_null());
        assert!(payload["default_password"].is_null());
    }

    #[test]
    fn password_write_success_payload_keeps_existing_shape() {
        let payload = build_password_write_success_payload("密码设置成功");

        assert_eq!(payload["success"], true);
        assert_eq!(payload["message"], "密码设置成功");
    }
}
