use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{sse::Event, Json, Sse},
    routing::{get, post},
    Router,
};
use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::chapters_error_mapper::{
    map_apply_partial_regenerate_error, map_create_chapter_regeneration_stream_workflow_error,
    map_create_partial_regeneration_stream_workflow_error, map_load_accessible_chapter_error,
    map_regeneration_tasks_query_request_error,
};
use crate::models::{chapter, regeneration_task};
use crate::services::auth::Claims;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};
use crate::services::chapter_regeneration_prepare_service::{
    build_full_chapter_regeneration_stream_request_from_route_payload,
    build_partial_regeneration_stream_workflow_request_from_route_payload,
    FullChapterRegenerationStreamRouteRequest, PartialRegenerationStreamRouteRequest,
};
use crate::services::chapter_regeneration_stream_workflow_service::{
    create_chapter_regeneration_stream_workflow, create_partial_regeneration_stream_workflow,
};
use crate::services::chapter_service::ChapterService;
use crate::utils::sse::default_sse_keep_alive;

const CHAPTER_REGENERATION_STREAM_ROUTE: &str = "/chapters/{chapter_id}/regenerate-stream";
const CHAPTER_PARTIAL_REGENERATION_STREAM_ROUTE: &str =
    "/chapters/{chapter_id}/partial-regenerate-stream";
const CHAPTER_APPLY_PARTIAL_REGENERATE_ROUTE: &str =
    "/chapters/{chapter_id}/apply-partial-regenerate";
const CHAPTER_REGENERATION_TASKS_ROUTE: &str = "/chapters/{chapter_id}/regeneration/tasks";
const REGENERATION_TASKS_LIMIT_DEFAULT: u64 = 10;
const REGENERATION_TASKS_LIMIT_MIN: i64 = 1;
const REGENERATION_TASKS_LIMIT_MAX: u64 = 50;

