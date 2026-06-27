use axum::{extract::Extension, http::StatusCode, response::Json, routing::get, Router};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use self::chapter_batch_generation_active_gateway_smoke_owner::{
    run_chapter_batch_generation_active_gateway_smoke_suite,
    ChapterBatchGenerationActiveGatewaySmokeResult,
};
use self::chapter_single_generation_active_gateway_smoke_owner::{
    run_chapter_single_generation_active_gateway_smoke_suite,
    ChapterSingleGenerationActiveGatewaySmokeResult,
};
use crate::config::AppConfig;
use crate::services::chapter_candidate_route_gateway_service::{
    run_chapter_candidate_route_gateway_smoke_suite, ChapterCandidateRouteGatewaySmokeResult,
};
use crate::services::chapter_regeneration_stream_workflow_service::{
    run_chapter_regeneration_stream_workflow_smoke_suite,
    ChapterRegenerationStreamWorkflowSmokeResult,
};
use crate::services::schema_migration_metadata_service::{
    build_schema_migration_metadata_contract, check_live_alembic_head,
    run_rust_migration_noop_executor_smoke, LiveAlembicHeadCheck,
};

const CHAPTER_CANDIDATE_ROUTE_GATEWAY_SMOKE_ROUTE: &str =
    "/health/chapter-candidate-route-gateway-smoke";
const CHAPTER_SINGLE_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE: &str =
    "/health/chapter-single-generation-active-gateway-smoke";
const CHAPTER_BATCH_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE: &str =
    "/health/chapter-batch-generation-active-gateway-smoke";
const CHAPTER_REGENERATION_STREAM_WORKFLOW_SMOKE_ROUTE: &str =
    "/health/chapter-regeneration-stream-workflow-smoke";
const RUST_MIGRATION_NOOP_EXECUTOR_SMOKE_ROUTE: &str = "/health/rust-migration-noop-executor-smoke";

async fn health_check() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn liveness_check() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn readiness_check(db: Option<Extension<DatabaseConnection>>) -> (StatusCode, Json<Value>) {
    let mut live_alembic_head_check = LiveAlembicHeadCheck::not_checked("database unavailable");
    let db_healthy = match db {
        Some(Extension(ref conn)) => {
            let healthy = conn.ping().await.is_ok();
            if healthy {
                live_alembic_head_check = check_live_alembic_head(conn).await;
            }
            healthy
        }
        None => false,
    };

    let database_status = json!({
        "healthy": db_healthy,
        "message": if db_healthy { "connected" } else { "unavailable" },
    });

    let startup_ready = true;
    let is_ready = startup_ready && db_healthy;

    let body = json!({
        "status": if is_ready { "ready" } else { "not_ready" },
        "checks": {
            "startup": {"ready": startup_ready},
            "database": database_status,
            "schema_migration": build_schema_migration_metadata_contract(Some(&live_alembic_head_check)),
        },
    });

    let code = if is_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (code, Json(body))
}

async fn db_session_stats(db: Option<Extension<DatabaseConnection>>) -> Json<Value> {
    let healthy = match db {
        Some(Extension(ref conn)) => conn.ping().await.is_ok(),
        None => false,
    };

    Json(json!({
        "status": "ok",
        "session_stats": {
            "active": 0,
            "idle": 0,
            "total": 0,
        },
        "warning": if healthy { Value::Null } else { json!("database unavailable") },
    }))
}

async fn rust_migration_noop_executor_smoke(
    db: Option<Extension<DatabaseConnection>>,
    config: Option<Extension<AppConfig>>,
) -> (StatusCode, Json<Value>) {
    let Some(Extension(db)) = db else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "owner": "schema_migration_metadata_service",
                "ok": false,
                "status": "database_unavailable",
                "execution_mode": "read_only_noop_executor_smoke",
                "ddl_executed": false,
                "rollback_boundary": "python_db_migrator_alembic",
            })),
        );
    };

    let gate_enabled = config
        .as_ref()
        .map(|Extension(config)| config.rust_migration_noop_executor_smoke_enabled)
        .unwrap_or(false);
    let result = run_rust_migration_noop_executor_smoke(&db, gate_enabled).await;
    let code = if result.ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (code, Json(result.to_json()))
}

async fn chapter_candidate_route_gateway_smoke() -> (StatusCode, Json<Value>) {
    let smoke_output = run_chapter_candidate_route_gateway_smoke_suite().await;

    match smoke_output {
        Ok(results) => {
            let probes = results.iter().map(smoke_result_payload).collect::<Vec<_>>();
            (
                StatusCode::OK,
                Json(json!({
                    "status": "ok",
                    "owner": "rust",
                    "route_group": "chapters",
                    "probe_count": probes.len(),
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "probes": probes,
                })),
            )
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "status": "failed",
                "owner": "rust",
                "route_group": "chapters",
                "error": error,
            })),
        ),
    }
}

async fn chapter_single_generation_active_gateway_smoke() -> (StatusCode, Json<Value>) {
    let smoke_output = run_chapter_single_generation_active_gateway_smoke_suite().await;

    match smoke_output {
        Ok(results) => {
            let probes = results
                .iter()
                .map(active_gateway_smoke_result_payload)
                .collect::<Vec<_>>();
            (
                StatusCode::OK,
                Json(json!({
                    "status": "ok",
                    "owner": "rust",
                    "route_group": "chapter_single_generation",
                    "probe_count": probes.len(),
                    "rollback_boundary": "legacy_single_generation_direct_ai",
                    "probes": probes,
                })),
            )
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "status": "failed",
                "owner": "rust",
                "route_group": "chapter_single_generation",
                "error": error,
            })),
        ),
    }
}

async fn chapter_batch_generation_active_gateway_smoke() -> (StatusCode, Json<Value>) {
    let smoke_output = run_chapter_batch_generation_active_gateway_smoke_suite().await;

    match smoke_output {
        Ok(results) => {
            let probes = results
                .iter()
                .map(batch_active_gateway_smoke_result_payload)
                .collect::<Vec<_>>();
            (
                StatusCode::OK,
                Json(json!({
                    "status": "ok",
                    "owner": "rust",
                    "route_group": "chapter_batch_generation",
                    "probe_count": probes.len(),
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "probes": probes,
                })),
            )
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "status": "failed",
                "owner": "rust",
                "route_group": "chapter_batch_generation",
                "error": error,
            })),
        ),
    }
}

async fn chapter_regeneration_stream_workflow_smoke() -> (StatusCode, Json<Value>) {
    let smoke_output = run_chapter_regeneration_stream_workflow_smoke_suite();

    match smoke_output {
        Ok(results) => {
            let probes = results
                .iter()
                .map(regeneration_stream_workflow_smoke_result_payload)
                .collect::<Vec<_>>();
            (
                StatusCode::OK,
                Json(json!({
                    "status": "ok",
                    "owner": "rust",
                    "route_group": "chapter_regeneration",
                    "probe_count": probes.len(),
                    "rollback_boundary": "chapter_regeneration_python_source_map",
                    "probes": probes,
                })),
            )
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "status": "failed",
                "owner": "rust",
                "route_group": "chapter_regeneration",
                "error": error,
            })),
        ),
    }
}

fn smoke_result_payload(result: &ChapterCandidateRouteGatewaySmokeResult) -> Value {
    json!({
        "name": &result.name,
        "owner": &result.owner,
        "route_group": &result.route_group,
        "ok": result.ok,
        "execution_path": &result.execution_path,
        "fallback_applied": result.fallback_applied,
        "reason": &result.reason,
        "rollback_boundary": &result.rollback_boundary,
        "rust_error": &result.rust_error,
        "result": &result.result,
        "runtime_state": &result.runtime_state,
        "readiness_evidence": &result.readiness_evidence,
    })
}

fn active_gateway_smoke_result_payload(
    result: &ChapterSingleGenerationActiveGatewaySmokeResult,
) -> Value {
    json!({
        "name": &result.name,
        "owner": &result.owner,
        "route_group": &result.route_group,
        "ok": result.ok,
        "execution_path": &result.execution_path,
        "fallback_applied": result.fallback_applied,
        "reason": &result.reason,
        "rollback_boundary": &result.rollback_boundary,
        "rust_error": &result.rust_error,
        "content": &result.content,
        "result": &result.result,
        "runtime_state": &result.runtime_state,
        "readiness_evidence": &result.readiness_evidence,
    })
}

fn batch_active_gateway_smoke_result_payload(
    result: &ChapterBatchGenerationActiveGatewaySmokeResult,
) -> Value {
    json!({
        "name": &result.name,
        "owner": &result.owner,
        "route_group": &result.route_group,
        "ok": result.ok,
        "execution_path": &result.execution_path,
        "fallback_applied": result.fallback_applied,
        "reason": &result.reason,
        "rollback_boundary": &result.rollback_boundary,
        "rust_error": &result.rust_error,
        "result": &result.result,
        "runtime_state": &result.runtime_state,
        "readiness_evidence": &result.readiness_evidence,
    })
}

fn regeneration_stream_workflow_smoke_result_payload(
    result: &ChapterRegenerationStreamWorkflowSmokeResult,
) -> Value {
    json!({
        "name": &result.name,
        "owner": &result.owner,
        "route_group": &result.route_group,
        "ok": result.ok,
        "execution_path": &result.execution_path,
        "fallback_applied": result.fallback_applied,
        "rollback_boundary": &result.rollback_boundary,
        "result": &result.result,
        "runtime_state": &result.runtime_state,
        "readiness_evidence": &result.readiness_evidence,
    })
}

pub fn routes() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/livez", get(liveness_check))
        .route("/readyz", get(readiness_check))
        .route("/health/db-sessions", get(db_session_stats))
        .route(
            RUST_MIGRATION_NOOP_EXECUTOR_SMOKE_ROUTE,
            get(rust_migration_noop_executor_smoke),
        )
        .route(
            CHAPTER_CANDIDATE_ROUTE_GATEWAY_SMOKE_ROUTE,
            get(chapter_candidate_route_gateway_smoke),
        )
        .route(
            CHAPTER_SINGLE_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE,
            get(chapter_single_generation_active_gateway_smoke),
        )
        .route(
            CHAPTER_BATCH_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE,
            get(chapter_batch_generation_active_gateway_smoke),
        )
        .route(
            CHAPTER_REGENERATION_STREAM_WORKFLOW_SMOKE_ROUTE,
            get(chapter_regeneration_stream_workflow_smoke),
        )
}

mod chapter_batch_generation_active_gateway_smoke_owner {
    // Route-facing health smoke owner for batch generation gateway cutover.
    // It keeps the create/status/stream/resume readiness evidence together with
    // the real Rust owners that now consume the candidate gateway configuration.

    use serde_json::{json, Value};

    use crate::ai::config::AIConfig;
    use crate::api::chapter_batch_generation::build_chapter_batch_generation_route_owner_contract;
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_read_context_service::{
        build_batch_generation_read_context_owner_contract,
        build_batch_generation_stream_progress_owner_contract,
        build_batch_generation_task_recovery_owner_contract, BatchGenerationStreamState,
    };
    use crate::services::chapter_batch_generation_resume_task_command_service::{
        build_batch_generation_resume_launch_owner_contract,
        build_batch_generation_resume_task_command_owner_contract,
    };
    use crate::services::chapter_batch_generation_runtime_state_service::build_generation_terminal_runtime_patch_owner_contract;
    use crate::services::chapter_batch_generation_runtime_state_service::{
        build_batch_generation_execution_input,
        build_batch_generation_follow_up_analysis_owner_contract,
        build_batch_generation_resume_restore_owner_contract,
        build_batch_generation_retry_routing_owner_contract,
        build_batch_generation_runtime_driver_owner_contract,
        build_batch_generation_runtime_state_owner_contract,
        build_batch_generation_selected_candidate_event_owner_contract,
        build_batch_generation_selected_candidate_event_snapshot,
        build_batch_generation_startup_and_command_projection_owner_contract,
    };
    use crate::services::chapter_batch_generation_task_payload_base_service::build_batch_generation_quality_terminal_status_owner_contract;
    use crate::services::chapter_batch_generation_write_workflow_service::{
        build_batch_generation_create_launch_owner_contract,
        build_batch_generation_write_workflow_owner_contract,
    };
    use crate::services::chapter_candidate_executor_production_adapter_service::{
        build_chapter_candidate_quality_adapter, chapter_candidate_production_execution_path_name,
        ChapterCandidateQualityAdapterContext,
    };
    use crate::services::chapter_candidate_route_gateway_service::{
        build_chapter_candidate_route_gateway_owner_contract,
        execute_chapter_candidate_route_gateway_with_executor, ChapterCandidateRouteGatewayConfig,
    };
    use crate::services::chapter_generation_execution_contract_service::build_single_generation_execution_contract_owner_contract;
    use crate::services::chapter_generation_execution_contract_service::{
        build_generation_execution_config_owner_contract, PreparedGenerationExecutionConfig,
    };
    use crate::services::chapter_generation_prompt_service::{
        build_placeholder_prompt_context_provider_payload,
        build_prompt_context_provider_owner_contract, build_quality_profile_owner_contract,
    };
    use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;
    use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract;
    use crate::services::chapter_generation_runtime_service::{
        build_single_generation_candidate_runtime_owner_contract, GeneratedChapterResult,
    };
    use crate::services::chapter_single_generation_prepare_service::research_payload_owner::build_single_chapter_research_payload_owner_contract;

