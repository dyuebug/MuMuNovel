use axum::{
    extract::{Extension, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::project_consistency_query_service::LoadProjectConsistencyContextError;
use crate::services::project_consistency_write_workflow_service::{
    check_project_consistency_write_workflow, fix_project_member_counts_write_workflow,
    fix_project_organizations_write_workflow, normalize_project_consistency_auto_fix,
    ProjectConsistencyWriteWorkflowError,
};
use crate::services::project_export_payload_adapter_service::{
    build_project_export_data_payload, build_project_export_txt_content,
    build_safe_project_export_json_filename, build_safe_project_export_txt_filename,
};
use crate::services::project_export_query_service::{
    load_project_export_context, load_project_export_context_with_non_empty_chapters,
    LoadProjectExportContextError,
};
use crate::services::project_import_workflow_service::{
    import_project_write_workflow, validate_project_import_payload,
    ImportProjectWriteWorkflowError, ValidateProjectImportPayloadError,
};
use crate::services::project_service::ProjectService;

#[derive(Deserialize)]
struct CreateRequest {
    title: String,
    description: Option<String>,
    theme: Option<String>,
    genre: Option<String>,
    outline_mode: Option<String>,
    target_words: Option<i32>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    title: Option<String>,
    description: Option<String>,
    theme: Option<String>,
    genre: Option<String>,
    status: Option<String>,
    target_words: Option<i32>,
    outline_mode: Option<String>,
    narrative_perspective: Option<String>,
    default_creative_mode: Option<String>,
    default_story_focus: Option<String>,
    default_plot_stage: Option<String>,
    default_story_creation_brief: Option<String>,
    default_quality_preset: Option<String>,
    default_quality_notes: Option<String>,
}

#[derive(Deserialize)]
struct ExportOptions {
    #[serde(default)]
    include_generation_history: bool,
    #[serde(default)]
    include_writing_styles: bool,
    #[serde(default)]
    include_careers: bool,
    #[serde(default)]
    include_memories: bool,
    #[serde(default)]
    include_plot_analysis: bool,
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    user_id: Option<String>,
}

#[derive(Deserialize)]
struct CheckProjectConsistencyQuery {
    #[serde(default)]
    auto_fix: Option<String>,
}

fn map_project_export_context_error(
    error: LoadProjectExportContextError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadProjectExportContextError::Context(error) => map_project_query_context_error(error),
        LoadProjectExportContextError::ProjectHasNoChapters => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project has no chapters"})),
        ),
    }
}

fn map_project_query_context_error(
    error: LoadProjectConsistencyContextError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadProjectConsistencyContextError::ProjectNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found"})),
        ),
        LoadProjectConsistencyContextError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

fn map_project_consistency_context_error(
    error: LoadProjectConsistencyContextError,
) -> (StatusCode, Json<Value>) {
    map_project_query_context_error(error)
}

fn map_validate_project_import_payload_error(
    error: ValidateProjectImportPayloadError,
) -> (StatusCode, Json<Value>) {
    match error {
        ValidateProjectImportPayloadError::InvalidJson(detail) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "valid": false,
                "version": null,
                "project_name": null,
                "statistics": {},
                "errors": [format!("Invalid JSON: {}", detail)],
                "warnings": [],
            })),
        ),
    }
}

fn map_import_project_write_workflow_error(
    error: ImportProjectWriteWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        ImportProjectWriteWorkflowError::PayloadTooLarge => (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"detail": "文件大小超过50MB限制"})),
        ),
        ImportProjectWriteWorkflowError::InvalidJson(detail) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("Invalid JSON: {}", detail)})),
        ),
        ImportProjectWriteWorkflowError::MissingProjectField => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "Missing project field"})),
        ),
        ImportProjectWriteWorkflowError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

