use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    detail_error, internal_detail_error, project_not_found_or_access_denied_error,
};

use crate::services::chapter_batch_generation_cancel_service::CancelBatchGenerationWorkflowError;
use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
use crate::services::chapter_batch_generation_write_workflow_service::{
    CreateBatchGenerationWriteWorkflowError, PrepareBatchGenerationCreateRequestError,
    ResumeBatchGenerationWriteWorkflowError,
};
use crate::services::project_access_query_service::ProjectAccessQueryError;

fn batch_generation_task_not_found_error() -> (StatusCode, Json<Value>) {
    detail_error(StatusCode::NOT_FOUND, "Batch generation task not found")
}

pub(crate) fn map_owned_batch_generation_task_route_error(
    error: LoadOwnedBatchGenerationTaskError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadOwnedBatchGenerationTaskError::TaskNotFound => batch_generation_task_not_found_error(),
        LoadOwnedBatchGenerationTaskError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_project_access_query_route_error(
    error: ProjectAccessQueryError,
) -> (StatusCode, Json<Value>) {
    match error {
        ProjectAccessQueryError::NotFoundOrAccessDenied => {
            project_not_found_or_access_denied_error()
        }
        ProjectAccessQueryError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_resume_batch_generation_task_command_config_route_error(
    error: ResumeBatchGenerationWriteWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        ResumeBatchGenerationWriteWorkflowError::Task(error) => {
            map_owned_batch_generation_task_route_error(error)
        }
        ResumeBatchGenerationWriteWorkflowError::Domain(error) => {
            detail_error(StatusCode::BAD_REQUEST, error.detail_message())
        }
        ResumeBatchGenerationWriteWorkflowError::Config(error) => {
            detail_error(StatusCode::BAD_REQUEST, error)
        }
    }
}

pub(crate) fn map_active_batch_generation_task_list_query_error(
    error: String,
) -> (StatusCode, Json<Value>) {
    internal_detail_error(error)
}

pub(crate) fn map_create_batch_generation_workflow_error(
    error: CreateBatchGenerationWriteWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        CreateBatchGenerationWriteWorkflowError::ProjectAccess(error) => {
            map_project_access_query_route_error(error)
        }
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidCount,
        ) => detail_error(StatusCode::BAD_REQUEST, "count must be greater than 0"),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::ChaptersNotFound,
        ) => detail_error(StatusCode::NOT_FOUND, "未找到指定范围内的章节"),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::Internal(error),
        ) => internal_detail_error(error),
        CreateBatchGenerationWriteWorkflowError::Config(error) => {
            detail_error(StatusCode::BAD_REQUEST, error)
        }
        CreateBatchGenerationWriteWorkflowError::Internal(error) => internal_detail_error(error),
    }
}

pub(crate) fn map_cancel_batch_generation_workflow_error(
    error: CancelBatchGenerationWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        CancelBatchGenerationWorkflowError::Task(error) => {
            map_owned_batch_generation_task_route_error(error)
        }
        CancelBatchGenerationWorkflowError::Domain(error) => {
            detail_error(StatusCode::BAD_REQUEST, error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_cancel_batch_generation_workflow_error, map_create_batch_generation_workflow_error,
        map_owned_batch_generation_task_route_error, map_project_access_query_route_error,
        map_resume_batch_generation_task_command_config_route_error,
    };
    use crate::services::chapter_batch_generation_cancel_service::CancelBatchGenerationWorkflowError;
    use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
    use crate::services::chapter_batch_generation_resume_task_command_service::ResumeBatchGenerationDomainError;
    use crate::services::chapter_batch_generation_write_workflow_service::{
        CreateBatchGenerationWriteWorkflowError, PrepareBatchGenerationCreateRequestError,
        ResumeBatchGenerationWriteWorkflowError,
    };
    use crate::services::project_access_query_service::ProjectAccessQueryError;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn project_access_query_not_found_or_access_denied_remains_not_found() {
        let response =
            map_project_access_query_route_error(ProjectAccessQueryError::NotFoundOrAccessDenied);

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Project not found or access denied" })
        );
    }

    #[test]
    fn project_access_query_internal_remains_internal_detail() {
        let response = map_project_access_query_route_error(ProjectAccessQueryError::Internal(
            "project lookup failed".to_string(),
        ));

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "project lookup failed" }));
    }

    #[test]
    fn owned_batch_generation_task_not_found_keeps_not_found_detail_message() {
        let response = map_owned_batch_generation_task_route_error(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Batch generation task not found" })
        );
    }

    #[test]
    fn owned_batch_generation_task_internal_error_keeps_internal_detail_message() {
        let response = map_owned_batch_generation_task_route_error(
            LoadOwnedBatchGenerationTaskError::Internal("task lookup failed".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "task lookup failed" }));
    }

    #[test]
    fn cancel_task_not_found_keeps_not_found_detail_message() {
        let response =
            map_cancel_batch_generation_workflow_error(CancelBatchGenerationWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::TaskNotFound,
            ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Batch generation task not found" })
        );
    }

    #[test]
    fn cancel_task_internal_error_keeps_internal_detail_message() {
        let response =
            map_cancel_batch_generation_workflow_error(CancelBatchGenerationWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::Internal("task lookup failed".to_string()),
            ));

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "task lookup failed" }));
    }

    #[test]
    fn create_batch_generation_invalid_count_remains_bad_request() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::InvalidCount,
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "count must be greater than 0" })
        );
    }

    #[test]
    fn create_batch_generation_project_access_denied_remains_not_found() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::ProjectAccess(
                ProjectAccessQueryError::NotFoundOrAccessDenied,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Project not found or access denied" })
        );
    }

    #[test]
    fn create_batch_generation_project_access_internal_remains_internal_detail() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::ProjectAccess(
                ProjectAccessQueryError::Internal("project lookup failed".to_string()),
            ),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "project lookup failed" }));
    }

    #[test]
    fn create_batch_generation_chapters_not_found_remains_not_found() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::ChaptersNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "未找到指定范围内的章节" }));
    }

    #[test]
    fn create_batch_generation_prepare_internal_error_remains_internal_detail() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::Internal("prepare failed".to_string()),
            ),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "prepare failed" }));
    }

    #[test]
    fn prepare_batch_generation_resume_not_found_remains_not_found() {
        let response = map_resume_batch_generation_task_command_config_route_error(
            ResumeBatchGenerationWriteWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::TaskNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Batch generation task not found" })
        );
    }

    #[test]
    fn prepare_batch_generation_resume_internal_lookup_error_remains_internal_detail() {
        let response = map_resume_batch_generation_task_command_config_route_error(
            ResumeBatchGenerationWriteWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::Internal("resume lookup failed".to_string()),
            ),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "resume lookup failed" }));
    }

    #[test]
    fn prepare_batch_generation_resume_domain_error_keeps_bad_request_detail_message() {
        let response = map_resume_batch_generation_task_command_config_route_error(
            ResumeBatchGenerationWriteWorkflowError::Domain(
                ResumeBatchGenerationDomainError::NoChaptersToResume,
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Batch generation task has no chapters to resume" })
        );
    }

    #[test]
    fn batch_generation_status_query_internal_error_remains_internal_detail() {
        let response = map_owned_batch_generation_task_route_error(
            LoadOwnedBatchGenerationTaskError::Internal("status query failed".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "status query failed" }));
    }

    #[test]
    fn batch_generation_status_stream_access_internal_error_remains_internal_detail() {
        let response = map_owned_batch_generation_task_route_error(
            LoadOwnedBatchGenerationTaskError::Internal("stream access failed".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "stream access failed" }));
    }
}