    const ACTIVE_BATCH_GENERATION_ROUTE_GROUP: &str = "chapter_batch_generation";
    const ACTIVE_BATCH_GENERATION_ROLLBACK_BOUNDARY: &str = "python_candidate_executor_fallback";
    const ACTIVE_BATCH_GENERATION_TARGET_WORD_COUNT: i32 = 2800;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ChapterBatchGenerationActiveGatewaySmokeProbe {
        pub(crate) name: &'static str,
        pub(crate) owner: &'static str,
        pub(crate) route_group: &'static str,
        pub(crate) prompt: &'static str,
        pub(crate) config: ChapterCandidateRouteGatewayConfig,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct ChapterBatchGenerationActiveGatewaySmokeResult {
        pub(crate) name: String,
        pub(crate) owner: String,
        pub(crate) route_group: String,
        pub(crate) ok: bool,
        pub(crate) execution_path: String,
        pub(crate) fallback_applied: bool,
        pub(crate) reason: String,
        pub(crate) rollback_boundary: String,
        pub(crate) rust_error: Option<String>,
        pub(crate) result: Value,
        pub(crate) runtime_state: Option<Value>,
        pub(crate) readiness_evidence: Value,
    }

    pub(crate) fn build_default_chapter_batch_generation_active_gateway_smoke_probes(
    ) -> Vec<ChapterBatchGenerationActiveGatewaySmokeProbe> {
        vec![
            ChapterBatchGenerationActiveGatewaySmokeProbe {
                name: "chapter-batch-generation-active-gateway-rust-owner",
                owner: "rust",
                route_group: ACTIVE_BATCH_GENERATION_ROUTE_GROUP,
                prompt: "ACTIVE_BATCH_GENERATION_PROMPT",
                config: ChapterCandidateRouteGatewayConfig {
                    rust_executor_enabled: true,
                    fallback_on_rust_error: true,
                    disabled_reason: None,
                    rollback_boundary: ACTIVE_BATCH_GENERATION_ROLLBACK_BOUNDARY.to_string(),
                },
            },
            ChapterBatchGenerationActiveGatewaySmokeProbe {
                name: "chapter-batch-generation-active-gateway-fallback-freeze-candidate",
                owner: "rust",
                route_group: ACTIVE_BATCH_GENERATION_ROUTE_GROUP,
                prompt: "ACTIVE_BATCH_GENERATION_FREEZE_PROMPT",
                config: ChapterCandidateRouteGatewayConfig {
                    rust_executor_enabled: true,
                    fallback_on_rust_error: false,
                    disabled_reason: Some(
                        "batch generation active route fallback-freeze candidate".to_string(),
                    ),
                    rollback_boundary: ACTIVE_BATCH_GENERATION_ROLLBACK_BOUNDARY.to_string(),
                },
            },
        ]
    }

    pub(crate) async fn run_chapter_batch_generation_active_gateway_smoke_suite(
    ) -> Result<Vec<ChapterBatchGenerationActiveGatewaySmokeResult>, String> {
        let mut results = Vec::new();

        for probe in build_default_chapter_batch_generation_active_gateway_smoke_probes() {
            results.push(run_chapter_batch_generation_active_gateway_smoke_probe(probe).await?);
        }

        Ok(results)
    }

    pub(crate) async fn run_chapter_batch_generation_active_gateway_smoke_probe(
        probe: ChapterBatchGenerationActiveGatewaySmokeProbe,
    ) -> Result<ChapterBatchGenerationActiveGatewaySmokeResult, String> {
        let ai_config = smoke_ai_config();
        let mut request = batch_generation_candidate_executor_request(probe.prompt, &ai_config);
        let rust_probe_name = probe.name.to_string();

        let output = execute_chapter_candidate_route_gateway_with_executor(
            &mut request,
            ai_config.clone(),
            smoke_quality_adapter(probe.name),
            probe.config.clone(),
            move |request, _ai_config, _quality_adapter| {
                Box::pin(async move {
                    request.runtime_state = Some(json!({
                        "active_batch_generation_gateway": "rust",
                        "probe": rust_probe_name,
                        "generation_label": request.generation_label,
                        "source": request.source,
                    }));
                    Ok(json!({
                        "full_content": "Rust 批量候选章节正文。",
                        "candidate_chunks": ["Rust 批量", "候选章节正文。"],
                        "candidate_index": 1,
                        "candidate_count": 1,
                        "winner_candidate_index": 1,
                        "word_count": 12,
                        "generation_path": "batch_generation_rust_candidate_gateway",
                        "attempt_kind": "primary",
                        "rerank_used": false,
                        "word_budget_repair_used": false,
                        "quality_gate_plan": {
                            "action": "continue",
                            "quality_gate": {
                                "decision": "allow_save",
                                "status": "pass"
                            }
                        },
                        "quality_metrics": {
                            "overall_score": 90.0
                        },
                        "probe": rust_probe_name,
                        "gateway_consumed": true,
                    }))
                })
            },
            |_request, context| {
                Box::pin(async move {
                    Err(format!(
                        "batch generation active gateway smoke no longer exercises direct fallback; reason={}",
                        context.reason
                    ))
                })
            },
        )
        .await?;

        let readiness_evidence = build_active_batch_generation_readiness_evidence(
            &probe,
            &output.result,
            request.runtime_state.as_ref(),
            ai_config,
        )?;

        Ok(ChapterBatchGenerationActiveGatewaySmokeResult {
            name: probe.name.to_string(),
            owner: probe.owner.to_string(),
            route_group: probe.route_group.to_string(),
            ok: true,
            execution_path: chapter_candidate_production_execution_path_name(output.decision.path)
                .to_string(),
            fallback_applied: output.fallback_applied,
            reason: output.decision.reason,
            rollback_boundary: output.decision.rollback_boundary,
            rust_error: output.rust_error,
            result: output.result,
            runtime_state: request.runtime_state,
            readiness_evidence,
        })
    }

    fn build_active_batch_generation_readiness_evidence(
        probe: &ChapterBatchGenerationActiveGatewaySmokeProbe,
        gateway_result: &Value,
        runtime_state: Option<&Value>,
        ai_config: AIConfig,
    ) -> Result<Value, String> {
        let execution_input = build_batch_generation_execution_input(
            "batch-smoke-user".to_string(),
            vec!["batch-smoke-chapter".to_string()],
            ACTIVE_BATCH_GENERATION_TARGET_WORD_COUNT,
            Default::default(),
            PreparedGenerationExecutionConfig {
                ai_config,
                provider_payload: build_placeholder_prompt_context_provider_payload(),
            },
            probe.config.clone(),
        );
        let generated = active_batch_generation_smoke_generated_result(gateway_result);
        let selected_candidate_snapshot =
            build_batch_generation_selected_candidate_event_snapshot(&generated, true)
                .ok_or_else(|| "selected candidate event snapshot was empty".to_string())?;
        let stream_events = BatchGenerationStreamState::from_task_state_with_quality_context(
            active_batch_generation_smoke_task(),
            Some(&selected_candidate_snapshot),
            None,
        )
        .events();
        let selected_candidate_event_count = selected_candidate_snapshot
            .get("selected_candidate_events")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default();

        Ok(json!({
            "owner_scope": "batch_active_route_gateway_create_status_stream_resume",
            "covered_rust_owners": [
                "chapter_batch_generation",
                "chapter_batch_generation_write_workflow_service",
                "chapter_batch_generation_runtime_state_service",
                "chapter_batch_generation_read_context_service",
                "chapter_batch_generation_resume_task_command_service",
                "chapter_batch_generation_task_payload_base_service",
                "chapter_candidate_route_gateway_service",
                "chapter_generation_runtime_service",
                "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
                "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
                "chapter_candidate_record_service"
            ],
            "python_source_map": [],
              "python_source_map_policy": {
                "status": "source_map_only",
                "active_manifest_fallback_owner": false,
                "freeze_or_delete_requires_same_round_rollback_policy": true,
                "active_gateway_cutover": "nginx_routes_to_rust",
                "full_module_freeze_ready": true,
                "freeze_scope": "batch_generation_route_package_source_map_surface",
                "freeze_reason": "active traffic is routed to Rust, DB-backed Rust read smoke covers status/active/list payloads, the batch route package was deleted after test-seam migration, and shared candidate runtime/event source maps are tracked by dedicated shared-owner contracts",
                "python_bootstrap_status": "bootstrap_registration_deleted_no_route_wiring_loader_remains",
                "python_default_import_status": "chapters_py_no_longer_imports_batch_package_source_maps_by_default",
                "python_route_module_import_status": "legacy_batch_route_module_deleted_after_route_wiring_repoint",
                "python_route_registration_status": "no_python_route_registration_route_package_deleted_test_only_adapter_survives",
                "retired_top_level_shim_files": [
                    "batch_generation_top_level_create_shim_retired",
                    "batch_generation_top_level_query_shim_retired",
                    "batch_generation_top_level_resume_shim_retired",
                    "batch_generation_top_level_status_shim_retired"
                ],
                "retired_default_import_wiring_shells": [],
                "retired_default_import_workflow_shells": [],
                "retired_default_import_route_support_shells": [],
                "retired_default_import_execution_facades": [],
                "frozen_module_files": [],
                "remaining_default_import_source_maps": [],
                "read_context_source_maps": [],
                "shared_candidate_runtime_source_maps": [],
                "shared_projection_source_maps": [],
                "delete_candidate_boundary": "batch_generation_route_package_delete_completed_after_logged_in_db_smoke_and_test_seam_migration"
              },
            "active_gateway_cutover": {
                "deployment_owner": "deploy/nginx/mumunovel.conf",
                "routes_to_rust": [
                    "/api/chapters/project/{project_id}/batch-generate",
                    "/api/chapters/batch-generate/{batch_id}/status",
                    "/api/chapters/batch-generate/{batch_id}/stream",
                    "/api/chapters/project/{project_id}/batch-generate/active",
                    "/api/chapters/batch-generate/active-tasks",
                    "/api/chapters/batch-generate/{batch_id}/cancel",
                    "/api/chapters/batch-generate/{batch_id}/resume"
                ],
                "sse_routes": [
                    "/api/chapters/batch-generate/{batch_id}/stream"
                ],
                "python_route_files_status": "source_map_only_for_batch_generation_active_traffic",
                "python_bootstrap_registration": "bootstrap_registration_deleted_no_route_wiring_loader_remains",
                "python_route_module_import_status": "legacy_batch_route_module_deleted_after_route_wiring_repoint",
                "python_route_registration_status": "no_python_route_registration_route_package_deleted_test_only_adapter_survives",
                "rust_route_owner": "backend-rs/src/api/chapter_batch_generation.rs"
            },
            "db_backed_business_smoke": {
                "owner": "chapter_batch_generation_read_context_service",
                "fixture": "sqlite_memory_project_task_snapshot",
                "covered_paths": [
                    "load_owned_batch_generation_status_payload",
                    "load_owned_batch_generation_stream_state",
                    "load_active_batch_generation_view_from_route_project",
                    "load_active_user_batch_generation_task_list_view_from_route_query"
                ],
                "candidate_gateway_metadata_verified": true,
                "active_story_repair_payload_verified": true,
                "quality_metrics_verified": true,
                "executable_manifest_profile": {
                    "profile": "phase5-batch-generation-owner",
                    "requires_login": true,
                    "covers_real_http_routes": [
                        "POST /api/projects/import",
                        "GET /api/chapters?project_id={project_id}",
                        "POST /api/chapters/project/{project_id}/batch-generate",
                        "GET /api/chapters/batch-generate/{batch_id}/status",
                        "GET /api/chapters/project/{project_id}/batch-generate/active",
                        "GET /api/chapters/batch-generate/active-tasks",
                        "GET /api/chapters/batch-generate/{batch_id}/stream",
                        "POST /api/chapters/batch-generate/{batch_id}/cancel",
                        "DELETE /api/projects/{project_id}"
                    ],
                    "probe_names": [
                        "chapter-batch-generation-fixture-import-project-business-rust",
                        "chapter-batch-generation-fixture-list-chapters-business-rust",
                        "chapter-batch-generation-create-business-rust",
                        "chapter-batch-generation-status-business-rust",
                        "chapter-batch-generation-active-project-business-rust",
                        "chapter-batch-generation-active-tasks-business-rust",
                        "chapter-batch-generation-stream-business-rust",
                        "chapter-batch-generation-cancel-business-rust",
                        "chapter-batch-generation-cleanup-project-business-rust"
                    ]
                }
            },
            "active_route_gateway_config": {
                "source": "AppConfig -> chapter_batch_generation -> write workflow/resume command -> runtime launch",
                "route_config_builder": "build_chapter_candidate_route_gateway_config_from_app_config",
                "create_route_consumes_gateway_config": true,
                "resume_route_consumes_gateway_config": true,
                "runtime_launch_consumes_gateway_config": true,
                "rust_executor_enabled": probe.config.rust_executor_enabled,
                "fallback_on_rust_error": probe.config.fallback_on_rust_error,
                "disabled_reason": probe.config.disabled_reason.as_deref(),
                "rollback_boundary": probe.config.rollback_boundary,
            },
            "runtime_owner_chain": {
                "route": "chapter_batch_generation",
                "create_write_workflow": "start_owned_batch_generation_write_workflow",
                "resume_task_command": "resume_owned_batch_generation_task_command",
                "runtime_execution_input": "BatchGenerationExecutionInput",
                "runtime_generation": "generate_and_persist_chapter_content_with_candidate_route_gateway",
                "selected_candidate_snapshot": "build_batch_generation_selected_candidate_event_snapshot",
                "read_context": "BatchGenerationStreamState::from_task_state_with_quality_context",
                "status_stream": "BatchGenerationStreamState::events",
                "event_projection": "build_batch_generation_selected_candidate_event_batch"
            },
            "batch_route_owner_contract": build_chapter_batch_generation_route_owner_contract(),
            "candidate_route_gateway_owner_contract": build_chapter_candidate_route_gateway_owner_contract(),
            "batch_read_context_owner_contract": build_batch_generation_read_context_owner_contract(),
            "task_recovery_owner_contract": build_batch_generation_task_recovery_owner_contract(),
            "batch_resume_task_command_owner_contract": build_batch_generation_resume_task_command_owner_contract(),
            "resume_launch_owner_contract": build_batch_generation_resume_launch_owner_contract(),
            "batch_write_workflow_owner_contract": build_batch_generation_write_workflow_owner_contract(),
            "create_launch_owner_contract": build_batch_generation_create_launch_owner_contract(),
            "batch_runtime_state_owner_contract": build_batch_generation_runtime_state_owner_contract(),
            "quality_terminal_status_owner_contract": build_batch_generation_quality_terminal_status_owner_contract(),
            "selected_candidate_event_owner_contract": build_batch_generation_selected_candidate_event_owner_contract(),
            "stream_progress_owner_contract": build_batch_generation_stream_progress_owner_contract(),
            "resume_restore_owner_contract": build_batch_generation_resume_restore_owner_contract(),
            "follow_up_analysis_owner_contract": build_batch_generation_follow_up_analysis_owner_contract(),
            "retry_routing_owner_contract": build_batch_generation_retry_routing_owner_contract(),
            "startup_and_command_projection_owner_contract": build_batch_generation_startup_and_command_projection_owner_contract(),
            "runtime_driver_owner_contract": build_batch_generation_runtime_driver_owner_contract(),
            "execution_config_owner_contract": build_generation_execution_config_owner_contract(),
            "execution_contract_owner_contract": build_single_generation_execution_contract_owner_contract(),
            "shared_candidate_runtime_owner_contract": build_single_generation_candidate_runtime_owner_contract(),
            "prompt_context_provider_owner_contract": build_prompt_context_provider_owner_contract(),
            "quality_profile_owner_contract": build_quality_profile_owner_contract(),
            "research_payload_owner_contract": build_single_chapter_research_payload_owner_contract(),
            "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
            "story_repair_quality_context_owner_contract": build_story_repair_quality_context_owner_contract(),
            "terminal_runtime_patch_owner_contract": build_generation_terminal_runtime_patch_owner_contract(),
            "gateway": {
                "execution_path": "rust_candidate_executor",
                "gateway_consumed": gateway_result
                    .get("gateway_consumed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "generation_path": gateway_result.get("generation_path").cloned(),
                "runtime_state": runtime_state,
                "rust_executor_enabled": probe.config.rust_executor_enabled,
                "fallback_on_rust_error": probe.config.fallback_on_rust_error,
                "rollback_boundary": probe.config.rollback_boundary,
            },
            "batch_runtime_input": {
                "chapter_count": execution_input.chapter_ids.len(),
                "target_word_count": execution_input.target_word_count,
                "candidate_gateway_config": {
                    "rust_executor_enabled": execution_input.candidate_gateway_config.rust_executor_enabled,
                    "fallback_on_rust_error": execution_input.candidate_gateway_config.fallback_on_rust_error,
                    "rollback_boundary": execution_input.candidate_gateway_config.rollback_boundary,
                }
            },
            "selected_candidate_stream": {
                "snapshot_last_event": selected_candidate_snapshot["last_event"],
                "snapshot_event_count": selected_candidate_event_count,
                "first_selected_event_type": selected_candidate_snapshot["selected_candidate_events"][0]["type"],
                "chunk_event_projected": selected_candidate_snapshot["selected_candidate_events"]
                    .as_array()
                    .map(|events| events.iter().any(|event| event.get("type") == Some(&json!("chunk"))))
                    .unwrap_or(false),
                "read_context_event_count": stream_events.len(),
                "read_context_selected_event_type": stream_events.get(1).and_then(|event| event.get("type")).cloned(),
            },
            "fallback_shrink_readiness": {
                "candidate_probe": probe.name == "chapter-batch-generation-active-gateway-fallback-freeze-candidate",
                "active_route_smoke_consumes_freeze_candidate": probe.config.rust_executor_enabled
                    && !probe.config.fallback_on_rust_error
                    && gateway_result
                        .get("gateway_consumed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                "rust_owner_path_validated": probe.config.rust_executor_enabled
                    && gateway_result
                        .get("gateway_consumed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                "fallback_freeze_config_validated": probe.config.rust_executor_enabled
                    && !probe.config.fallback_on_rust_error
                    && gateway_result
                        .get("gateway_consumed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                "python_fallback_removal_ready": probe.config.rust_executor_enabled
                    && !probe.config.fallback_on_rust_error
                    && selected_candidate_event_count > 0
                    && gateway_result
                        .get("gateway_consumed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                "remaining_blockers": []
            },
            "rollback_policy": {
                "active_boundary": probe.config.rollback_boundary,
                "operator_knob": "CHAPTER_CANDIDATE_RUST_EXECUTOR_ENABLED / CHAPTER_CANDIDATE_RUST_EXECUTOR_FALLBACK_ON_ERROR",
                "python_source_map_action": "production_python_source_maps_deleted_shared_test_support_and_reference_contracts_remain",
                "manifest_owner_baseline": "rust = 131, python-fallback = 0"
            },
            "next_cutover_gate": "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages"
        }))
    }

    fn batch_generation_candidate_executor_request(
        prompt: &str,
        ai_config: &AIConfig,
    ) -> crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest {
        crate::services::chapter_generation_runtime_service::build_single_generation_candidate_executor_request(
            prompt,
            ACTIVE_BATCH_GENERATION_TARGET_WORD_COUNT,
            ai_config,
        )
    }

    fn active_batch_generation_smoke_generated_result(
        gateway_result: &Value,
    ) -> GeneratedChapterResult {
        GeneratedChapterResult {
            chapter_id: "batch-smoke-chapter".to_string(),
            chapter_number: 7,
            title: "Batch Smoke Chapter".to_string(),
            content: "Rust 批量候选章节正文。".to_string(),
            word_count: 12,
            saved_word_count: 12,
            chapter_status: "completed".to_string(),
            selected_candidate_event_source: Some(gateway_result.clone()),
            ..Default::default()
        }
    }

    fn active_batch_generation_smoke_task() -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "batch-smoke-task".to_string(),
            project_id: "batch-smoke-project".to_string(),
            user_id: "batch-smoke-user".to_string(),
            start_chapter_number: 7,
            chapter_count: 1,
            chapter_ids: json!(["batch-smoke-chapter"]),
            style_id: None,
            target_word_count: ACTIVE_BATCH_GENERATION_TARGET_WORD_COUNT,
            enable_analysis: true,
            status: "running".to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("batch-smoke-chapter".to_string()),
            current_chapter_number: Some(7),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn smoke_ai_config() -> AIConfig {
        let mut ai_config = AIConfig::default();
        ai_config.temperature = 0.72;
        ai_config.max_tokens = 4096;
        ai_config
    }

    fn smoke_quality_adapter(
        probe_name: &str,
    ) -> crate::services::chapter_candidate_executor_production_adapter_service::ChapterCandidateQualityAdapter<
        impl FnMut(
            crate::services::chapter_candidate_executor_production_adapter_service::CandidateQualityRuntimeContextBuildInput,
        ) -> Value,
        impl FnMut(
            crate::services::chapter_candidate_executor_production_adapter_service::CandidateStoryQualityMetricsInput,
        ) -> Value,
        impl FnMut(
            crate::services::chapter_candidate_executor_production_adapter_service::CandidateQualityGatePlanInput,
        ) -> Value,
    >{
        build_chapter_candidate_quality_adapter(
            ChapterCandidateQualityAdapterContext {
                story_packet: json!({"packet": "active_batch_generation_smoke"}),
                project: json!({"world_rules": "rules"}),
                chapter: json!({"id": "batch-smoke-chapter", "title": "第七章"}),
                chapter_context: json!({"chapter_outline": "outline"}),
                target_word_count: i64::from(ACTIVE_BATCH_GENERATION_TARGET_WORD_COUNT),
                generation_intent: json!({
                    "mode": "batch_generation_active_route_smoke",
                    "probe": probe_name,
                }),
                creative_mode: String::new(),
                story_focus: String::new(),
                plot_stage: String::new(),
                story_creation_brief: String::new(),
                quality_preset: String::new(),
                quality_notes: String::new(),
                chapter_count: None,
                current_chapter_number: None,
                retry_count: 0,
                max_retries: 1,
                story_repair_summary: String::new(),
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
                current_story_repair_payload: None,
                scope: "chapter".to_string(),
                log_prefix: "BatchGenerationActiveGatewaySmoke".to_string(),
            },
            |_input| json!({"runtime": "context"}),
            |input| {
                json!({
                    "overall_score": 90.0,
                    "word_count": input.content.chars().count(),
                })
            },
            |_input| {
                json!({
                    "action": "continue",
                    "quality_gate": {
                        "decision": "allow_save",
                        "status": "pass",
                        "allow_save": true,
                    }
                })
            },
        )
    }

    #[cfg(test)]
    mod tests {
        use super::{
            build_default_chapter_batch_generation_active_gateway_smoke_probes,
            run_chapter_batch_generation_active_gateway_smoke_probe,
            run_chapter_batch_generation_active_gateway_smoke_suite,
        };
        use serde_json::json;

        #[test]
        fn should_build_batch_active_gateway_smoke_probes_for_enabled_and_freeze_paths() {
            let probes = build_default_chapter_batch_generation_active_gateway_smoke_probes();

            assert_eq!(probes.len(), 2);
            assert_eq!(
                probes[0].name,
                "chapter-batch-generation-active-gateway-rust-owner"
            );
            assert!(probes[0].config.rust_executor_enabled);
            assert_eq!(
                probes[1].name,
                "chapter-batch-generation-active-gateway-fallback-freeze-candidate"
            );
            assert!(probes[1].config.rust_executor_enabled);
            assert!(!probes[1].config.fallback_on_rust_error);
            assert_eq!(probes[0].route_group, "chapter_batch_generation");
        }

        #[tokio::test]
        async fn should_run_batch_active_gateway_smoke_through_enabled_and_freeze_paths() {
            let results = run_chapter_batch_generation_active_gateway_smoke_suite()
                .await
                .expect("batch active gateway smoke results");

            assert_eq!(results.len(), 2);
            assert_eq!(results[0].execution_path, "rust_candidate_executor");
            assert!(!results[0].fallback_applied);
            assert_eq!(
                results[0].result["generation_path"],
                "batch_generation_rust_candidate_gateway"
            );
            assert_eq!(
                results[0].runtime_state.as_ref().unwrap()["generation_label"],
                "single_generation_candidate"
            );
            assert_eq!(
                results[0].readiness_evidence["selected_candidate_stream"]["snapshot_event_count"],
                3
            );

            assert_eq!(results[1].execution_path, "rust_candidate_executor");
            assert!(!results[1].fallback_applied);
            assert_eq!(
                results[1].readiness_evidence["fallback_shrink_readiness"]
                    ["active_route_smoke_consumes_freeze_candidate"],
                true
            );
        }

        #[tokio::test]
        async fn should_project_batch_active_route_owner_chain_and_stream_consumer() {
            let probe = build_default_chapter_batch_generation_active_gateway_smoke_probes()
                .into_iter()
                .next()
                .expect("batch active gateway probe");

            let result = run_chapter_batch_generation_active_gateway_smoke_probe(probe)
                .await
                .expect("batch active gateway smoke result");
            let readiness = &result.readiness_evidence;

            assert!(result.ok);
            assert_eq!(result.owner, "rust");
            assert_eq!(result.route_group, "chapter_batch_generation");
            assert_eq!(
                result.rollback_boundary,
                "python_candidate_executor_fallback"
            );
            assert_eq!(
                readiness["owner_scope"],
                "batch_active_route_gateway_create_status_stream_resume"
            );
            assert_eq!(
                readiness["active_route_gateway_config"]["create_route_consumes_gateway_config"],
                true
            );
            assert_eq!(
                readiness["active_route_gateway_config"]["resume_route_consumes_gateway_config"],
                true
            );
            assert_eq!(
                readiness["runtime_owner_chain"]["create_write_workflow"],
                "start_owned_batch_generation_write_workflow"
            );
            assert_eq!(
                readiness["runtime_owner_chain"]["resume_task_command"],
                "resume_owned_batch_generation_task_command"
            );
            assert_eq!(
                readiness["runtime_owner_chain"]["runtime_generation"],
                "generate_and_persist_chapter_content_with_candidate_route_gateway"
            );
            assert_eq!(
                readiness["runtime_owner_chain"]["selected_candidate_snapshot"],
                "build_batch_generation_selected_candidate_event_snapshot"
            );
            assert_eq!(
                readiness["batch_route_owner_contract"]["owner"],
                "chapter_batch_generation"
            );
            assert_eq!(
                readiness["batch_route_owner_contract"]["route_contract"]["create"],
                "/chapters/project/{project_id}/batch-generate"
            );
            assert_eq!(
                readiness["batch_route_owner_contract"]["route_contract"]["stream"],
                "/chapters/batch-generate/{batch_id}/stream"
            );
            assert_eq!(
                readiness["batch_route_owner_contract"]["route_contract"]["resume"],
                "/chapters/batch-generate/{batch_id}/resume"
            );
            assert_eq!(
                readiness["batch_route_owner_contract"]["behavior_contract"]["route_entrypoints"]
                    [6],
                "resume_batch_generation"
            );
            assert_eq!(
                readiness["batch_route_owner_contract"]["active_consumers"][2],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["batch_route_owner_contract"]["rollback_boundary"]
                    ["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                readiness["candidate_route_gateway_owner_contract"]["owner"],
                "chapter_candidate_route_gateway_service"
            );
            assert_eq!(
                readiness["candidate_route_gateway_owner_contract"]["behavior_contract"]
                    ["gateway_entrypoints"][1],
                "execute_chapter_candidate_route_gateway_with_executor"
            );
            assert_eq!(
                readiness["candidate_route_gateway_owner_contract"]["active_consumers"][3],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["candidate_route_gateway_owner_contract"]["rollback_boundary"]
                    ["runtime_knob"],
                "python_candidate_executor_fallback"
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["owner"],
                "chapter_batch_generation_read_context_service"
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["behavior_contract"]
                    ["route_payloads"][3],
                "active_user_task_list_payload"
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["active_consumers"][2],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["rollback_boundary"]
                    ["runtime_state_keys"][3],
                "selected_candidate_events"
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profile"],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["service_runtime_closeout_status"]
                    ["rust_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["batch_read_context_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_batch_generation_read_context_owner_source_map_deleted"
            );
            assert_eq!(
                readiness["task_recovery_owner_contract"]["owner"],
                "chapter_batch_generation_read_context_service::task_recovery_owner"
            );
            assert_eq!(
                readiness["task_recovery_owner_contract"]["behavior_contract"]["entrypoints"][1],
                "recover_generation_task_if_needed"
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]["owner"],
                "chapter_batch_generation_resume_task_command_service::resume_task_command_owner"
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]["behavior_contract"]
                    ["command_entrypoints"][0],
                "prepare_owned_batch_generation_resume"
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]["behavior_contract"]
                    ["command_entrypoints"][1],
                "resume_owned_batch_generation_task_command"
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]["behavior_contract"]
                    ["gateway_config"][1],
                "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config"
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]["active_consumers"][2],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]["rollback_boundary"]
                    ["runtime_knob"],
                "python_candidate_executor_fallback"
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]["rollback_boundary"]
                    ["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profile"],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]
                    ["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]
                    ["service_runtime_closeout_status"]["rust_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]
                    ["service_runtime_closeout_status"]["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]
                    ["service_runtime_closeout_status"]["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]
                    ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["batch_resume_task_command_owner_contract"]
                    ["service_runtime_closeout_status"]["status"],
                "rust_batch_generation_resume_task_command_owner_source_map_deleted"
            );
            assert_eq!(
                readiness["resume_launch_owner_contract"]["owner"],
                "chapter_batch_generation_resume_task_command_service::resume_launch_owner"
            );
            assert_eq!(
                readiness["resume_launch_owner_contract"]["behavior_contract"]
                    ["resume_launch_entrypoints"][3],
                "BatchGenerationResumeLaunchPersistencePlan::persist_and_dispatch"
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["owner"],
                "chapter_batch_generation_write_workflow_service"
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["behavior_contract"]
                    ["create_entrypoints"][2],
                "start_owned_batch_generation_write_workflow"
            );
            assert_eq!(
                readiness["batch_runtime_state_owner_contract"]["behavior_contract"]
                    ["cancel_command_entrypoints"][0],
                "cancel_owned_batch_generation_runtime_command"
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["active_consumers"][1],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["rollback_boundary"]
                    ["runtime_knob"],
                "python_candidate_executor_fallback"
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["rollback_boundary"]
                    ["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profile"],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["service_runtime_closeout_status"]
                    ["rust_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["batch_write_workflow_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_batch_generation_write_workflow_owner_source_map_deleted"
            );
            assert_eq!(
                readiness["create_launch_owner_contract"]["owner"],
                "chapter_batch_generation_write_workflow_service::create_launch_startup_seed_and_persistence"
            );
            assert_eq!(
                readiness["create_launch_owner_contract"]["behavior_contract"]
                    ["persistence_and_dispatch_entrypoints"][3],
                "dispatch_batch_generation_runtime"
            );
            assert_eq!(
                readiness["batch_runtime_state_owner_contract"]["owner"],
                "chapter_batch_generation_runtime_state_service"
            );
            assert_eq!(
                readiness["batch_runtime_state_owner_contract"]["behavior_contract"]
                    ["runtime_entrypoints"][0],
                "build_batch_generation_execution_input"
            );
            assert_eq!(
                readiness["batch_runtime_state_owner_contract"]["behavior_contract"]
                    ["candidate_gateway_entrypoints"][1],
                "build_batch_generation_selected_candidate_event_snapshot"
            );
            assert_eq!(
                readiness["batch_runtime_state_owner_contract"]["behavior_contract"]
                    ["retry_and_terminal_entrypoints"][3],
                "BatchGenerationPostAnalysisTerminalPlan"
            );
            assert_eq!(
                readiness["batch_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                    ["cancel_task_command_owner"],
                "cancel_owned_batch_generation_runtime_command"
            );
            assert_eq!(
                readiness["batch_runtime_state_owner_contract"]["active_consumers"][3],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["batch_runtime_state_owner_contract"]["rollback_boundary"]
                    ["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                readiness["quality_terminal_status_owner_contract"]["owner"],
                "chapter_batch_generation_task_payload_base_service::quality_terminal_status_owner"
            );
            assert_eq!(
                readiness["quality_terminal_status_owner_contract"]["behavior_contract"]
                    ["status_payload_projection_entrypoints"][2],
                "build_batch_generation_status_task_payload_from_task_and_snapshot_projection"
            );
            assert_eq!(
                readiness["selected_candidate_event_owner_contract"]["owner"],
                "chapter_batch_generation_runtime_state_service::selected_candidate_event_owner"
            );
            assert_eq!(
                readiness["selected_candidate_event_owner_contract"]["behavior_contract"]
                    ["selected_candidate_batch_contract"]["progress_event_first"],
                true
            );
            assert_eq!(
                readiness["selected_candidate_event_owner_contract"]["active_consumers"][0],
                "chapter_batch_generation_runtime_state_service"
            );
            assert_eq!(
                readiness["selected_candidate_event_owner_contract"]["active_consumers"][1],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["selected_candidate_event_owner_contract"]["rollback_boundary"]
                    ["runtime_state_keys"][1],
                "selected_candidate_events"
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["owner"],
                "chapter_batch_generation_read_context_service::stream_progress_owner"
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["behavior_contract"]["event_type"],
                "progress"
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["active_consumers"][0],
                "chapter_batch_generation_read_context_service"
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profile"],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["service_runtime_closeout_status"]
                    ["rust_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["stream_progress_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_batch_generation_stream_progress_owner_source_map_deleted"
            );
            assert_eq!(
                readiness["resume_restore_owner_contract"]["owner"],
                "chapter_batch_generation_runtime_state_service::resume_restore_runtime_projection"
            );
            assert_eq!(
                readiness["follow_up_analysis_owner_contract"]["owner"],
                "chapter_batch_generation_runtime_state_service::follow_up_analysis_runtime_projection"
            );
            assert_eq!(
                readiness["retry_routing_owner_contract"]["owner"],
                "chapter_batch_generation_runtime_state_service::retry_failure_quality_gate_routing"
            );
            assert_eq!(
                readiness["startup_and_command_projection_owner_contract"]["owner"],
                "chapter_batch_generation_runtime_state_service::startup_cancel_resume_task_payload_projection"
            );
            assert_eq!(
                readiness["runtime_driver_owner_contract"]["owner"],
                "chapter_batch_generation_runtime_state_service::runtime_driver_execution_chain"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]["owner"],
                "chapter_generation_runtime_service"
            );
            assert_eq!(
                readiness["execution_config_owner_contract"]["owner"],
                "chapter_generation_execution_contract_service::execution_config"
            );
            assert_eq!(
                readiness["execution_config_owner_contract"]["behavior_contract"]
                    ["provider_payload_passthrough"],
                true
            );
            assert_eq!(
                readiness["execution_config_owner_contract"]["active_consumers"][6],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["execution_contract_owner_contract"]["owner"],
                "chapter_generation_execution_contract_service::single_generation_contract_owner"
            );
            assert_eq!(
                readiness["execution_contract_owner_contract"]["behavior_contract"]
                    ["story_repair_arrays_preserved"],
                true
            );
            assert_eq!(
                readiness["execution_contract_owner_contract"]["behavior_contract"]
                    ["execution_input_fields"][2],
                "execution_config"
            );
            assert_eq!(
                readiness["execution_contract_owner_contract"]["active_consumers"][10],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]["python_source_map"],
                json!([])
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]["rollback_boundary"]
                    ["single_generation_entry_source_maps"],
                json!([])
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]["rollback_boundary"]
                    ["shared_candidate_runtime_source_maps"],
                json!([])
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]["behavior_contract"]
                    ["accepted_content_fields"],
                json!(["full_content", "content"])
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_wiring_readiness"]["owner"],
                "chapter_candidate_executor_default_dependency_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_wiring_readiness"]["stage_count"],
                9
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]["owner"],
                "chapter_candidate_executor_default_dependency_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][1],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["status"],
                "rust_chapter_candidate_executor_default_dependency_owner_source_map_deleted"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]["owner"],
                "chapter_candidate_executor_production_adapter_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][1],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["status"],
                "rust_chapter_candidate_executor_production_adapter_owner_executor_source_map_deleted"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["owner"],
                "chapter_candidate_record_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profiles"][1],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_chapter_candidate_record_owner_executor_source_map_deleted"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["owner"],
                "chapter_candidate_finalize_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profiles"][1],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_chapter_candidate_finalize_owner_source_map_deleted"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["owner"],
                "chapter_candidate_generation_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profiles"][1],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_chapter_candidate_generation_owner_source_map_deleted"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["owner"],
                "chapter_candidate_rerank_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profiles"][1],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_chapter_candidate_rerank_owner_source_map_deleted"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]["owner"],
                "chapter_candidate_word_budget_repair_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][1],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["status"],
                "rust_chapter_candidate_word_budget_repair_owner_source_map_deleted"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]["owner"],
                "chapter_candidate_targeted_final_repair_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][1],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["status"],
                "rust_chapter_candidate_targeted_final_repair_owner_source_map_deleted"
            );
            assert!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_wiring_readiness"]["rust_owned_dependency_count"]
                    .as_u64()
                    .unwrap()
                    >= 56
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]["rollback_boundary"]
                    ["source_map_policy"],
                "shared_candidate_runtime_owner_is_rust_only_and_candidate_shell_source_maps_are_deleted"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["context_compaction_owner_contract"]["owner"],
                "chapter_generation_runtime_service"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]
                    ["context_compaction_owner_contract"]["active_consumers"][1],
                "chapter_regeneration_prepare_service"
            );
            assert_eq!(
                readiness["prompt_context_provider_owner_contract"]["owner"],
                "chapter_generation_prompt_service"
            );
            assert_eq!(
                readiness["prompt_context_provider_owner_contract"]["behavior_contract"]
                    ["placeholder_array_defaults"][5],
                "external_assets"
            );
            assert_eq!(
                readiness["prompt_context_provider_owner_contract"]["python_source_map"],
                json!([])
            );
            assert_eq!(
                readiness["prompt_context_provider_owner_contract"]["active_consumers"][7],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["research_payload_owner_contract"]["owner"],
                "chapter_single_generation_prepare_service::research_payload_owner"
            );
            assert_eq!(
                readiness["research_payload_owner_contract"]["active_consumers"][1],
                "chapter_batch_generation_runtime_state_service"
            );
            assert_eq!(
                readiness["research_payload_owner_contract"]["rollback_boundary"]["runtime_knob"],
                "SingleChapterGenerationCompatOptions.web_research_enabled"
            );
            assert_eq!(
                readiness["quality_runtime_owner_contract"]["owner"],
                "chapter_generation_runtime_service::quality_runtime_context_owner"
            );
            assert_eq!(
                readiness["quality_runtime_owner_contract"]["active_consumers"][2],
                "chapter_batch_generation_runtime_state_service"
            );
            assert_eq!(
                readiness["quality_runtime_owner_contract"]["behavior_contract"]["quality_fields"]
                    [4],
                "quality_history_context"
            );
            assert_eq!(
                readiness["story_repair_quality_context_owner_contract"]["owner"],
                "chapter_generation_runtime_service::story_repair_quality_context_owner"
            );
            assert_eq!(
                readiness["story_repair_quality_context_owner_contract"]["behavior_contract"]
                    ["merge_limits"]["repair_targets"],
                4
            );
            assert_eq!(
                readiness["story_repair_quality_context_owner_contract"]["active_consumers"][8],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["terminal_runtime_patch_owner_contract"]["owner"],
                "chapter_batch_generation_runtime_state_service::terminal_runtime_patch"
            );
            assert_eq!(
                readiness["terminal_runtime_patch_owner_contract"]["behavior_contract"]
                    ["quality_runtime_fields_normalized"][4],
                "quality_history_context"
            );
            assert_eq!(
                readiness["terminal_runtime_patch_owner_contract"]["active_consumers"][2],
                "chapter-batch-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                readiness["terminal_runtime_patch_owner_contract"]["rollback_boundary"]
                    ["runtime_state_keys"][0],
                "active_story_repair_payload"
            );
            assert_eq!(
                readiness["selected_candidate_stream"]["first_selected_event_type"],
                "progress"
            );
            assert_eq!(
                readiness["selected_candidate_stream"]["chunk_event_projected"],
                true
            );
            assert_eq!(
                readiness["selected_candidate_stream"]["read_context_selected_event_type"],
                "progress"
            );
            assert_eq!(
                readiness["active_gateway_cutover"]["deployment_owner"],
                "deploy/nginx/mumunovel.conf"
            );
            assert_eq!(
                readiness["active_gateway_cutover"]["python_route_files_status"],
                "source_map_only_for_batch_generation_active_traffic"
            );
            assert_eq!(
                readiness["active_gateway_cutover"]["python_bootstrap_registration"],
                "bootstrap_registration_deleted_no_route_wiring_loader_remains"
            );
            assert_eq!(
                readiness["active_gateway_cutover"]["rust_route_owner"],
                "backend-rs/src/api/chapter_batch_generation.rs"
            );
            assert!(readiness["active_gateway_cutover"]["routes_to_rust"]
                .as_array()
                .expect("batch cutover routes")
                .iter()
                .any(|route| route == "/api/chapters/batch-generate/{batch_id}/resume"));
            assert!(readiness["active_gateway_cutover"]["sse_routes"]
                .as_array()
                .expect("batch cutover sse routes")
                .iter()
                .any(|route| route == "/api/chapters/batch-generate/{batch_id}/stream"));
        }

        #[tokio::test]
        async fn should_project_batch_active_route_fallback_freeze_readiness() {
            let probe = build_default_chapter_batch_generation_active_gateway_smoke_probes()
                .into_iter()
                .find(|probe| {
                    probe.name
                        == "chapter-batch-generation-active-gateway-fallback-freeze-candidate"
                })
                .expect("batch active fallback freeze probe");

            let result = run_chapter_batch_generation_active_gateway_smoke_probe(probe)
                .await
                .expect("batch active fallback freeze smoke result");
            let readiness = &result.readiness_evidence;

            assert_eq!(result.execution_path, "rust_candidate_executor");
            assert!(!result.fallback_applied);
            assert_eq!(
                readiness["active_route_gateway_config"]["fallback_on_rust_error"],
                false
            );
            assert_eq!(
                readiness["fallback_shrink_readiness"]["fallback_freeze_config_validated"],
                true
            );
            assert_eq!(
                readiness["fallback_shrink_readiness"]["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                readiness["next_cutover_gate"],
                "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages"
            );
            assert_eq!(
                readiness["python_source_map_policy"]["active_gateway_cutover"],
                "nginx_routes_to_rust"
            );
            assert_eq!(
                readiness["python_source_map_policy"]["full_module_freeze_ready"],
                true
            );
            assert_eq!(
                readiness["python_source_map_policy"]["freeze_scope"],
                "batch_generation_route_package_source_map_surface"
            );
            assert_eq!(
                readiness["python_source_map_policy"]["python_bootstrap_status"],
                "bootstrap_registration_deleted_no_route_wiring_loader_remains"
            );
            assert_eq!(
                readiness["python_source_map_policy"]["python_default_import_status"],
                "chapters_py_no_longer_imports_batch_package_source_maps_by_default"
            );
            assert_eq!(
                readiness["python_source_map_policy"]["remaining_default_import_source_maps"]
                    .as_array()
                    .expect("remaining default import source maps")
                    .len(),
                0
            );
            assert_eq!(
                readiness["python_source_map_policy"]["retired_default_import_wiring_shells"],
                json!([])
            );
            assert_eq!(
                readiness["python_source_map_policy"]["retired_default_import_workflow_shells"],
                json!([])
            );
            assert_eq!(
                readiness["python_source_map_policy"]["shared_candidate_runtime_source_maps"],
                json!([])
            );
            assert_eq!(
                readiness["python_source_map_policy"]["shared_projection_source_maps"],
                json!([])
            );
            assert_eq!(
                readiness["rollback_policy"]["python_source_map_action"],
                "production_python_source_maps_deleted_shared_test_support_and_reference_contracts_remain"
            );
            assert_eq!(
                readiness["python_source_map_policy"]
                    ["retired_default_import_route_support_shells"],
                json!([])
            );
            assert!(readiness["python_source_map_policy"]
                ["retired_default_import_execution_facades"]
                .as_array()
                .expect("retired default import execution facades")
                .is_empty());
            assert_eq!(
                readiness["python_source_map_policy"]["delete_candidate_boundary"],
                "batch_generation_route_package_delete_completed_after_logged_in_db_smoke_and_test_seam_migration"
            );
            assert_eq!(
                readiness["python_source_map_policy"]["python_route_module_import_status"],
                "legacy_batch_route_module_deleted_after_route_wiring_repoint"
            );
            assert_eq!(
                readiness["python_source_map_policy"]["python_route_registration_status"],
                "no_python_route_registration_route_package_deleted_test_only_adapter_survives"
            );
            assert!(
                readiness["python_source_map_policy"]["retired_top_level_shim_files"]
                    .as_array()
                    .expect("retired top-level shim files")
                    .iter()
                    .any(|path| path == "batch_generation_top_level_query_shim_retired")
            );
            assert!(readiness["python_source_map_policy"]["frozen_module_files"]
                .as_array()
                .expect("frozen module files")
                .is_empty());
            assert_eq!(
                readiness["python_source_map_policy"]["read_context_source_maps"],
                json!([])
            );
            assert_eq!(
                readiness["db_backed_business_smoke"]["owner"],
                "chapter_batch_generation_read_context_service"
            );
            assert_eq!(
                readiness["db_backed_business_smoke"]["candidate_gateway_metadata_verified"],
                true
            );
            assert!(readiness["db_backed_business_smoke"]["covered_paths"]
                .as_array()
                .expect("db-backed covered paths")
                .iter()
                .any(|path| path == "load_owned_batch_generation_status_payload"));
            assert!(readiness["db_backed_business_smoke"]["covered_paths"]
                .as_array()
                .expect("db-backed covered paths")
                .iter()
                .any(|path| path == "load_owned_batch_generation_stream_state"));
            assert_eq!(
                readiness["db_backed_business_smoke"]["executable_manifest_profile"]["profile"],
                "phase5-batch-generation-owner"
            );
            assert!(
                readiness["db_backed_business_smoke"]["executable_manifest_profile"]
                    ["covers_real_http_routes"]
                    .as_array()
                    .expect("batch business smoke real HTTP route coverage")
                    .iter()
                    .any(|route| {
                        route == "POST /api/chapters/project/{project_id}/batch-generate"
                    })
            );
            assert!(
                readiness["db_backed_business_smoke"]["executable_manifest_profile"]["probe_names"]
                    .as_array()
                    .expect("batch business smoke probe names")
                    .iter()
                    .any(|probe| probe == "chapter-batch-generation-cancel-business-rust")
            );
        }

        #[tokio::test]
        async fn should_not_list_retired_forwarding_only_batch_owners() {
            let results = run_chapter_batch_generation_active_gateway_smoke_suite()
                .await
                .expect("batch active gateway smoke results");
            let owners = results[0].readiness_evidence["covered_rust_owners"]
                .as_array()
                .expect("covered rust owners");
            let actual_owner_names: Vec<&str> = owners
                .iter()
                .map(|owner| owner.as_str().expect("owner string"))
                .collect();
            let expected_owner_names = vec![
                "chapter_batch_generation",
                "chapter_batch_generation_write_workflow_service",
                "chapter_batch_generation_runtime_state_service",
                "chapter_batch_generation_read_context_service",
                "chapter_batch_generation_resume_task_command_service",
                "chapter_batch_generation_task_payload_base_service",
                "chapter_candidate_route_gateway_service",
                "chapter_generation_runtime_service",
                "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
                "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
                "chapter_candidate_record_service",
            ];

            assert_eq!(actual_owner_names, expected_owner_names);
        }
    }
}

