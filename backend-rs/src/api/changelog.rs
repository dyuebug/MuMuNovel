use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
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

const CHANGELOG_PAGE_DEFAULT: u32 = 1;
const CHANGELOG_PAGE_MIN: i64 = 1;
const CHANGELOG_PER_PAGE_DEFAULT: u32 = 30;
const CHANGELOG_PER_PAGE_MIN: i64 = 1;
const CHANGELOG_PER_PAGE_MAX: u32 = 100;

#[derive(Debug, Deserialize, Default)]
struct ChangelogRouteQuery {
    page: Option<i64>,
    per_page: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChangelogQueryRequestError {
    PageTooSmall,
    PerPageTooSmall,
    PerPageTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChangelogQueryRequest {
    page: u32,
    per_page: u32,
}

impl ChangelogQueryRequest {
    fn from_route_query(query: ChangelogRouteQuery) -> Result<Self, ChangelogQueryRequestError> {
        Ok(Self {
            page: validate_optional_min(
                query.page,
                CHANGELOG_PAGE_DEFAULT,
                CHANGELOG_PAGE_MIN,
                ChangelogQueryRequestError::PageTooSmall,
            )?,
            per_page: validate_optional_range(
                query.per_page,
                CHANGELOG_PER_PAGE_DEFAULT,
                CHANGELOG_PER_PAGE_MIN,
                CHANGELOG_PER_PAGE_MAX,
                ChangelogQueryRequestError::PerPageTooSmall,
                ChangelogQueryRequestError::PerPageTooLarge,
            )?,
        })
    }
}

fn validate_optional_min(
    value: Option<i64>,
    default: u32,
    min: i64,
    too_small: ChangelogQueryRequestError,
) -> Result<u32, ChangelogQueryRequestError> {
    let Some(value) = value else {
        return Ok(default);
    };
    if value < min {
        return Err(too_small);
    }
    Ok(value as u32)
}

fn validate_optional_range(
    value: Option<i64>,
    default: u32,
    min: i64,
    max: u32,
    too_small: ChangelogQueryRequestError,
    too_large: ChangelogQueryRequestError,
) -> Result<u32, ChangelogQueryRequestError> {
    let Some(value) = value else {
        return Ok(default);
    };
    if value < min {
        return Err(too_small);
    }
    if value > max as i64 {
        return Err(too_large);
    }
    Ok(value as u32)
}

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
        .query(&[
            ("author", "dyuebug"),
            ("page", &page.to_string()),
            ("per_page", &per_page.to_string()),
        ])
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("GitHub API returned {}", status));
    }

    let raw: Vec<Value> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

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
    Query(params): Query<ChangelogRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request =
        ChangelogQueryRequest::from_route_query(params).map_err(map_changelog_query_error)?;

    if request.page == 1 && is_cache_valid() {
        if let Some(data) = read_cache() {
            let cache_time = CACHE
                .read()
                .ok()
                .and_then(|g| g.as_ref().map(|c| c.timestamp.to_rfc3339()));
            return Ok(Json(json!({
                "commits": data,
                "cached": true,
                "cache_time": cache_time,
            })));
        }
    }

    match fetch_github_commits(request.page, request.per_page).await {
        Ok(commits) => {
            if request.page == 1 {
                write_cache(commits.clone());
            }
            Ok(Json(
                json!({"commits": commits, "cached": false, "cache_time": null}),
            ))
        }
        Err(e) => Err((StatusCode::BAD_GATEWAY, Json(json!({"detail": e})))),
    }
}

fn map_changelog_query_error(error: ChangelogQueryRequestError) -> (StatusCode, Json<Value>) {
    let detail = match error {
        ChangelogQueryRequestError::PageTooSmall => "page must be greater than or equal to 1",
        ChangelogQueryRequestError::PerPageTooSmall => {
            "per_page must be greater than or equal to 1"
        }
        ChangelogQueryRequestError::PerPageTooLarge => "per_page must be less than or equal to 100",
    };

    (StatusCode::BAD_REQUEST, Json(json!({ "detail": detail })))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::{
        map_changelog_query_error, ChangelogQueryRequest, ChangelogQueryRequestError,
        ChangelogRouteQuery,
    };

    #[test]
    fn changelog_query_request_validates_python_query_bounds() {
        let default_request =
            ChangelogQueryRequest::from_route_query(ChangelogRouteQuery::default())
                .expect("python defaults should be valid");

        assert_eq!(default_request.page, 1);
        assert_eq!(default_request.per_page, 30);

        let upper_bound_request = ChangelogQueryRequest::from_route_query(ChangelogRouteQuery {
            page: Some(2),
            per_page: Some(100),
        })
        .expect("python upper bound should be valid");

        assert_eq!(upper_bound_request.page, 2);
        assert_eq!(upper_bound_request.per_page, 100);

        assert_eq!(
            ChangelogQueryRequest::from_route_query(ChangelogRouteQuery {
                page: Some(0),
                per_page: None,
            }),
            Err(ChangelogQueryRequestError::PageTooSmall)
        );
        assert_eq!(
            ChangelogQueryRequest::from_route_query(ChangelogRouteQuery {
                page: None,
                per_page: Some(0),
            }),
            Err(ChangelogQueryRequestError::PerPageTooSmall)
        );
        assert_eq!(
            ChangelogQueryRequest::from_route_query(ChangelogRouteQuery {
                page: None,
                per_page: Some(101),
            }),
            Err(ChangelogQueryRequestError::PerPageTooLarge)
        );
    }

    #[test]
    fn changelog_query_errors_match_python_query_bounds() {
        let cases = [
            (
                ChangelogQueryRequestError::PageTooSmall,
                "page must be greater than or equal to 1",
            ),
            (
                ChangelogQueryRequestError::PerPageTooSmall,
                "per_page must be greater than or equal to 1",
            ),
            (
                ChangelogQueryRequestError::PerPageTooLarge,
                "per_page must be less than or equal to 100",
            ),
        ];

        for (error, expected_detail) in cases {
            let (status, body) = map_changelog_query_error(error);

            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(body.0["detail"], expected_detail);
        }
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
