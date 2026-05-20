use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    chapter_not_found_or_access_denied_error, detail_error, internal_detail_error,
    project_not_found_or_access_denied_error,
};

use crate::services::chapter_batch_generation_active_list_query_service::LoadActiveBatchGenerationTaskListQueryError;
use crate::services::chapter_batch_generation_active_query_service::LoadActiveBatchGenerationQueryError;
use crate::services::chapter_batch_generation_cancel_service::CancelBatchGenerationWorkflowError;
use crate::services::chapter_batch_generation_create_workflow_service::{
    CreateBatchGenerationWorkflowDomainError, CreateBatchGenerationWorkflowError,
};
use crate::services::chapter_batch_generation_resume_service::PrepareBatchGenerationResumeRequestError;
use crate::services::chapter_batch_generation_status_query_service::LoadBatchGenerationStatusQueryError;
use crate::services::chapter_batch_generation_status_stream_service::BatchGenerationStatusStreamAccessError;
use crate::services::chapter_single_generation_background_workflow_service::CreateSingleGenerationBackgroundWorkflowError;
use crate::services::chapter_single_generation_request_service::PrepareSingleChapterGenerationRequestError;

fn batch_generation_task_not_found_error() -> (StatusCode, Json<Value>) {
    detail_error(StatusCode::NOT_FOUND, "Batch generation task not found")
}

enum SingleChapterGenerationError {
    ChapterNotFound,
    ChapterNotFoundOrAccessDenied,
    Config(String),
    Internal(String),
}