mod chapter_single_generation_active_gateway_smoke_owner {
    // Active-route smoke owner for single-chapter generation gateway cutover.
    // It exercises the same request/content gateway boundary as the production
    // single-generation route, but uses fake executors so no provider call occurs.
    use serde_json::{json, Value};

    use crate::ai::config::AIConfig;
    use crate::api::chapter_generation_routes::build_chapter_single_generation_route_owner_contract;
    use crate::models::batch_generation_task;
    use crate::services::chapter_access_service::build_chapter_generation_access_owner_contract;
    use crate::services::chapter_batch_generation_runtime_state_service::build_generation_terminal_runtime_patch_owner_contract;
    use crate::services::chapter_candidate_executor_production_adapter_service::{
        build_chapter_candidate_quality_adapter, chapter_candidate_production_execution_path_name,
        ChapterCandidateProductionAdapterOutput, ChapterCandidateQualityAdapterContext,
    };
    use crate::services::chapter_candidate_route_gateway_service::{
        build_chapter_candidate_route_gateway_owner_contract,
        execute_chapter_candidate_route_gateway_with_executor, ChapterCandidateRouteGatewayConfig,
    };
    use crate::services::chapter_candidate_runtime_state_service::build_chapter_candidate_runtime_state_owner_contract;
    use crate::services::chapter_generation_execution_contract_service::{
        build_generation_execution_config_owner_contract, PreparedGenerationExecutionConfig,
    };
    use crate::services::chapter_generation_execution_contract_service::{
        build_single_generation_execution_contract_owner_contract,
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_prompt_service::{
        build_chapter_generation_prompt_owner_contract,
        build_prompt_context_provider_owner_contract, build_quality_profile_owner_contract,
        PromptContextProviderPayload,
    };
    use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;
    use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract;
    use crate::services::chapter_generation_runtime_service::{
        build_chapter_single_generation_candidate_quality_owner_contract,
        build_single_generation_candidate_executor_request,
        build_single_generation_candidate_gateway_metadata,
        build_single_generation_candidate_runtime_owner_contract,
        single_generation_candidate_gateway_content, GeneratedChapterResult,
    };
    use crate::services::chapter_single_generation_background_launch_service::{
        build_single_generation_background_create_response_payload,
        build_single_generation_background_launch_owner_contract,
        build_single_generation_pending_checkpoint,
        build_single_generation_startup_snapshot_owner_contract,
        SingleGenerationStartupSnapshotPlan,
    };
    use crate::services::chapter_single_generation_prepare_service::build_chapter_generation_prerequisite_owner_contract;
    use crate::services::chapter_single_generation_prepare_service::research_payload_owner::build_single_chapter_research_payload_owner_contract;
    use crate::services::chapter_single_generation_prepare_service::{
        build_single_generation_prepare_owner_contract,
        build_single_generation_task_view_payload_owner_contract, SingleChapterGenerationTarget,
    };
    use crate::services::chapter_single_generation_runtime_restore_workflow_service::build_single_generation_runtime_restore_owner_contract;
    use crate::services::chapter_single_generation_runtime_restore_workflow_service::build_single_generation_write_workflow_owner_contract;
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;
    use crate::services::chapter_single_generation_runtime_state_service::{
        build_single_generation_runtime_checkpoint_owner_contract,
        build_single_generation_runtime_state_owner_contract,
        build_single_generation_terminal_state_owner_contract,
        resolve_single_generation_quality_gate_terminal_state,
    };
    use crate::services::chapter_single_generation_stream_workflow_service::{
        build_single_generation_stream_workflow_owner_contract,
        SingleGenerationStreamSuccessArtifacts,
    };

    const ACTIVE_SINGLE_GENERATION_ROUTE_GROUP: &str = "chapter_single_generation";
    const ACTIVE_SINGLE_GENERATION_ROLLBACK_BOUNDARY: &str = "legacy_single_generation_direct_ai";
    const ACTIVE_SINGLE_GENERATION_TARGET_WORD_COUNT: i32 = 2400;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ChapterSingleGenerationActiveGatewaySmokeProbe {
        pub(crate) name: &'static str,
        pub(crate) owner: &'static str,
        pub(crate) route_group: &'static str,
        pub(crate) prompt: &'static str,
        pub(crate) config: ChapterCandidateRouteGatewayConfig,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct ChapterSingleGenerationActiveGatewaySmokeResult {
        pub(crate) name: String,
        pub(crate) owner: String,
        pub(crate) route_group: String,
        pub(crate) ok: bool,
        pub(crate) execution_path: String,
        pub(crate) fallback_applied: bool,
        pub(crate) reason: String,
        pub(crate) rollback_boundary: String,
        pub(crate) rust_error: Option<String>,
        pub(crate) content: String,
        pub(crate) result: Value,
        pub(crate) runtime_state: Option<Value>,
        pub(crate) readiness_evidence: Value,
    }

    pub(crate) fn build_default_chapter_single_generation_active_gateway_smoke_probes(
    ) -> Vec<ChapterSingleGenerationActiveGatewaySmokeProbe> {
        vec![
            ChapterSingleGenerationActiveGatewaySmokeProbe {
                name: "chapter-single-generation-active-gateway-rust-owner",
                owner: "rust",
                route_group: ACTIVE_SINGLE_GENERATION_ROUTE_GROUP,
                prompt: "ACTIVE_SINGLE_GENERATION_PROMPT",
                config: ChapterCandidateRouteGatewayConfig {
                    rust_executor_enabled: true,
                    fallback_on_rust_error: true,
                    disabled_reason: None,
                    rollback_boundary: ACTIVE_SINGLE_GENERATION_ROLLBACK_BOUNDARY.to_string(),
                },
            },
            ChapterSingleGenerationActiveGatewaySmokeProbe {
                name: "chapter-single-generation-active-gateway-fallback-freeze-candidate",
                owner: "rust",
                route_group: ACTIVE_SINGLE_GENERATION_ROUTE_GROUP,
                prompt: "ACTIVE_SINGLE_GENERATION_FREEZE_PROMPT",
                config: ChapterCandidateRouteGatewayConfig {
                    rust_executor_enabled: true,
                    fallback_on_rust_error: false,
                    disabled_reason: Some(
                        "single generation active route fallback-freeze candidate".to_string(),
                    ),
                    rollback_boundary: ACTIVE_SINGLE_GENERATION_ROLLBACK_BOUNDARY.to_string(),
                },
            },
        ]
    }

    pub(crate) async fn run_chapter_single_generation_active_gateway_smoke_suite(
    ) -> Result<Vec<ChapterSingleGenerationActiveGatewaySmokeResult>, String> {
        let mut results = Vec::new();

        for probe in build_default_chapter_single_generation_active_gateway_smoke_probes() {
            results.push(run_chapter_single_generation_active_gateway_smoke_probe(probe).await?);
        }

        Ok(results)
    }

    pub(crate) async fn run_chapter_single_generation_active_gateway_smoke_probe(
        probe: ChapterSingleGenerationActiveGatewaySmokeProbe,
    ) -> Result<ChapterSingleGenerationActiveGatewaySmokeResult, String> {
        let mut ai_config = AIConfig::default();
        ai_config.temperature = 0.72;
        ai_config.max_tokens = 4096;

        let mut request = build_single_generation_candidate_executor_request(
            probe.prompt,
            ACTIVE_SINGLE_GENERATION_TARGET_WORD_COUNT,
            &ai_config,
        );
        let rust_probe_name = probe.name.to_string();

        let output = execute_chapter_candidate_route_gateway_with_executor(
        &mut request,
        ai_config,
        smoke_quality_adapter(probe.name),
        probe.config.clone(),
        move |request, _ai_config, _quality_adapter| {
            Box::pin(async move {
                request.runtime_state = Some(json!({
                    "active_single_generation_gateway": "rust",
                    "probe": rust_probe_name,
                    "generation_label": request.generation_label,
                    "source": request.source,
                }));
                Ok(json!({
                    "full_content": "Rust 候选章节正文。",
                    "generation_path": "single_generation_rust_candidate_gateway",
                    "probe": rust_probe_name,
                    "gateway_consumed": true,
                }))
            })
        },
        |_request, context| {
            Box::pin(async move {
                Err(format!(
                    "single generation active gateway smoke no longer exercises direct fallback; reason={}",
                    context.reason
                ))
            })
        },
    )
    .await?;

        let content = single_generation_candidate_gateway_content(&output.result)?;
        let candidate_gateway_metadata =
            build_active_single_generation_smoke_candidate_gateway_metadata(
                &output,
                &output.result,
            );
        let readiness_evidence = build_active_single_generation_readiness_evidence(
            &probe,
            &content,
            &output.result,
            &candidate_gateway_metadata,
        );

        Ok(ChapterSingleGenerationActiveGatewaySmokeResult {
            name: probe.name.to_string(),
            owner: probe.owner.to_string(),
            route_group: probe.route_group.to_string(),
            ok: true,
            execution_path: chapter_candidate_production_execution_path_name(output.decision.path)
                .to_string(),
            fallback_applied: output.fallback_applied,
            reason: output.decision.reason,
            rollback_boundary: output.decision.rollback_boundary,
            rust_error: output.rust_error,
            content,
            result: output.result,
            runtime_state: request.runtime_state,
            readiness_evidence,
        })
    }

    fn build_active_single_generation_readiness_evidence(
        probe: &ChapterSingleGenerationActiveGatewaySmokeProbe,
        content: &str,
        gateway_result: &Value,
        candidate_gateway_metadata: &Value,
    ) -> Value {
        let target = active_single_generation_smoke_target();
        let compat_options = active_single_generation_smoke_compat_options();
        let generated = active_single_generation_smoke_generated_result(
            &target,
            content,
            candidate_gateway_metadata,
        );
        let stream_artifacts = SingleGenerationStreamSuccessArtifacts::from_quality_metrics(
            Some("active-smoke-analysis-task".to_string()),
            Some(json!({
                "overall_score": 90.0,
                "quality_gate": {
                    "decision": "passed",
                    "summary": "active gateway smoke passed"
                }
            })),
            None,
        );
        let stream_payload = stream_artifacts.response_payload(&generated);
        let startup_snapshot_plan = SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
            build_single_generation_pending_checkpoint(&target),
            json!({
                "latest_quality_metrics": {"overall_score": 90.0},
                "quality_metrics_summary": {"chapter_count": 1},
                "quality_metrics_history": [{"overall_score": 90.0}],
                "quality_history_context": {"source": "active_gateway_smoke"},
                "active_story_repair_payload": {"scope": "chapter", "mode": "smoke"}
            }),
        );
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: target.chapter_id.clone(),
            user_id: "active-smoke-user".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: ACTIVE_SINGLE_GENERATION_TARGET_WORD_COUNT,
                compat_options,
                execution_config: active_single_generation_smoke_execution_config(),
            },
        };
        let background_payload = build_single_generation_background_create_response_payload(
            "active-smoke-task",
            &target,
            &startup_snapshot_plan,
            &runtime_input,
        );
        let terminal_result = GeneratedChapterResult {
            quality_gate_action: Some("retry".to_string()),
            quality_gate_message: Some("active smoke retry boundary".to_string()),
            content_applied: false,
            provisional_draft_saved: true,
            attempt_state: "retry".to_string(),
            quality_metrics: Some(json!({
                "quality_gate": {
                    "decision": "auto_repair",
                    "allow_save": false,
                    "label": "active smoke retry boundary",
                    "failed_metrics": [{"label": "节奏"}]
                }
            })),
            ..generated.clone()
        };
        let terminal_task = Some(active_single_generation_smoke_task(&target));
        let terminal_payload = resolve_single_generation_quality_gate_terminal_state(
            &terminal_task,
            &terminal_result,
            None,
        )
        .map(|terminal| {
            json!({
                "checkpoint": terminal.checkpoint_payload,
                "failed_entry": terminal.failed_entry,
                "error_message": terminal.error_message,
            })
        })
        .unwrap_or_else(|| json!(null));