fn map_project_consistency_write_workflow_error(
    error: ProjectConsistencyWriteWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        ProjectConsistencyWriteWorkflowError::Context(error) => {
            map_project_consistency_context_error(error)
        }
        ProjectConsistencyWriteWorkflowError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

async fn create_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match ProjectService::create(
        &db,
        &claims.sub,
        &body.title,
        body.description.as_deref(),
        body.theme.as_deref(),
        body.genre.as_deref(),
        body.outline_mode.as_deref(),
        body.target_words,
    )
    .await
    {
        Ok(project) => Ok((StatusCode::CREATED, Json(json!(project)))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn list_projects(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let uid = query.user_id.as_deref().unwrap_or(&claims.sub);
    match ProjectService::list(&db, uid).await {
        Ok(projects) => Ok(Json(json!(projects))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn get_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ProjectService::get(&db, &project_id, &claims.sub).await {
        Ok(Some(project)) => Ok(Json(json!(project))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ProjectService::update(
        &db,
        &project_id,
        &claims.sub,
        body.title.as_deref(),
        body.description.as_deref(),
        body.theme.as_deref(),
        body.genre.as_deref(),
        body.status.as_deref(),
        body.target_words,
        body.outline_mode.as_deref(),
        body.narrative_perspective.as_deref(),
        body.default_creative_mode.as_deref(),
        body.default_story_focus.as_deref(),
        body.default_plot_stage.as_deref(),
        body.default_story_creation_brief.as_deref(),
        body.default_quality_preset.as_deref(),
        body.default_quality_notes.as_deref(),
    )
    .await
    {
        Ok(Some(project)) => Ok(Json(json!(project))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ProjectService::delete(&db, &project_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(
            json!({"success": true, "message": "Project deleted successfully"}),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn export_project_data(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(options): Json<ExportOptions>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let context = load_project_export_context(&db, &project_id, &claims.sub)
        .await
        .map_err(map_project_export_context_error)?;
    let project = context.project;
    let chapters = context.chapters;

    let export_payload = build_project_export_data_payload(
        &project,
        &chapters,
        options.include_generation_history,
        options.include_writing_styles,
        options.include_careers,
        options.include_memories,
        options.include_plot_analysis,
    );
    let filename = build_safe_project_export_json_filename(&project.title);
    let encoded_filename = filename.clone();
    let body = serde_json::to_vec_pretty(&export_payload).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename*=UTF-8''{}", encoded_filename),
        )
        .body(axum::body::Body::from(body))
        .unwrap())
}

async fn export_project_txt(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let context =
        load_project_export_context_with_non_empty_chapters(&db, &project_id, &claims.sub)
            .await
            .map_err(map_project_export_context_error)?;
    let project = context.project;
    let chapters = context.chapters;

    let text = build_project_export_txt_content(&project, &chapters);
    let filename = build_safe_project_export_txt_filename(&project.title);
    let headers = [
        (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
        (
            header::CONTENT_DISPOSITION,
            &format!("attachment; filename=\"{}\"", filename),
        ),
    ];

    Ok((headers, text).into_response())
}

async fn validate_import(
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut file_data: Vec<u8> = Vec::new();
    let mut file_found = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": format!("Failed to read uploaded file: {}", e)})),
                )
            })?;
            file_data = bytes.to_vec();
            file_found = true;
            break;
        }
    }

    if !file_found {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "Missing file field"})),
        ));
    }

    let payload = validate_project_import_payload(&file_data)
        .map_err(map_validate_project_import_payload_error)?;

    Ok(Json(payload))
}

async fn import_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut file_data: Vec<u8> = Vec::new();
    let mut file_found = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": format!("Failed to read uploaded file: {}", error)})),
                )
            })?;
            file_data = bytes.to_vec();
            file_found = true;
            break;
        }
    }

    if !file_found {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "Missing file field"})),
        ));
    }

    let payload = import_project_write_workflow(&db, &claims.sub, &file_data)
        .await
        .map_err(map_import_project_write_workflow_error)?;

    Ok(Json(payload))
}

async fn fix_project_organizations(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = fix_project_organizations_write_workflow(&db, &project_id, &claims.sub)
        .await
        .map_err(map_project_consistency_write_workflow_error)?;

    Ok(Json(payload))
}