fn map_single_chapter_generation_error(
    error: SingleChapterGenerationError,
) -> (StatusCode, Json<Value>) {
    match error {
        SingleChapterGenerationError::ChapterNotFound => {
            detail_error(StatusCode::NOT_FOUND, "Chapter not found")
        }
        SingleChapterGenerationError::ChapterNotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        SingleChapterGenerationError::Config(error) => detail_error(StatusCode::BAD_REQUEST, error),
        SingleChapterGenerationError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_active_batch_generation_query_error(
    error: LoadActiveBatchGenerationQueryError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadActiveBatchGenerationQueryError::ProjectNotFoundOrAccessDenied => {
            project_not_found_or_access_denied_error()
        }
        LoadActiveBatchGenerationQueryError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_active_batch_generation_task_list_query_error(
    error: LoadActiveBatchGenerationTaskListQueryError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadActiveBatchGenerationTaskListQueryError::Internal(error) => {
            internal_detail_error(error)
        }
    }
}

pub(crate) fn map_create_batch_generation_workflow_error(
    error: CreateBatchGenerationWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        CreateBatchGenerationWorkflowError::ProjectNotFoundOrAccessDenied => {
            project_not_found_or_access_denied_error()
        }
        CreateBatchGenerationWorkflowError::Domain(
            CreateBatchGenerationWorkflowDomainError::InvalidCount,
        ) => detail_error(StatusCode::BAD_REQUEST, "count must be greater than 0"),
        CreateBatchGenerationWorkflowError::Domain(
            CreateBatchGenerationWorkflowDomainError::ChaptersNotFound,
        ) => detail_error(StatusCode::NOT_FOUND, "未找到指定范围内的章节"),
        CreateBatchGenerationWorkflowError::Config(error) => {
            detail_error(StatusCode::BAD_REQUEST, error)
        }
        CreateBatchGenerationWorkflowError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_cancel_batch_generation_workflow_error(
    error: CancelBatchGenerationWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        CancelBatchGenerationWorkflowError::TaskNotFound => batch_generation_task_not_found_error(),
        CancelBatchGenerationWorkflowError::Domain(error) => {
            detail_error(StatusCode::BAD_REQUEST, error)
        }
        CancelBatchGenerationWorkflowError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_prepare_batch_generation_resume_request_error(
    error: PrepareBatchGenerationResumeRequestError,
) -> (StatusCode, Json<Value>) {
    match error {
        PrepareBatchGenerationResumeRequestError::NotFound => {
            batch_generation_task_not_found_error()
        }
        PrepareBatchGenerationResumeRequestError::Domain(error)
        | PrepareBatchGenerationResumeRequestError::Config(error) => {
            detail_error(StatusCode::BAD_REQUEST, error)
        }
        PrepareBatchGenerationResumeRequestError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_batch_generation_status_query_error(
    error: LoadBatchGenerationStatusQueryError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadBatchGenerationStatusQueryError::TaskNotFound => {
            batch_generation_task_not_found_error()
        }
        LoadBatchGenerationStatusQueryError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_batch_generation_status_stream_access_error(
    error: BatchGenerationStatusStreamAccessError,
) -> (StatusCode, Json<Value>) {
    match error {
        BatchGenerationStatusStreamAccessError::TaskNotFound => {
            batch_generation_task_not_found_error()
        }
        BatchGenerationStatusStreamAccessError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_single_chapter_generation_request_error(
    error: PrepareSingleChapterGenerationRequestError,
) -> (StatusCode, Json<Value>) {
    match error {
        PrepareSingleChapterGenerationRequestError::ChapterNotFound => {
            map_single_chapter_generation_error(SingleChapterGenerationError::ChapterNotFound)
        }
        PrepareSingleChapterGenerationRequestError::ChapterNotFoundOrAccessDenied => {
            map_single_chapter_generation_error(
                SingleChapterGenerationError::ChapterNotFoundOrAccessDenied,
            )
        }
        PrepareSingleChapterGenerationRequestError::Config(error) => {
            map_single_chapter_generation_error(SingleChapterGenerationError::Config(error))
        }
        PrepareSingleChapterGenerationRequestError::Internal(error) => {
            map_single_chapter_generation_error(SingleChapterGenerationError::Internal(error))
        }
    }
}

pub(crate) fn map_create_single_generation_background_workflow_error(
    error: CreateSingleGenerationBackgroundWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        CreateSingleGenerationBackgroundWorkflowError::ChapterNotFound => {
            map_single_chapter_generation_error(SingleChapterGenerationError::ChapterNotFound)
        }
        CreateSingleGenerationBackgroundWorkflowError::ChapterNotFoundOrAccessDenied => {
            map_single_chapter_generation_error(
                SingleChapterGenerationError::ChapterNotFoundOrAccessDenied,
            )
        }
        CreateSingleGenerationBackgroundWorkflowError::Config(error) => {
            map_single_chapter_generation_error(SingleChapterGenerationError::Config(error))
        }
        CreateSingleGenerationBackgroundWorkflowError::Internal(error) => {
            map_single_chapter_generation_error(SingleChapterGenerationError::Internal(error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_batch_generation_status_query_error, map_cancel_batch_generation_workflow_error,
        map_create_batch_generation_workflow_error,
        map_create_single_generation_background_workflow_error,
        map_prepare_batch_generation_resume_request_error,
    };
    use crate::services::chapter_batch_generation_cancel_service::CancelBatchGenerationWorkflowError;
    use crate::services::chapter_batch_generation_create_workflow_service::{
        CreateBatchGenerationWorkflowDomainError, CreateBatchGenerationWorkflowError,
    };
    use crate::services::chapter_batch_generation_resume_service::PrepareBatchGenerationResumeRequestError;
    use crate::services::chapter_batch_generation_status_query_service::LoadBatchGenerationStatusQueryError;
    use crate::services::chapter_single_generation_background_workflow_service::CreateSingleGenerationBackgroundWorkflowError;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn cancel_task_not_found_keeps_not_found_detail_message() {
        let response = map_cancel_batch_generation_workflow_error(
            CancelBatchGenerationWorkflowError::TaskNotFound,
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Batch generation task not found" })
        );
    }

    #[test]
    fn single_generation_background_not_found_or_access_denied_remains_404() {
        let response = map_create_single_generation_background_workflow_error(
            CreateSingleGenerationBackgroundWorkflowError::ChapterNotFoundOrAccessDenied,
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn single_generation_background_config_error_maps_to_bad_request() {
        let response = map_create_single_generation_background_workflow_error(
            CreateSingleGenerationBackgroundWorkflowError::Config("model missing".to_string()),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "model missing" }));
    }

    #[test]
    fn create_batch_generation_invalid_count_remains_bad_request() {
        let response =
            map_create_batch_generation_workflow_error(CreateBatchGenerationWorkflowError::Domain(
                CreateBatchGenerationWorkflowDomainError::InvalidCount,
            ));

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "count must be greater than 0" })
        );
    }

    #[test]
    fn prepare_batch_generation_resume_not_found_remains_not_found() {
        let response = map_prepare_batch_generation_resume_request_error(
            PrepareBatchGenerationResumeRequestError::NotFound,
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Batch generation task not found" })
        );
    }

    #[test]
    fn batch_generation_status_query_internal_error_remains_internal_detail() {
        let response = map_batch_generation_status_query_error(
            LoadBatchGenerationStatusQueryError::Internal("status query failed".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "status query failed" }));
    }
}
