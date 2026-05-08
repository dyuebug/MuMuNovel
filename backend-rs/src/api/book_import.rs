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
use crate::services::book_import_service::BookImportService;

const MAX_TXT_SIZE: usize = 50 * 1024 * 1024; // 50MB

async fn create_task(
    Extension(service): Extension<Arc<BookImportService>>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut file_content: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut project_id: Option<String> = None;
    let mut create_new_project = true;
    let mut import_mode = "append".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let fname = field.file_name().unwrap_or("unknown.txt").to_string();
                filename = Some(fname);
                let data = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"detail": format!("读取文件失败: {}", e)})),
                    )
                })?;
                file_content = Some(data.to_vec());
            }
            "project_id" => {
                let val = field.text().await.unwrap_or_default();
                if !val.is_empty() {
                    project_id = Some(val);
                }
            }
            "create_new_project" => {
                let val = field.text().await.unwrap_or_default();
                if val == "false" || val == "0" {
                    create_new_project = false;
                }
            }
            "import_mode" => {
                let val = field.text().await.unwrap_or_default();
                if !val.is_empty() {
                    import_mode = val;
                }
            }
            _ => {}
        }
    }

    let filename = filename.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "未提供文件"})),
        )
    })?;

    if !filename.to_lowercase().ends_with(".txt") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "仅支持 .txt 文件"})),
        ));
    }

    if import_mode != "append" && import_mode != "overwrite" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "import_mode 仅支持 append 或 overwrite"})),
        ));
    }

    if project_id.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "当前仅支持新建项目导入，不支持指定 project_id"})),
        ));
    }

    if !create_new_project {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "当前仅支持新建项目导入"})),
        ));
    }

    let file_content = file_content.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "文件内容为空"})),
        )
    })?;

    if file_content.len() > MAX_TXT_SIZE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"detail": "文件大小超过 50MB 限制"})),
        ));
    }

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
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let project_suggestion = &body["project_suggestion"];
    let chapters = body["chapters"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    let outlines = body["outlines"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    let import_mode = body["import_mode"].as_str().unwrap_or("append");

    match service
        .apply_import(
            &db,
            &task_id,
            &claims.sub,
            project_suggestion,
            chapters,
            outlines,
            import_mode,
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
    Json(body): Json<Value>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let project_suggestion = body["project_suggestion"].clone();
    let chapters = body["chapters"].as_array().cloned().unwrap_or_default();
    let outlines = body["outlines"].as_array().cloned().unwrap_or_default();
    let import_mode = body["import_mode"].as_str().unwrap_or("append").to_string();
    let user_id = claims.sub.clone();

    tokio::spawn(async move {
        service
            .apply_import_stream(
                &db,
                &task_id,
                &user_id,
                &project_suggestion,
                &chapters,
                &outlines,
                &import_mode,
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
    Json(body): Json<serde_json::Value>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let steps: Vec<String> = body["steps"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let user_id = claims.sub.clone();

    tokio::spawn(async move {
        service
            .retry_stream(&db, &task_id, &user_id, &steps, &channel)
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
