use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::sync::{OnceLock, RwLock};

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("MuMuNovel-App")
            .build()
            .expect("Failed to create HTTP client")
    })
}

struct Cache {
    data: Vec<Value>,
    timestamp: DateTime<Utc>,
    ttl: Duration,
}

static CACHE: RwLock<Option<Cache>> = RwLock::new(None);

fn is_cache_valid() -> bool {
    if let Ok(guard) = CACHE.read() {
        if let Some(ref cache) = *guard {
            return Utc::now() - cache.timestamp < cache.ttl;
        }
    }
    false
}

fn read_cache() -> Option<Vec<Value>> {
    CACHE.read().ok()?.as_ref().map(|c| c.data.clone())
}

fn write_cache(data: Vec<Value>) {
    if let Ok(mut guard) = CACHE.write() {
        *guard = Some(Cache {
            data,
            timestamp: Utc::now(),
            ttl: Duration::hours(1),
        });
    }
}

async fn fetch_github_commits(page: u32, per_page: u32) -> Result<Vec<Value>, String> {
    let url = "https://api.github.com/repos/dyuebug/MuMuNovel/commits";
    let resp = client()
        .get(url)
        .query(&[("author", "dyuebug"), ("page", &page.to_string()), ("per_page", &per_page.to_string())])
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("GitHub API returned {}", status));
    }

    let raw: Vec<Value> = resp.json().await.map_err(|e| format!("Failed to parse response: {}", e))?;

    let commits: Vec<Value> = raw
        .into_iter()
        .filter_map(|c| {
            let sha = c.get("sha")?.as_str()?;
            let commit = c.get("commit")?;
            let author_info = commit.get("author")?;
            let html_url = c.get("html_url")?.as_str()?;

            let gh_author = c.get("author").and_then(|a| {
                Some(json!({
                    "login": a.get("login")?.as_str()?,
                    "avatar_url": a.get("avatar_url")?.as_str()?,
                }))
            });

            Some(json!({
                "sha": sha,
                "commit": {
                    "author": {
                        "name": author_info.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        "email": author_info.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                        "date": author_info.get("date").and_then(|v| v.as_str()).unwrap_or(""),
                    },
                    "message": commit.get("message").and_then(|v| v.as_str()).unwrap_or(""),
                },
                "html_url": html_url,
                "author": gh_author,
            }))
        })
        .collect();

    Ok(commits)
}

async fn get_changelog(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let page: u32 = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let per_page: u32 = params.get("per_page").and_then(|p| p.parse().ok()).unwrap_or(30).min(100);

    if page == 1 && is_cache_valid() {
        if let Some(data) = read_cache() {
            let cache_time = CACHE.read().ok().and_then(|g| g.as_ref().map(|c| c.timestamp.to_rfc3339()));
            return Ok(Json(json!({
                "commits": data,
                "cached": true,
                "cache_time": cache_time,
            })));
        }
    }

    match fetch_github_commits(page, per_page).await {
        Ok(commits) => {
            if page == 1 {
                write_cache(commits.clone());
            }
            Ok(Json(json!({"commits": commits, "cached": false, "cache_time": null})))
        }
        Err(e) => Err((StatusCode::BAD_GATEWAY, Json(json!({"detail": e})))),
    }
}

async fn refresh_changelog() -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Clear cache
    if let Ok(mut guard) = CACHE.write() {
        *guard = None;
    }

    match fetch_github_commits(1, 30).await {
        Ok(commits) => {
            let count = commits.len();
            let now = Utc::now();
            write_cache(commits);
            Ok(Json(json!({
                "success": true,
                "message": "缓存已刷新",
                "commit_count": count,
                "cache_time": now.to_rfc3339(),
            })))
        }
        Err(e) => Err((StatusCode::BAD_GATEWAY, Json(json!({"detail": e})))),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/changelog", get(get_changelog))
        .route("/changelog/refresh", post(refresh_changelog))
}
