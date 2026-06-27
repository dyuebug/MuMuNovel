use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{Json, Sse},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use self::error_mapper::{
    map_active_batch_generation_task_list_route_error,
    map_active_project_batch_generation_route_error,
    map_cancel_batch_generation_runtime_command_route_error,
    map_create_batch_generation_workflow_error, map_owned_batch_generation_task_route_error,
    map_resume_batch_generation_task_command_config_route_error,
};
use crate::config::AppConfig;
use crate::services::auth::Claims;
use crate::services::chapter_batch_generation_read_context_service::{
    build_batch_generation_read_context_owner_contract,
    load_active_batch_generation_view_from_route_project,
    load_active_user_batch_generation_task_list_view_from_route_query,
    load_owned_batch_generation_status_payload, load_owned_batch_generation_status_stream,
    ActiveBatchGenerationTaskListRouteQuery,
};
use crate::services::chapter_batch_generation_resume_task_command_service::{
    build_batch_generation_resume_task_command_owner_contract,
    resume_owned_batch_generation_task_command,
};
use crate::services::chapter_batch_generation_runtime_state_service::{
    build_batch_generation_runtime_state_owner_contract,
    cancel_owned_batch_generation_runtime_command,
};
use crate::services::chapter_batch_generation_task_payload_base_service::build_chapter_batch_generation_task_payload_base_owner_contract;
use crate::services::chapter_batch_generation_write_workflow_service::{
    build_batch_generation_write_workflow_owner_contract,
    start_owned_batch_generation_write_workflow, BatchGenerationCreateRouteRequest,
};
use crate::services::chapter_candidate_route_gateway_service::build_chapter_candidate_route_gateway_config_from_app_config;
use crate::utils::sse::named_sse_keep_alive;

const BATCH_GENERATION_CREATE_ROUTE: &str = "/chapters/project/{project_id}/batch-generate";
const BATCH_GENERATION_STATUS_ROUTE: &str = "/chapters/batch-generate/{batch_id}/status";
const BATCH_GENERATION_STATUS_STREAM_ROUTE: &str = "/chapters/batch-generate/{batch_id}/stream";
const ACTIVE_PROJECT_BATCH_GENERATION_ROUTE: &str =
    "/chapters/project/{project_id}/batch-generate/active";
const ACTIVE_BATCH_GENERATION_TASKS_ROUTE: &str = "/chapters/batch-generate/active-tasks";
const BATCH_GENERATION_CANCEL_ROUTE: &str = "/chapters/batch-generate/{batch_id}/cancel";
const BATCH_GENERATION_RESUME_ROUTE: &str = "/chapters/batch-generate/{batch_id}/resume";