async fn fix_project_member_counts(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = fix_project_member_counts_write_workflow(&db, &project_id, &claims.sub)
        .await
        .map_err(map_project_consistency_write_workflow_error)?;

    Ok(Json(payload))
}

async fn check_project_consistency(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<CheckProjectConsistencyQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = check_project_consistency_write_workflow(
        &db,
        &project_id,
        &claims.sub,
        normalize_project_consistency_auto_fix(query.auto_fix.as_deref()),
    )
    .await
    .map_err(map_project_consistency_write_workflow_error)?;

    Ok(Json(payload))
}

pub fn routes() -> Router {
    Router::new()
        .route("/projects", post(create_project).get(list_projects))
        .route(
            "/projects/{project_id}",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/projects/{project_id}/export", get(export_project_txt))
        .route(
            "/projects/{project_id}/export-data",
            post(export_project_data),
        )
        .route(
            "/projects/{project_id}/check-consistency",
            post(check_project_consistency),
        )
        .route(
            "/projects/{project_id}/fix-organizations",
            post(fix_project_organizations),
        )
        .route(
            "/projects/{project_id}/fix-member-counts",
            post(fix_project_member_counts),
        )
        .route("/projects/validate-import", post(validate_import))
        .route("/projects/import", post(import_project))
}

#[cfg(test)]
mod tests {
    use super::{
        map_import_project_write_workflow_error, map_project_consistency_write_workflow_error,
        map_project_export_context_error, map_validate_project_import_payload_error,
    };
    use crate::services::project_consistency_query_service::LoadProjectConsistencyContextError;
    use crate::services::project_consistency_write_workflow_service::ProjectConsistencyWriteWorkflowError;
    use crate::services::project_export_query_service::LoadProjectExportContextError;
    use crate::services::project_import_workflow_service::{
        ImportProjectWriteWorkflowError, ValidateProjectImportPayloadError,
    };
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn fix_project_organizations_not_found_keeps_existing_transport_detail() {
        let response = map_project_consistency_write_workflow_error(
            ProjectConsistencyWriteWorkflowError::Context(
                LoadProjectConsistencyContextError::ProjectNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "Project not found" }));
    }

    #[test]
    fn fix_project_member_counts_internal_keeps_detail_passthrough() {
        let response = map_project_consistency_write_workflow_error(
            ProjectConsistencyWriteWorkflowError::Internal("member count failed".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "member count failed" }));
    }

    #[test]
    fn check_project_consistency_not_found_keeps_existing_transport_detail() {
        let response = map_project_consistency_write_workflow_error(
            ProjectConsistencyWriteWorkflowError::Context(
                LoadProjectConsistencyContextError::ProjectNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "Project not found" }));
    }

    #[test]
    fn export_project_context_not_found_keeps_existing_transport_detail() {
        let response = map_project_export_context_error(LoadProjectExportContextError::Context(
            LoadProjectConsistencyContextError::ProjectNotFound,
        ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "Project not found" }));
    }

    #[test]
    fn export_project_context_no_chapters_keeps_specific_transport_detail() {
        let response =
            map_project_export_context_error(LoadProjectExportContextError::ProjectHasNoChapters);

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Project has no chapters" })
        );
    }

    #[test]
    fn validate_project_import_invalid_json_keeps_existing_payload_shape() {
        let response = map_validate_project_import_payload_error(
            ValidateProjectImportPayloadError::InvalidJson("boom".to_string()),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({
                "valid": false,
                "version": null,
                "project_name": null,
                "statistics": {},
                "errors": ["Invalid JSON: boom"],
                "warnings": [],
            })
        );
    }

    #[test]
    fn import_project_payload_too_large_keeps_existing_chinese_detail() {
        let response = map_import_project_write_workflow_error(
            ImportProjectWriteWorkflowError::PayloadTooLarge,
        );

        assert_eq!(response.0, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(response.1 .0, json!({ "detail": "文件大小超过50MB限制" }));
    }
}
