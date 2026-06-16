use std::sync::Arc;

use axum::response::sse::Event;
use axum::{
    extract::{Extension, Multipart, Path},
    http::StatusCode,
    response::{Json, Sse},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::services::auth::Claims;
use crate::services::book_import_service::BookImportService;

const MAX_TXT_SIZE: usize = 50 * 1024 * 1024; // 50MB
const BOOK_IMPORT_TASKS_ROUTE: &str = "/book-import/tasks";
const BOOK_IMPORT_TASK_ROUTE: &str = "/book-import/tasks/{task_id}";
const BOOK_IMPORT_PREVIEW_ROUTE: &str = "/book-import/tasks/{task_id}/preview";
const BOOK_IMPORT_APPLY_ROUTE: &str = "/book-import/tasks/{task_id}/apply";
const BOOK_IMPORT_APPLY_STREAM_ROUTE: &str = "/book-import/tasks/{task_id}/apply-stream";
const BOOK_IMPORT_RETRY_STREAM_ROUTE: &str = "/book-import/tasks/{task_id}/retry-stream";

#[derive(Debug, Clone, PartialEq, Eq)]
enum BuildBookImportCreateTaskRequestError {
    MissingFile,
    EmptyFileContent,
    UnsupportedFileType,
    UnsupportedImportMode,
    ProjectIdNotSupported,
    ExistingProjectImportNotSupported,
    FileTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BookImportCreateTaskRequest {
    filename: String,
    file_content: Vec<u8>,
    import_mode: String,
}

impl BookImportCreateTaskRequest {
    fn filename(&self) -> &str {
        self.filename.as_str()
    }

    fn into_file_content(self) -> Vec<u8> {
        self.file_content
    }

    fn import_mode(&self) -> &str {
        self.import_mode.as_str()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BookImportCreateTaskRouteFields {
    file_content: Option<Vec<u8>>,
    filename: Option<String>,
    project_id: Option<String>,
    create_new_project: bool,
    import_mode: String,
}

impl BookImportCreateTaskRouteFields {
    fn new() -> Self {
        Self {
            file_content: None,
            filename: None,
            project_id: None,
            create_new_project: true,
            import_mode: "append".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BookImportApplyRequest {
    project_suggestion: Value,
    chapters: Vec<Value>,
    outlines: Vec<Value>,
    import_mode: String,
}

impl BookImportApplyRequest {
    fn project_suggestion(&self) -> &Value {
        &self.project_suggestion
    }

    fn chapters(&self) -> &[Value] {
        self.chapters.as_slice()
    }

    fn outlines(&self) -> &[Value] {
        self.outlines.as_slice()
    }

    fn import_mode(&self) -> &str {
        self.import_mode.as_str()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct BookImportApplyRouteRequest {
    #[serde(default)]
    project_suggestion: Option<Value>,
    #[serde(default)]
    chapters: Option<Value>,
    #[serde(default)]
    outlines: Option<Value>,
    #[serde(default)]
    import_mode: Option<Value>,
}

impl BookImportApplyRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "project_suggestion": self.project_suggestion,
            "chapters": self.chapters,
            "outlines": self.outlines,
            "import_mode": self.import_mode,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BookImportRetryRequest {
    steps: Vec<String>,
}

impl BookImportRetryRequest {
    fn steps(&self) -> &[String] {
        self.steps.as_slice()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct BookImportRetryRouteRequest {
    #[serde(default)]
    steps: Option<Value>,
}

impl BookImportRetryRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "steps": self.steps,
        })
    }
}

fn build_book_import_create_task_request_from_route_fields(
    fields: BookImportCreateTaskRouteFields,
    max_txt_size: usize,
) -> Result<BookImportCreateTaskRequest, BuildBookImportCreateTaskRequestError> {
    let filename = fields
        .filename
        .ok_or(BuildBookImportCreateTaskRequestError::MissingFile)?;

    if !filename.to_lowercase().ends_with(".txt") {
        return Err(BuildBookImportCreateTaskRequestError::UnsupportedFileType);
    }

    if fields.import_mode != "append" && fields.import_mode != "overwrite" {
        return Err(BuildBookImportCreateTaskRequestError::UnsupportedImportMode);
    }

    if fields.project_id.is_some() {
        return Err(BuildBookImportCreateTaskRequestError::ProjectIdNotSupported);
    }

    if !fields.create_new_project {
        return Err(BuildBookImportCreateTaskRequestError::ExistingProjectImportNotSupported);
    }

    let file_content = fields
        .file_content
        .ok_or(BuildBookImportCreateTaskRequestError::EmptyFileContent)?;

    if file_content.is_empty() {
        return Err(BuildBookImportCreateTaskRequestError::EmptyFileContent);
    }

    if file_content.len() > max_txt_size {
        return Err(BuildBookImportCreateTaskRequestError::FileTooLarge);
    }

    Ok(BookImportCreateTaskRequest {
        filename,
        file_content,
        import_mode: fields.import_mode,
    })
}

fn build_book_import_apply_request_from_route_body(body: &Value) -> BookImportApplyRequest {
    BookImportApplyRequest {
        project_suggestion: body
            .get("project_suggestion")
            .cloned()
            .unwrap_or(Value::Null),
        chapters: body
            .get("chapters")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        outlines: body
            .get("outlines")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        import_mode: body
            .get("import_mode")
            .and_then(Value::as_str)
            .unwrap_or("append")
            .to_string(),
    }
}

fn build_book_import_apply_request_from_route_payload(
    route_request: BookImportApplyRouteRequest,
) -> BookImportApplyRequest {
    build_book_import_apply_request_from_route_body(&route_request.into_body())
}

fn build_book_import_retry_request_from_route_body(body: &Value) -> BookImportRetryRequest {
    BookImportRetryRequest {
        steps: body
            .get("steps")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|value| value.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn build_book_import_retry_request_from_route_payload(
    route_request: BookImportRetryRouteRequest,
) -> BookImportRetryRequest {
    build_book_import_retry_request_from_route_body(&route_request.into_body())
}

#[cfg(test)]
fn build_book_import_route_owner_contract() -> Value {
    json!({
        "owner": "book_import",
        "scope": "book_import_create_status_preview_cancel_apply_retry_stream_route_group",
        "python_source_map": [
            "backend/app/api/book_import.py",
            "backend/app/services/book_import_service.py",
            "backend/app/schemas/book_import.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/api/book_import.rs",
            "backend-rs/src/services/book_import_service.rs",
            "deploy/strangler-gateway-probes.json"
        ],
        "route_contract": {
            "create_task": BOOK_IMPORT_TASKS_ROUTE,
            "task_status": BOOK_IMPORT_TASK_ROUTE,
            "cancel": BOOK_IMPORT_TASK_ROUTE,
            "preview": BOOK_IMPORT_PREVIEW_ROUTE,
            "apply": BOOK_IMPORT_APPLY_ROUTE,
            "apply_stream": BOOK_IMPORT_APPLY_STREAM_ROUTE,
            "retry_stream": BOOK_IMPORT_RETRY_STREAM_ROUTE
        },
        "behavior_contract": {
            "route_entrypoints": [
                "create_task",
                "get_task_status",
                "get_preview",
                "cancel_task",
                "apply_import",
                "apply_stream",
                "retry_stream"
            ],
            "request_owners": [
                "BookImportCreateTaskRouteFields",
                "BookImportApplyRouteRequest",
                "BookImportRetryRouteRequest"
            ],
            "service_handoffs": [
                "BookImportService::create_task",
                "BookImportService::get_task_status",
                "BookImportService::get_preview",
                "BookImportService::cancel_task",
                "BookImportService::apply_import",
                "BookImportService::apply_import_stream",
                "BookImportService::retry_stream"
            ],
            "stream_routes": [
                "apply_stream",
                "retry_stream"
            ],
            "create_task_policy": {
                "max_txt_size_bytes": MAX_TXT_SIZE,
                "supported_file_type": ".txt",
                "supported_import_modes": [
                    "append",
                    "overwrite"
                ],
                "existing_project_import_supported": false
            },
            "error_mapping": {
                "task_not_found": 404,
                "permission_denied": 403,
                "preview_not_completed": 400,
                "create_payload_too_large": 413
            }
        },
        "readiness_evidence": [
            "book-import-create-task-auth-guard-rust",
            "book-import-task-status-auth-guard-rust",
            "book-import-preview-auth-guard-rust",
            "book-import-cancel-auth-guard-rust",
            "book-import-apply-auth-guard-rust",
            "book-import-retry-stream-auth-guard-rust",
            "book-import-apply-stream-auth-guard-rust",
            "book-import-create-task-business-rust",
            "book-import-task-status-business-rust",
            "book-import-cancel-business-rust",
            "book-import-missing-status-business-rust",
            "book-import-missing-preview-business-rust",
            "book-import-missing-apply-business-rust",
            "book-import-missing-retry-stream-business-rust",
            "book-import-missing-apply-stream-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-book-import-business-owner",
            "business_probes": [
                "book-import-create-task-business-rust",
                "book-import-task-status-business-rust",
                "book-import-cancel-business-rust",
                "book-import-missing-status-business-rust",
                "book-import-missing-preview-business-rust",
                "book-import-missing-apply-business-rust",
                "book-import-missing-retry-stream-business-rust",
                "book-import-missing-apply-stream-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-book-import-business-owner",
            "owner_profile_probe_count": 8,
            "business_probe_count": 8,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Book import route business smoke is covered by phase5-book-import-business-owner; final completion now requires explicit source-map freeze/delete/repoint approval with same-round rollback policy.",
        "validation_boundary": [
            "cargo test api::book_import",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-book-import-business-owner",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_book_import_route_service_schema_files_as_source_map_until_explicit_freeze_delete_round",
            "python_route_files_status": "source_map_only_for_book_import_route_group",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit source-map freeze/delete/repoint approval",
                "provider-backed apply/apply-stream success smoke if final physical closeout requires AI import success coverage"
            ],
            "freeze_reason": "Rust book_import route group has dedicated phase5-book-import-business-owner probes for task create/status/cancel, logged-in missing-task JSON errors, and missing-task SSE error shells; final Python source-map freeze/delete/repoint still requires explicit approval and rollback policy.",
            "retired_manifest_fallbacks": [
                "book-import-create-task-auth-guard-python-fallback",
                "book-import-task-status-auth-guard-python-fallback",
                "book-import-preview-auth-guard-python-fallback",
                "book-import-cancel-auth-guard-python-fallback",
                "book-import-apply-auth-guard-python-fallback",
                "book-import-retry-stream-auth-guard-python-fallback",
                "book-import-apply-stream-auth-guard-python-fallback"
            ],
            "rollback_files": [
                "backend/app/api/book_import.py",
                "backend/app/services/book_import_service.py",
                "backend/app/schemas/book_import.py"
            ]
        }
    })
}

async fn create_task(
    Extension(service): Extension<Arc<BookImportService>>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut fields = BookImportCreateTaskRouteFields::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let fname = field.file_name().unwrap_or("unknown.txt").to_string();
                fields.filename = Some(fname);
                let data = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"detail": format!("读取文件失败: {}", e)})),
                    )
                })?;
                fields.file_content = Some(data.to_vec());
            }
            "project_id" => {
                let val = field.text().await.unwrap_or_default();
                if !val.is_empty() {
                    fields.project_id = Some(val);
                }
            }
            "create_new_project" => {
                let val = field.text().await.unwrap_or_default();
                if val == "false" || val == "0" {
                    fields.create_new_project = false;
                }
            }
            "import_mode" => {
                let val = field.text().await.unwrap_or_default();
                if !val.is_empty() {
                    fields.import_mode = val;
                }
            }
            _ => {}
        }
    }

    let request = build_book_import_create_task_request_from_route_fields(fields, MAX_TXT_SIZE)
        .map_err(|error| match error {
            BuildBookImportCreateTaskRequestError::MissingFile => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "未提供文件"})),
            ),
            BuildBookImportCreateTaskRequestError::EmptyFileContent => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "文件内容为空"})),
            ),
            BuildBookImportCreateTaskRequestError::UnsupportedFileType => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "仅支持 .txt 文件"})),
            ),
            BuildBookImportCreateTaskRequestError::UnsupportedImportMode => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "import_mode 仅支持 append 或 overwrite"})),
            ),
            BuildBookImportCreateTaskRequestError::ProjectIdNotSupported => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "当前仅支持新建项目导入，不支持指定 project_id"})),
            ),
            BuildBookImportCreateTaskRequestError::ExistingProjectImportNotSupported => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "当前仅支持新建项目导入"})),
            ),
            BuildBookImportCreateTaskRequestError::FileTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({"detail": "文件大小超过 50MB 限制"})),
            ),
        })?;

    let filename = request.filename().to_string();
    let import_mode = request.import_mode().to_string();
    let file_content = request.into_file_content();

    let result = service
        .create_task(&claims.sub, &filename, file_content, &import_mode)
        .await;
    Ok(Json(result))
}

