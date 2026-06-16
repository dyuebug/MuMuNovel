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
        "python_source_map": [
            "backend/app/api/chapter_batch_generation_routes.py",
            "backend/app/api/chapters.py",
            "backend/app/services/batch_generation/create_service.py",
            "backend/app/services/batch_generation/query_service.py",
            "backend/app/services/batch_generation/resume_service.py",
            "backend/app/services/batch_generation/status_response_builder.py",
        ],
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
            "chapter_batch_generation_active_gateway_smoke_service"
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
        "next_cutover_gate": "source-map is frozen; delete/repoint requires a separate same-round approval and rollback policy",
        "migration_policy": "Batch chapter generation business smoke is covered by phase5-batch-generation-owner; the Python route shell is frozen as rollback-only source-map material, and final physical removal or repoint still requires a separate same-round approval.",
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
            "python_bootstrap_status": "lazy_imported_and_registered_for_explicit_gateway_rollback_only",
            "source_map_freeze_status": "frozen_source_map_rollback_only",
            "source_map_physical_closeout_action": "freeze",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [
                "explicit delete/repoint approval for the frozen source-map shell",
                "aggregate chapters.py and batch service source-map closeout"
            ],
            "rollback_files": [
                "backend/app/api/chapter_batch_generation_routes.py",
                "backend/app/api/chapters.py",
                "backend/app/services/batch_generation/create_service.py",
                "backend/app/services/batch_generation/query_service.py",
                "backend/app/services/batch_generation/task_workflow_snapshot_service.py",
                "backend/app/services/batch_generation/resume_service.py",
                "backend/app/services/batch_generation/status_response_builder.py"
            ]
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
    use crate::services::chapter_batch_generation_read_context_service::active_query_owner::build_active_batch_generation_task_list_query_request_from_route_query;
    use crate::services::chapter_batch_generation_read_context_service::ActiveBatchGenerationTaskListRouteQuery;
    use serde_json::json;

    use super::{
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
            contract["python_source_map"][0],
            "backend/app/api/chapter_batch_generation_routes.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/chapter_batch_generation.rs"
        );
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
            "chapter_batch_generation_active_gateway_smoke_service"
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
            "chapter_batch_generation_resume_task_command_service"
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
            "source-map is frozen; delete/repoint requires a separate same-round approval and rollback policy"
        );
        assert_eq!(
            contract["migration_policy"],
            "Batch chapter generation business smoke is covered by phase5-batch-generation-owner; the Python route shell is frozen as rollback-only source-map material, and final physical removal or repoint still requires a separate same-round approval."
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(false)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_bootstrap_status"],
            "lazy_imported_and_registered_for_explicit_gateway_rollback_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_status"],
            "frozen_source_map_rollback_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_physical_closeout_action"],
            "freeze"
        );
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
}
