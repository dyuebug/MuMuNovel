use axum::{http::StatusCode, response::Json};
use serde_json::{json, Value};

use crate::api::chapters_error_mapper::internal_detail_error;
use crate::api::chapters_error_mapper::map_create_chapter_analysis_task_error;
use crate::services::memories_query_service::{
    LoadProjectAccessError, LoadProjectChapterAnalysisPayloadError,
    MemoriesProjectQueryContextError, OwnedProjectMemoriesQueryError,
};
use crate::services::memories_write_workflow_service::{
    AnalyzeChapterMemoriesWriteWorkflowError, MemoriesProjectWriteContextError,
};

pub(crate) type MemoriesRouteError = (StatusCode, Json<Value>);

pub(crate) fn project_not_found_or_access_denied_error() -> MemoriesRouteError {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"detail": "项目不存在或无权限"})),
    )
}

fn map_project_access_error(error: LoadProjectAccessError) -> MemoriesRouteError {
    match error {
        LoadProjectAccessError::NotFoundOrAccessDenied => {
            project_not_found_or_access_denied_error()
        }
        LoadProjectAccessError::Internal(detail) => internal_detail_error(detail),
    }
}

fn map_memories_project_query_context_error(
    error: MemoriesProjectQueryContextError,
) -> MemoriesRouteError {
    match error {
        MemoriesProjectQueryContextError::ProjectAccess(error) => map_project_access_error(error),
        MemoriesProjectQueryContextError::Internal(detail) => internal_detail_error(detail),
    }
}

pub(crate) fn map_memories_project_write_context_error(
    error: MemoriesProjectWriteContextError,
) -> MemoriesRouteError {
    match error {
        MemoriesProjectWriteContextError::ProjectAccess(error) => map_project_access_error(error),
        MemoriesProjectWriteContextError::Internal(detail) => internal_detail_error(detail),
    }
}

pub(crate) fn map_project_chapter_analysis_payload_error(
    error: LoadProjectChapterAnalysisPayloadError,
) -> MemoriesRouteError {
    match error {
        LoadProjectChapterAnalysisPayloadError::Context(error) => {
            map_memories_project_query_context_error(error)
        }
        LoadProjectChapterAnalysisPayloadError::AnalysisNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "该章节还未进行分析"})),
        ),
    }
}

pub(crate) fn map_analyze_chapter_memories_write_workflow_error(
    error: AnalyzeChapterMemoriesWriteWorkflowError,
) -> MemoriesRouteError {
    match error {
        AnalyzeChapterMemoriesWriteWorkflowError::Context(error) => {
            map_memories_project_write_context_error(error)
        }
        AnalyzeChapterMemoriesWriteWorkflowError::CreateTask(error) => {
            map_create_chapter_analysis_task_error(error)
        }
    }
}

pub(crate) fn map_owned_project_memories_query_error(
    error: OwnedProjectMemoriesQueryError,
) -> MemoriesRouteError {
    map_memories_project_query_context_error(error)
}

#[cfg(test)]
mod tests {
    use super::{
        map_analyze_chapter_memories_write_workflow_error,
        map_memories_project_write_context_error, map_owned_project_memories_query_error,
        map_project_access_error, map_project_chapter_analysis_payload_error,
    };
    use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
    use crate::services::memories_query_service::{
        LoadMemoryStatsPayloadError, LoadProjectAccessError,
        LoadProjectChapterAnalysisPayloadError, LoadProjectMemoriesPayloadError,
        LoadUnresolvedForeshadowsPayloadError, MemoriesProjectQueryContextError,
        SearchProjectMemoriesPayloadError,
    };
    use crate::services::memories_write_workflow_service::{
        AnalyzeChapterMemoriesWriteWorkflowError, DeleteChapterMemoriesWriteWorkflowError,
        MemoriesProjectWriteContextError,
    };
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn project_access_not_found_keeps_chinese_not_found_detail() {
        let response = map_project_access_error(LoadProjectAccessError::NotFoundOrAccessDenied);

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
    }

    #[test]
    fn project_memories_internal_error_keeps_internal_detail() {
        let response =
            map_owned_project_memories_query_error(LoadProjectMemoriesPayloadError::ProjectAccess(
                LoadProjectAccessError::Internal("db exploded".to_string()),
            ));

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "db exploded" }));
    }

    #[test]
    fn chapter_analysis_not_found_keeps_existing_chinese_detail() {
        let response = map_project_chapter_analysis_payload_error(
            LoadProjectChapterAnalysisPayloadError::AnalysisNotFound,
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "该章节还未进行分析" }));
    }

    #[test]
    fn search_project_memories_not_found_reuses_project_access_mapping() {
        let response = map_owned_project_memories_query_error(
            SearchProjectMemoriesPayloadError::ProjectAccess(
                LoadProjectAccessError::NotFoundOrAccessDenied,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
    }

    #[test]
    fn unresolved_foreshadows_internal_error_keeps_internal_detail() {
        let response = map_owned_project_memories_query_error(
            LoadUnresolvedForeshadowsPayloadError::ProjectAccess(LoadProjectAccessError::Internal(
                "foreshadow failed".to_string(),
            )),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "foreshadow failed" }));
    }

    #[test]
    fn memory_stats_not_found_reuses_project_access_mapping() {
        let response =
            map_owned_project_memories_query_error(LoadMemoryStatsPayloadError::ProjectAccess(
                LoadProjectAccessError::NotFoundOrAccessDenied,
            ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
    }

    #[test]
    fn analyze_chapter_memories_create_task_error_reuses_existing_mapping() {
        let response = map_analyze_chapter_memories_write_workflow_error(
            AnalyzeChapterMemoriesWriteWorkflowError::CreateTask(
                CreateChapterAnalysisTaskError::ChapterEmpty,
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "章节不存在或内容为空" }));
    }

    #[test]
    fn delete_chapter_memories_internal_error_keeps_internal_detail() {
        let response = map_memories_project_write_context_error(
            DeleteChapterMemoriesWriteWorkflowError::Internal("delete failed".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "delete failed" }));
    }

    #[test]
    fn analyze_chapter_memories_context_not_found_reuses_shared_write_context_mapping() {
        let response = map_analyze_chapter_memories_write_workflow_error(
            AnalyzeChapterMemoriesWriteWorkflowError::Context(
                MemoriesProjectWriteContextError::ProjectAccess(
                    LoadProjectAccessError::NotFoundOrAccessDenied,
                ),
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
    }

    #[test]
    fn analyze_chapter_memories_context_internal_reuses_shared_write_context_mapping() {
        let response = map_analyze_chapter_memories_write_workflow_error(
            AnalyzeChapterMemoriesWriteWorkflowError::Context(
                MemoriesProjectWriteContextError::Internal("task creation failed".to_string()),
            ),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "task creation failed" }));
    }

    #[test]
    fn chapter_analysis_context_project_access_reuses_shared_context_mapping() {
        let response = map_project_chapter_analysis_payload_error(
            LoadProjectChapterAnalysisPayloadError::Context(
                MemoriesProjectQueryContextError::ProjectAccess(
                    LoadProjectAccessError::NotFoundOrAccessDenied,
                ),
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
    }

    #[test]
    fn chapter_analysis_context_internal_reuses_shared_context_mapping() {
        let response = map_project_chapter_analysis_payload_error(
            LoadProjectChapterAnalysisPayloadError::Context(
                MemoriesProjectQueryContextError::Internal("analysis query failed".to_string()),
            ),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "analysis query failed" }));
    }
}