async fn get_task_status(
    Extension(service): Extension<Arc<BookImportService>>,
    Extension(claims): Extension<Claims>,
    Path(task_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match service.get_task_status(&task_id, &claims.sub).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let status = if e == "任务不存在" {
                StatusCode::NOT_FOUND
            } else if e.starts_with("无权") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::BAD_REQUEST
            };
            Err((status, Json(json!({"detail": e}))))
        }
    }
}

async fn get_preview(
    Extension(service): Extension<Arc<BookImportService>>,
    Extension(claims): Extension<Claims>,
    Path(task_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match service.get_preview(&task_id, &claims.sub).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let status = if e == "任务不存在" {
                StatusCode::NOT_FOUND
            } else if e.starts_with("无权") {
                StatusCode::FORBIDDEN
            } else if e.contains("尚未完成") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err((status, Json(json!({"detail": e}))))
        }
    }
}

async fn cancel_task(
    Extension(service): Extension<Arc<BookImportService>>,
    Extension(claims): Extension<Claims>,
    Path(task_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match service.cancel_task(&task_id, &claims.sub).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let status = if e == "任务不存在" {
                StatusCode::NOT_FOUND
            } else if e.starts_with("无权") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::BAD_REQUEST
            };
            Err((status, Json(json!({"detail": e}))))
        }
    }
}