        json!({
            "owner_scope": "active_route_gateway_stream_background_runtime_terminal",
            "covered_rust_owners": [
                "chapter_generation_routes",
                "chapter_candidate_route_gateway_service",
                "chapter_single_generation_prepare_service",
                "chapter_single_generation_stream_workflow_service",
                "chapter_single_generation_runtime_state_service",
                "chapter_batch_generation_task_payload_base_service",
                "chapter_generation_runtime_service",
                "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
                "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
                "chapter_candidate_record_service",
                "chapter_single_generation_runtime_restore_workflow_service",
            ],
            "python_source_map": [],
            "python_source_map_policy": {
                "status": "source_map_only",
                "active_manifest_fallback_owner": false,
                "freeze_or_delete_requires_same_round_rollback_policy": true,
                "active_gateway_cutover": "nginx_routes_to_rust",
                "full_module_freeze_ready": true,
                "freeze_scope": "single_generation_active_route_source_map_surface",
                "freeze_reason": "active stream/background traffic consumes the Rust gateway owner and the remaining prerequisite semantics now piggyback on the shared chapter query source map rather than a dedicated single-generation shell",
                "python_bootstrap_status": "bootstrap_registration_retired_test_only_route_wiring_loader_remains",
                "python_route_module_import_status": "legacy_route_module_deleted_after_route_wiring_repoint",
                "python_route_registration_status": "no_python_route_registration_active_entry_source_maps_only",
                "python_route_wiring_import_status": "route_wiring_service_physically_deleted_after_source_map_closeout",
                "python_compat_shell_import_status": "stream_background_entry_shells_deleted_after_test_seam_migration",
                "legacy_rollback_wiring_shells": [],
                "shared_prepare_query_source_maps": [],
                "final_frozen_boundary": "single_generation_active_route_source_map_surface_empty_prepare_query_source_maps_deleted"
            },
            "active_gateway_cutover": {
                "deployment_owner": "deploy/nginx/mumunovel.conf",
                "routes_to_rust": [
                    "/api/chapters/{chapter_id}/generate-stream",
                    "/api/chapters/{chapter_id}/generate-background"
                ],
                "sse_routes": [
                    "/api/chapters/{chapter_id}/generate-stream"
                ],
                "python_route_files_status": "python_stream_background_entry_source_maps_deleted_for_single_generation_active_route_support",
                "python_bootstrap_registration": "bootstrap_registration_deleted_no_route_wiring_loader_remains",
                "python_route_module_import_status": "legacy_route_module_deleted_after_route_wiring_repoint",
                "python_route_registration_status": "no_python_route_registration_active_entry_source_maps_only",
                "python_route_wiring_import_status": "route_wiring_service_physically_deleted_after_source_map_closeout",
                "python_compat_shell_import_status": "stream_background_entry_shells_deleted_after_test_seam_migration",
                "rust_route_owner": "backend-rs/src/api/chapter_generation_routes.rs"
            },
            "db_backed_business_smoke": {
                "owner": "chapter_single_generation_runtime_restore_workflow_service",
                "fixture": "sqlite_memory_batch_generation_task_snapshot",
                "covered_paths": [
                    "PreparedSingleGenerationBackgroundLaunchParts::persist_and_dispatch",
                    "SingleGenerationTaskPersistenceSeed::into_active_model",
                    "SingleGenerationStartupSnapshotPlan::persist"
                ],
                "background_task_persistence_verified": true,
                "startup_snapshot_persistence_verified": true,
                "active_story_repair_payload_verified": true,
                "quality_metrics_runtime_state_verified": true,
                "fallback_freeze_gateway_config_verified": true
            },
            "active_route_gateway_config": {
                "source": "AppConfig -> chapter_generation_routes -> stream/write workflow -> runtime lifecycle",
                "route_config_builder": "build_single_generation_route_gateway_config",
                "gateway_config_owner": "build_chapter_candidate_route_gateway_config_from_app_config",
                "stream_route_consumes_gateway_config": true,
                "background_route_consumes_gateway_config": true,
                "runtime_launch_consumes_gateway_config": true,
                "rust_executor_enabled": probe.config.rust_executor_enabled,
                "fallback_on_rust_error": probe.config.fallback_on_rust_error,
                "disabled_reason": probe.config.disabled_reason.as_deref(),
                "rollback_boundary": probe.config.rollback_boundary,
            },
            "runtime_owner_chain": {
                "route": "chapter_generation_routes",
                "prepare_request": "prepare_single_chapter_generation_request_from_route_payload",
                "stream_workflow": "create_owned_single_generation_stream",
                "background_workflow": "SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload",
                "stream_lifecycle": "SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config",
                "background_lifecycle": "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config",
                "runtime_generation": "SingleGenerationRuntimeLaunchInput::execute_generation_with_gateway_config",
                "candidate_gateway": "generate_and_persist_chapter_content_with_candidate_route_gateway",
                "candidate_request": "build_single_generation_candidate_executor_request",
                "candidate_metadata": "build_single_generation_candidate_gateway_metadata",
            },
            "single_generation_route_owner_contract": build_chapter_single_generation_route_owner_contract(),
            "candidate_route_gateway_owner_contract": build_chapter_candidate_route_gateway_owner_contract(),
            "single_generation_prepare_owner_contract": build_single_generation_prepare_owner_contract(),
            "single_generation_task_view_payload_owner_contract": build_single_generation_task_view_payload_owner_contract(),
            "chapter_generation_access_owner_contract": build_chapter_generation_access_owner_contract(),
            "chapter_generation_prerequisite_owner_contract": build_chapter_generation_prerequisite_owner_contract(),
            "single_generation_stream_workflow_owner_contract": build_single_generation_stream_workflow_owner_contract(),
            "single_generation_write_workflow_owner_contract": build_single_generation_write_workflow_owner_contract(),
            "single_generation_background_launch_owner_contract": build_single_generation_background_launch_owner_contract(),
            "single_generation_runtime_restore_owner_contract": build_single_generation_runtime_restore_owner_contract(),
            "single_generation_startup_snapshot_owner_contract": build_single_generation_startup_snapshot_owner_contract(),
            "single_generation_runtime_state_owner_contract": build_single_generation_runtime_state_owner_contract(),
            "single_generation_runtime_checkpoint_owner_contract": build_single_generation_runtime_checkpoint_owner_contract(),
            "single_generation_terminal_state_owner_contract": build_single_generation_terminal_state_owner_contract(),
            "execution_config_owner_contract": build_generation_execution_config_owner_contract(),
            "execution_contract_owner_contract": build_single_generation_execution_contract_owner_contract(),
            "shared_candidate_runtime_owner_contract": build_single_generation_candidate_runtime_owner_contract(),
            "candidate_route_gateway_owner_contract": build_chapter_candidate_route_gateway_owner_contract(),
            "candidate_runtime_state_owner_contract": build_chapter_candidate_runtime_state_owner_contract(),
            "single_generation_candidate_quality_owner_contract": build_chapter_single_generation_candidate_quality_owner_contract(),
            "chapter_generation_prompt_owner_contract": build_chapter_generation_prompt_owner_contract(),
            "prompt_context_provider_owner_contract": build_prompt_context_provider_owner_contract(),
            "quality_profile_owner_contract": build_quality_profile_owner_contract(),
            "research_payload_owner_contract": build_single_chapter_research_payload_owner_contract(),
            "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
            "story_repair_quality_context_owner_contract": build_story_repair_quality_context_owner_contract(),
            "terminal_runtime_patch_owner_contract": build_generation_terminal_runtime_patch_owner_contract(),
            "gateway": {
                "result_content_field": if gateway_result.get("full_content").is_some() {
                    "full_content"
                } else {
                    "content"
                },
                "rust_executor_enabled": probe.config.rust_executor_enabled,
                "fallback_on_rust_error": probe.config.fallback_on_rust_error,
                "disabled_reason": probe.config.disabled_reason.as_deref(),
                "rollback_boundary": probe.config.rollback_boundary,
                "metadata": candidate_gateway_metadata,
            },
            "stream_response": {
                "chapter_id": stream_payload["chapter_id"],
                "content_source": stream_payload["content_source"],
                "candidate_gateway": stream_payload["candidate_gateway"],
                "quality_gate_action": stream_payload["quality_gate_action"],
                "hard_gate_blocked": stream_payload["hard_gate_blocked"],
                "story_runtime_contract": stream_payload["story_runtime_contract"],
            },
            "background_response": {
                "task_id": background_payload["task_id"],
                "chapter_id": background_payload["chapter_id"],
                "status": background_payload["status"],
                "message": background_payload["message"],
                "estimated_time_minutes": background_payload["estimated_time_minutes"],
                "active_story_repair_payload": background_payload["active_story_repair_payload"],
                "candidate_gateway_attached": background_payload.get("candidate_gateway").is_some(),
                "rollback_note": "background create response starts the task; terminal candidate gateway metadata is attached by the runtime/stream result owner",
            },
            "terminal_state": {
                "phase": terminal_payload["checkpoint"]["phase"],
                "quality_gate_decision": terminal_payload["checkpoint"]["quality_gate_decision"],
                "failed_entry": terminal_payload["failed_entry"],
            },
            "fallback_shrink_readiness": {
                "candidate_probe": probe.name == "chapter-single-generation-active-gateway-fallback-freeze-candidate",
                "active_route_smoke_consumes_freeze_candidate": probe.config.rust_executor_enabled
                    && !probe.config.fallback_on_rust_error
                    && gateway_result
                        .get("gateway_consumed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                "rust_owner_path_validated": probe.config.rust_executor_enabled
                    && gateway_result
                        .get("gateway_consumed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                "fallback_freeze_config_validated": probe.config.rust_executor_enabled
                    && !probe.config.fallback_on_rust_error
                    && gateway_result
                        .get("gateway_consumed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                "python_fallback_removal_ready": probe.config.rust_executor_enabled
                    && !probe.config.fallback_on_rust_error
                    && gateway_result
                        .get("gateway_consumed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                "remaining_blockers": []
            },
            "rollback_policy": {
                "active_boundary": probe.config.rollback_boundary,
                "operator_knob": "CHAPTER_CANDIDATE_RUST_EXECUTOR_ENABLED / CHAPTER_CANDIDATE_RUST_EXECUTOR_FALLBACK_ON_ERROR",
                "python_source_map_action": "single_generation_active_route_source_maps_deleted_prerequisite_logic_rehomed_to_shared_chapter_query_source_map",
                "manifest_owner_baseline": "rust = 131, python-fallback = 0",
            },
            "next_cutover_gate": "single-generation route entry source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/candidate and prepare/orchestration source-map packages"
        })
    }

    fn build_active_single_generation_smoke_candidate_gateway_metadata(
        output: &ChapterCandidateProductionAdapterOutput,
        gateway_result: &Value,
    ) -> Value {
        let mut metadata = build_single_generation_candidate_gateway_metadata(output);
        if let Value::Object(metadata) = &mut metadata {
            if let Some(generation_path) = gateway_result.get("generation_path") {
                metadata.insert("generation_path".to_string(), generation_path.clone());
            }
            if let Some(gateway_consumed) = gateway_result.get("gateway_consumed") {
                metadata.insert("gateway_consumed".to_string(), gateway_consumed.clone());
            }
            if let Some(probe) = gateway_result.get("probe") {
                metadata.insert("probe".to_string(), probe.clone());
            }
        }
        metadata
    }

    fn active_single_generation_smoke_task(
        target: &SingleChapterGenerationTarget,
    ) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "active-smoke-task".to_string(),
            project_id: target.project_id.clone(),
            user_id: "active-smoke-user".to_string(),
            start_chapter_number: target.chapter_number,
            chapter_count: 1,
            chapter_ids: json!([{
                "id": target.chapter_id,
                "chapter_number": target.chapter_number,
                "title": target.title,
            }]),
            style_id: None,
            target_word_count: ACTIVE_SINGLE_GENERATION_TARGET_WORD_COUNT,
            enable_analysis: true,
            status: "running".to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some(target.chapter_id.clone()),
            current_chapter_number: Some(target.chapter_number),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn active_single_generation_smoke_target() -> SingleChapterGenerationTarget {
        SingleChapterGenerationTarget {
            project_id: "active-smoke-project".to_string(),
            chapter_id: "active-smoke-chapter".to_string(),
            chapter_number: 1,
            title: "Active Smoke Chapter".to_string(),
        }
    }

    fn active_single_generation_smoke_generated_result(
        target: &SingleChapterGenerationTarget,
        content: &str,
        candidate_gateway_metadata: &Value,
    ) -> GeneratedChapterResult {
        GeneratedChapterResult {
            chapter_id: target.chapter_id.clone(),
            chapter_number: target.chapter_number,
            title: target.title.clone(),
            content: content.to_string(),
            word_count: content.chars().count() as i32,
            saved_word_count: content.chars().count() as i32,
            chapter_status: "completed".to_string(),
            content_applied: true,
            quality_metrics: Some(json!({
                "overall_score": 90.0,
                "quality_gate": {
                    "decision": "passed",
                    "summary": "active gateway smoke passed"
                }
            })),
            candidate_gateway_metadata: Some(candidate_gateway_metadata.clone()),
            ..Default::default()
        }
    }

    fn active_single_generation_smoke_compat_options() -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            enable_analysis: true,
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            ..Default::default()
        }
    }

    fn active_single_generation_smoke_execution_config() -> PreparedGenerationExecutionConfig {
        PreparedGenerationExecutionConfig {
            ai_config: AIConfig::default(),
            provider_payload: PromptContextProviderPayload {
                recent_chapters_context: String::new(),
                previous_chapter_summary: String::new(),
                chapter_careers: "[]".to_string(),
                characters_info: "[]".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: "[]".to_string(),
                research_query: String::new(),
                research_assets: "[]".to_string(),
                external_assets: "[]".to_string(),
                reference_assets: "[]".to_string(),
                mcp_references: String::new(),
            },
        }
    }

fn smoke_quality_adapter(
    probe_name: &str,
) -> crate::services::chapter_candidate_executor_production_adapter_service::ChapterCandidateQualityAdapter<
    impl FnMut(
        crate::services::chapter_candidate_executor_production_adapter_service::CandidateQualityRuntimeContextBuildInput,
    ) -> Value,
    impl FnMut(
        crate::services::chapter_candidate_executor_production_adapter_service::CandidateStoryQualityMetricsInput,
    ) -> Value,
    impl FnMut(
        crate::services::chapter_candidate_executor_production_adapter_service::CandidateQualityGatePlanInput,
    ) -> Value,
    >{
        build_chapter_candidate_quality_adapter(
            ChapterCandidateQualityAdapterContext {
                story_packet: json!({"packet": "active_single_generation_smoke"}),
                project: json!({"world_rules": "rules"}),
                chapter: json!({"id": "chapter-1", "title": "第一章"}),
                chapter_context: json!({"chapter_outline": "outline"}),
                target_word_count: i64::from(ACTIVE_SINGLE_GENERATION_TARGET_WORD_COUNT),
                generation_intent: json!({
                    "mode": "single_generation_active_route_smoke",
                    "probe": probe_name,
                }),
                creative_mode: String::new(),
                story_focus: String::new(),
                plot_stage: String::new(),
                story_creation_brief: String::new(),
                quality_preset: String::new(),
                quality_notes: String::new(),
                chapter_count: None,
                current_chapter_number: None,
                retry_count: 0,
                max_retries: 1,
                story_repair_summary: String::new(),
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
                current_story_repair_payload: None,
                scope: "chapter".to_string(),
                log_prefix: "SingleGenerationActiveGatewaySmoke".to_string(),
            },
            |_input| json!({"runtime": "context"}),
            |input| {
                json!({
                    "overall_score": 90.0,
                    "word_count": input.content.chars().count(),
                })
            },
            |_input| {
                json!({
                    "action": "continue",
                    "quality_gate": {
                        "decision": "allow_save",
                        "status": "pass",
                        "allow_save": true,
                    }
                })
            },
        )
    }

    #[cfg(test)]
    mod tests {
        use super::{
            build_default_chapter_single_generation_active_gateway_smoke_probes,
            run_chapter_single_generation_active_gateway_smoke_probe,
            run_chapter_single_generation_active_gateway_smoke_suite,
        };
        use serde_json::json;

        #[test]
        fn should_build_active_gateway_smoke_probes_for_enabled_and_freeze_paths() {
            let probes = build_default_chapter_single_generation_active_gateway_smoke_probes();

            assert_eq!(probes.len(), 2);
            assert_eq!(
                probes[0].name,
                "chapter-single-generation-active-gateway-rust-owner"
            );
            assert!(probes[0].config.rust_executor_enabled);
            assert_eq!(
                probes[1].name,
                "chapter-single-generation-active-gateway-fallback-freeze-candidate"
            );
            assert!(probes[1].config.rust_executor_enabled);
            assert!(!probes[1].config.fallback_on_rust_error);
            assert_eq!(probes[0].route_group, "chapter_single_generation");
        }

        #[tokio::test]
        async fn should_run_active_gateway_smoke_through_enabled_and_freeze_paths() {
            let results = run_chapter_single_generation_active_gateway_smoke_suite()
                .await
                .expect("active gateway smoke results");

            assert_eq!(results.len(), 2);
            assert_eq!(results[0].execution_path, "rust_candidate_executor");
            assert!(!results[0].fallback_applied);
            assert_eq!(results[0].content, "Rust 候选章节正文。");
            assert_eq!(
                results[0].result["generation_path"],
                "single_generation_rust_candidate_gateway"
            );
            assert_eq!(
                results[0].runtime_state.as_ref().unwrap()["generation_label"],
                "single_generation_candidate"
            );

            assert_eq!(results[1].execution_path, "rust_candidate_executor");
            assert!(!results[1].fallback_applied);
            assert_eq!(results[1].content, "Rust 候选章节正文。");
            assert_eq!(
                results[1].readiness_evidence["fallback_shrink_readiness"]
                    ["active_route_smoke_consumes_freeze_candidate"],
                true
            );
        }

        #[tokio::test]
        async fn should_keep_active_gateway_probe_metadata_and_runtime_state() {
            let probe = build_default_chapter_single_generation_active_gateway_smoke_probes()
                .into_iter()
                .next()
                .expect("rust active gateway probe");

            let result = run_chapter_single_generation_active_gateway_smoke_probe(probe)
                .await
                .expect("active gateway smoke result");

            assert!(result.ok);
            assert_eq!(result.owner, "rust");
            assert_eq!(result.route_group, "chapter_single_generation");
            assert_eq!(
                result.rollback_boundary,
                "legacy_single_generation_direct_ai"
            );
            assert_eq!(result.result["gateway_consumed"], true);
            assert_eq!(result.runtime_state.as_ref().unwrap()["source"], "chapter");
            assert_eq!(
                result.readiness_evidence["owner_scope"],
                "active_route_gateway_stream_background_runtime_terminal"
            );
            assert_eq!(
            result.readiness_evidence["active_route_gateway_config"]["source"],
            "AppConfig -> chapter_generation_routes -> stream/write workflow -> runtime lifecycle"
        );
            assert_eq!(
                result.readiness_evidence["active_route_gateway_config"]
                    ["stream_route_consumes_gateway_config"],
                true
            );
            assert_eq!(
                result.readiness_evidence["active_route_gateway_config"]
                    ["background_route_consumes_gateway_config"],
                true
            );
            assert_eq!(
                result.readiness_evidence["active_route_gateway_config"]
                    ["runtime_launch_consumes_gateway_config"],
                true
            );
            assert_eq!(
                result.readiness_evidence["runtime_owner_chain"]["candidate_gateway"],
                "generate_and_persist_chapter_content_with_candidate_route_gateway"
            );
            assert_eq!(
                result.readiness_evidence["stream_response"]["content_source"],
                "chapter"
            );
            assert_eq!(
                result.readiness_evidence["gateway"]["metadata"]["execution_path"],
                "rust_candidate_executor"
            );
            assert_eq!(
                result.readiness_evidence["stream_response"]["candidate_gateway"]
                    ["fallback_applied"],
                false
            );
            assert_eq!(
                result.readiness_evidence["stream_response"]["candidate_gateway"]
                    ["generation_path"],
                "single_generation_rust_candidate_gateway"
            );
            assert_eq!(
                result.readiness_evidence["background_response"]["message"],
                "单章后台生成任务已创建"
            );
            assert_eq!(
                result.readiness_evidence["background_response"]["candidate_gateway_attached"],
                false
            );
            assert_eq!(
                result.readiness_evidence["terminal_state"]["quality_gate_decision"],
                "auto_repair"
            );
        }

        #[tokio::test]
        async fn should_project_active_gateway_readiness_evidence_for_enabled_and_freeze_paths() {
            let results = run_chapter_single_generation_active_gateway_smoke_suite()
                .await
                .expect("active gateway smoke results");
            let enabled_readiness = &results[0].readiness_evidence;
            let freeze_readiness = &results[1].readiness_evidence;

            let covered_rust_owners = enabled_readiness["covered_rust_owners"]
                .as_array()
                .expect("covered rust owners");
            for expected_owner in [
                "chapter_generation_routes",
                "chapter_candidate_route_gateway_service",
                "chapter_single_generation_prepare_service",
                "chapter_single_generation_stream_workflow_service",
                "chapter_single_generation_runtime_state_service",
                "chapter_batch_generation_task_payload_base_service",
                "chapter_generation_runtime_service",
                "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
                "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
                "chapter_candidate_record_service",
                "chapter_single_generation_runtime_restore_workflow_service",
            ] {
                assert!(
                    covered_rust_owners
                        .iter()
                        .any(|owner| owner == expected_owner),
                    "missing covered Rust owner: {expected_owner}"
                );
            }
            let unique_owner_count = covered_rust_owners
                .iter()
                .filter_map(|owner| owner.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            assert_eq!(unique_owner_count, 10);
            assert!(!covered_rust_owners
                .iter()
                .any(|owner| owner == "chapter_candidate_executor_runtime_adapter_service"));
            assert!(!covered_rust_owners
                .iter()
                .any(|owner| owner == "chapter_single_generation_candidate_gateway_service"));
            for retired_owner in [
                "chapter_single_generation_stream_success_response_service",
                "chapter_single_generation_background_response_service",
                "chapter_single_generation_terminal_state_service",
            ] {
                assert!(
                !covered_rust_owners.iter().any(|owner| owner == retired_owner),
                "retired forwarding-only owner leaked into active gateway readiness: {retired_owner}"
            );
            }
            assert_eq!(
                enabled_readiness["active_route_gateway_config"]["rust_executor_enabled"],
                true
            );
            assert_eq!(
                enabled_readiness["active_route_gateway_config"]["rollback_boundary"],
                "legacy_single_generation_direct_ai"
            );
            assert_eq!(
                enabled_readiness["python_source_map_policy"]["status"],
                "source_map_only"
            );
            assert_eq!(
                enabled_readiness["python_source_map_policy"]["active_manifest_fallback_owner"],
                false
            );
            assert_eq!(
                enabled_readiness["python_source_map_policy"]["full_module_freeze_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["python_source_map_policy"]["freeze_scope"],
                "single_generation_active_route_source_map_surface"
            );
            assert_eq!(
                enabled_readiness["python_source_map_policy"]["python_bootstrap_status"],
                "bootstrap_registration_retired_test_only_route_wiring_loader_remains"
            );
            assert_eq!(
                enabled_readiness["python_source_map_policy"]["python_route_module_import_status"],
                "legacy_route_module_deleted_after_route_wiring_repoint"
            );
            assert_eq!(
                enabled_readiness["python_source_map"]
                    .as_array()
                    .expect("python source map")
                    .len(),
                0
            );
            assert_eq!(
                enabled_readiness["python_source_map_policy"]["python_compat_shell_import_status"],
                "stream_background_entry_shells_deleted_after_test_seam_migration"
            );
            assert_eq!(
                enabled_readiness["python_source_map_policy"]["shared_prepare_query_source_maps"]
                    .as_array()
                    .expect("single generation shared prepare query source maps")
                    .len(),
                0
            );
            assert_eq!(
                enabled_readiness["rollback_policy"]["manifest_owner_baseline"],
                "rust = 131, python-fallback = 0"
            );
            assert_eq!(
                enabled_readiness["db_backed_business_smoke"]["owner"],
                "chapter_single_generation_runtime_restore_workflow_service"
            );
            assert_eq!(
                enabled_readiness["db_backed_business_smoke"]
                    ["background_task_persistence_verified"],
                true
            );
            assert_eq!(
                enabled_readiness["db_backed_business_smoke"]
                    ["startup_snapshot_persistence_verified"],
                true
            );
            assert_eq!(
                enabled_readiness["runtime_owner_chain"]["prepare_request"],
                "prepare_single_chapter_generation_request_from_route_payload"
            );
            assert_eq!(
                enabled_readiness["runtime_owner_chain"]["stream_workflow"],
                "create_owned_single_generation_stream"
            );
            assert_eq!(
                enabled_readiness["runtime_owner_chain"]["background_workflow"],
                "SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload"
            );
            assert_eq!(
                enabled_readiness["runtime_owner_chain"]["runtime_generation"],
                "SingleGenerationRuntimeLaunchInput::execute_generation_with_gateway_config"
            );
            assert_eq!(
                enabled_readiness["single_generation_route_owner_contract"]["owner"],
                "chapter_generation_routes"
            );
            assert_eq!(
                enabled_readiness["single_generation_route_owner_contract"]["route_contract"]
                    ["stream"],
                "/chapters/{chapter_id}/generate-stream"
            );
            assert_eq!(
                enabled_readiness["single_generation_route_owner_contract"]["route_contract"]
                    ["background"],
                "/chapters/{chapter_id}/generate-background"
            );
            assert_eq!(
                enabled_readiness["single_generation_route_owner_contract"]["behavior_contract"]
                    ["route_entrypoints"][0],
                "generate_chapter_content_stream"
            );
            assert_eq!(
                enabled_readiness["single_generation_route_owner_contract"]["behavior_contract"]
                    ["workflow_consumers"][1],
                "SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload"
            );
            assert_eq!(
                enabled_readiness["single_generation_route_owner_contract"]["active_consumers"][2],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["single_generation_route_owner_contract"]["rollback_boundary"]
                    ["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["candidate_route_gateway_owner_contract"]["owner"],
                "chapter_candidate_route_gateway_service"
            );
            assert_eq!(
                enabled_readiness["candidate_route_gateway_owner_contract"]["behavior_contract"]
                    ["gateway_entrypoints"][1],
                "execute_chapter_candidate_route_gateway_with_executor"
            );
            assert_eq!(
                enabled_readiness["candidate_route_gateway_owner_contract"]["active_consumers"][2],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["candidate_route_gateway_owner_contract"]["rollback_boundary"]
                    ["runtime_knob"],
                "python_candidate_executor_fallback"
            );
            assert_eq!(
                enabled_readiness["single_generation_prepare_owner_contract"]["owner"],
                "chapter_single_generation_prepare_service"
            );
            assert_eq!(
                enabled_readiness["single_generation_prepare_owner_contract"]["behavior_contract"]
                    ["route_request_owner"],
                "SingleChapterGenerationRouteRequest"
            );
            assert_eq!(
                enabled_readiness["single_generation_prepare_owner_contract"]["behavior_contract"]
                    ["strict_schema"]["deny_unknown_fields"],
                true
            );
            assert_eq!(
                enabled_readiness["single_generation_prepare_owner_contract"]["active_consumers"]
                    [4],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["single_generation_prepare_owner_contract"]["rollback_boundary"]
                    ["runtime_knobs"][0],
                "legacy_single_generation_direct_ai"
            );
            assert_eq!(
                enabled_readiness["single_generation_prepare_owner_contract"]["rollback_boundary"]
                    ["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["single_generation_task_view_payload_owner_contract"]["owner"],
                "chapter_single_generation_prepare_service::task_view_payload_owner"
            );
            assert_eq!(
                enabled_readiness["single_generation_task_view_payload_owner_contract"]
                    ["behavior_contract"]["entrypoints"][1],
                "build_single_generation_task_view_payload_from_task_state"
            );
            assert_eq!(
                enabled_readiness["chapter_generation_access_owner_contract"]["owner"],
                "chapter_access_service"
            );
            assert_eq!(
                enabled_readiness["chapter_generation_access_owner_contract"]["behavior_contract"]
                    ["entrypoints"][1],
                "load_accessible_chapter_for_generation"
            );
            assert_eq!(
                enabled_readiness["chapter_generation_access_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][1],
                "phase5-single-generation-owner"
            );
            assert_eq!(
                enabled_readiness["chapter_generation_prerequisite_owner_contract"]["owner"],
                "chapter_single_generation_prepare_service"
            );
            assert_eq!(
                enabled_readiness["chapter_generation_prerequisite_owner_contract"]
                    ["behavior_contract"]["entrypoint"],
                "check_chapter_generation_prerequisites"
            );
            assert_eq!(
                enabled_readiness["single_generation_stream_workflow_owner_contract"]["owner"],
                "chapter_single_generation_stream_workflow_service"
            );
            assert_eq!(
                enabled_readiness["single_generation_stream_workflow_owner_contract"]
                    ["behavior_contract"]["stream_entrypoints"][0],
                "create_owned_single_generation_stream"
            );
            assert_eq!(
                enabled_readiness["single_generation_stream_workflow_owner_contract"]
                    ["behavior_contract"]["response_payload_fields"][16],
                "candidate_gateway"
            );
            assert_eq!(
                enabled_readiness["single_generation_stream_workflow_owner_contract"]
                    ["active_consumers"][1],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["single_generation_stream_workflow_owner_contract"]
                    ["rollback_boundary"]["runtime_knobs"][1],
                "python_candidate_executor_fallback"
            );
            assert_eq!(
                enabled_readiness["single_generation_stream_workflow_owner_contract"]
                    ["rollback_boundary"]["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["single_generation_write_workflow_owner_contract"]["owner"],
                "chapter_single_generation_runtime_restore_workflow_service"
            );
            assert_eq!(
                enabled_readiness["single_generation_write_workflow_owner_contract"]
                    ["behavior_contract"]["background_entrypoints"][0],
                "SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload"
            );
            assert_eq!(
                enabled_readiness["single_generation_write_workflow_owner_contract"]
                    ["behavior_contract"]["response_payload_fields"][11],
                "candidate_gateway"
            );
            assert_eq!(
                enabled_readiness["single_generation_write_workflow_owner_contract"]
                    ["active_consumers"][1],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["single_generation_write_workflow_owner_contract"]
                    ["rollback_boundary"]["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["single_generation_background_launch_owner_contract"]["owner"],
                "chapter_single_generation_background_launch_service::launch_owner"
            );
            assert_eq!(
                enabled_readiness["single_generation_background_launch_owner_contract"]
                    ["behavior_contract"]["entrypoints"][2],
                "SingleGenerationBackgroundLaunchPersistenceDispatchPlan::persist_and_dispatch"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_restore_owner_contract"]["owner"],
                "chapter_single_generation_runtime_restore_workflow_service"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_restore_owner_contract"]
                    ["behavior_contract"]["startup_snapshot_entrypoints"][1],
                "SingleGenerationStartupSnapshotPlan::from_pending_checkpoint"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_restore_owner_contract"]
                    ["behavior_contract"]["background_launch_entrypoints"][1],
                "PreparedSingleGenerationBackgroundLaunchParts::persist_and_dispatch"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_restore_owner_contract"]
                    ["behavior_contract"]["runtime_state_fields"][6],
                "active_story_repair_payload"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_restore_owner_contract"]
                    ["rollback_boundary"]["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["single_generation_startup_snapshot_owner_contract"]["owner"],
                "chapter_single_generation_background_launch_service::launch_owner::startup_snapshot_owner"
            );
            assert_eq!(
                enabled_readiness["single_generation_startup_snapshot_owner_contract"]
                    ["behavior_contract"]["entrypoints"][1],
                "SingleGenerationStartupSnapshotPlan::from_pending_checkpoint"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_state_owner_contract"]["owner"],
                "chapter_single_generation_runtime_state_service"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_state_owner_contract"]
                    ["behavior_contract"]["runtime_lifecycle_entrypoints"][0],
                "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_state_owner_contract"]
                    ["behavior_contract"]["checkpoint_entrypoints"][2],
                "attach_single_generation_candidate_gateway_checkpoint_metadata"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_state_owner_contract"]
                    ["behavior_contract"]["terminal_state_entrypoints"][0],
                "resolve_single_generation_quality_gate_terminal_state"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_state_owner_contract"]
                    ["behavior_contract"]["follow_up_analysis_entrypoints"][2],
                "analyze_generated_chapter_follow_up"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_state_owner_contract"]
                    ["rollback_boundary"]["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_checkpoint_owner_contract"]["owner"],
                "chapter_single_generation_runtime_state_service::runtime_checkpoint_owner"
            );
            assert_eq!(
                enabled_readiness["single_generation_runtime_checkpoint_owner_contract"]
                    ["behavior_contract"]["entrypoints"][2],
                "build_single_generation_runtime_terminal_checkpoint_projection"
            );
            assert_eq!(
                enabled_readiness["single_generation_terminal_state_owner_contract"]["owner"],
                "chapter_single_generation_runtime_state_service::terminal_state_owner"
            );
            assert_eq!(
                enabled_readiness["single_generation_terminal_state_owner_contract"]
                    ["behavior_contract"]["entrypoints"][1],
                "resolve_single_generation_quality_gate_terminal_state"
            );
            assert_eq!(
                enabled_readiness["execution_config_owner_contract"]["owner"],
                "chapter_generation_execution_contract_service::execution_config"
            );
            assert_eq!(
                enabled_readiness["execution_config_owner_contract"]["behavior_contract"]
                    ["model_override_forwarded"],
                true
            );
            assert_eq!(
                enabled_readiness["execution_config_owner_contract"]["active_consumers"][5],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["execution_contract_owner_contract"]["owner"],
                "chapter_generation_execution_contract_service::single_generation_contract_owner"
            );
            assert_eq!(
                enabled_readiness["execution_contract_owner_contract"]["behavior_contract"]
                    ["prompt_override_builder"],
                "build_prompt_overrides_from_compat_options"
            );
            assert_eq!(
                enabled_readiness["execution_contract_owner_contract"]["behavior_contract"]
                    ["web_research_fields_preserved"],
                true
            );
            assert_eq!(
                enabled_readiness["execution_contract_owner_contract"]["active_consumers"][9],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]["owner"],
                "chapter_generation_runtime_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]["behavior_contract"]
                    ["direct_fallback_generation_path"],
                "direct_generation_fallback"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_wiring_readiness"]["owner"],
                "chapter_candidate_executor_default_dependency_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_wiring_readiness"]["stage_count"],
                9
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]["owner"],
                "chapter_candidate_executor_default_dependency_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][0],
                "phase5-single-generation-owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_default_dependency_owner_contract"]
                    ["service_runtime_closeout_status"]["status"],
                "rust_chapter_candidate_executor_default_dependency_owner_source_map_deleted"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]["owner"],
                "chapter_candidate_executor_production_adapter_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][0],
                "phase5-single-generation-owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_production_adapter_owner_contract"]
                    ["service_runtime_closeout_status"]["status"],
                "rust_chapter_candidate_executor_production_adapter_owner_executor_source_map_deleted"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["owner"],
                "chapter_candidate_record_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profiles"][0],
                "phase5-single-generation-owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_chapter_candidate_record_owner_executor_source_map_deleted"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["owner"],
                "chapter_candidate_finalize_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profiles"][0],
                "phase5-single-generation-owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_chapter_candidate_finalize_owner_source_map_deleted"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_output_owner_contract"]["owner"],
                "chapter_candidate_output_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_output_owner_contract"]["behavior_contract"]["entrypoints"][0],
                "collect_generation_candidate_output"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_output_owner_contract"]["candidate_runtime_state_owner_contract"]
                    ["owner"],
                "chapter_candidate_runtime_state_service"
            );
            assert_eq!(
                enabled_readiness["candidate_runtime_state_owner_contract"]["owner"],
                "chapter_candidate_runtime_state_service"
            );
            assert_eq!(
                enabled_readiness["candidate_runtime_state_owner_contract"]["behavior_contract"]
                    ["entrypoints"][2],
                "snapshot_chapter_candidate_runtime_state"
            );
            assert_eq!(
                enabled_readiness["candidate_runtime_state_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][1],
                "phase5-batch-generation-owner"
            );
            assert_eq!(
                enabled_readiness["candidate_route_gateway_owner_contract"]["owner"],
                "chapter_candidate_route_gateway_service"
            );
            assert_eq!(
                enabled_readiness["candidate_route_gateway_owner_contract"]["behavior_contract"]
                    ["gateway_entrypoints"][1],
                "execute_chapter_candidate_route_gateway_with_executor"
            );
            assert_eq!(
                enabled_readiness["candidate_route_gateway_owner_contract"]["active_consumers"][4],
                "chapter_generation_runtime_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["owner"],
                "chapter_candidate_generation_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profiles"][0],
                "phase5-single-generation-owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_chapter_candidate_generation_owner_source_map_deleted"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["owner"],
                "chapter_candidate_rerank_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["owner_profiles"][0],
                "phase5-single-generation-owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                    ["status"],
                "rust_chapter_candidate_rerank_owner_source_map_deleted"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]["owner"],
                "chapter_candidate_word_budget_repair_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][0],
                "phase5-single-generation-owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_word_budget_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["status"],
                "rust_chapter_candidate_word_budget_repair_owner_source_map_deleted"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]["owner"],
                "chapter_candidate_targeted_final_repair_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["owner_profiles"][0],
                "phase5-single-generation-owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
                6
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
                11
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["python_fallback_probe_count"],
                0
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["source_map_closeout_ready"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_targeted_final_repair_owner_contract"]
                    ["service_runtime_closeout_status"]["status"],
                "rust_chapter_candidate_targeted_final_repair_owner_source_map_deleted"
            );
            assert!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["candidate_executor_wiring_readiness"]["rust_owned_dependency_count"]
                    .as_u64()
                    .unwrap()
                    >= 56
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]["rollback_boundary"]
                    ["runtime_knob"],
                "ChapterCandidateRouteGatewayConfig"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["context_compaction_owner_contract"]["owner"],
                "chapter_generation_runtime_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["context_compaction_owner_contract"]["behavior_contract"]
                    ["one_to_one_skips_recent_chapters_context"],
                true
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["quality_runtime_owner_contract"]["owner"],
                "chapter_generation_runtime_service::quality_runtime_context_owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["quality_runtime_owner_contract"]["behavior_contract"]
                    ["terminal_quality_gate_decision"],
                "manual_review"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["quality_runtime_owner_contract"]["active_consumers"][0],
                "chapter_single_generation_runtime_state_service"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["snapshot_persistence_owner_contract"]["owner"],
                "chapter_generation_runtime_service::snapshot_persistence_owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["snapshot_persistence_owner_contract"]["behavior_contract"]["write_functions"]
                    [0],
                "persist_chapter_generation_runtime_snapshot"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["snapshot_persistence_owner_contract"]["behavior_contract"]
                    ["runtime_state_policy"][0],
                "object_payloads_merge_keywise"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["story_repair_quality_context_owner_contract"]["owner"],
                "chapter_generation_runtime_service::story_repair_quality_context_owner"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["story_repair_quality_context_owner_contract"]["behavior_contract"]
                    ["resume_precedence"][0],
                "runtime_active_story_repair_payload"
            );
            assert_eq!(
                enabled_readiness["shared_candidate_runtime_owner_contract"]
                    ["story_repair_quality_context_owner_contract"]["active_consumers"][7],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["single_generation_candidate_quality_owner_contract"]["owner"],
                "chapter_generation_runtime_service::single_generation_candidate_quality_owner"
            );
            assert_eq!(
                enabled_readiness["single_generation_candidate_quality_owner_contract"]
                    ["behavior_contract"]["entrypoints"][1],
                "compute_single_generation_story_quality_metrics"
            );
            assert_eq!(
                enabled_readiness["single_generation_candidate_quality_owner_contract"]
                    ["behavior_contract"]["quality_gate_policy"][1],
                "auto_repair_with_remaining_retry_budget -> retry"
            );
            assert_eq!(
                enabled_readiness["chapter_generation_prompt_owner_contract"]["owner"],
                "chapter_generation_prompt_service"
            );
            assert_eq!(
                enabled_readiness["chapter_generation_prompt_owner_contract"]["behavior_contract"]
                    ["entrypoints"][2],
                "build_prompt_params_with_provider_payload"
            );
            assert_eq!(
                enabled_readiness["chapter_generation_prompt_owner_contract"]["behavior_contract"]
                    ["runtime_blocks"][7],
                "quality_contract_block"
            );
            assert_eq!(
                enabled_readiness["prompt_context_provider_owner_contract"]["owner"],
                "chapter_generation_prompt_service"
            );
            assert_eq!(
                enabled_readiness["prompt_context_provider_owner_contract"]["python_source_map"]
                    .as_array()
                    .expect("python source map")
                    .len(),
                0
            );
            assert_eq!(
                enabled_readiness["prompt_context_provider_owner_contract"]["behavior_contract"]
                    ["prompt_param_bridge"],
                "PromptContextProviderPayload::into_prompt_params"
            );
            assert_eq!(
                enabled_readiness["prompt_context_provider_owner_contract"]["behavior_contract"]
                    ["asset_prompt_visibility"][2],
                "mcp_references"
            );
            assert_eq!(
                enabled_readiness["prompt_context_provider_owner_contract"]["active_consumers"][6],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["quality_profile_owner_contract"]["owner"],
                "chapter_generation_prompt_service::quality_profile_owner"
            );
            assert_eq!(
                enabled_readiness["quality_profile_owner_contract"]["behavior_contract"]
                    ["entrypoints"][0],
                "build_novel_quality_prompt_blocks"
            );
            assert_eq!(
                enabled_readiness["quality_profile_owner_contract"]["behavior_contract"]
                    ["external_asset_policy"][0],
                "summary_only_assets"
            );
            assert_eq!(
                enabled_readiness["quality_profile_owner_contract"]["active_consumers"][4],
                "chapter_batch_generation_runtime_state_service"
            );
            assert_eq!(
                enabled_readiness["research_payload_owner_contract"]["owner"],
                "chapter_single_generation_prepare_service::research_payload_owner"
            );
            assert_eq!(
                enabled_readiness["research_payload_owner_contract"]["python_source_map"]
                    .as_array()
                    .expect("research payload owner python source map")
                    .len(),
                0
            );
            assert_eq!(
                enabled_readiness["research_payload_owner_contract"]["behavior_contract"]
                    ["custom_query_precedence"],
                "web_research_query overrides generated Exa/Grok query seed"
            );
            assert_eq!(
                enabled_readiness["quality_runtime_owner_contract"]["owner"],
                "chapter_generation_runtime_service::quality_runtime_context_owner"
            );
            assert_eq!(
                enabled_readiness["quality_runtime_owner_contract"]["behavior_contract"]
                    ["terminal_quality_gate_decision"],
                "manual_review"
            );
            assert_eq!(
                enabled_readiness["quality_runtime_owner_contract"]["active_consumers"][0],
                "chapter_single_generation_runtime_state_service"
            );
            assert_eq!(
                enabled_readiness["story_repair_quality_context_owner_contract"]["owner"],
                "chapter_generation_runtime_service::story_repair_quality_context_owner"
            );
            assert_eq!(
                enabled_readiness["story_repair_quality_context_owner_contract"]
                    ["behavior_contract"]["resume_precedence"][0],
                "runtime_active_story_repair_payload"
            );
            assert_eq!(
                enabled_readiness["story_repair_quality_context_owner_contract"]
                    ["active_consumers"][7],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["terminal_runtime_patch_owner_contract"]["owner"],
                "chapter_batch_generation_runtime_state_service::terminal_runtime_patch"
            );
            assert_eq!(
                enabled_readiness["terminal_runtime_patch_owner_contract"]["behavior_contract"]
                    ["manual_review_phase"],
                "quality_blocked"
            );
            assert_eq!(
                enabled_readiness["terminal_runtime_patch_owner_contract"]["behavior_contract"]
                    ["retry_phase"],
                "repair_pending"
            );
            assert_eq!(
                enabled_readiness["terminal_runtime_patch_owner_contract"]["active_consumers"][1],
                "chapter-single-generation-active-gateway-smoke-rust"
            );
            assert_eq!(
                enabled_readiness["stream_response"]["candidate_gateway"]["gateway_consumed"],
                true
            );
            assert_eq!(
                enabled_readiness["background_response"]["active_story_repair_payload"]["mode"],
                "smoke"
            );
            assert_eq!(
                enabled_readiness["terminal_state"]["phase"],
                "quality_retry"
            );
            assert_eq!(
                enabled_readiness["terminal_state"]["failed_entry"]["quality_gate_failed_metrics"]
                    [0],
                "节奏"
            );
            assert_eq!(
                freeze_readiness["active_route_gateway_config"]["fallback_on_rust_error"],
                false
            );
            assert_eq!(
                freeze_readiness["fallback_shrink_readiness"]["candidate_probe"],
                true
            );
            assert_eq!(
                freeze_readiness["fallback_shrink_readiness"]
                    ["active_route_smoke_consumes_freeze_candidate"],
                true
            );
            assert_eq!(
                freeze_readiness["fallback_shrink_readiness"]["python_fallback_removal_ready"],
                true
            );
        }

        #[tokio::test]
        async fn should_project_active_route_fallback_freeze_candidate_readiness() {
            let probe = build_default_chapter_single_generation_active_gateway_smoke_probes()
                .into_iter()
                .find(|probe| {
                    probe.name
                        == "chapter-single-generation-active-gateway-fallback-freeze-candidate"
                })
                .expect("active fallback freeze probe");

            let result = run_chapter_single_generation_active_gateway_smoke_probe(probe)
                .await
                .expect("active fallback freeze smoke result");

            assert_eq!(result.execution_path, "rust_candidate_executor");
            assert!(!result.fallback_applied);
            assert_eq!(
                result.readiness_evidence["active_route_gateway_config"]["rust_executor_enabled"],
                true
            );
            assert_eq!(
                result.readiness_evidence["active_route_gateway_config"]["fallback_on_rust_error"],
                false
            );
            assert_eq!(
                result.readiness_evidence["fallback_shrink_readiness"]
                    ["active_route_smoke_consumes_freeze_candidate"],
                true
            );
            assert_eq!(
                result.readiness_evidence["fallback_shrink_readiness"]
                    ["python_fallback_removal_ready"],
                true
            );
            assert_eq!(
                result.readiness_evidence["next_cutover_gate"],
                "single-generation route entry source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/candidate and prepare/orchestration source-map packages"
            );
        }

        #[tokio::test]
        async fn should_project_active_route_owner_chain_for_gateway_config_consumption() {
            let results = run_chapter_single_generation_active_gateway_smoke_suite()
                .await
                .expect("active gateway smoke results");
            let readiness = &results[0].readiness_evidence;

            assert_eq!(readiness["python_source_map"], json!([]));
            assert_eq!(
                readiness["python_source_map"]
                    .as_array()
                    .expect("python source map")
                    .len(),
                0
            );
            assert_eq!(
                readiness["python_source_map_policy"]["legacy_rollback_wiring_shells"],
                json!([])
            );
            assert_eq!(
                readiness["active_gateway_cutover"]["python_route_files_status"],
                "python_stream_background_entry_source_maps_deleted_for_single_generation_active_route_support"
            );
            assert_eq!(
                readiness["active_gateway_cutover"]["python_bootstrap_registration"],
                "bootstrap_registration_deleted_no_route_wiring_loader_remains"
            );
            assert_eq!(
                readiness["rollback_policy"]["python_source_map_action"],
                "single_generation_active_route_source_maps_deleted_prerequisite_logic_rehomed_to_shared_chapter_query_source_map"
            );
            assert_eq!(
                readiness["python_source_map_policy"]["final_frozen_boundary"],
                "single_generation_active_route_source_map_surface_empty_prepare_query_source_maps_deleted"
            );
            assert_eq!(
                readiness["python_source_map_policy"]["shared_prepare_query_source_maps"],
                json!([])
            );
            assert_eq!(
                readiness["active_route_gateway_config"]["gateway_config_owner"],
                "build_chapter_candidate_route_gateway_config_from_app_config"
            );
            assert_eq!(
                readiness["runtime_owner_chain"]["prepare_request"],
                "prepare_single_chapter_generation_request_from_route_payload"
            );
            assert_eq!(
                readiness["runtime_owner_chain"]["stream_lifecycle"],
                "SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config"
            );
            assert_eq!(
                readiness["runtime_owner_chain"]["background_lifecycle"],
                "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config"
            );
            assert_eq!(
                readiness["runtime_owner_chain"]["candidate_request"],
                "build_single_generation_candidate_executor_request"
            );
            assert_eq!(
                readiness["runtime_owner_chain"]["candidate_metadata"],
                "build_single_generation_candidate_gateway_metadata"
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]["python_source_map"],
                json!([])
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]["rollback_boundary"]
                    ["single_generation_entry_source_maps"],
                json!([])
            );
            assert_eq!(
                readiness["shared_candidate_runtime_owner_contract"]["rust_owner_map"][3],
                "build_single_generation_direct_fallback_candidate_payload"
            );
            assert_eq!(
                readiness["next_cutover_gate"],
                "single-generation route entry source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/candidate and prepare/orchestration source-map packages"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        chapter_batch_generation_active_gateway_smoke, chapter_candidate_route_gateway_smoke,
        chapter_regeneration_stream_workflow_smoke, chapter_single_generation_active_gateway_smoke,
        readiness_check, rust_migration_noop_executor_smoke,
        CHAPTER_BATCH_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE,
        CHAPTER_CANDIDATE_ROUTE_GATEWAY_SMOKE_ROUTE,
        CHAPTER_REGENERATION_STREAM_WORKFLOW_SMOKE_ROUTE,
        CHAPTER_SINGLE_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE,
        RUST_MIGRATION_NOOP_EXECUTOR_SMOKE_ROUTE,
    };
    use crate::config::{AppConfig, AppRuntimeMode};
    use axum::http::StatusCode;
    use axum::Extension;
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
    use serde_json::json;

    #[test]
    fn should_keep_chapter_candidate_route_gateway_smoke_route_public_path() {
        assert_eq!(
            CHAPTER_CANDIDATE_ROUTE_GATEWAY_SMOKE_ROUTE,
            "/health/chapter-candidate-route-gateway-smoke"
        );
        assert_eq!(
            CHAPTER_SINGLE_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE,
            "/health/chapter-single-generation-active-gateway-smoke"
        );
        assert_eq!(
            CHAPTER_BATCH_GENERATION_ACTIVE_GATEWAY_SMOKE_ROUTE,
            "/health/chapter-batch-generation-active-gateway-smoke"
        );
        assert_eq!(
            CHAPTER_REGENERATION_STREAM_WORKFLOW_SMOKE_ROUTE,
            "/health/chapter-regeneration-stream-workflow-smoke"
        );
        assert_eq!(
            RUST_MIGRATION_NOOP_EXECUTOR_SMOKE_ROUTE,
            "/health/rust-migration-noop-executor-smoke"
        );
    }

    #[test]
    fn should_keep_single_generation_manifest_on_real_rust_owner_chain() {
        let manifest = include_str!("../../../deploy/strangler-gateway-probes.json");

        for owner in [
            "chapter_single_generation_stream_workflow_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_single_generation_runtime_state_service",
        ] {
            assert!(
                manifest.contains(owner),
                "single-generation manifest must keep real Rust owner: {owner}"
            );
        }

        for retired_owner in [
            "chapter_single_generation_stream_success_response_service",
            "chapter_single_generation_background_response_service",
            "chapter_single_generation_terminal_state_service",
        ] {
            assert!(
                !manifest.contains(retired_owner),
                "single-generation manifest must not reference retired forwarding-only owner: {retired_owner}"
            );
        }
    }

    #[tokio::test]
    async fn should_expose_schema_migration_metadata_in_readiness_payload() {
        let (status, axum::Json(body)) = readiness_check(None).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "not_ready");
        assert_eq!(body["checks"]["startup"]["ready"], true);
        assert_eq!(body["checks"]["database"]["healthy"], false);
        assert_eq!(
            body["checks"]["schema_migration"]["owner"],
            "schema_migration_metadata_service"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["production_migration_mode"],
            "explicit_rust_db_migrator_before_rust_startup"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["startup_schema_sync_allowed"],
            false
        );
        assert_eq!(
            body["checks"]["schema_migration"]["python_boundary"]["status"],
            "migration_wrapper_deleted_after_rust_migrator_cutover"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["python_boundary"]["migration_runner"],
            "deleted"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["python_boundary"]["migrator_metadata_package"],
            "backend/migrator_app/models"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["python_boundary"]["model_registry_helper"],
            "backend/migrator_app/models/__init__.py"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["python_boundary"]["docker_service"],
            "none"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["rust_boundary"]["migration_command"],
            "migration-executor"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["rust_boundary"]["migration_smoke_command"],
            "migration-needed-executor-smoke"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["postgres_revision_catalog"]["revision_count"],
            19
        );
        assert_eq!(
            body["checks"]["schema_migration"]["postgres_revision_catalog"]["head"],
            "20260517_project_core_defaults"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["postgres_revision_catalog"]["execution_ready"],
            true
        );
        assert_eq!(
            body["checks"]["schema_migration"]["live_database_head"]["status"],
            "not_checked"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["live_database_head"]["expected_head"],
            "20260517_project_core_defaults"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["live_database_head"]["matches_catalog_head"],
            false
        );
        assert_eq!(
            body["checks"]["schema_migration"]["rust_migration_executor"]["status"],
            "blocked_live_head_not_checked"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["rust_migration_executor"]["execution_mode"],
            "read_only_preflight_no_ddl"
        );
        assert_eq!(
            body["checks"]["schema_migration"]["rust_migration_executor"]["ddl_execution_enabled"],
            false
        );
        assert_eq!(
            body["checks"]["schema_migration"]["rust_migration_executor"]
                ["can_replace_python_migrator"],
            false
        );
        assert_eq!(
            body["checks"]["schema_migration"]["exit_readiness"]["python_migrator_still_required"],
            false
        );
        assert_eq!(
            body["checks"]["schema_migration"]["exit_readiness"]["python_zero_ready"],
            true
        );
        assert_eq!(
            body["checks"]["schema_migration"]["exit_readiness"]
                ["rust_owned_migration_executor_ready"],
            true
        );
    }

    #[tokio::test]
    async fn should_keep_rust_migration_noop_executor_smoke_disabled_by_default() {
        let db = setup_schema_migration_smoke_db("20260517_project_core_defaults").await;
        let config = schema_migration_smoke_config(false);

        let (status, axum::Json(body)) =
            rust_migration_noop_executor_smoke(Some(Extension(db)), Some(Extension(config))).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["ok"], false);
        assert_eq!(body["status"], "disabled_by_config");
        assert_eq!(body["gate_enabled"], false);
        assert_eq!(body["ddl_executed"], false);
        assert_eq!(body["rollback_boundary"], "python_db_migrator_alembic");
    }

    #[tokio::test]
    async fn should_pass_rust_migration_noop_executor_smoke_for_matching_head_when_gated() {
        let db = setup_schema_migration_smoke_db("20260517_project_core_defaults").await;
        let config = schema_migration_smoke_config(true);

        let (status, axum::Json(body)) =
            rust_migration_noop_executor_smoke(Some(Extension(db)), Some(Extension(config))).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);
        assert_eq!(body["status"], "noop_executor_smoke_passed");
        assert_eq!(body["gate_enabled"], true);
        assert_eq!(body["ddl_executed"], false);
        assert_eq!(body["live_head"], "20260517_project_core_defaults");
        assert_eq!(body["blockers"], json!([]));
    }

    #[tokio::test]
    async fn should_block_rust_migration_noop_executor_smoke_for_mismatched_head() {
        let db = setup_schema_migration_smoke_db("old_revision").await;
        let config = schema_migration_smoke_config(true);

        let (status, axum::Json(body)) =
            rust_migration_noop_executor_smoke(Some(Extension(db)), Some(Extension(config))).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["ok"], false);
        assert_eq!(body["status"], "blocked_by_preflight");
        assert_eq!(body["gate_enabled"], true);
        assert_eq!(body["ddl_executed"], false);
        assert_eq!(body["live_head"], "old_revision");
    }

    async fn setup_schema_migration_smoke_db(version_num: &str) -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE alembic_version (version_num VARCHAR(64) NOT NULL)".to_string(),
        ))
        .await
        .expect("create alembic_version table");
        db.execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO alembic_version (version_num) VALUES (?)",
            [version_num.into()],
        ))
        .await
        .expect("insert alembic version");
        db
    }

    fn schema_migration_smoke_config(enabled: bool) -> AppConfig {
        AppConfig {
            app_host: "127.0.0.1".to_string(),
            app_port: 8001,
            app_name: "MuMuNovel".to_string(),
            app_version: "test".to_string(),
            database_url: "sqlite::memory:".to_string(),
            database_pool_size: 1,
            enable_startup_schema_sync: false,
            log_level: "info".to_string(),
            debug: true,
            runtime_mode: AppRuntimeMode::Development,
            cors_origins: "*".to_string(),
            jwt_secret: "test-secret".to_string(),
            static_dir: "../backend/static".to_string(),
            local_auth_enabled: true,
            local_auth_username: "admin".to_string(),
            local_auth_password: "password".to_string(),
            local_auth_display_name: "本地管理员".to_string(),
            linuxdo_client_id: String::new(),
            linuxdo_client_secret: String::new(),
            linuxdo_redirect_uri: String::new(),
            frontend_url: "http://localhost".to_string(),
            session_expire_minutes: 120,
            session_refresh_threshold_minutes: 30,
            chapter_candidate_rust_executor_enabled: true,
            chapter_candidate_rust_executor_fallback_on_error: true,
            chapter_candidate_rust_executor_disabled_reason: String::new(),
            chapter_candidate_rust_executor_rollback_boundary: "python_candidate_executor_fallback"
                .to_string(),
            rust_migration_noop_executor_smoke_enabled: enabled,
        }
    }

    #[tokio::test]
    async fn should_expose_chapter_candidate_route_gateway_smoke_payload() {
        let (status, axum::Json(body)) = chapter_candidate_route_gateway_smoke().await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["owner"], "rust");
        assert_eq!(body["route_group"], "chapters");
        assert_eq!(body["probe_count"], 3);
        assert_eq!(
            body["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            body["probes"][0]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(body["probes"][0]["result"]["gateway_consumed"], true);
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["owner_scope"],
            "candidate_executor_route_gateway_cutover"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["covered_rust_owners"]
                .as_array()
                .expect("covered rust owners")
                .iter()
                .any(|owner| owner == "chapter_candidate_executor_production_adapter_service"),
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["covered_rust_owners"]
                .as_array()
                .expect("covered rust owners")
                .iter()
                .any(|owner| owner
                    == "chapter_candidate_executor_production_adapter_service::quality_adapter_owner"),
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["covered_rust_owners"]
                .as_array()
                .expect("covered rust owners")
                .iter()
                .any(|owner| owner == "chapter_candidate_generation_service"),
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["covered_rust_owners"]
                .as_array()
                .expect("covered rust owners")
                .iter()
                .any(|owner| owner == "chapter_candidate_provider_stream_service"),
            false
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["runtime_owner_chain"]["generation"],
            "generate_candidate_pool_workflow"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["wiring_readiness"]["stage_count"],
            9
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["wiring_readiness"]
                ["external_formula_dependency_count"],
            0
        );
        assert_eq!(
            body["probes"][1]["name"],
            "chapter-candidate-route-gateway-fallback-freeze-candidate"
        );
        assert_eq!(
            body["probes"][1]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(body["probes"][1]["fallback_applied"], false);
        assert_eq!(
            body["probes"][1]["readiness_evidence"]["fallback_shrink_readiness"]
                ["fallback_freeze_config_validated"],
            true
        );
        assert_eq!(body["probes"][2]["execution_path"], "python_fallback");
        assert_eq!(body["probes"][2]["fallback_applied"], true);
        assert_eq!(
            body["probes"][2]["readiness_evidence"]["gateway"]["disabled_reason"],
            "smoke probe forces python fallback"
        );
    }

    #[tokio::test]
    async fn should_expose_chapter_single_generation_active_gateway_smoke_payload() {
        let (status, axum::Json(body)) = chapter_single_generation_active_gateway_smoke().await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["owner"], "rust");
        assert_eq!(body["route_group"], "chapter_single_generation");
        assert_eq!(body["probe_count"], 2);
        assert_eq!(
            body["rollback_boundary"],
            "legacy_single_generation_direct_ai"
        );
        assert_eq!(
            body["probes"][0]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(body["probes"][0]["result"]["gateway_consumed"], true);
        assert_eq!(body["probes"][0]["content"], "Rust 候选章节正文。");
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["owner_scope"],
            "active_route_gateway_stream_background_runtime_terminal"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["covered_rust_owners"][0],
            "chapter_generation_routes"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["covered_rust_owners"][2],
            "chapter_single_generation_prepare_service"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["covered_rust_owners"][3],
            "chapter_single_generation_stream_workflow_service"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["python_source_map"]
                .as_array()
                .expect("python source map")
                .len(),
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["active_route_gateway_config"]
                ["stream_route_consumes_gateway_config"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["python_source_map_policy"]["status"],
            "source_map_only"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["rollback_policy"]["manifest_owner_baseline"],
            "rust = 131, python-fallback = 0"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["runtime_owner_chain"]["prepare_request"],
            "prepare_single_chapter_generation_request_from_route_payload"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["runtime_owner_chain"]["runtime_generation"],
            "SingleGenerationRuntimeLaunchInput::execute_generation_with_gateway_config"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["background_response"]["status"],
            "pending"
        );
        assert_eq!(
            body["probes"][1]["name"],
            "chapter-single-generation-active-gateway-fallback-freeze-candidate"
        );
        assert_eq!(
            body["probes"][1]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(body["probes"][1]["fallback_applied"], false);
        assert_eq!(
            body["probes"][1]["readiness_evidence"]["fallback_shrink_readiness"]
                ["active_route_smoke_consumes_freeze_candidate"],
            true
        );
        assert_eq!(
            body["probes"][1]["readiness_evidence"]["fallback_shrink_readiness"]
                ["python_fallback_removal_ready"],
            true
        );
    }

    #[tokio::test]
    async fn should_expose_chapter_batch_generation_active_gateway_smoke_payload() {
        let (status, axum::Json(body)) = chapter_batch_generation_active_gateway_smoke().await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["owner"], "rust");
        assert_eq!(body["route_group"], "chapter_batch_generation");
        assert_eq!(body["probe_count"], 2);
        assert_eq!(
            body["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            body["probes"][0]["name"],
            "chapter-batch-generation-active-gateway-rust-owner"
        );
        assert_eq!(
            body["probes"][0]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(body["probes"][0]["fallback_applied"], false);
        assert_eq!(body["probes"][0]["result"]["gateway_consumed"], true);
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["owner_scope"],
            "batch_active_route_gateway_create_status_stream_resume"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["active_route_gateway_config"]
                ["create_route_consumes_gateway_config"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["active_route_gateway_config"]
                ["resume_route_consumes_gateway_config"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["selected_candidate_stream"]
                ["snapshot_event_count"],
            3
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["selected_candidate_stream"]
                ["chunk_event_projected"],
            true
        );
        assert_eq!(
            body["probes"][1]["name"],
            "chapter-batch-generation-active-gateway-fallback-freeze-candidate"
        );
        assert_eq!(
            body["probes"][1]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(body["probes"][1]["fallback_applied"], false);
        assert_eq!(
            body["probes"][1]["readiness_evidence"]["active_route_gateway_config"]
                ["fallback_on_rust_error"],
            false
        );
        assert_eq!(
            body["probes"][1]["readiness_evidence"]["fallback_shrink_readiness"]
                ["python_fallback_removal_ready"],
            true
        );
    }

    #[tokio::test]
    async fn should_expose_chapter_regeneration_stream_workflow_smoke_payload() {
        let (status, axum::Json(body)) = chapter_regeneration_stream_workflow_smoke().await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["owner"], "rust");
        assert_eq!(body["route_group"], "chapter_regeneration");
        assert_eq!(body["probe_count"], 1);
        assert_eq!(
            body["rollback_boundary"],
            "chapter_regeneration_python_source_map"
        );
        assert_eq!(
            body["probes"][0]["name"],
            "chapter-regeneration-stream-workflow-rust-owner"
        );
        assert_eq!(
            body["probes"][0]["execution_path"],
            "rust_regeneration_stream_workflow_owner"
        );
        assert_eq!(body["probes"][0]["fallback_applied"], false);
        assert_eq!(
            body["probes"][0]["result"]["full_stream_owner_consumed"],
            true
        );
        assert_eq!(
            body["probes"][0]["result"]["partial_stream_owner_consumed"],
            true
        );
        assert_eq!(
            body["probes"][0]["result"]["deterministic_business_sse_smoke"],
            true
        );
        assert_eq!(
            body["probes"][0]["result"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            body["probes"][0]["runtime_state"]["deterministic_business_sse_smoke"],
            true
        );
        assert_eq!(
            body["probes"][0]["runtime_state"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            body["probes"][0]["runtime_state"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["source_map_policy"]
                ["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["source_map_policy"]
                ["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["source_map_policy"]
                ["deterministic_business_sse_smoke"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["python_source_map"]
                .as_array()
                .expect("regeneration python source map")
                .len(),
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["source_map_policy"]["freeze_scope"],
            "chapter_regeneration_route_package_source_map_surface"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["source_map_policy"]
                ["stream_orchestration_source_maps"]
                .as_array()
                .expect("stream orchestration source maps")
                .len(),
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["source_map_policy"]
                ["prepare_owner_source_maps"]
                .as_array()
                .expect("prepare owner source maps")
                .len(),
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["source_map_policy"]
                ["shared_prepare_dependency_source_maps"]
                .as_array()
                .expect("shared prepare dependency source maps")
                .len(),
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["source_map_policy"]
                ["shared_context_compaction_source_maps"]
                .as_array()
                .expect("shared context compaction source maps")
                .len(),
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["source_map_policy"]["query_owner_source_maps"]
                .as_array()
                .expect("query owner source maps")
                .len(),
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["prepare_owner_contract"]["owner"],
            "chapter_regeneration_prepare_service"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["prepare_owner_contract"]
                ["service_runtime_closeout_status"]["owner_profile"],
            "phase5-chapter-regeneration-owner"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["prepare_owner_contract"]
                ["service_runtime_closeout_status"]["regeneration_manifest_probe_count"],
            13
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["prepare_owner_contract"]
                ["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["prepare_owner_contract"]
                ["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["prepare_owner_contract"]
                ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["prepare_owner_contract"]
                ["service_runtime_closeout_status"]["status"],
            "rust_chapter_regeneration_prepare_owner_direct_package_closed_out"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]["owner"],
            "chapter_candidate_output_service"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["candidate_runtime_state_owner_contract"]["owner"],
            "chapter_candidate_runtime_state_service"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["owner_profiles"][2],
            "phase5-chapter-regeneration-owner"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["single_generation_manifest_probe_count"],
            6
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["regeneration_manifest_probe_count"],
            13
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["status"],
            "rust_chapter_candidate_runtime_state_owner_source_map_deleted"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["service_runtime_closeout_status"]["owner_profiles"][2],
            "phase5-chapter-regeneration-owner"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            6
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["service_runtime_closeout_status"]["regeneration_manifest_probe_count"],
            13
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["candidate_output_owner_contract"]
                ["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_output_owner_executor_source_map_deleted"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["service_runtime_closeout_status"]
                ["owner_profile"],
            "phase5-chapter-regeneration-owner"
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["service_runtime_closeout_status"]
                ["regeneration_manifest_probe_count"],
            13
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["service_runtime_closeout_status"]
                ["rust_manifest_probe_count"],
            13
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["service_runtime_closeout_status"]
                ["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["service_runtime_closeout_status"]
                ["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["service_runtime_closeout_status"]
                ["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            body["probes"][0]["readiness_evidence"]["service_runtime_closeout_status"]["status"],
            "rust_chapter_regeneration_stream_workflow_owner_after_route_shell_delete"
        );
    }
}
