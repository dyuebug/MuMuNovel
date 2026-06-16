use axum::{
    extract::Query,
    http::HeaderMap,
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
const CHANGELOG_SMOKE_PROBE_HEADER: &str = "X-Smoke-Probe";
const CHANGELOG_SMOKE_OWNER_PROFILE: &str = "phase5-changelog-owner";
const CHANGELOG_LIST_ROUTE: &str = "/changelog";
const CHANGELOG_REFRESH_ROUTE: &str = "/changelog/refresh";

const CHANGELOG_PAGE_DEFAULT: u32 = 1;
const CHANGELOG_PAGE_MIN: i64 = 1;
const CHANGELOG_PER_PAGE_DEFAULT: u32 = 30;
const CHANGELOG_PER_PAGE_MIN: i64 = 1;
const CHANGELOG_PER_PAGE_MAX: u32 = 100;

#[cfg(test)]
fn build_changelog_route_owner_contract() -> Value {
    json!({
        "owner": "changelog",
        "scope": "public_changelog_route_group",
        "python_source_map": [
            "backend/app/api/changelog.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/api/changelog.rs",
            "deploy/strangler-gateway-probes.json"
        ],
        "route_contract": {
            "list": CHANGELOG_LIST_ROUTE,
            "refresh": CHANGELOG_REFRESH_ROUTE
        },
        "behavior_contract": {
            "route_entrypoints": [
                "get_changelog",
                "refresh_changelog"
            ],
            "query_bounds": {
                "page_default": CHANGELOG_PAGE_DEFAULT,
                "page_min": CHANGELOG_PAGE_MIN,
                "per_page_default": CHANGELOG_PER_PAGE_DEFAULT,
                "per_page_min": CHANGELOG_PER_PAGE_MIN,
                "per_page_max": CHANGELOG_PER_PAGE_MAX
            },
            "smoke_probe_header": CHANGELOG_SMOKE_PROBE_HEADER,
            "cache_contract": {
                "page_one_cache_ttl_hours": 1,
                "refresh_clears_or_rewrites_cache": true
            }
        },
        "readiness_evidence": [
            "changelog-public-rust",
            "changelog-refresh-public-rust"
        ],
        "owner_profile": {
            "name": CHANGELOG_SMOKE_OWNER_PROFILE,
            "business_probes": [
                "changelog-public-rust",
                "changelog-refresh-public-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": CHANGELOG_SMOKE_OWNER_PROFILE,
            "readiness_probe_count": 2,
            "business_probe_count": 2,
            "auth_guard_probe_count": 0,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Changelog route business smoke is covered by phase5-changelog-owner; final completion now requires explicit source-map freeze/delete/repoint approval with same-round rollback policy.",
        "validation_boundary": [
            "cargo test api::changelog",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-changelog-owner",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_changelog_route_file_as_source_map_until_explicit_freeze_delete_round",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit source-map freeze/delete/repoint approval"
            ],
            "freeze_reason": "Rust changelog route group has dedicated phase5-changelog-owner probes for list and refresh behavior; final Python source-map freeze/delete/repoint still requires explicit approval and rollback policy."
        }
    })
}

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

fn changelog_smoke_probe_enabled(headers: &HeaderMap) -> bool {
    headers
        .get(CHANGELOG_SMOKE_PROBE_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .map(|value| value == CHANGELOG_SMOKE_OWNER_PROFILE)
        .unwrap_or(false)
}

fn build_smoke_changelog_commits(page: u32, per_page: u32) -> Vec<Value> {
    let commits = vec![
        json!({
            "sha": "smokecommit0001",
            "commit": {
                "author": {
                    "name": "dyuebug",
                    "email": "smoke@example.com",
                    "date": "2026-06-11T15:00:00Z",
                },
                "message": "feat(smoke): verify changelog rust owner"
            },
            "html_url": "https://github.com/dyuebug/MuMuNovel/commit/smokecommit0001",
            "author": {
                "login": "dyuebug",
                "avatar_url": "https://avatars.githubusercontent.com/u/smoke?v=4",
            },
        }),
        json!({
            "sha": "smokecommit0002",
            "commit": {
                "author": {
                    "name": "dyuebug",
                    "email": "smoke@example.com",
                    "date": "2026-06-10T09:30:00Z",
                },
                "message": "chore(smoke): refresh deterministic upstream payload"
            },
            "html_url": "https://github.com/dyuebug/MuMuNovel/commit/smokecommit0002",
            "author": {
                "login": "dyuebug",
                "avatar_url": "https://avatars.githubusercontent.com/u/smoke?v=4",
            },
        }),
    ];

    let start = page.saturating_sub(1) as usize * per_page as usize;
    if start >= commits.len() {
        return Vec::new();
    }
    let end = (start + per_page as usize).min(commits.len());
    commits[start..end].to_vec()
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
    headers: HeaderMap,
    Query(params): Query<ChangelogRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request =
        ChangelogQueryRequest::from_route_query(params).map_err(map_changelog_query_error)?;

    if changelog_smoke_probe_enabled(&headers) {
        return Ok(Json(json!({
            "commits": build_smoke_changelog_commits(request.page, request.per_page),
            "cached": false,
            "cache_time": null,
        })));
    }

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
    use axum::{
        extract::Query,
        http::{HeaderMap, HeaderValue, StatusCode},
    };

    use super::{
        build_changelog_route_owner_contract, build_smoke_changelog_commits,
        changelog_smoke_probe_enabled, get_changelog, map_changelog_query_error, refresh_changelog,
        ChangelogQueryRequest, ChangelogQueryRequestError, ChangelogRouteQuery,
        CHANGELOG_LIST_ROUTE, CHANGELOG_PER_PAGE_MAX, CHANGELOG_REFRESH_ROUTE,
        CHANGELOG_SMOKE_OWNER_PROFILE, CHANGELOG_SMOKE_PROBE_HEADER,
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

    #[test]
    fn changelog_smoke_probe_header_is_strictly_opt_in() {
        let mut headers = HeaderMap::new();
        assert!(!changelog_smoke_probe_enabled(&headers));

        headers.insert(
            CHANGELOG_SMOKE_PROBE_HEADER,
            HeaderValue::from_static(CHANGELOG_SMOKE_OWNER_PROFILE),
        );
        assert!(changelog_smoke_probe_enabled(&headers));

        headers.insert(
            CHANGELOG_SMOKE_PROBE_HEADER,
            HeaderValue::from_static("phase5-other-owner"),
        );
        assert!(!changelog_smoke_probe_enabled(&headers));
    }

    #[test]
    fn smoke_changelog_commits_follow_page_and_per_page_contract() {
        let first_page = build_smoke_changelog_commits(1, 1);
        assert_eq!(first_page.len(), 1);
        assert_eq!(first_page[0]["sha"], "smokecommit0001");

        let second_page = build_smoke_changelog_commits(2, 1);
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0]["sha"], "smokecommit0002");

        let empty_page = build_smoke_changelog_commits(3, 1);
        assert!(empty_page.is_empty());
    }

    #[tokio::test]
    async fn get_changelog_supports_smoke_probe_without_external_upstream() {
        let mut headers = HeaderMap::new();
        headers.insert(
            CHANGELOG_SMOKE_PROBE_HEADER,
            HeaderValue::from_static(CHANGELOG_SMOKE_OWNER_PROFILE),
        );

        let payload = get_changelog(
            headers,
            Query(ChangelogRouteQuery {
                page: Some(1),
                per_page: Some(1),
            }),
        )
        .await
        .expect("smoke probe should bypass external upstream")
        .0;

        assert_eq!(payload["cached"], false);
        assert_eq!(payload["cache_time"], serde_json::Value::Null);
        assert_eq!(payload["commits"].as_array().map(Vec::len), Some(1));
        assert_eq!(payload["commits"][0]["sha"], "smokecommit0001");
    }

    #[tokio::test]
    async fn refresh_changelog_supports_smoke_probe_without_external_upstream() {
        let mut headers = HeaderMap::new();
        headers.insert(
            CHANGELOG_SMOKE_PROBE_HEADER,
            HeaderValue::from_static(CHANGELOG_SMOKE_OWNER_PROFILE),
        );

        let payload = refresh_changelog(headers)
            .await
            .expect("smoke refresh should bypass external upstream")
            .0;

        assert_eq!(payload["success"], true);
        assert_eq!(payload["message"], "缓存已刷新");
        assert_eq!(payload["commit_count"], 2);
        assert!(payload["cache_time"].as_str().is_some());
    }

    #[test]
    fn should_publish_changelog_route_owner_contract() {
        let contract = build_changelog_route_owner_contract();

        assert_eq!(contract["owner"], "changelog");
        assert_eq!(contract["scope"], "public_changelog_route_group");
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/api/changelog.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/changelog.rs"
        );
        assert_eq!(contract["route_contract"]["list"], CHANGELOG_LIST_ROUTE);
        assert_eq!(
            contract["route_contract"]["refresh"],
            CHANGELOG_REFRESH_ROUTE
        );
        assert_eq!(
            contract["behavior_contract"]["query_bounds"]["per_page_max"],
            CHANGELOG_PER_PAGE_MAX
        );
        assert_eq!(
            contract["readiness_evidence"][1],
            "changelog-refresh-public-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            CHANGELOG_SMOKE_OWNER_PROFILE
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"]
                .as_array()
                .expect("business probes should be present")
                .len(),
            2
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            2
        );
        assert_eq!(contract["business_smoke_status"]["business_probe_count"], 2);
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            0
        );
        assert_eq!(contract["business_smoke_status"]["fixture_probe_count"], 0);
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("phase5-changelog-owner"));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
    }
}

async fn refresh_changelog(headers: HeaderMap) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if changelog_smoke_probe_enabled(&headers) {
        let commits = build_smoke_changelog_commits(1, CHANGELOG_PER_PAGE_DEFAULT);
        let count = commits.len();
        let now = Utc::now();
        write_cache(commits);
        return Ok(Json(json!({
            "success": true,
            "message": "缓存已刷新",
            "commit_count": count,
            "cache_time": now.to_rfc3339(),
        })));
    }

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
        .route(CHANGELOG_LIST_ROUTE, get(get_changelog))
        .route(CHANGELOG_REFRESH_ROUTE, post(refresh_changelog))
}