async fn apply_import(
    Extension(service): Extension<Arc<BookImportService>>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(task_id): Path<String>,
    Json(body): Json<BookImportApplyRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_book_import_apply_request_from_route_payload(body);

    match service
        .apply_import(
            &db,
            &task_id,
            &claims.sub,
            request.project_suggestion(),
            request.chapters(),
            request.outlines(),
            request.import_mode(),
        )
        .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            let status = if e.contains("不存在") {
                StatusCode::NOT_FOUND
            } else if e.contains("无权") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::BAD_REQUEST
            };
            Err((status, Json(json!({"detail": e}))))
        }
    }
}

async fn apply_stream(
    Extension(service): Extension<Arc<BookImportService>>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(task_id): Path<String>,
    Json(body): Json<BookImportApplyRouteRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let request = build_book_import_apply_request_from_route_payload(body);
    let user_id = claims.sub.clone();

    tokio::spawn(async move {
        service
            .apply_import_stream(
                &db,
                &task_id,
                &user_id,
                request.project_suggestion(),
                request.chapters(),
                request.outlines(),
                request.import_mode(),
                &channel,
            )
            .await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn retry_stream(
    Extension(service): Extension<Arc<BookImportService>>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(task_id): Path<String>,
    Json(body): Json<BookImportRetryRouteRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let request = build_book_import_retry_request_from_route_payload(body);
    let user_id = claims.sub.clone();

    tokio::spawn(async move {
        service
            .retry_stream(&db, &task_id, &user_id, request.steps(), &channel)
            .await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

pub fn routes() -> Router {
    Router::new()
        .route(BOOK_IMPORT_TASKS_ROUTE, post(create_task))
        .route(
            BOOK_IMPORT_TASK_ROUTE,
            get(get_task_status).delete(cancel_task),
        )
        .route(BOOK_IMPORT_PREVIEW_ROUTE, get(get_preview))
        .route(BOOK_IMPORT_APPLY_ROUTE, post(apply_import))
        .route(BOOK_IMPORT_APPLY_STREAM_ROUTE, post(apply_stream))
        .route(BOOK_IMPORT_RETRY_STREAM_ROUTE, post(retry_stream))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_book_import_apply_request_from_route_body,
        build_book_import_apply_request_from_route_payload,
        build_book_import_create_task_request_from_route_fields,
        build_book_import_retry_request_from_route_body,
        build_book_import_retry_request_from_route_payload, build_book_import_route_owner_contract,
        BookImportApplyRouteRequest, BookImportCreateTaskRouteFields, BookImportRetryRouteRequest,
        BuildBookImportCreateTaskRequestError, BOOK_IMPORT_APPLY_ROUTE,
        BOOK_IMPORT_APPLY_STREAM_ROUTE, BOOK_IMPORT_PREVIEW_ROUTE, BOOK_IMPORT_RETRY_STREAM_ROUTE,
        BOOK_IMPORT_TASKS_ROUTE, BOOK_IMPORT_TASK_ROUTE, MAX_TXT_SIZE,
    };

    #[test]
    fn should_publish_book_import_route_owner_contract() {
        let contract = build_book_import_route_owner_contract();

        assert_eq!(contract["owner"], "book_import");
        assert_eq!(
            contract["scope"],
            "book_import_create_status_preview_cancel_apply_retry_stream_route_group"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/api/book_import.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/book_import.rs"
        );
        assert!(contract["rust_owner_map"]
            .as_array()
            .expect("rust_owner_map should be an array")
            .iter()
            .all(|value| value != "backend-rs/src/services/book_import_request_service.rs"));
        assert_eq!(
            contract["route_contract"]["create_task"],
            BOOK_IMPORT_TASKS_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["task_status"],
            BOOK_IMPORT_TASK_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["apply_stream"],
            BOOK_IMPORT_APPLY_STREAM_ROUTE
        );
        assert_eq!(
            contract["behavior_contract"]["create_task_policy"]["max_txt_size_bytes"],
            MAX_TXT_SIZE
        );
        assert_eq!(
            contract["readiness_evidence"][14],
            "book-import-missing-apply-stream-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-book-import-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][7],
            "book-import-missing-apply-stream-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["owner_profile_probe_count"],
            json!(8)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(8)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy should be a string")
            .contains("phase5-book-import-business-owner"));
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
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"][0],
            "explicit source-map freeze/delete/repoint approval"
        );
    }

    #[test]
    fn should_keep_book_import_route_group_paths_stable() {
        assert_eq!(BOOK_IMPORT_TASKS_ROUTE, "/book-import/tasks");
        assert_eq!(BOOK_IMPORT_TASK_ROUTE, "/book-import/tasks/{task_id}");
        assert_eq!(
            BOOK_IMPORT_PREVIEW_ROUTE,
            "/book-import/tasks/{task_id}/preview"
        );
        assert_eq!(
            BOOK_IMPORT_APPLY_ROUTE,
            "/book-import/tasks/{task_id}/apply"
        );
        assert_eq!(
            BOOK_IMPORT_APPLY_STREAM_ROUTE,
            "/book-import/tasks/{task_id}/apply-stream"
        );
        assert_eq!(
            BOOK_IMPORT_RETRY_STREAM_ROUTE,
            "/book-import/tasks/{task_id}/retry-stream"
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_keeps_valid_fields() {
        let request = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1, 2, 3]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "overwrite".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect("valid create task request should build");

        assert_eq!(request.filename(), "novel.txt");
        assert_eq!(request.clone().into_file_content(), vec![1, 2, 3]);
        assert_eq!(request.import_mode(), "overwrite");
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_missing_file() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields::new(),
            MAX_TXT_SIZE,
        )
        .expect_err("missing file should fail");

        assert_eq!(error, BuildBookImportCreateTaskRequestError::MissingFile);
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_empty_content() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "append".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("empty content should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::EmptyFileContent
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_unsupported_type() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1]),
                filename: Some("novel.md".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "append".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("unsupported file type should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::UnsupportedFileType
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_invalid_import_mode() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "replace".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("unsupported import mode should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::UnsupportedImportMode
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_project_id() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1]),
                filename: Some("novel.txt".to_string()),
                project_id: Some("project-1".to_string()),
                create_new_project: true,
                import_mode: "append".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("project_id should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::ProjectIdNotSupported
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_non_new_project_mode() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: false,
                import_mode: "append".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("existing project import should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::ExistingProjectImportNotSupported
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_large_file() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1, 2, 3, 4]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "append".to_string(),
            },
            3,
        )
        .expect_err("oversized file should fail");

        assert_eq!(error, BuildBookImportCreateTaskRequestError::FileTooLarge);
    }

    #[test]
    fn build_book_import_apply_request_from_route_body_keeps_payload_fields() {
        let request = build_book_import_apply_request_from_route_body(&json!({
            "project_suggestion": {
                "title": "项目标题"
            },
            "chapters": [{"title": "第一章"}],
            "outlines": [{"title": "第一节"}],
            "import_mode": "overwrite"
        }));

        assert_eq!(request.project_suggestion(), &json!({"title": "项目标题"}));
        assert_eq!(request.chapters(), &[json!({"title": "第一章"})]);
        assert_eq!(request.outlines(), &[json!({"title": "第一节"})]);
        assert_eq!(request.import_mode(), "overwrite");
    }

    #[test]
    fn build_book_import_apply_request_from_route_body_uses_existing_defaults() {
        let request = build_book_import_apply_request_from_route_body(&json!({}));

        assert!(request.project_suggestion().is_null());
        assert!(request.chapters().is_empty());
        assert!(request.outlines().is_empty());
        assert_eq!(request.import_mode(), "append");
    }

    #[test]
    fn build_book_import_apply_request_from_route_payload_keeps_existing_contract() {
        let request =
            build_book_import_apply_request_from_route_payload(BookImportApplyRouteRequest {
                project_suggestion: Some(json!({"title": "项目标题"})),
                chapters: Some(json!([{"title": "第一章"}])),
                outlines: Some(json!([{"title": "第一节"}])),
                import_mode: Some(json!("overwrite")),
            });

        assert_eq!(request.project_suggestion(), &json!({"title": "项目标题"}));
        assert_eq!(request.chapters(), &[json!({"title": "第一章"})]);
        assert_eq!(request.outlines(), &[json!({"title": "第一节"})]);
        assert_eq!(request.import_mode(), "overwrite");
    }

    #[test]
    fn build_book_import_retry_request_from_route_body_filters_non_string_steps() {
        let request = build_book_import_retry_request_from_route_body(&json!({
            "steps": ["parse", 3, "import", null, true]
        }));

        assert_eq!(
            request.steps(),
            &["parse".to_string(), "import".to_string()]
        );
    }

    #[test]
    fn build_book_import_retry_request_from_route_body_defaults_to_empty_steps() {
        let request = build_book_import_retry_request_from_route_body(&json!({
            "steps": "invalid"
        }));

        assert!(request.steps().is_empty());
    }

    #[test]
    fn build_book_import_retry_request_from_route_payload_keeps_compat_parsing() {
        let request =
            build_book_import_retry_request_from_route_payload(BookImportRetryRouteRequest {
                steps: Some(json!(["parse", 3, "import", null, true])),
            });

        assert_eq!(
            request.steps(),
            &["parse".to_string(), "import".to_string()]
        );
    }
}
