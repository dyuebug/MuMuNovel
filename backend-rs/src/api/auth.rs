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

use crate::config::AppConfig;
use crate::models::{user, user_password};
use crate::services::auth::{AuthService, Claims};
use crate::services::auth_password_workflow_service::{
    initialize_password_workflow, load_password_status_workflow, set_password_workflow,
};

const OAUTH_STATE_TTL: Duration = Duration::from_secs(300);
const OAUTH_STATE_COOKIE: &str = "oauth_states";
const DEFAULT_COOKIE_MAX_AGE: i64 = 604800;
const COOKIE_PATH: &str = "/";
const COOKIE_SAME_SITE: &str = "Lax";

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
            // Python 后端 AuthMiddleware 依赖 user_id cookie
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
        .route("/auth/login", post(login))
        .route("/auth/local/login", post(local_login))
        .route("/auth/bind/login", post(bind_login))
        .route("/auth/refresh", post(refresh_session))
        .route("/auth/linuxdo/url", get(get_linuxdo_auth_url))
        .route("/auth/linuxdo/callback", get(get_linuxdo_callback))
        .route("/auth/callback", get(get_linuxdo_callback))
        .route("/auth/register", post(register))
        .route("/auth/logout", post(logout))
        .route("/auth/config", get(get_auth_config))
        .route("/auth/user", get(get_current_user))
        .route("/auth/password/status", get(get_password_status))
        .route("/auth/password/set", post(set_password))
        .route("/auth/password/initialize", post(initialize_password))
}

#[cfg(test)]
mod tests {
    use super::{render_cookie, CookieSpec, COOKIE_PATH, COOKIE_SAME_SITE};

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
}