type LoadRegenerationTasksPayloadError = LoadAccessibleChapterError;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct RegenerationTasksRouteQuery {
    limit: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegenerationTasksQueryRequest {
    limit: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegenerationTasksQueryRequestError {
    LimitTooSmall,
    LimitTooLarge,
}

impl RegenerationTasksQueryRequest {
    fn from_route_query(
        route_query: RegenerationTasksRouteQuery,
    ) -> Result<Self, RegenerationTasksQueryRequestError> {
        let Some(limit) = route_query.limit else {
            return Ok(Self {
                limit: REGENERATION_TASKS_LIMIT_DEFAULT,
            });
        };

        if limit < REGENERATION_TASKS_LIMIT_MIN {
            return Err(RegenerationTasksQueryRequestError::LimitTooSmall);
        }
        if limit > REGENERATION_TASKS_LIMIT_MAX as i64 {
            return Err(RegenerationTasksQueryRequestError::LimitTooLarge);
        }

        Ok(Self {
            limit: limit as u64,
        })
    }

    fn limit(&self) -> u64 {
        self.limit
    }
}

fn build_regeneration_tasks_query_request_from_route_query(
    route_query: RegenerationTasksRouteQuery,
) -> Result<RegenerationTasksQueryRequest, RegenerationTasksQueryRequestError> {
    RegenerationTasksQueryRequest::from_route_query(route_query)
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct ApplyPartialRegenerateRouteRequest {
    new_text: Option<Value>,
    start_position: Option<i64>,
    end_position: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ApplyPartialRegenerateRequest {
    new_text: Option<String>,
    start_position: Option<i64>,
    end_position: Option<i64>,
}

pub(crate) enum ApplyPartialRegenerateError {
    EmptyContent,
    WorkflowMetaText,
    InvalidRange,
    Chapter(LoadAccessibleChapterError),
    Internal(String),
}

impl ApplyPartialRegenerateRequest {
    fn new(
        new_text: Option<String>,
        start_position: Option<i64>,
        end_position: Option<i64>,
    ) -> Self {
        Self {
            new_text,
            start_position,
            end_position,
        }
    }

    fn from_route_request(route_request: ApplyPartialRegenerateRouteRequest) -> Self {
        Self::new(
            route_request.new_text.and_then(coerce_apply_new_text_value),
            route_request.start_position,
            route_request.end_position,
        )
    }

    fn new_text(&self) -> Option<&str> {
        self.new_text.as_deref()
    }

    fn start_position(&self) -> i64 {
        self.start_position.unwrap_or(0)
    }

    fn end_position(&self) -> i64 {
        self.end_position.unwrap_or(0)
    }
}

fn coerce_apply_new_text_value(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => Some(value),
        Value::Bool(value) => Some(if value { "True" } else { "False" }.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

fn build_apply_partial_regenerate_request_from_route_payload(
    route_request: ApplyPartialRegenerateRouteRequest,
) -> ApplyPartialRegenerateRequest {
    ApplyPartialRegenerateRequest::from_route_request(route_request)
}

fn datetime_to_string(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

async fn load_regeneration_tasks_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    limit: u64,
) -> Result<Value, String> {
    let tasks = regeneration_task::Entity::find()
        .filter(regeneration_task::Column::ChapterId.eq(chapter_id.to_string()))
        .order_by_desc(regeneration_task::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let task_items: Vec<Value> = tasks
        .iter()
        .map(|task| {
            json!({
                "task_id": task.id,
                "status": task.status,
                "version_number": task.version_number,
                "version_note": task.version_note,
                "original_word_count": task.original_word_count,
                "regenerated_word_count": task.regenerated_word_count,
                "created_at": datetime_to_string(task.created_at),
                "completed_at": datetime_to_string(task.completed_at),
            })
        })
        .collect();

    Ok(json!({
        "chapter_id": chapter_id,
        "total": task_items.len(),
        "tasks": task_items,
    }))
}

async fn load_owned_regeneration_tasks_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: RegenerationTasksQueryRequest,
) -> Result<Value, LoadRegenerationTasksPayloadError> {
    let _ = load_accessible_chapter(db, chapter_id, user_id).await?;

    load_regeneration_tasks_payload(db, chapter_id, request.limit())
        .await
        .map_err(LoadAccessibleChapterError::Internal)
}

async fn apply_owned_partial_regenerate_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: ApplyPartialRegenerateRequest,
) -> Result<Value, ApplyPartialRegenerateError> {
    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(ApplyPartialRegenerateError::Chapter)?;

    apply_partial_regenerate_payload(
        db,
        chapter_id,
        user_id,
        &chapter,
        request.new_text().unwrap_or_default(),
        request.start_position(),
        request.end_position(),
    )
    .await
}

async fn apply_partial_regenerate_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    chapter: &chapter::Model,
    new_text_raw: &str,
    start_position: i64,
    end_position: i64,
) -> Result<Value, ApplyPartialRegenerateError> {
    let new_content =
        prepare_partial_regenerate_apply(chapter, new_text_raw, start_position, end_position)?;

    match ChapterService::update(
        db,
        chapter_id,
        user_id,
        None,
        Some(&new_content),
        None,
        None,
    )
    .await
    {
        Ok(Some(updated)) => Ok(json!({
            "success": true,
            "chapter_id": chapter_id,
            "word_count": updated.word_count,
            "old_word_count": chapter.word_count,
            "message": "局部改写已应用",
        })),
        Ok(None) => Err(ApplyPartialRegenerateError::Chapter(
            LoadAccessibleChapterError::NotFoundOrAccessDenied,
        )),
        Err(error) => Err(ApplyPartialRegenerateError::Internal(error)),
    }
}

fn prepare_partial_regenerate_apply(
    chapter: &chapter::Model,
    new_text_raw: &str,
    start_position: i64,
    end_position: i64,
) -> Result<String, ApplyPartialRegenerateError> {
    let (new_text, _) = sanitize_generated_narrative_text(new_text_raw);
    if new_text.trim().is_empty() {
        return Err(ApplyPartialRegenerateError::EmptyContent);
    }
    if contains_chapter_workflow_meta_text(&new_text) {
        return Err(ApplyPartialRegenerateError::WorkflowMetaText);
    }

    let current_content = chapter.content.clone().unwrap_or_default();
    let content_chars: Vec<char> = current_content.chars().collect();
    let content_length = content_chars.len();
    if start_position < 0
        || end_position < 0
        || start_position >= end_position
        || end_position as usize > content_length
    {
        return Err(ApplyPartialRegenerateError::InvalidRange);
    }
    let start_position = start_position as usize;
    let end_position = end_position as usize;

    let prefix: String = content_chars[..start_position].iter().collect();
    let suffix: String = content_chars[end_position..].iter().collect();

    Ok(format!("{prefix}{new_text}{suffix}"))
}

#[cfg(test)]
fn build_chapter_regeneration_route_owner_contract() -> Value {
    json!({
        "owner": "chapter_regeneration",
        "route_group": "chapters",
        "rust_owner": "backend-rs/src/api/chapter_regeneration_routes.rs",
        "routes": {
            "regenerate_stream": CHAPTER_REGENERATION_STREAM_ROUTE,
            "partial_regenerate_stream": CHAPTER_PARTIAL_REGENERATION_STREAM_ROUTE,
            "apply_partial_regenerate": CHAPTER_APPLY_PARTIAL_REGENERATE_ROUTE,
            "regeneration_tasks": CHAPTER_REGENERATION_TASKS_ROUTE
        },
        "methods": {
            "regenerate_stream": ["POST"],
            "partial_regenerate_stream": ["POST"],
            "apply_partial_regenerate": ["POST"],
            "regeneration_tasks": ["GET"]
        },
        "service_owners": [
            "backend-rs/src/services/chapter_regeneration_prepare_service.rs",
            "backend-rs/src/services/chapter_regeneration_stream_workflow_service.rs",
            "backend-rs/src/models/regeneration_task.rs"
        ],
        "readiness_probes": [
            "chapters-regenerate-stream-auth-guard-rust",
            "chapters-partial-regenerate-stream-auth-guard-rust",
            "chapters-apply-partial-regenerate-auth-guard-rust",
            "chapters-regeneration-tasks-auth-guard-rust",
            "chapter-regeneration-stream-workflow-smoke-rust",
            "chapter-regeneration-full-stream-logged-in-not-found-rust",
            "chapter-regeneration-partial-stream-logged-in-not-found-rust",
            "chapter-regeneration-apply-partial-logged-in-not-found-rust",
            "chapter-regeneration-tasks-logged-in-not-found-rust",
            "chapter-regeneration-fixture-import-project-business-rust",
            "chapter-regeneration-fixture-list-chapter-business-rust",
            "chapter-regeneration-configure-mock-openai-business-rust",
            "chapter-regeneration-full-stream-business-rust",
            "chapter-regeneration-partial-stream-business-rust",
            "chapter-regeneration-apply-partial-business-rust",
            "chapter-regeneration-tasks-business-rust",
            "chapter-regeneration-fixture-delete-project-business-rust"
        ],
        "source_map_files": [
            "backend/app/api/chapter_regeneration_routes.py",
            "backend/app/api/chapter_partial_regeneration_routes.py",
            "backend/app/schemas/regeneration.py",
            "backend/app/services/regeneration_task_service.py",
            "backend/app/services/partial_regeneration_service.py",
            "backend/app/services/chapter_regeneration_stream_service.py",
            "backend/app/services/chapter_regeneration_query_service.py",
            "backend/app/services/chapter_regeneration_context_service.py"
        ],
        "behavior_contract": {
            "regenerate_stream": "full chapter regeneration SSE route consumes normalized regeneration prepare request",
            "partial_regenerate_stream": "partial regeneration SSE route consumes normalized selection/range/context request",
            "apply_partial_regenerate": "apply route preserves new_text/start_position/end_position payload contract",
            "regeneration_tasks": "task list limit defaults to 10 and only accepts 1..=50"
        },
        "business_success_probes": [
            "chapter-regeneration-fixture-import-project-business-rust",
            "chapter-regeneration-fixture-list-chapter-business-rust",
            "chapter-regeneration-configure-mock-openai-business-rust",
            "chapter-regeneration-full-stream-business-rust",
            "chapter-regeneration-partial-stream-business-rust",
            "chapter-regeneration-apply-partial-business-rust",
            "chapter-regeneration-tasks-business-rust",
            "chapter-regeneration-fixture-delete-project-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-chapter-regeneration-owner",
            "business_probes": [
                "chapter-regeneration-fixture-import-project-business-rust",
                "chapter-regeneration-fixture-list-chapter-business-rust",
                "chapter-regeneration-configure-mock-openai-business-rust",
                "chapter-regeneration-full-stream-business-rust",
                "chapter-regeneration-partial-stream-business-rust",
                "chapter-regeneration-apply-partial-business-rust",
                "chapter-regeneration-tasks-business-rust",
                "chapter-regeneration-fixture-delete-project-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-chapter-regeneration-owner",
            "route_contract_probe_count": 17,
            "readiness_probe_count": 13,
            "route_group_probe_count": 13,
            "workflow_smoke_probe_count": 1,
            "business_probe_count": 5,
            "logged_in_not_found_probe_count": 4,
            "auth_guard_probe_count": 0,
            "route_contract_auth_guard_probe_count": 4,
            "fixture_probe_count": 4,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "source-map has been repointed to the Rust route owner; final physical deletion still requires a separate same-round approval and rollback policy",
        "migration_policy": "Chapter regeneration business smoke is covered by phase5-chapter-regeneration-owner; the Python route shells have been repointed to rollback/source-map-only status, and final physical deletion still requires a separate same-round approval.",
        "smoke_gap": "Deterministic logged-in full/partial SSE, apply, task-list business smoke, and current-source owner-profile live proof now exist; remaining gap is explicit Python source-map delete/repoint approval.",
        "rollback_boundary": {
            "source_map_policy": "keep_python_regeneration_route_schema_and_service_files_as_source_map_until_explicit_freeze_delete_round",
            "source_map_freeze_status": "frozen_source_map_rollback_only",
            "source_map_physical_closeout_action": "repoint",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit delete approval for the repointed source-map shell"
            ],
            "rollback_reference": "Keep Python regeneration route/schema/service files as repointed rollback/source-map-only references until explicit delete approval is granted."
        }
    })
}

#[cfg(test)]
fn build_chapter_regeneration_query_owner_contract() -> Value {
    json!({
        "owner": "chapter_regeneration_routes",
        "scope": "regeneration_tasks_query_and_payload_owner",
        "python_source_map": [
            "backend/app/api/chapter_regeneration_routes.py",
            "backend/app/services/chapter_regeneration_query_service.py",
            "backend/app/services/regeneration_task_service.py",
            "backend/app/models/regeneration_task.py",
            "backend/app/models/chapter.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/api/chapter_regeneration_routes.rs",
            "backend-rs/src/services/chapter_access_service.rs",
            "backend-rs/src/models/regeneration_task.rs",
            "backend-rs/src/models/chapter.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_regeneration_tasks_query_request_from_route_query",
                "load_owned_regeneration_tasks_payload",
                "load_regeneration_tasks_payload",
                "datetime_to_string"
            ],
            "route_query_fields": [
                "limit"
            ],
            "limit_policy": {
                "default": REGENERATION_TASKS_LIMIT_DEFAULT,
                "min": REGENERATION_TASKS_LIMIT_MIN,
                "max": REGENERATION_TASKS_LIMIT_MAX,
                "too_small_error": "LimitTooSmall",
                "too_large_error": "LimitTooLarge"
            },
            "query_policy": [
                "load chapter through chapter access owner before listing tasks",
                "filter regeneration_task.chapter_id by requested chapter_id",
                "order by created_at descending",
                "apply validated limit"
            ],
            "task_item_fields": [
                "task_id",
                "status",
                "version_number",
                "version_note",
                "original_word_count",
                "regenerated_word_count",
                "created_at",
                "completed_at"
            ],
            "payload_fields": [
                "chapter_id",
                "total",
                "tasks"
            ],
            "datetime_format": "%Y-%m-%dT%H:%M:%S",
            "error_contract": [
                "LoadAccessibleChapterError::NotFoundOrAccessDenied",
                "LoadAccessibleChapterError::Internal",
                "RegenerationTasksQueryRequestError::LimitTooSmall",
                "RegenerationTasksQueryRequestError::LimitTooLarge"
            ]
        },
        "validation_boundary": [
            "cargo test api::chapter_regeneration_routes",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_regeneration_routes::get_regeneration_tasks",
            "chapter-regeneration-tasks-business-rust",
            "chapters-regeneration-tasks-auth-guard-rust"
        ],
        "rollback_boundary": {
            "python_source_map": "chapter_regeneration_query_service_python_source_map",
            "python_fallback_removal_ready": false,
            "approval_required": "explicit delete/repoint approval for the frozen source-map shell"
        }
    })
}

#[cfg(test)]
fn build_chapter_regeneration_apply_owner_contract() -> Value {
    json!({
        "owner": "chapter_regeneration_routes",
        "scope": "partial_regeneration_apply_payload_and_persistence_owner",
        "python_source_map": [
            "backend/app/api/chapter_partial_regeneration_routes.py",
            "backend/app/services/chapter_content_apply_service.py",
            "backend/app/services/partial_regeneration_service.py",
            "backend/app/models/chapter.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/api/chapter_regeneration_routes.rs",
            "backend-rs/src/services/chapter_access_service.rs",
            "backend-rs/src/services/chapter_narrative_cleaner_service.rs",
            "backend-rs/src/services/chapter_service.rs",
            "backend-rs/src/models/chapter.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_apply_partial_regenerate_request_from_route_payload",
                "apply_owned_partial_regenerate_payload",
                "apply_partial_regenerate_payload",
                "prepare_partial_regenerate_apply"
            ],
            "route_payload_fields": [
                "new_text",
                "start_position",
                "end_position"
            ],
            "payload_coercion": [
                "null maps to missing text",
                "string is preserved",
                "bool maps to Python-style True/False",
                "number maps through JSON number string",
                "array/object maps through JSON string"
            ],
            "apply_policy": [
                "default positions are 0",
                "load chapter through chapter access owner",
                "sanitize generated narrative before applying",
                "reject empty or workflow-meta-only narrative",
                "use char-indexed start/end positions",
                "replace current chapter slice with sanitized new text",
                "persist through ChapterService::update"
            ],
            "success_payload_fields": [
                "success",
                "chapter_id",
                "word_count",
                "old_word_count",
                "message"
            ],
            "error_contract": [
                "EmptyContent",
                "WorkflowMetaText",
                "InvalidRange",
                "Chapter(NotFoundOrAccessDenied)",
                "Internal"
            ]
        },
        "validation_boundary": [
            "cargo test api::chapter_regeneration_routes",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_regeneration_routes::apply_partial_regenerate",
            "chapter-regeneration-apply-partial-business-rust",
            "chapters-apply-partial-regenerate-auth-guard-rust"
        ],
        "rollback_boundary": {
            "python_source_map": "chapter_partial_regeneration_routes_python_source_map",
            "python_fallback_removal_ready": false,
            "approval_required": "explicit delete/repoint approval for the frozen source-map shell"
        }
    })
}

async fn apply_partial_regenerate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ApplyPartialRegenerateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_apply_partial_regenerate_request_from_route_payload(body);
    let payload = apply_owned_partial_regenerate_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(map_apply_partial_regenerate_error)?;
    Ok(Json(payload))
}

async fn regenerate_chapter_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<FullChapterRegenerationStreamRouteRequest>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let request = build_full_chapter_regeneration_stream_request_from_route_payload(body);
    let stream =
        create_chapter_regeneration_stream_workflow(&db, &claims.sub, &chapter_id, request)
            .await
            .map_err(map_create_chapter_regeneration_stream_workflow_error)?;

    Ok(Sse::new(stream).keep_alive(default_sse_keep_alive()))
}

async fn partial_regenerate_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<PartialRegenerationStreamRouteRequest>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let request = build_partial_regeneration_stream_workflow_request_from_route_payload(body);
    let stream =
        create_partial_regeneration_stream_workflow(&db, &claims.sub, &chapter_id, request)
            .await
            .map_err(map_create_partial_regeneration_stream_workflow_error)?;

    Ok(Sse::new(stream).keep_alive(default_sse_keep_alive()))
}

