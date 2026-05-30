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
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::services::auth::Claims;
use crate::services::book_import_request_service::{
    build_book_import_apply_request_from_route_payload,
    build_book_import_create_task_request_from_route_fields,
    build_book_import_retry_request_from_route_payload, BookImportApplyRouteRequest,
    BookImportCreateTaskRouteFields, BookImportRetryRouteRequest,
    BuildBookImportCreateTaskRequestError,
};
use crate::services::book_import_service::BookImportService;

const MAX_TXT_SIZE: usize = 50 * 1024 * 1024; // 50MB

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
        .route("/book-import/tasks", post(create_task))
        .route(
            "/book-import/tasks/{task_id}",
            get(get_task_status).delete(cancel_task),
        )
        .route("/book-import/tasks/{task_id}/preview", get(get_preview))
        .route("/book-import/tasks/{task_id}/apply", post(apply_import))
        .route(
            "/book-import/tasks/{task_id}/apply-stream",
            post(apply_stream),
        )
        .route(
            "/book-import/tasks/{task_id}/retry-stream",
            post(retry_stream),
        )
}