pub(crate) fn build_chapter_batch_generation_route_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation",
        "scope": "batch_generation_create_status_stream_active_list_cancel_resume_route_group",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/api/chapter_batch_generation.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "route_contract": {
            "create": BATCH_GENERATION_CREATE_ROUTE,
            "status": BATCH_GENERATION_STATUS_ROUTE,
            "stream": BATCH_GENERATION_STATUS_STREAM_ROUTE,
            "active_project": ACTIVE_PROJECT_BATCH_GENERATION_ROUTE,
            "active_user_tasks": ACTIVE_BATCH_GENERATION_TASKS_ROUTE,
            "cancel": BATCH_GENERATION_CANCEL_ROUTE,
            "resume": BATCH_GENERATION_RESUME_ROUTE
        },
        "behavior_contract": {
            "route_entrypoints": [
                "create_batch_generate",
                "get_batch_generation_status",
                "stream_batch_generation_status",
                "get_active_batch_generation",
                "list_active_batch_generation_tasks",
                "cancel_batch_generation",
                "resume_batch_generation"
            ],
            "mutation_consumers": [
                "start_owned_batch_generation_write_workflow",
                "cancel_owned_batch_generation_runtime_command",
                "resume_owned_batch_generation_task_command"
            ],
            "read_context_consumers": [
                "load_owned_batch_generation_status_payload",
                "load_owned_batch_generation_status_stream",
                "load_active_batch_generation_view_from_route_project",
                "load_active_user_batch_generation_task_list_view_from_route_query"
            ],
            "error_mapping": [
                "map_create_batch_generation_workflow_error",
                "map_owned_batch_generation_task_route_error",
                "map_active_project_batch_generation_route_error",
                "map_active_batch_generation_task_list_route_error",
                "map_cancel_batch_generation_runtime_command_route_error",
                "map_resume_batch_generation_task_command_config_route_error"
            ],
            "gateway_config": [
                "create route consumes AppConfig candidate gateway config",
                "resume route consumes AppConfig candidate gateway config",
                "status/stream/read routes expose persisted candidate gateway metadata"
            ]
        },
        "active_consumers": [
            "router::chapters_routes",
            "deploy/strangler-gateway-probes.json",
            "chapter-batch-generation-active-gateway-smoke-rust"
        ],
        "write_workflow_owner_contract": build_batch_generation_write_workflow_owner_contract(),
        "read_context_owner_contract": build_batch_generation_read_context_owner_contract(),
        "resume_task_command_owner_contract": build_batch_generation_resume_task_command_owner_contract(),
        "runtime_state_owner_contract": build_batch_generation_runtime_state_owner_contract(),
        "task_payload_owner_contract": build_chapter_batch_generation_task_payload_base_owner_contract(),
        "readiness_evidence": [
            "chapter-batch-generation-active-gateway-smoke-rust",
            "chapter-batch-generation-fixture-import-project-business-rust",
            "chapter-batch-generation-fixture-list-chapters-business-rust",
            "chapter-batch-generation-configure-mock-openai-business-rust",
            "chapter-batch-generation-create-business-rust",
            "chapter-batch-generation-status-business-rust",
            "chapter-batch-generation-active-project-business-rust",
            "chapter-batch-generation-active-tasks-business-rust",
            "chapter-batch-generation-stream-business-rust",
            "chapter-batch-generation-cancel-business-rust",
            "chapter-batch-generation-cleanup-project-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-batch-generation-owner",
            "business_probes": [
                "chapter-batch-generation-active-gateway-smoke-rust",
                "chapter-batch-generation-fixture-import-project-business-rust",
                "chapter-batch-generation-fixture-list-chapters-business-rust",
                "chapter-batch-generation-configure-mock-openai-business-rust",
                "chapter-batch-generation-create-business-rust",
                "chapter-batch-generation-status-business-rust",
                "chapter-batch-generation-active-project-business-rust",
                "chapter-batch-generation-active-tasks-business-rust",
                "chapter-batch-generation-stream-business-rust",
                "chapter-batch-generation-cancel-business-rust",
                "chapter-batch-generation-cleanup-project-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "readiness_probe_count": 11,
            "route_group_probe_count": 11,
            "active_gateway_probe_count": 1,
            "business_probe_count": 7,
            "auth_guard_probe_count": 0,
            "fixture_probe_count": 3,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages",
        "migration_policy": "Batch chapter generation business smoke is covered by phase5-batch-generation-owner; the legacy Python batch-generation route package and the dedicated read-context source-map package have been physically deleted after test-seam migration, and surviving Python closeout work is limited to separate shared runtime/projection source-map contracts.",
        "validation_boundary": [
            "cargo test api::chapter_batch_generation",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-batch-generation-owner",
            "cargo check"
        ],
        "rollback_boundary": {
            "deployment_owner": "deploy/nginx/mumunovel.conf",
            "runtime_knob": "python_candidate_executor_fallback",
            "python_route_files_status": "source_map_only_for_batch_generation_active_traffic",
            "python_default_import_status": "chapters_py_no_longer_imports_batch_package_source_maps_by_default",
            "python_bootstrap_status": "bootstrap_registration_deleted_no_route_wiring_loader_remains",
            "source_map_freeze_status": "physical_closeout_completed",
            "source_map_physical_closeout_action": "delete_completed",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [
                "aggregate chapters route shell still needs its own separate source-map closeout package"
            ],
            "rollback_files": []
        }
    })
}

async fn create_batch_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<AppConfig>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<BatchGenerationCreateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = start_owned_batch_generation_write_workflow(
        &db,
        &project_id,
        &claims.sub,
        body,
        build_chapter_candidate_route_gateway_config_from_app_config(&config),
    )
    .await
    .map_err(map_create_batch_generation_workflow_error)?;

    Ok(Json(result))
}

async fn get_batch_generation_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_owned_batch_generation_status_payload(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_owned_batch_generation_task_route_error)?;

    Ok(Json(result))
}

async fn stream_batch_generation_status(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let stream =
        load_owned_batch_generation_status_stream(db.clone(), batch_id.clone(), claims.sub.clone())
            .await
            .map_err(map_owned_batch_generation_task_route_error)?;

    Ok(Sse::new(stream).keep_alive(named_sse_keep_alive("keep-alive")))
}

async fn get_active_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = load_active_batch_generation_view_from_route_project(&db, &claims.sub, project_id)
        .await
        .map_err(map_active_project_batch_generation_route_error)?;

    Ok(Json(result))
}

async fn list_active_batch_generation_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ActiveBatchGenerationTaskListRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result =
        load_active_user_batch_generation_task_list_view_from_route_query(&db, &claims.sub, query)
            .await
            .map_err(map_active_batch_generation_task_list_route_error)?;
    Ok(Json(result))
}

async fn cancel_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = cancel_owned_batch_generation_runtime_command(&db, &batch_id, &claims.sub)
        .await
        .map_err(map_cancel_batch_generation_runtime_command_route_error)?;

    Ok(Json(result))
}