async fn get_regeneration_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<RegenerationTasksRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_regeneration_tasks_query_request_from_route_query(query)
        .map_err(map_regeneration_tasks_query_request_error)?;
    let payload = load_owned_regeneration_tasks_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(map_load_accessible_chapter_error)?;
    Ok(Json(payload))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(
            CHAPTER_REGENERATION_STREAM_ROUTE,
            post(regenerate_chapter_stream),
        )
        .route(
            CHAPTER_PARTIAL_REGENERATION_STREAM_ROUTE,
            post(partial_regenerate_stream),
        )
        .route(
            CHAPTER_APPLY_PARTIAL_REGENERATE_ROUTE,
            post(apply_partial_regenerate),
        )
        .route(
            CHAPTER_REGENERATION_TASKS_ROUTE,
            get(get_regeneration_tasks),
        )
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use super::{
        build_apply_partial_regenerate_request_from_route_payload,
        build_chapter_regeneration_apply_owner_contract,
        build_chapter_regeneration_query_owner_contract,
        build_chapter_regeneration_route_owner_contract,
        build_regeneration_tasks_query_request_from_route_query, datetime_to_string,
        prepare_partial_regenerate_apply, ApplyPartialRegenerateError,
        ApplyPartialRegenerateRequest, ApplyPartialRegenerateRouteRequest,
        FullChapterRegenerationStreamRouteRequest, PartialRegenerationStreamRouteRequest,
        RegenerationTasksQueryRequestError, RegenerationTasksRouteQuery,
        CHAPTER_APPLY_PARTIAL_REGENERATE_ROUTE, CHAPTER_PARTIAL_REGENERATION_STREAM_ROUTE,
        CHAPTER_REGENERATION_STREAM_ROUTE, CHAPTER_REGENERATION_TASKS_ROUTE,
        REGENERATION_TASKS_LIMIT_DEFAULT, REGENERATION_TASKS_LIMIT_MAX,
    };
    use crate::models::chapter;
    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_regeneration_prepare_service::{
        build_full_chapter_regeneration_stream_request_from_route_payload,
        build_partial_regeneration_stream_workflow_request_from_route_payload,
    };
    use serde_json::json;

    fn chapter_with_content(content: &str) -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            title: "测试章节".to_string(),
            chapter_number: 1,
            content: Some(content.to_string()),
            summary: None,
            word_count: content.chars().count() as i32,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    fn valid_prepared_apply(result: Result<String, ApplyPartialRegenerateError>) -> String {
        match result {
            Ok(prepared) => prepared,
            Err(_) => panic!("partial regenerate apply should be valid"),
        }
    }

    fn apply_error(
        result: Result<String, ApplyPartialRegenerateError>,
    ) -> ApplyPartialRegenerateError {
        match result {
            Ok(_) => panic!("partial regenerate apply should be rejected"),
            Err(error) => error,
        }
    }

    #[test]
    fn should_publish_chapter_regeneration_route_owner_contract() {
        let contract = build_chapter_regeneration_route_owner_contract();

        assert_eq!(contract["owner"], "chapter_regeneration");
        assert_eq!(contract["route_group"], "chapters");
        assert_eq!(
            contract["rust_owner"],
            "backend-rs/src/api/chapter_regeneration_routes.rs"
        );
        assert_eq!(
            contract["routes"]["regenerate_stream"],
            CHAPTER_REGENERATION_STREAM_ROUTE
        );
        assert_eq!(
            contract["routes"]["partial_regenerate_stream"],
            CHAPTER_PARTIAL_REGENERATION_STREAM_ROUTE
        );
        assert_eq!(
            contract["routes"]["apply_partial_regenerate"],
            CHAPTER_APPLY_PARTIAL_REGENERATE_ROUTE
        );
        assert_eq!(
            contract["routes"]["regeneration_tasks"],
            CHAPTER_REGENERATION_TASKS_ROUTE
        );
        assert_eq!(contract["service_owners"].as_array().unwrap().len(), 3);
        assert_eq!(contract["readiness_probes"].as_array().unwrap().len(), 17);
        assert_eq!(
            contract["readiness_probes"][16],
            "chapter-regeneration-fixture-delete-project-business-rust"
        );
        assert_eq!(
            contract["business_success_probes"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-chapter-regeneration-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"]
                .as_array()
                .expect("business probes should be present")
                .len(),
            8
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["owner_profile"],
            "phase5-chapter-regeneration-owner"
        );
        assert_eq!(
            contract["business_smoke_status"]["route_contract_probe_count"],
            json!(17)
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(13)
        );
        assert_eq!(
            contract["business_smoke_status"]["route_group_probe_count"],
            json!(13)
        );
        assert_eq!(
            contract["business_smoke_status"]["workflow_smoke_probe_count"],
            json!(1)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(5)
        );
        assert_eq!(
            contract["business_smoke_status"]["logged_in_not_found_probe_count"],
            json!(4)
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["route_contract_auth_guard_probe_count"],
            json!(4)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(4)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "source-map has been repointed to the Rust route owner; final physical deletion still requires a separate same-round approval and rollback policy"
        );
        assert_eq!(
            contract["migration_policy"],
            "Chapter regeneration business smoke is covered by phase5-chapter-regeneration-owner; the Python route shells have been repointed to rollback/source-map-only status, and final physical deletion still requires a separate same-round approval."
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_status"],
            "frozen_source_map_rollback_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_physical_closeout_action"],
            "repoint"
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(false)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(false)
        );
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 8);
        assert!(contract["smoke_gap"]
            .as_str()
            .unwrap_or_default()
            .contains("full/partial SSE"));
        assert!(!contract["smoke_gap"]
            .as_str()
            .unwrap_or_default()
            .contains("still need logged-in"));
        assert!(!contract["smoke_gap"]
            .as_str()
            .unwrap_or_default()
            .contains("live owner-profile execution"));
        assert!(contract["smoke_gap"]
            .as_str()
            .unwrap_or_default()
            .contains("source-map delete/repoint approval"));
    }

    #[test]
    fn should_keep_chapter_regeneration_route_group_paths_stable() {
        assert_eq!(
            CHAPTER_REGENERATION_STREAM_ROUTE,
            "/chapters/{chapter_id}/regenerate-stream"
        );
        assert_eq!(
            CHAPTER_PARTIAL_REGENERATION_STREAM_ROUTE,
            "/chapters/{chapter_id}/partial-regenerate-stream"
        );
        assert_eq!(
            CHAPTER_APPLY_PARTIAL_REGENERATE_ROUTE,
            "/chapters/{chapter_id}/apply-partial-regenerate"
        );
        assert_eq!(
            CHAPTER_REGENERATION_TASKS_ROUTE,
            "/chapters/{chapter_id}/regeneration/tasks"
        );
    }

    #[test]
    fn should_validate_regeneration_tasks_limit_like_python_query() {
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: None,
            })
            .expect("default limit should be valid")
            .limit(),
            10
        );
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: Some(25),
            })
            .expect("explicit in-range limit should be valid")
            .limit(),
            25
        );
        assert!(build_regeneration_tasks_query_request_from_route_query(
            RegenerationTasksRouteQuery { limit: Some(0) }
        )
        .is_err());
        assert!(build_regeneration_tasks_query_request_from_route_query(
            RegenerationTasksRouteQuery { limit: Some(99) }
        )
        .is_err());
    }

    #[test]
    fn should_format_regeneration_task_datetime() {
        let datetime = NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse");

        assert_eq!(
            datetime_to_string(Some(datetime)),
            Some("2026-05-17T12:30:45".to_string())
        );
        assert_eq!(datetime_to_string(None), None);
    }

    #[test]
    fn should_alias_access_not_found_error_for_regeneration_tasks_query() {
        let error: LoadAccessibleChapterError = LoadAccessibleChapterError::NotFoundOrAccessDenied;

        assert_eq!(error, LoadAccessibleChapterError::NotFoundOrAccessDenied);
    }

    #[test]
    fn should_alias_access_internal_error_for_regeneration_tasks_query() {
        let error: LoadAccessibleChapterError =
            LoadAccessibleChapterError::Internal("boom".to_string());

        assert_eq!(
            error,
            LoadAccessibleChapterError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_publish_chapter_regeneration_query_owner_contract() {
        let contract = build_chapter_regeneration_query_owner_contract();

        assert_eq!(contract["owner"], "chapter_regeneration_routes");
        assert_eq!(
            contract["scope"],
            "regeneration_tasks_query_and_payload_owner"
        );
        assert_eq!(
            contract["python_source_map"][1],
            "backend/app/services/chapter_regeneration_query_service.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/chapter_regeneration_routes.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][1],
            "load_owned_regeneration_tasks_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["limit_policy"]["default"],
            REGENERATION_TASKS_LIMIT_DEFAULT
        );
        assert_eq!(
            contract["behavior_contract"]["limit_policy"]["max"],
            REGENERATION_TASKS_LIMIT_MAX
        );
        assert_eq!(
            contract["behavior_contract"]["query_policy"][2],
            "order by created_at descending"
        );
        assert_eq!(
            contract["behavior_contract"]["task_item_fields"][7],
            "completed_at"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_regeneration_routes::get_regeneration_tasks"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
    }

    #[test]
    fn should_reject_negative_regeneration_tasks_limit_like_python_query() {
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: Some(-1),
            }),
            Err(RegenerationTasksQueryRequestError::LimitTooSmall)
        );
    }

    #[test]
    fn should_prepare_partial_regenerate_apply_content() {
        let chapter = chapter_with_content("一二三四五");
        let prepared =
            valid_prepared_apply(prepare_partial_regenerate_apply(&chapter, "替换文本", 1, 4));

        assert_eq!(prepared, "一替换文本五");
    }

    #[test]
    fn should_reject_empty_partial_regenerate_apply_content() {
        let chapter = chapter_with_content("一二三");
        let error = apply_error(prepare_partial_regenerate_apply(&chapter, "   ", 0, 1));

        assert!(matches!(error, ApplyPartialRegenerateError::EmptyContent));
    }

    #[test]
    fn should_reject_meta_only_partial_regenerate_apply_content_as_empty() {
        let chapter = chapter_with_content("一二三");
        let error = apply_error(prepare_partial_regenerate_apply(
            &chapter,
            "```markdown\n作为AI：我将开始执行\n流程说明",
            0,
            1,
        ));

        assert!(matches!(error, ApplyPartialRegenerateError::EmptyContent));
    }

    #[test]
    fn should_reject_invalid_partial_regenerate_apply_range() {
        let chapter = chapter_with_content("一二三");
        let error = apply_error(prepare_partial_regenerate_apply(&chapter, "替换", 2, 2));

        assert!(matches!(error, ApplyPartialRegenerateError::InvalidRange));
    }

    #[test]
    fn should_reject_negative_partial_regenerate_apply_range() {
        let chapter = chapter_with_content("一二三");
        let error = apply_error(prepare_partial_regenerate_apply(&chapter, "替换", -1, 2));

        assert!(matches!(error, ApplyPartialRegenerateError::InvalidRange));
    }

    #[test]
    fn should_keep_apply_partial_regenerate_route_payload_contract() {
        let route_request = ApplyPartialRegenerateRouteRequest {
            new_text: Some(json!("新文本")),
            start_position: Some(12),
            end_position: Some(24),
        };
        let request = build_apply_partial_regenerate_request_from_route_payload(route_request);

        assert_eq!(request.new_text(), Some("新文本"));
        assert_eq!(request.start_position(), 12);
        assert_eq!(request.end_position(), 24);
    }

    #[test]
    fn should_coerce_apply_partial_regenerate_new_text_like_python_dict() {
        let number_request = build_apply_partial_regenerate_request_from_route_payload(
            ApplyPartialRegenerateRouteRequest {
                new_text: Some(json!(123)),
                start_position: Some(-1),
                end_position: Some(2),
            },
        );
        let bool_request = build_apply_partial_regenerate_request_from_route_payload(
            ApplyPartialRegenerateRouteRequest {
                new_text: Some(json!(true)),
                start_position: None,
                end_position: None,
            },
        );

        assert_eq!(number_request.new_text(), Some("123"));
        assert_eq!(number_request.start_position(), -1);
        assert_eq!(number_request.end_position(), 2);
        assert_eq!(bool_request.new_text(), Some("True"));
    }

    #[test]
    fn should_alias_chapter_access_not_found_error_for_partial_apply() {
        let error = ApplyPartialRegenerateError::Chapter(
            LoadAccessibleChapterError::NotFoundOrAccessDenied,
        );

        assert!(matches!(
            error,
            ApplyPartialRegenerateError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied
            )
        ));
    }

    #[test]
    fn should_alias_chapter_access_internal_error_for_partial_apply() {
        let error = ApplyPartialRegenerateError::Chapter(LoadAccessibleChapterError::Internal(
            "boom".to_string(),
        ));

        assert!(matches!(
            error,
            ApplyPartialRegenerateError::Chapter(LoadAccessibleChapterError::Internal(detail))
            if detail == "boom"
        ));
    }

    #[test]
    fn should_keep_apply_partial_regenerate_request_defaults_contract() {
        let request = ApplyPartialRegenerateRequest::default();

        assert_eq!(request.new_text(), None);
        assert_eq!(request.start_position(), 0);
        assert_eq!(request.end_position(), 0);
    }

    #[test]
    fn should_publish_chapter_regeneration_apply_owner_contract() {
        let contract = build_chapter_regeneration_apply_owner_contract();

        assert_eq!(contract["owner"], "chapter_regeneration_routes");
        assert_eq!(
            contract["scope"],
            "partial_regeneration_apply_payload_and_persistence_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/api/chapter_partial_regeneration_routes.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/chapter_regeneration_routes.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][1],
            "apply_owned_partial_regenerate_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["payload_coercion"][2],
            "bool maps to Python-style True/False"
        );
        assert_eq!(
            contract["behavior_contract"]["apply_policy"][5],
            "replace current chapter slice with sanitized new text"
        );
        assert_eq!(
            contract["behavior_contract"]["success_payload_fields"][4],
            "message"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_regeneration_routes::apply_partial_regenerate"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
    }

    #[test]
    fn should_build_full_chapter_regeneration_stream_request_from_route_payload() {
        let route_request = FullChapterRegenerationStreamRouteRequest {
            target_word_count: Some(2200),
            custom_instructions: Some("补强冲突".to_string()),
            selected_suggestion_indices: vec![json!(2), json!("skip"), json!(4)],
            focus_areas: vec![json!("结构"), json!(3), json!("情绪")],
            story_creation_brief: Some("brief".to_string()),
            quality_notes: Some("notes".to_string()),
            story_repair_summary: Some("summary".to_string()),
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("climax".to_string()),
            quality_preset: Some("plot_drive".to_string()),
            enable_web_research: Some(true),
            web_research_query: Some("检索背景资料".to_string()),
            preserve_elements: Some(json!({
                "preserve_structure": true,
                "preserve_dialogues": ["对白1", 3],
                "preserve_plot_points": ["反转点"],
                "preserve_character_traits": false
            })),
            story_repair_targets: vec![json!("逻辑"), json!(7), json!("节奏")],
            story_preserve_strengths: vec![json!("悬念"), json!(false)],
        };
        let request =
            build_full_chapter_regeneration_stream_request_from_route_payload(route_request);

        assert_eq!(request.target_word_count(), 2200);
        assert_eq!(request.custom_instructions(), "补强冲突");
        assert_eq!(
            request.selected_suggestion_indices(),
            &["2".to_string(), "4".to_string()]
        );
        assert_eq!(
            request.focus_areas(),
            &["结构".to_string(), "情绪".to_string()]
        );
        assert_eq!(request.story_creation_brief(), "brief");
        assert_eq!(request.quality_notes(), "notes");
        assert_eq!(request.story_repair_summary(), "summary");
        assert_eq!(request.creative_mode(), "hook");
        assert_eq!(request.story_focus(), "advance_plot");
        assert_eq!(request.plot_stage(), "climax");
        assert_eq!(request.quality_preset(), "plot_drive");
        assert_eq!(request.enable_web_research(), Some(true));
        assert_eq!(request.web_research_query(), Some("检索背景资料"));
        assert!(request.preserve_structure());
        assert_eq!(request.preserve_dialogues(), &["对白1".to_string()]);
        assert_eq!(request.preserve_plot_points(), &["反转点".to_string()]);
        assert!(!request.preserve_character_traits());
        assert_eq!(
            request.story_repair_targets(),
            &["逻辑".to_string(), "节奏".to_string()]
        );
        assert_eq!(request.story_preserve_strengths(), &["悬念".to_string()]);
    }

    #[test]
    fn should_build_full_chapter_regeneration_stream_request_with_defaults() {
        let route_request = FullChapterRegenerationStreamRouteRequest::default();
        let request =
            build_full_chapter_regeneration_stream_request_from_route_payload(route_request);

        assert_eq!(request.target_word_count(), 3000);
        assert_eq!(request.custom_instructions(), "");
        assert!(request.selected_suggestion_indices().is_empty());
        assert!(request.focus_areas().is_empty());
        assert_eq!(request.story_creation_brief(), "");
        assert_eq!(request.quality_notes(), "");
        assert_eq!(request.story_repair_summary(), "");
        assert_eq!(request.creative_mode(), "");
        assert_eq!(request.story_focus(), "");
        assert_eq!(request.plot_stage(), "");
        assert_eq!(request.quality_preset(), "");
        assert_eq!(request.enable_web_research(), None);
        assert_eq!(request.web_research_query(), None);
        assert!(!request.preserve_structure());
        assert!(request.preserve_dialogues().is_empty());
        assert!(request.preserve_plot_points().is_empty());
        assert!(request.preserve_character_traits());
        assert!(request.story_repair_targets().is_empty());
        assert!(request.story_preserve_strengths().is_empty());
    }

    #[test]
    fn should_build_partial_regeneration_stream_workflow_request_from_route_payload() {
        let route_request = PartialRegenerationStreamRouteRequest {
            selected_text: "选中文本".to_string(),
            start_position: 12,
            end_position: 24,
            user_instructions: " 请更紧凑一些 ".to_string(),
            context_chars: Some(800),
            style_id: Some(3),
            length_mode: Some(" expand ".to_string()),
            target_word_count: Some(1500),
            enable_web_research: Some(true),
            web_research_query: Some(" 检索背景资料 ".to_string()),
        };
        let request =
            build_partial_regeneration_stream_workflow_request_from_route_payload(route_request);

        assert_eq!(request.selected_text(), "选中文本");
        assert_eq!(request.start_position(), 12);
        assert_eq!(request.end_position(), 24);
        assert_eq!(request.context_chars(), 800);
        assert_eq!(request.user_instructions(), "请更紧凑一些");
        assert_eq!(request.length_mode(), Some("expand"));
        assert_eq!(request.target_word_count(), Some(1500));
        assert_eq!(request.style_id(), Some(3));
        assert!(request.web_research_enabled());
        assert_eq!(request.web_research_query(), Some("检索背景资料"));
    }
}
