use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

use crate::services::chapter_crud_workflow_service::{
    ChapterCrudPayloadError, CrudPayloadError, ListChaptersByProjectPathPayloadError,
    ProjectCrudNotFound, ProjectCrudPayloadError,
};

pub type ChapterCrudRouteError = (StatusCode, Json<Value>);

fn success_message_error(status: StatusCode, message: impl Into<String>) -> ChapterCrudRouteError {
    (
        status,
        Json(json!({
            "success": false,
            "message": message.into(),
        })),
    )
}

fn detail_error(status: StatusCode, detail: impl Into<String>) -> ChapterCrudRouteError {
    (status, Json(json!({ "detail": detail.into() })))
}

fn internal_success_message_error(detail: impl Into<String>) -> ChapterCrudRouteError {
    success_message_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
}

fn project_not_found_or_access_denied_message_error() -> ChapterCrudRouteError {
    success_message_error(StatusCode::NOT_FOUND, "Project not found or access denied")
}

fn chapter_not_found_or_access_denied_message_error() -> ChapterCrudRouteError {
    success_message_error(StatusCode::NOT_FOUND, "Chapter not found or access denied")
}

fn map_crud_success_message_error<TNotFound>(
    error: CrudPayloadError<TNotFound>,
    not_found_error: impl FnOnce(TNotFound) -> ChapterCrudRouteError,
) -> ChapterCrudRouteError {
    match error {
        CrudPayloadError::NotFound(not_found) => not_found_error(not_found),
        CrudPayloadError::Internal(detail) => internal_success_message_error(detail),
    }
}

pub(crate) fn map_project_crud_success_message_error(
    error: ProjectCrudPayloadError,
) -> ChapterCrudRouteError {
    map_crud_success_message_error(error, |_| {
        project_not_found_or_access_denied_message_error()
    })
}

pub(crate) fn map_chapter_crud_success_message_error(
    error: ChapterCrudPayloadError,
) -> ChapterCrudRouteError {
    map_crud_success_message_error(error, |_| {
        chapter_not_found_or_access_denied_message_error()
    })
}

pub fn map_list_chapters_by_project_path_payload_error(
    error: ListChaptersByProjectPathPayloadError,
) -> ChapterCrudRouteError {
    match error {
        CrudPayloadError::NotFound(ProjectCrudNotFound::ProjectNotFound) => {
            detail_error(StatusCode::NOT_FOUND, "Project not found")
        }
        CrudPayloadError::Internal(detail) => {
            detail_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_chapter_crud_success_message_error, map_list_chapters_by_project_path_payload_error,
        map_project_crud_success_message_error,
    };
    use crate::services::chapter_crud_workflow_service::{
        ChapterCrudNotFound, CrudPayloadError, ProjectCrudNotFound,
    };
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn project_crud_success_message_owner_keeps_not_found_shape() {
        let response = map_project_crud_success_message_error(CrudPayloadError::NotFound(
            ProjectCrudNotFound::ProjectNotFound,
        ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "success": false, "message": "Project not found or access denied" })
        );
    }

    #[test]
    fn chapter_crud_success_message_owner_keeps_not_found_shape() {
        let response = map_chapter_crud_success_message_error(CrudPayloadError::NotFound(
            ChapterCrudNotFound::ChapterNotFound,
        ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "success": false, "message": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn create_chapter_project_not_found_keeps_success_message_shape() {
        let response = map_project_crud_success_message_error(
            crate::services::chapter_crud_workflow_service::CreateChapterPayloadError::NotFound(
                ProjectCrudNotFound::ProjectNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "success": false, "message": "Project not found or access denied" })
        );
    }

    #[test]
    fn list_chapters_by_project_path_project_not_found_keeps_detail_shape() {
        let response = map_list_chapters_by_project_path_payload_error(CrudPayloadError::NotFound(
            ProjectCrudNotFound::ProjectNotFound,
        ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "Project not found" }));
    }

    #[test]
    fn get_chapter_not_found_keeps_success_message_shape() {
        let response = map_chapter_crud_success_message_error(
            crate::services::chapter_crud_workflow_service::GetChapterPayloadError::NotFound(
                ChapterCrudNotFound::ChapterNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "success": false, "message": "Chapter not found or access denied" })
        );
    }
}