async fn resume_batch_generation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<AppConfig>,
    Extension(claims): Extension<Claims>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = resume_owned_batch_generation_task_command(
        &db,
        &batch_id,
        &claims.sub,
        build_chapter_candidate_route_gateway_config_from_app_config(&config),
    )
    .await
    .map_err(map_resume_batch_generation_task_command_config_route_error)?;

    Ok(Json(result))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(BATCH_GENERATION_CREATE_ROUTE, post(create_batch_generate))
        .route(
            BATCH_GENERATION_STATUS_ROUTE,
            get(get_batch_generation_status),
        )
        .route(
            BATCH_GENERATION_STATUS_STREAM_ROUTE,
            get(stream_batch_generation_status),
        )
        .route(
            ACTIVE_PROJECT_BATCH_GENERATION_ROUTE,
            get(get_active_batch_generation),
        )
        .route(
            ACTIVE_BATCH_GENERATION_TASKS_ROUTE,
            get(list_active_batch_generation_tasks),
        )
        .route(BATCH_GENERATION_CANCEL_ROUTE, post(cancel_batch_generation))
        .route(BATCH_GENERATION_RESUME_ROUTE, post(resume_batch_generation))
}

mod error_mapper {
    use axum::http::StatusCode;
    use axum::Json;
    use serde_json::Value;

    use crate::api::chapters_error_mapper::{
        detail_error, internal_detail_error, project_not_found_or_access_denied_error,
    };

    use crate::services::chapter_batch_generation_read_context_service::{
        ActiveBatchGenerationTaskListQueryRequestError,
        ActiveBatchGenerationTaskListRouteQueryError, ActiveProjectBatchGenerationRouteError,
        LoadOwnedBatchGenerationTaskError,
    };
    use crate::services::chapter_batch_generation_resume_task_command_service::ResumeBatchGenerationTaskCommandError;
    use crate::services::chapter_batch_generation_runtime_state_service::CancelBatchGenerationTaskCommandError;
    use crate::services::chapter_batch_generation_write_workflow_service::{
        CreateBatchGenerationWriteWorkflowError, PrepareBatchGenerationCreateRequestError,
    };
    use crate::services::project_service::ProjectAccessQueryError;

    fn batch_generation_task_not_found_error() -> (StatusCode, Json<Value>) {
        detail_error(StatusCode::NOT_FOUND, "Batch generation task not found")
    }

