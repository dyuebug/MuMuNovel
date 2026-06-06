use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    detail_error, internal_detail_error, project_not_found_or_access_denied_error,
};

use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
use crate::services::chapter_batch_generation_task_view_query_service::{
    ActiveBatchGenerationTaskListQueryRequestError, ActiveBatchGenerationTaskListRouteQueryError,
    ActiveProjectBatchGenerationRouteError,
};
use crate::services::chapter_batch_generation_write_workflow_service::{
    CancelBatchGenerationWriteWorkflowError, CreateBatchGenerationWriteWorkflowError,
    PrepareBatchGenerationCreateRequestError, ResumeBatchGenerationWriteWorkflowError,
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

pub(crate) fn map_active_project_batch_generation_route_error(
    error: ActiveProjectBatchGenerationRouteError,
) -> (StatusCode, Json<Value>) {
    match error {
        ActiveProjectBatchGenerationRouteError::Query(error) => {
            map_project_access_query_route_error(error)
        }
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

pub(crate) fn map_active_batch_generation_task_list_query_request_error(
    error: ActiveBatchGenerationTaskListQueryRequestError,
) -> (StatusCode, Json<Value>) {
    match error {
        ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall => detail_error(
            StatusCode::BAD_REQUEST,
            "limit must be greater than or equal to 1",
        ),
        ActiveBatchGenerationTaskListQueryRequestError::LimitTooLarge => detail_error(
            StatusCode::BAD_REQUEST,
            "limit must be less than or equal to 100",
        ),
    }
}

pub(crate) fn map_active_batch_generation_task_list_route_error(
    error: ActiveBatchGenerationTaskListRouteQueryError,
) -> (StatusCode, Json<Value>) {
    match error {
        ActiveBatchGenerationTaskListRouteQueryError::Request(error) => {
            map_active_batch_generation_task_list_query_request_error(error)
        }
        ActiveBatchGenerationTaskListRouteQueryError::Query(error) => {
            map_active_batch_generation_task_list_query_error(error)
        }
    }
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
            PrepareBatchGenerationCreateRequestError::InvalidCountTooLarge,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "count must be less than or equal to 20",
        ),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooSmall,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "target_word_count must be greater than or equal to 500",
        ),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooLarge,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "target_word_count must be less than or equal to 10000",
        ),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidMaxRetries,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "max_retries must be between 0 and 5",
        ),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidCreativeMode,
        ) => detail_error(StatusCode::BAD_REQUEST, "creative_mode is invalid"),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidStoryFocus,
        ) => detail_error(StatusCode::BAD_REQUEST, "story_focus is invalid"),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidPlotStage,
        ) => detail_error(StatusCode::BAD_REQUEST, "plot_stage is invalid"),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidQualityPreset,
        ) => detail_error(StatusCode::BAD_REQUEST, "quality_preset is invalid"),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::StoryCreationBriefTooLong,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "story_creation_brief must be at most 1200 characters",
        ),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::QualityNotesTooLong,
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            "quality_notes must be at most 600 characters",
        ),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::ProjectHasNoChapters,
        ) => detail_error(StatusCode::NOT_FOUND, "项目下暂无章节"),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::ChaptersNotFound,
        ) => detail_error(StatusCode::NOT_FOUND, "未找到指定范围内的章节"),
        CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::PrerequisitesBlocked(error),
        ) => detail_error(
            StatusCode::BAD_REQUEST,
            format!("批量生成前置检查未通过：{error}"),
        ),
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
    error: CancelBatchGenerationWriteWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        CancelBatchGenerationWriteWorkflowError::Task(error) => {
            map_owned_batch_generation_task_route_error(error)
        }
        CancelBatchGenerationWriteWorkflowError::Domain(error) => {
            detail_error(StatusCode::BAD_REQUEST, error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_active_batch_generation_task_list_query_request_error,
        map_active_batch_generation_task_list_route_error,
        map_active_project_batch_generation_route_error,
        map_cancel_batch_generation_workflow_error, map_create_batch_generation_workflow_error,
        map_owned_batch_generation_task_route_error, map_project_access_query_route_error,
        map_resume_batch_generation_task_command_config_route_error,
    };
    use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
    use crate::services::chapter_batch_generation_resume_task_command_service::ResumeBatchGenerationDomainError;
    use crate::services::chapter_batch_generation_task_view_query_service::{
        ActiveBatchGenerationTaskListQueryRequestError,
        ActiveBatchGenerationTaskListRouteQueryError, ActiveProjectBatchGenerationRouteError,
    };
    use crate::services::chapter_batch_generation_write_workflow_service::{
        CancelBatchGenerationWriteWorkflowError, CreateBatchGenerationWriteWorkflowError,
        PrepareBatchGenerationCreateRequestError, ResumeBatchGenerationWriteWorkflowError,
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
    fn active_project_batch_generation_route_error_keeps_project_access_mapping() {
        let not_found_response = map_active_project_batch_generation_route_error(
            ActiveProjectBatchGenerationRouteError::Query(
                ProjectAccessQueryError::NotFoundOrAccessDenied,
            ),
        );
        let internal_response = map_active_project_batch_generation_route_error(
            ActiveProjectBatchGenerationRouteError::Query(ProjectAccessQueryError::Internal(
                "project lookup failed".to_string(),
            )),
        );

        assert_eq!(not_found_response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            not_found_response.1 .0,
            json!({ "detail": "Project not found or access denied" })
        );
        assert_eq!(internal_response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            internal_response.1 .0,
            json!({ "detail": "project lookup failed" })
        );
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
        let response = map_cancel_batch_generation_workflow_error(
            CancelBatchGenerationWriteWorkflowError::Task(
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
    fn cancel_task_internal_error_keeps_internal_detail_message() {
        let response = map_cancel_batch_generation_workflow_error(
            CancelBatchGenerationWriteWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::Internal("task lookup failed".to_string()),
            ),
        );

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
    fn create_batch_generation_count_upper_bound_matches_python_contract() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::InvalidCountTooLarge,
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "count must be less than or equal to 20" })
        );
    }

    #[test]
    fn create_batch_generation_target_word_count_lower_bound_matches_python_contract() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooSmall,
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "target_word_count must be greater than or equal to 500" })
        );
    }

    #[test]
    fn create_batch_generation_target_word_count_upper_bound_matches_python_contract() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooLarge,
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "target_word_count must be less than or equal to 10000" })
        );
    }

    #[test]
    fn create_batch_generation_max_retries_bounds_match_python_contract() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::InvalidMaxRetries,
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "max_retries must be between 0 and 5" })
        );
    }

    #[test]
    fn create_batch_generation_generation_choice_errors_remain_bad_request() {
        let cases = [
            (
                PrepareBatchGenerationCreateRequestError::InvalidCreativeMode,
                "creative_mode is invalid",
            ),
            (
                PrepareBatchGenerationCreateRequestError::InvalidStoryFocus,
                "story_focus is invalid",
            ),
            (
                PrepareBatchGenerationCreateRequestError::InvalidPlotStage,
                "plot_stage is invalid",
            ),
            (
                PrepareBatchGenerationCreateRequestError::InvalidQualityPreset,
                "quality_preset is invalid",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_create_batch_generation_workflow_error(
                CreateBatchGenerationWriteWorkflowError::Prepare(error),
            );

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn create_batch_generation_generation_text_length_errors_remain_bad_request() {
        let cases = [
            (
                PrepareBatchGenerationCreateRequestError::StoryCreationBriefTooLong,
                "story_creation_brief must be at most 1200 characters",
            ),
            (
                PrepareBatchGenerationCreateRequestError::QualityNotesTooLong,
                "quality_notes must be at most 600 characters",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_create_batch_generation_workflow_error(
                CreateBatchGenerationWriteWorkflowError::Prepare(error),
            );

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn active_batch_generation_task_list_limit_errors_match_python_query_bounds() {
        let cases = [
            (
                ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall,
                "limit must be greater than or equal to 1",
            ),
            (
                ActiveBatchGenerationTaskListQueryRequestError::LimitTooLarge,
                "limit must be less than or equal to 100",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_active_batch_generation_task_list_query_request_error(error);

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn active_batch_generation_task_list_route_error_keeps_query_and_request_mapping() {
        let request_response = map_active_batch_generation_task_list_route_error(
            ActiveBatchGenerationTaskListRouteQueryError::Request(
                ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall,
            ),
        );
        let query_response = map_active_batch_generation_task_list_route_error(
            ActiveBatchGenerationTaskListRouteQueryError::Query("boom".to_string()),
        );

        assert_eq!(request_response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            request_response.1 .0,
            json!({ "detail": "limit must be greater than or equal to 1" })
        );
        assert_eq!(query_response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(query_response.1 .0, json!({ "detail": "boom" }));
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
    fn create_batch_generation_project_has_no_chapters_remains_not_found() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::ProjectHasNoChapters,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "项目下暂无章节" }));
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
    fn create_batch_generation_prerequisites_blocked_matches_python_detail() {
        let response = map_create_batch_generation_workflow_error(
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::PrerequisitesBlocked(
                    "前置章节尚未完成: 2 章".to_string(),
                ),
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "批量生成前置检查未通过：前置章节尚未完成: 2 章" })
        );
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
                ResumeBatchGenerationDomainError::NoChaptersLeftToResume,
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "No chapters left to resume" })
        );
    }

    #[test]
    fn prepare_batch_generation_resume_single_chapter_unavailable_keeps_bad_request_detail_message()
    {
        let response = map_resume_batch_generation_task_command_config_route_error(
            ResumeBatchGenerationWriteWorkflowError::Domain(
                ResumeBatchGenerationDomainError::SingleChapterUnavailable(
                    "Chapter not found or access denied".to_string(),
                ),
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
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
