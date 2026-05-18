use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::services::chapter_batch_generation_active_list_query_service::LoadActiveBatchGenerationTaskListQueryError;
use crate::services::chapter_batch_generation_active_query_service::LoadActiveBatchGenerationQueryError;
use crate::services::chapter_batch_generation_cancel_service::CancelBatchGenerationWorkflowError;
use crate::services::chapter_batch_generation_create_workflow_service::{
    CreateBatchGenerationWorkflowDomainError, CreateBatchGenerationWorkflowError,
};
use crate::services::chapter_batch_generation_resume_service::PrepareBatchGenerationResumeRequestError;
use crate::services::chapter_batch_generation_status_query_service::LoadBatchGenerationStatusQueryError;
use crate::services::chapter_batch_generation_stream_access_service::BatchGenerationStatusStreamAccessError;
use crate::services::chapter_single_generation_request_service::PrepareSingleChapterGenerationRequestError;

pub fn map_active_batch_generation_query_error(
    error: LoadActiveBatchGenerationQueryError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadActiveBatchGenerationQueryError::ProjectNotFoundOrAccessDenied => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found or access denied"})),
        ),
        LoadActiveBatchGenerationQueryError::Internal(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        ),
    }
}

pub fn map_active_batch_generation_task_list_query_error(
    error: LoadActiveBatchGenerationTaskListQueryError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadActiveBatchGenerationTaskListQueryError::Internal(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        ),
    }
}

pub fn map_create_batch_generation_workflow_error(
    error: CreateBatchGenerationWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        CreateBatchGenerationWorkflowError::ProjectNotFoundOrAccessDenied => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found or access denied"})),
        ),
        CreateBatchGenerationWorkflowError::Domain(
            CreateBatchGenerationWorkflowDomainError::InvalidCount,
        ) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "count must be greater than 0"})),
        ),
        CreateBatchGenerationWorkflowError::Domain(
            CreateBatchGenerationWorkflowDomainError::ChaptersNotFound,
        ) => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "未找到指定范围内的章节"})),
        ),
        CreateBatchGenerationWorkflowError::Config(error) => {
            (StatusCode::BAD_REQUEST, Json(json!({"detail": error})))
        }
        CreateBatchGenerationWorkflowError::Internal(error) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": error})))
        }
    }
}

pub fn map_cancel_batch_generation_workflow_error(
    error: CancelBatchGenerationWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        CancelBatchGenerationWorkflowError::TaskNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Batch generation task not found"})),
        ),
        CancelBatchGenerationWorkflowError::Domain(error) => {
            (StatusCode::BAD_REQUEST, Json(json!({"detail": error})))
        }
        CancelBatchGenerationWorkflowError::Internal(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        ),
    }
}

pub fn map_prepare_batch_generation_resume_request_error(
    error: PrepareBatchGenerationResumeRequestError,
) -> (StatusCode, Json<Value>) {
    match error {
        PrepareBatchGenerationResumeRequestError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Batch generation task not found"})),
        ),
        PrepareBatchGenerationResumeRequestError::Domain(error)
        | PrepareBatchGenerationResumeRequestError::Config(error) => {
            (StatusCode::BAD_REQUEST, Json(json!({"detail": error})))
        }
        PrepareBatchGenerationResumeRequestError::Internal(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        ),
    }
}

pub fn map_batch_generation_status_query_error(
    error: LoadBatchGenerationStatusQueryError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadBatchGenerationStatusQueryError::TaskNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Batch generation task not found"})),
        ),
        LoadBatchGenerationStatusQueryError::Internal(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        ),
    }
}

pub fn map_batch_generation_status_stream_access_error(
    error: BatchGenerationStatusStreamAccessError,
) -> (StatusCode, Json<Value>) {
    match error {
        BatchGenerationStatusStreamAccessError::TaskNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Batch generation task not found"})),
        ),
        BatchGenerationStatusStreamAccessError::Internal(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        ),
    }
}

pub fn map_single_chapter_generation_request_error(
    error: PrepareSingleChapterGenerationRequestError,
) -> (StatusCode, Json<Value>) {
    match error {
        PrepareSingleChapterGenerationRequestError::ChapterNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found"})),
        ),
        PrepareSingleChapterGenerationRequestError::ChapterNotFoundOrAccessDenied => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found or access denied"})),
        ),
        PrepareSingleChapterGenerationRequestError::Config(error) => {
            (StatusCode::BAD_REQUEST, Json(json!({"detail": error})))
        }
        PrepareSingleChapterGenerationRequestError::Internal(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error})),
        ),
    }
}