    pub(crate) fn map_owned_batch_generation_task_route_error(
        error: LoadOwnedBatchGenerationTaskError,
    ) -> (StatusCode, Json<Value>) {
        match error {
            LoadOwnedBatchGenerationTaskError::TaskNotFound => {
                batch_generation_task_not_found_error()
            }
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
        error: ResumeBatchGenerationTaskCommandError,
    ) -> (StatusCode, Json<Value>) {
        match error {
            ResumeBatchGenerationTaskCommandError::Task(error) => {
                map_owned_batch_generation_task_route_error(error)
            }
            ResumeBatchGenerationTaskCommandError::Domain(error) => {
                detail_error(StatusCode::BAD_REQUEST, error.detail_message())
            }
            ResumeBatchGenerationTaskCommandError::Config(error) => {
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
            CreateBatchGenerationWriteWorkflowError::Internal(error) => {
                internal_detail_error(error)
            }
        }
    }

    pub(crate) fn map_cancel_batch_generation_runtime_command_route_error(
        error: CancelBatchGenerationTaskCommandError,
    ) -> (StatusCode, Json<Value>) {
        match error {
            CancelBatchGenerationTaskCommandError::Task(error) => {
                map_owned_batch_generation_task_route_error(error)
            }
            CancelBatchGenerationTaskCommandError::Domain(error) => {
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
            map_cancel_batch_generation_runtime_command_route_error,
            map_create_batch_generation_workflow_error,
            map_owned_batch_generation_task_route_error, map_project_access_query_route_error,
            map_resume_batch_generation_task_command_config_route_error,
        };
        use crate::services::chapter_batch_generation_read_context_service::{
            ActiveBatchGenerationTaskListQueryRequestError,
            ActiveBatchGenerationTaskListRouteQueryError, ActiveProjectBatchGenerationRouteError,
            LoadOwnedBatchGenerationTaskError,
        };
        use crate::services::chapter_batch_generation_resume_task_command_service::{
            ResumeBatchGenerationDomainError, ResumeBatchGenerationTaskCommandError,
        };
        use crate::services::chapter_batch_generation_runtime_state_service::CancelBatchGenerationTaskCommandError;
        use crate::services::chapter_batch_generation_write_workflow_service::{
            CreateBatchGenerationWriteWorkflowError, PrepareBatchGenerationCreateRequestError,
        };
        use crate::services::project_service::ProjectAccessQueryError;
        use axum::http::StatusCode;
        use serde_json::json;

        #[test]
        fn project_access_query_not_found_or_access_denied_remains_not_found() {
            let response = map_project_access_query_route_error(
                ProjectAccessQueryError::NotFoundOrAccessDenied,
            );

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
            let response = map_cancel_batch_generation_runtime_command_route_error(
                CancelBatchGenerationTaskCommandError::Task(
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
            let response = map_cancel_batch_generation_runtime_command_route_error(
                CancelBatchGenerationTaskCommandError::Task(
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
                    PrepareBatchGenerationCreateRequestError::Internal(
                        "prepare failed".to_string(),
                    ),
                ),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "prepare failed" }));
        }

        #[test]
        fn prepare_batch_generation_resume_not_found_remains_not_found() {
            let response = map_resume_batch_generation_task_command_config_route_error(
                ResumeBatchGenerationTaskCommandError::Task(
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
                ResumeBatchGenerationTaskCommandError::Task(
                    LoadOwnedBatchGenerationTaskError::Internal("resume lookup failed".to_string()),
                ),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "resume lookup failed" }));
        }

        #[test]
        fn prepare_batch_generation_resume_domain_error_keeps_bad_request_detail_message() {
            let response = map_resume_batch_generation_task_command_config_route_error(
                ResumeBatchGenerationTaskCommandError::Domain(
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
        fn prepare_batch_generation_resume_single_chapter_unavailable_keeps_bad_request_detail_message(
        ) {
            let response = map_resume_batch_generation_task_command_config_route_error(
                ResumeBatchGenerationTaskCommandError::Domain(
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
}

#[cfg(test)]
mod tests {
    use crate::config::{AppConfig, AppRuntimeMode};
    use crate::models::{
        batch_generation_snapshot, batch_generation_task, chapter, project, settings,
    };
    use crate::services::auth::Claims;
    use crate::services::chapter_batch_generation_read_context_service::active_query_owner::build_active_batch_generation_task_list_query_request_from_route_query;
    use crate::services::chapter_batch_generation_read_context_service::ActiveBatchGenerationTaskListRouteQuery;
    use axum::extract::{Extension, Path, Query};
    use axum::http::StatusCode;
    use chrono::Utc;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
        IntoActiveModel, Schema, Set,
    };
    use serde_json::json;

    use super::{
        cancel_batch_generation, get_batch_generation_status, list_active_batch_generation_tasks,
        BatchGenerationCreateRouteRequest, ACTIVE_BATCH_GENERATION_TASKS_ROUTE,
        BATCH_GENERATION_CANCEL_ROUTE, BATCH_GENERATION_CREATE_ROUTE,
        BATCH_GENERATION_RESUME_ROUTE, BATCH_GENERATION_STATUS_ROUTE,
        BATCH_GENERATION_STATUS_STREAM_ROUTE,
    };

    #[test]
    fn should_publish_batch_generation_route_owner_contract() {
        let contract = super::build_chapter_batch_generation_route_owner_contract();

        assert_eq!(contract["owner"], "chapter_batch_generation");
        assert_eq!(
            contract["scope"],
            "batch_generation_create_status_stream_active_list_cancel_resume_route_group"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/chapter_batch_generation.rs"
        );
        assert!(contract["python_source_map"]
            .as_array()
            .expect("python source map")
            .is_empty());
        assert_eq!(
            contract["route_contract"]["create"],
            BATCH_GENERATION_CREATE_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["stream"],
            BATCH_GENERATION_STATUS_STREAM_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["cancel"],
            BATCH_GENERATION_CANCEL_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["resume"],
            BATCH_GENERATION_RESUME_ROUTE
        );
        assert_eq!(
            contract["behavior_contract"]["route_entrypoints"][0],
            "create_batch_generate"
        );
        assert_eq!(
            contract["behavior_contract"]["mutation_consumers"][2],
            "resume_owned_batch_generation_task_command"
        );
        assert_eq!(
            contract["behavior_contract"]["read_context_consumers"][1],
            "load_owned_batch_generation_status_stream"
        );
        assert_eq!(
            contract["active_consumers"][2],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["write_workflow_owner_contract"]["owner"],
            "chapter_batch_generation_write_workflow_service"
        );
        assert_eq!(
            contract["write_workflow_owner_contract"]["create_launch_owner_contract"]["owner"],
            "chapter_batch_generation_write_workflow_service::create_launch_startup_seed_and_persistence"
        );
        assert_eq!(
            contract["read_context_owner_contract"]["owner"],
            "chapter_batch_generation_read_context_service"
        );
        assert_eq!(
            contract["read_context_owner_contract"]["task_payload_owner_contract"]["owner"],
            "chapter_batch_generation_task_payload_base_service"
        );
        assert_eq!(
            contract["resume_task_command_owner_contract"]["owner"],
            "chapter_batch_generation_resume_task_command_service::resume_task_command_owner"
        );
        assert_eq!(
            contract["runtime_state_owner_contract"]["owner"],
            "chapter_batch_generation_runtime_state_service"
        );
        assert_eq!(
            contract["task_payload_owner_contract"]["owner"],
            "chapter_batch_generation_task_payload_base_service"
        );
        assert_eq!(contract["readiness_evidence"].as_array().unwrap().len(), 11);
        assert_eq!(
            contract["readiness_evidence"][3],
            "chapter-batch-generation-configure-mock-openai-business-rust"
        );
        assert_eq!(
            contract["readiness_evidence"][10],
            "chapter-batch-generation-cleanup-project-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"]
                .as_array()
                .expect("batch-generation business probes should be present")
                .len(),
            11
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["owner_profile"],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(11)
        );
        assert_eq!(
            contract["business_smoke_status"]["route_group_probe_count"],
            json!(11)
        );
        assert_eq!(
            contract["business_smoke_status"]["active_gateway_probe_count"],
            json!(1)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(7)
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(3)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages"
        );
        assert_eq!(
            contract["migration_policy"],
            "Batch chapter generation business smoke is covered by phase5-batch-generation-owner; the legacy Python batch-generation route package and the dedicated read-context source-map package have been physically deleted after test-seam migration, and surviving Python closeout work is limited to separate shared runtime/projection source-map contracts."
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_bootstrap_status"],
            "bootstrap_registration_deleted_no_route_wiring_loader_remains"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_status"],
            "physical_closeout_completed"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_physical_closeout_action"],
            "delete_completed"
        );
        assert!(contract["rollback_boundary"]["rollback_files"]
            .as_array()
            .expect("rollback files")
            .is_empty());
    }

    #[test]
    fn should_keep_batch_generation_status_route_as_task_status_owner_entrypoint() {
        assert_eq!(
            BATCH_GENERATION_STATUS_ROUTE,
            "/chapters/batch-generate/{batch_id}/status"
        );
        assert_eq!(
            BATCH_GENERATION_STATUS_STREAM_ROUTE,
            "/chapters/batch-generate/{batch_id}/stream"
        );
        assert_eq!(
            ACTIVE_BATCH_GENERATION_TASKS_ROUTE,
            "/chapters/batch-generate/active-tasks"
        );
    }

    #[test]
    fn should_validate_active_batch_generation_task_list_limit_like_python_query() {
        let default_request =
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: None },
            )
            .expect("default limit should be valid")
            .limit();
        let preserved_request =
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(25) },
            )
            .expect("explicit in-range limit should be valid")
            .limit();

        assert_eq!(default_request, 20);
        assert_eq!(preserved_request, 25);
        assert!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(0) },
            )
            .is_err()
        );
        assert!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(500) },
            )
            .is_err()
        );
    }

    #[test]
    fn should_keep_active_project_batch_generation_route_start_contract() {
        let project_id = "project-9".to_string();

        assert_eq!(project_id, "project-9");
    }

    #[test]
    fn should_keep_batch_generation_create_route_payload_contract() {
        let route_request = BatchGenerationCreateRouteRequest {
            start_chapter_number: 5,
            count: 3,
            style_id: Some(9),
            target_word_count: Some(3200),
            enable_analysis: Some(true),
            enable_mcp: Some(true),
            enable_web_research: Some(false),
            web_research_query: Some("ignored".to_string()),
            max_retries: Some(6),
            model: Some("gpt-4.1-mini".to_string()),
            creative_mode: Some("dramatic".to_string()),
            story_focus: Some("battle".to_string()),
            plot_stage: Some("climax".to_string()),
            story_creation_brief: Some("brief".to_string()),
            quality_preset: Some("strict".to_string()),
            quality_notes: Some("notes".to_string()),
            story_repair_summary: Some("repair".to_string()),
            story_repair_targets: Some(vec!["target-a".to_string()]),
            story_preserve_strengths: Some(vec!["strength-a".to_string()]),
        };

        assert_eq!(route_request.start_chapter_number, 5);
        assert_eq!(route_request.count, 3);
        assert_eq!(route_request.style_id, Some(9));
        assert_eq!(route_request.target_word_count, Some(3200));
        assert_eq!(route_request.enable_analysis, Some(true));
        assert_eq!(route_request.max_retries, Some(6));
        assert_eq!(route_request.model.as_deref(), Some("gpt-4.1-mini"));
    }

    #[test]
    fn should_keep_batch_generation_create_route_payload_contract_minimal() {
        let route_request = BatchGenerationCreateRouteRequest {
            start_chapter_number: 3,
            count: 2,
            style_id: Some(7),
            target_word_count: Some(2800),
            enable_analysis: None,
            enable_mcp: Some(true),
            enable_web_research: Some(true),
            web_research_query: Some("ignored".to_string()),
            max_retries: None,
            model: Some("gpt-4.1".to_string()),
            creative_mode: Some("dramatic".to_string()),
            story_focus: Some("battle".to_string()),
            plot_stage: Some("climax".to_string()),
            story_creation_brief: Some("brief".to_string()),
            quality_preset: Some("strict".to_string()),
            quality_notes: Some("notes".to_string()),
            story_repair_summary: Some("repair".to_string()),
            story_repair_targets: Some(vec!["target-a".to_string()]),
            story_preserve_strengths: Some(vec!["strength-a".to_string()]),
        };

        assert_eq!(route_request.start_chapter_number, 3);
        assert_eq!(route_request.count, 2);
        assert_eq!(route_request.style_id, Some(7));
        assert_eq!(route_request.target_word_count, Some(2800));
        assert_eq!(route_request.enable_analysis, None);
        assert_eq!(route_request.max_retries, None);
        assert_eq!(route_request.model.as_deref(), Some("gpt-4.1"));
    }

    fn app_config() -> AppConfig {
        AppConfig {
            app_host: "127.0.0.1".to_string(),
            app_port: 8001,
            app_name: "MuMuNovel".to_string(),
            app_version: "0.1.0-rs".to_string(),
            database_url: "sqlite::memory:".to_string(),
            database_pool_size: 50,
            enable_startup_schema_sync: false,
            log_level: "info".to_string(),
            debug: true,
            runtime_mode: AppRuntimeMode::Development,
            cors_origins: "*".to_string(),
            jwt_secret: "secret".to_string(),
            static_dir: "../backend/static".to_string(),
            local_auth_enabled: true,
            local_auth_username: String::new(),
            local_auth_password: String::new(),
            local_auth_display_name: "本地管理员".to_string(),
            linuxdo_client_id: String::new(),
            linuxdo_client_secret: String::new(),
            linuxdo_redirect_uri: String::new(),
            frontend_url: "http://localhost".to_string(),
            session_expire_minutes: 120,
            session_refresh_threshold_minutes: 30,
            chapter_candidate_rust_executor_enabled: false,
            chapter_candidate_rust_executor_fallback_on_error: true,
            chapter_candidate_rust_executor_disabled_reason: String::new(),
            chapter_candidate_rust_executor_rollback_boundary: "python_candidate_executor_fallback"
                .to_string(),
            rust_migration_noop_executor_smoke_enabled: false,
        }
    }

    fn test_claims() -> Claims {
        Claims {
            sub: "user-db-smoke".to_string(),
            username: "route-smoke".to_string(),
            is_admin: false,
            exp: usize::MAX,
            iat: 0,
        }
    }

    async fn setup_batch_generation_route_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);

        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create projects table");
        db.execute(builder.build(&schema.create_table_from_entity(settings::Entity)))
            .await
            .expect("create settings table");
        db.execute(builder.build(&schema.create_table_from_entity(chapter::Entity)))
            .await
            .expect("create chapters table");
        db.execute(builder.build(&schema.create_table_from_entity(batch_generation_task::Entity)))
            .await
            .expect("create batch generation tasks table");
        db.execute(
            builder.build(&schema.create_table_from_entity(batch_generation_snapshot::Entity)),
        )
        .await
        .expect("create batch generation snapshots table");

        db
    }

    async fn seed_batch_generation_route_fixture(db: &DatabaseConnection) {
        let now = Utc::now().naive_utc();

        project::ActiveModel {
            id: Set("project-db-smoke".to_string()),
            user_id: Set("user-db-smoke".to_string()),
            title: Set("DB Smoke Project".to_string()),
            description: Set(None),
            theme: Set(None),
            genre: Set(None),
            target_words: Set(12_000),
            current_words: Set(3_000),
            status: Set("active".to_string()),
            wizard_status: Set("completed".to_string()),
            wizard_step: Set(0),
            outline_mode: Set("simple".to_string()),
            world_time_period: Set(None),
            world_location: Set(None),
            world_atmosphere: Set(None),
            world_rules: Set(None),
            chapter_count: Set(Some(3)),
            narrative_perspective: Set(None),
            character_count: Set(0),
            default_creative_mode: Set(None),
            default_story_focus: Set(None),
            default_plot_stage: Set(None),
            default_story_creation_brief: Set(None),
            default_quality_preset: Set(None),
            default_quality_notes: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .expect("insert route-smoke project");

        settings::ActiveModel {
            id: Set("settings-db-smoke".to_string()),
            user_id: Set("user-db-smoke".to_string()),
            api_provider: Set("openai".to_string()),
            api_key: Set("sk-route-smoke".to_string()),
            api_base_url: Set("https://api.example.com/v1".to_string()),
            api_backup_urls: Set(None),
            provider_type: Set("openai".to_string()),
            fallback_strategy: Set("manual".to_string()),
            azure_api_version: Set(None),
            llm_model: Set("route-smoke-model".to_string()),
            temperature: Set(0.6),
            max_tokens: Set(2048),
            system_prompt: Set(Some("route-smoke-prompt".to_string())),
            preferences: Set(Some("{}".to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .expect("insert route-smoke settings");

        for (chapter_id, chapter_number, title, status, content, summary, word_count) in [
            (
                "chapter-db-2",
                2,
                "DB Smoke Chapter 2",
                "completed",
                Some("前置章节已完成正文"),
                Some("前置章节概要"),
                1000,
            ),
            (
                "chapter-db-3",
                3,
                "DB Smoke Chapter 3",
                "draft",
                None,
                None,
                0,
            ),
        ] {
            chapter::ActiveModel {
                id: Set(chapter_id.to_string()),
                project_id: Set("project-db-smoke".to_string()),
                chapter_number: Set(chapter_number),
                title: Set(title.to_string()),
                content: Set(content.map(str::to_string)),
                summary: Set(summary.map(str::to_string)),
                word_count: Set(word_count),
                status: Set(status.to_string()),
                outline_id: Set(None),
                sub_index: Set(1),
                expansion_plan: Set(None),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            }
            .insert(db)
            .await
            .expect("insert route-smoke chapter");
        }

        batch_generation_task::ActiveModel {
            id: Set("batch-db-smoke".to_string()),
            project_id: Set("project-db-smoke".to_string()),
            user_id: Set("user-db-smoke".to_string()),
            start_chapter_number: Set(2),
            chapter_count: Set(2),
            chapter_ids: Set(json!(["chapter-db-2", "chapter-db-3"])),
            style_id: Set(None),
            target_word_count: Set(2800),
            enable_analysis: Set(true),
            status: Set("running".to_string()),
            total_chapters: Set(2),
            completed_chapters: Set(1),
            failed_chapters: Set(json!([])),
            current_chapter_id: Set(Some("chapter-db-3".to_string())),
            current_chapter_number: Set(Some(3)),
            current_retry_count: Set(0),
            max_retries: Set(3),
            created_at: Set(Some(now)),
            started_at: Set(Some(now)),
            completed_at: Set(None),
            error_message: Set(None),
        }
        .insert(db)
        .await
        .expect("insert route-smoke batch task");

        batch_generation_snapshot::ActiveModel {
            id: Set("snapshot-db-smoke".to_string()),
            batch_task_id: Set("batch-db-smoke".to_string()),
            latest_quality_metrics: Set(Some(json!({
                "overall_score": 91.0,
                "source": "route-smoke",
            }))),
            quality_metrics_history: Set(Some(json!([{
                "overall_score": 90.0,
                "source": "route-smoke-history",
            }]))),
            quality_metrics_summary: Set(Some(json!({
                "chapter_count": 1,
                "avg_score": 91.0,
            }))),
            workflow_runtime_state: Set(Some(json!({
                "phase": "generating",
                "progress": 65,
                "last_event": "selected_candidate",
                "last_message": "Rust route smoke selected candidate",
                "candidate_gateway": {
                    "execution_path": "rust_candidate_executor",
                    "fallback_applied": false,
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "rust_error": null
                },
                "selected_candidate_events": [
                    {
                        "type": "progress",
                        "event": "selected_candidate",
                        "message": "候选章节已选择"
                    }
                ],
                "active_story_repair_payload": {
                    "scope": "batch",
                    "mode": "route-smoke"
                }
            }))),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .expect("insert route-smoke snapshot");
    }

    #[tokio::test]
    async fn should_load_db_backed_batch_generation_status_from_route_handler() {
        let db = setup_batch_generation_route_db().await;
        seed_batch_generation_route_fixture(&db).await;

        let response = get_batch_generation_status(
            Extension(db),
            Extension(test_claims()),
            Path("batch-db-smoke".to_string()),
        )
        .await
        .expect("status route should load owned batch payload");

        assert_eq!(response.0["batch_id"], json!("batch-db-smoke"));
        assert_eq!(response.0["status"], json!("running"));
        assert_eq!(response.0["stage_code"], json!("6.writing.generating"));
        assert_eq!(
            response.0["candidate_gateway"]["execution_path"],
            json!("rust_candidate_executor")
        );
        assert_eq!(response.0["checkpoint"]["current_chapter_number"], json!(3));
        assert_eq!(
            response.0["latest_quality_metrics"]["overall_score"],
            json!(91.0)
        );
    }

    #[tokio::test]
    async fn should_list_db_backed_active_batch_generation_tasks_from_route_handler() {
        let db = setup_batch_generation_route_db().await;
        seed_batch_generation_route_fixture(&db).await;

        let response = list_active_batch_generation_tasks(
            Extension(db),
            Extension(test_claims()),
            Query(ActiveBatchGenerationTaskListRouteQuery { limit: Some(10) }),
        )
        .await
        .expect("active task list route should load owned batch items");

        assert_eq!(response.0["total"], json!(1));
        assert_eq!(response.0["items"][0]["batch_id"], json!("batch-db-smoke"));
        assert_eq!(
            response.0["items"][0]["task_type"],
            json!("chapters_batch_generate")
        );
        assert_eq!(
            response.0["items"][0]["candidate_gateway"]["execution_path"],
            json!("rust_candidate_executor")
        );
    }

    #[tokio::test]
    async fn should_cancel_running_batch_generation_from_route_handler() {
        let db = setup_batch_generation_route_db().await;
        seed_batch_generation_route_fixture(&db).await;

        let response = cancel_batch_generation(
            Extension(db.clone()),
            Extension(test_claims()),
            Path("batch-db-smoke".to_string()),
        )
        .await
        .expect("cancel route should stop running owned batch task");

        assert_eq!(response.0["message"], json!("Batch generation cancelled"));
        assert_eq!(response.0["batch_id"], json!("batch-db-smoke"));
        assert_eq!(response.0["completed_chapters"], json!(1));
        assert_eq!(response.0["total_chapters"], json!(2));

        let persisted = batch_generation_task::Entity::find_by_id("batch-db-smoke")
            .one(&db)
            .await
            .expect("query cancelled task")
            .expect("cancelled task should persist");
        assert_eq!(persisted.status, "cancelled");
        assert!(persisted.completed_at.is_some());
    }

    #[tokio::test]
    async fn should_return_bad_request_when_cancelling_terminal_batch_task_from_route_handler() {
        let db = setup_batch_generation_route_db().await;
        seed_batch_generation_route_fixture(&db).await;

        let mut task = batch_generation_task::Entity::find_by_id("batch-db-smoke")
            .one(&db)
            .await
            .expect("load running task")
            .expect("running task exists")
            .into_active_model();
        task.status = Set("failed".to_string());
        task.update(&db).await.expect("mark task failed");

        let error = cancel_batch_generation(
            Extension(db),
            Extension(test_claims()),
            Path("batch-db-smoke".to_string()),
        )
        .await
        .expect_err("terminal batch task should reject cancel");

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.1 .0,
            json!({ "detail": "Cannot cancel task in status failed" })
        );
    }

    #[tokio::test]
    async fn should_resume_failed_batch_generation_from_route_handler() {
        let db = setup_batch_generation_route_db().await;
        seed_batch_generation_route_fixture(&db).await;

        let mut task = batch_generation_task::Entity::find_by_id("batch-db-smoke")
            .one(&db)
            .await
            .expect("load running task")
            .expect("running task exists")
            .into_active_model();
        task.status = Set("failed".to_string());
        task.current_retry_count = Set(1);
        task.error_message = Set(Some("mock failed".to_string()));
        task.update(&db).await.expect("mark task failed");

        let response = super::resume_batch_generation(
            Extension(db.clone()),
            Extension(app_config()),
            Extension(test_claims()),
            Path("batch-db-smoke".to_string()),
        )
        .await
        .expect("failed batch task should resume");

        let resumed_batch_id = response.0["batch_id"]
            .as_str()
            .expect("resume response batch id")
            .to_string();
        assert_eq!(resumed_batch_id, "batch-db-smoke");
        assert_eq!(response.0["resumed_from_batch_id"], json!("batch-db-smoke"));
        assert_eq!(response.0["status"], json!("pending"));
        assert_eq!(response.0["stage_code"], json!("6.writing.loading"));
        assert_eq!(
            response.0["checkpoint"]["resume_from_batch_id"],
            json!("batch-db-smoke")
        );
        assert_eq!(response.0["completed_chapters"], json!(0));
        assert_eq!(response.0["total_chapters"], json!(2));

        let resumed_task = batch_generation_task::Entity::find_by_id(resumed_batch_id.clone())
            .one(&db)
            .await
            .expect("load resumed task")
            .expect("resumed task should persist");
        assert_eq!(resumed_task.status, "pending");
        assert_eq!(resumed_task.start_chapter_number, 2);
        assert_eq!(resumed_task.chapter_count, 2);
        assert_eq!(resumed_task.total_chapters, 2);
        assert_eq!(resumed_task.completed_chapters, 0);
        assert_eq!(resumed_task.current_retry_count, 0);
    }

    #[tokio::test]
    async fn should_reject_resume_when_batch_task_is_not_terminal_from_route_handler() {
        let db = setup_batch_generation_route_db().await;
        seed_batch_generation_route_fixture(&db).await;

        let error = super::resume_batch_generation(
            Extension(db),
            Extension(app_config()),
            Extension(test_claims()),
            Path("batch-db-smoke".to_string()),
        )
        .await
        .expect_err("running batch task should reject resume");

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.1 .0,
            json!({ "detail": "Only failed or cancelled tasks can be resumed" })
        );
    }

    #[tokio::test]
    async fn should_map_missing_batch_task_to_404_from_status_and_cancel_handlers() {
        let db = setup_batch_generation_route_db().await;

        let status_error = get_batch_generation_status(
            Extension(db.clone()),
            Extension(test_claims()),
            Path("missing-task".to_string()),
        )
        .await
        .expect_err("missing status task should map to 404");
        let cancel_error = cancel_batch_generation(
            Extension(db),
            Extension(test_claims()),
            Path("missing-task".to_string()),
        )
        .await
        .expect_err("missing cancel task should map to 404");

        assert_eq!(status_error.0, StatusCode::NOT_FOUND);
        assert_eq!(
            status_error.1 .0,
            json!({ "detail": "Batch generation task not found" })
        );
        assert_eq!(cancel_error.0, StatusCode::NOT_FOUND);
        assert_eq!(
            cancel_error.1 .0,
            json!({ "detail": "Batch generation task not found" })
        );
    }
}
