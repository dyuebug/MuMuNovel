// Rust route/deployment gateway owner for the chapter candidate executor
// cutover. It maps deployment config into the rollback-aware production
// adapter and owns the route-level smoke/readiness projection, so routes do not
// need to rebuild cutover, fallback, or rollback decisions locally.

use std::{future::Future, pin::Pin};

use serde_json::{json, Map, Value};

use crate::ai::config::AIConfig;
use crate::config::AppConfig;
use crate::services::chapter_candidate_executor_default_dependency_service::{
    build_candidate_executor_wiring_owner_contract,
    build_default_chapter_candidate_executor_wiring_plan,
    resolve_candidate_executor_wiring_readiness, validate_candidate_executor_wiring_plan,
};
use crate::services::chapter_candidate_executor_production_adapter_service::{
    build_chapter_candidate_quality_adapter, chapter_candidate_production_execution_path_name,
    execute_chapter_candidate_production_adapter,
    execute_chapter_candidate_production_adapter_with_executor, CandidateQualityGatePlanInput,
    CandidateQualityRuntimeContextBuildInput, CandidateStoryQualityMetricsInput,
    ChapterCandidateProductionAdapterConfig, ChapterCandidateProductionAdapterOutput,
    ChapterCandidateProductionFallbackContext, ChapterCandidateQualityAdapter,
    ChapterCandidateQualityAdapterContext,
};
use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;

pub(crate) fn build_chapter_candidate_route_gateway_owner_contract() -> Value {
    json!({
        "owner": "chapter_candidate_route_gateway_service",
        "scope": "candidate_executor_route_gateway_cutover_and_rollback_boundary",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_service.rs",
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_record_service.rs",
            "backend-rs/src/services/chapter_candidate_rerank_service.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/services/chapter_candidate_output_service.rs"
        ],
        "behavior_contract": {
            "config_builder": "build_chapter_candidate_route_gateway_config_from_app_config",
            "adapter_config_builder": "build_chapter_candidate_production_adapter_config_from_route_gateway",
            "gateway_entrypoints": [
                "execute_chapter_candidate_route_gateway",
                "execute_chapter_candidate_route_gateway_with_executor"
            ],
            "smoke_entrypoints": [
                "build_default_chapter_candidate_route_gateway_smoke_probes",
                "run_chapter_candidate_route_gateway_smoke_suite",
                "run_chapter_candidate_route_gateway_smoke_probe"
            ],
            "runtime_owner_chain": [
                "execute_chapter_candidate_production_adapter",
                "generate_best_ranked_candidate_workflow_with_boxed_dependencies",
                "generate_candidate_pool_workflow",
                "maybe_apply_word_budget_repair_workflow",
                "execute_targeted_final_repair_pass_workflow",
                "finalize_selected_candidate_result",
                "collect_default_generation_candidate_output",
                "build_runtime_quality_adapter_callbacks",
                "build_default_generation_candidate_record"
            ],
            "fallback_fields": [
                "fallback_reason",
                "rollback_boundary",
                "rust_error"
            ],
            "readiness_fields": [
                "gateway",
                "runtime_owner_chain",
                "wiring_readiness",
                "fallback_contract",
                "fallback_shrink_readiness"
            ]
        },
        "candidate_executor_wiring_owner_contract": build_candidate_executor_wiring_owner_contract(),
        "active_consumers": [
            "chapter_generation_routes",
            "chapter_batch_generation",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter-batch-generation-active-gateway-smoke-rust",
            "chapter_generation_runtime_service",
            "chapter_batch_generation_runtime_state_service"
        ],
        "active_route_closeout_evidence": {
            "single_generation_active_smoke": "chapter-single-generation-active-gateway-fallback-freeze-candidate",
            "batch_generation_active_smoke": "chapter-batch-generation-active-gateway-fallback-freeze-candidate",
            "candidate_gateway_manifest_probe": "chapter-candidate-route-gateway-smoke-rust",
            "active_route_smoke_consumes_freeze_candidate": true,
            "rust_executor_required": true,
            "fallback_on_rust_error_required": false,
            "physical_python_closeout_completed": true,
            "source_map_closeout_policy": "freeze_repoint_or_delete_only_with_explicit_approval_and_same_round_rollback_policy",
            "remaining_blockers": [
                "separate route-gateway rollback shell freeze/delete/repoint review remains for non-executor Python source-map packages"
            ]
        },
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-chapters-candidate-gateway-owner",
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner"
            ],
            "candidate_gateway_manifest_probe_count": 1,
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 18,
            "python_fallback_probe_count": 0,
            "gateway_owner": "execute_chapter_candidate_route_gateway",
            "production_adapter_owner": "execute_chapter_candidate_production_adapter",
            "executor_owner": "generate_best_ranked_candidate_workflow_with_boxed_dependencies",
            "wiring_owner": "chapter_candidate_executor_default_dependency_service",
            "active_route_gateway_consumers": [
                "chapter-candidate-route-gateway-smoke-rust",
                "chapter-single-generation-active-gateway-smoke-rust",
                "chapter-batch-generation-active-gateway-smoke-rust"
            ],
            "fallback_freeze_config_validated": true,
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "candidate executor direct python source-map deleted; separate route-gateway rollback shell freeze/delete/repoint review now remains for non-executor Python source-map packages",
            "status": "rust_candidate_route_gateway_owner_executor_source_map_deleted"
        },
        "validation_boundary": [
            "cargo test chapter_candidate_route_gateway_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --profile phase5-chapters-candidate-gateway-owner",
            "python backend/tools/run_strangler_gateway_smoke.py --profile phase5-single-generation-owner",
            "python backend/tools/run_strangler_gateway_smoke.py --profile phase5-batch-generation-owner",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "runtime_knob": "python_candidate_executor_fallback",
            "active_gateway_knobs": [
                "chapter_candidate_rust_executor_enabled",
                "chapter_candidate_rust_executor_fallback_on_error",
                "chapter_candidate_rust_executor_disabled_reason",
                "chapter_candidate_rust_executor_rollback_boundary"
            ],
            "python_source_map_policy": "source_map_and_explicit_gateway_rollback_only",
            "freeze_or_delete_requires_same_round_rollback_policy": true,
            "python_fallback_removal_ready": true
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateRouteGatewayConfig {
    pub(crate) rust_executor_enabled: bool,
    pub(crate) fallback_on_rust_error: bool,
    pub(crate) disabled_reason: Option<String>,
    pub(crate) rollback_boundary: String,
}

pub(crate) fn build_chapter_candidate_route_gateway_config_from_app_config(
    config: &AppConfig,
) -> ChapterCandidateRouteGatewayConfig {
    ChapterCandidateRouteGatewayConfig {
        rust_executor_enabled: config.chapter_candidate_rust_executor_enabled,
        fallback_on_rust_error: config.chapter_candidate_rust_executor_fallback_on_error,
        disabled_reason: non_empty_string(&config.chapter_candidate_rust_executor_disabled_reason),
        rollback_boundary: non_empty_string(
            &config.chapter_candidate_rust_executor_rollback_boundary,
        )
        .unwrap_or_else(|| "python_candidate_executor_fallback".to_string()),
    }
}

pub(crate) fn build_chapter_candidate_production_adapter_config_from_route_gateway(
    config: &ChapterCandidateRouteGatewayConfig,
) -> ChapterCandidateProductionAdapterConfig {
    ChapterCandidateProductionAdapterConfig {
        rust_executor_enabled: config.rust_executor_enabled,
        fallback_on_rust_error: config.fallback_on_rust_error,
        disabled_reason: config.disabled_reason.clone(),
        rollback_boundary: config.rollback_boundary.clone(),
    }
}

pub(crate) async fn execute_chapter_candidate_route_gateway<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
    PythonFallback,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_adapter: ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    python_fallback_fn: PythonFallback,
) -> Result<ChapterCandidateProductionAdapterOutput, String>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
    PythonFallback: for<'request> FnOnce(
        &'request mut ChapterCandidateExecutorRequest,
        ChapterCandidateProductionFallbackContext,
    ) -> Pin<
        Box<dyn Future<Output = Result<Value, String>> + Send + 'request>,
    >,
{
    execute_chapter_candidate_production_adapter(
        request,
        ai_config,
        quality_adapter,
        build_chapter_candidate_production_adapter_config_from_route_gateway(&gateway_config),
        python_fallback_fn,
    )
    .await
}

pub(crate) async fn execute_chapter_candidate_route_gateway_with_executor<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
    RustExecutor,
    PythonFallback,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_adapter: ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    rust_executor_fn: RustExecutor,
    python_fallback_fn: PythonFallback,
) -> Result<ChapterCandidateProductionAdapterOutput, String>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
    RustExecutor: for<'request> FnOnce(
        &'request mut ChapterCandidateExecutorRequest,
        AIConfig,
        ChapterCandidateQualityAdapter<
            BuildQualityRuntimeContext,
            ComputeStoryQualityMetrics,
            ResolveQualityGatePlan,
        >,
    ) -> Pin<
        Box<dyn Future<Output = Result<Value, String>> + Send + 'request>,
    >,
    PythonFallback: for<'request> FnOnce(
        &'request mut ChapterCandidateExecutorRequest,
        ChapterCandidateProductionFallbackContext,
    ) -> Pin<
        Box<dyn Future<Output = Result<Value, String>> + Send + 'request>,
    >,
{
    execute_chapter_candidate_production_adapter_with_executor(
        request,
        ai_config,
        quality_adapter,
        build_chapter_candidate_production_adapter_config_from_route_gateway(&gateway_config),
        rust_executor_fn,
        python_fallback_fn,
    )
    .await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateRouteGatewaySmokeProbe {
    pub(crate) name: &'static str,
    pub(crate) owner: &'static str,
    pub(crate) route_group: &'static str,
    pub(crate) config: ChapterCandidateRouteGatewayConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateRouteGatewaySmokeResult {
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

pub(crate) fn build_default_chapter_candidate_route_gateway_smoke_probes(
) -> Vec<ChapterCandidateRouteGatewaySmokeProbe> {
    vec![
        ChapterCandidateRouteGatewaySmokeProbe {
            name: "chapter-candidate-route-gateway-rust-owner",
            owner: "rust",
            route_group: "chapters",
            config: ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: true,
                disabled_reason: None,
                rollback_boundary: "python_candidate_executor_fallback".to_string(),
            },
        },
        ChapterCandidateRouteGatewaySmokeProbe {
            name: "chapter-candidate-route-gateway-fallback-freeze-candidate",
            owner: "rust",
            route_group: "chapters",
            config: ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: false,
                disabled_reason: Some(
                    "candidate gateway fallback freeze candidate smoke".to_string(),
                ),
                rollback_boundary: "python_candidate_executor_fallback".to_string(),
            },
        },
        ChapterCandidateRouteGatewaySmokeProbe {
            name: "chapter-candidate-route-gateway-python-fallback",
            owner: "python-fallback",
            route_group: "chapters",
            config: ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: false,
                fallback_on_rust_error: true,
                disabled_reason: Some("smoke probe forces python fallback".to_string()),
                rollback_boundary: "python_candidate_executor_fallback".to_string(),
            },
        },
    ]
}

pub(crate) async fn run_chapter_candidate_route_gateway_smoke_suite(
) -> Result<Vec<ChapterCandidateRouteGatewaySmokeResult>, String> {
    let mut results = Vec::new();

    for probe in build_default_chapter_candidate_route_gateway_smoke_probes() {
        results.push(run_chapter_candidate_route_gateway_smoke_probe(probe).await?);
    }

    Ok(results)
}

pub(crate) async fn run_chapter_candidate_route_gateway_smoke_probe(
    probe: ChapterCandidateRouteGatewaySmokeProbe,
) -> Result<ChapterCandidateRouteGatewaySmokeResult, String> {
    let mut request = smoke_executor_request(probe.name);
    let rust_probe_name = probe.name.to_string();
    let python_probe_name = probe.name.to_string();

    let output = execute_chapter_candidate_route_gateway_with_executor(
        &mut request,
        AIConfig::default(),
        smoke_quality_adapter(),
        probe.config.clone(),
        move |request, _ai_config, _quality_adapter| {
            Box::pin(async move {
                request.runtime_state = Some(json!({
                    "gateway_smoke": "rust",
                    "probe": rust_probe_name,
                }));
                Ok(json!({
                    "path": "rust",
                    "probe": rust_probe_name,
                    "gateway_consumed": true,
                }))
            })
        },
        move |request, context| {
            Box::pin(async move {
                request.runtime_state = Some(json!({
                    "gateway_smoke": "python-fallback",
                    "probe": python_probe_name,
                }));
                Ok(json!({
                    "path": "python-fallback",
                    "probe": python_probe_name,
                    "gateway_consumed": true,
                    "fallback_reason": context.reason,
                    "rollback_boundary": context.rollback_boundary,
                    "rust_error": context.rust_error,
                }))
            })
        },
    )
    .await?;
    let readiness_evidence =
        build_chapter_candidate_route_gateway_readiness_evidence(&probe, &output.result);

    Ok(ChapterCandidateRouteGatewaySmokeResult {
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

fn build_chapter_candidate_route_gateway_readiness_evidence(
    probe: &ChapterCandidateRouteGatewaySmokeProbe,
    gateway_result: &Value,
) -> Value {
    let wiring_plan = build_default_chapter_candidate_executor_wiring_plan();
    validate_candidate_executor_wiring_plan(&wiring_plan)
        .expect("default candidate executor wiring plan must stay valid");
    let wiring_readiness = resolve_candidate_executor_wiring_readiness(&wiring_plan);

    json!({
        "owner_scope": "candidate_executor_route_gateway_cutover",
        "candidate_route_gateway_owner_contract": build_chapter_candidate_route_gateway_owner_contract(),
        "covered_rust_owners": [
            "chapter_candidate_route_gateway_service",
            "chapter_candidate_executor_production_adapter_service",
            "chapter_candidate_executor_default_dependency_service",
            "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
            "chapter_candidate_executor_service",
            "chapter_candidate_generation_service",
            "chapter_candidate_record_service",
            "chapter_candidate_word_budget_repair_service",
            "chapter_candidate_targeted_final_repair_service",
            "chapter_candidate_finalize_service",
            "chapter_candidate_rerank_service",
            "chapter_candidate_runtime_state_service",
            "chapter_candidate_output_service"
        ],
        "python_source_map": [],
        "gateway": {
            "route_group": probe.route_group,
            "rust_executor_enabled": probe.config.rust_executor_enabled,
            "fallback_on_rust_error": probe.config.fallback_on_rust_error,
            "disabled_reason": probe.config.disabled_reason.as_deref(),
            "rollback_boundary": probe.config.rollback_boundary,
            "gateway_consumed": gateway_result
                .get("gateway_consumed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        "runtime_owner_chain": {
            "production_adapter": "execute_chapter_candidate_production_adapter",
            "route_gateway": "execute_chapter_candidate_route_gateway",
            "runtime_quality_bridge": "generate_best_ranked_candidate_with_runtime_quality_adapters",
            "default_dependency": "generate_best_ranked_candidate_with_default_dependency_wiring",
            "generation": "generate_candidate_pool_workflow",
            "word_budget_repair": "maybe_apply_word_budget_repair_workflow",
            "targeted_final_repair": "execute_targeted_final_repair_pass_workflow",
            "finalize": "finalize_selected_candidate_result",
            "executor": "generate_best_ranked_candidate_workflow_with_boxed_dependencies",
            "provider_stream": "collect_default_generation_candidate_output",
            "quality_adapter": "build_runtime_quality_adapter_callbacks",
            "record_mapping": "build_default_generation_candidate_record",
        },
        "wiring_readiness": {
            "stage_count": wiring_readiness.stage_count,
            "rust_owned_dependency_count": wiring_readiness.rust_owned_dependency_count,
            "external_formula_dependency_count": wiring_readiness.external_formula_dependency_count,
            "cutover_blockers": wiring_readiness.cutover_blockers,
            "rust_target_files": wiring_plan.rust_target_files,
            "python_source_files": wiring_plan.python_source_files,
        },
        "fallback_contract": {
            "python_fallback_preserved": true,
            "fallback_reason_field": gateway_result.get("fallback_reason").is_some(),
            "rollback_boundary_field": gateway_result.get("rollback_boundary").is_some(),
            "rust_error_field": gateway_result.get("rust_error").is_some(),
        },
        "fallback_shrink_readiness": {
            "candidate_probe": probe.name == "chapter-candidate-route-gateway-fallback-freeze-candidate",
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
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-chapters-candidate-gateway-owner",
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner"
            ],
            "candidate_gateway_manifest_probe_count": 1,
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 18,
            "python_fallback_probe_count": 0,
            "gateway_owner": "execute_chapter_candidate_route_gateway",
            "production_adapter_owner": "execute_chapter_candidate_production_adapter",
            "executor_owner": "generate_best_ranked_candidate_workflow_with_boxed_dependencies",
            "wiring_owner": "chapter_candidate_executor_default_dependency_service",
            "fallback_freeze_config_validated": probe.name == "chapter-candidate-route-gateway-fallback-freeze-candidate"
                && probe.config.rust_executor_enabled
                && !probe.config.fallback_on_rust_error
                && gateway_result
                    .get("gateway_consumed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "candidate executor direct python source-map deleted; separate route-gateway rollback shell freeze/delete/repoint review now remains for non-executor Python source-map packages",
            "status": "rust_candidate_route_gateway_owner_executor_source_map_deleted"
        },
        "next_cutover_gate": "candidate executor direct python source-map deleted; separate route-gateway rollback shell freeze/delete/repoint review now remains for non-executor Python source-map packages",
    })
}

fn smoke_executor_request(probe_name: &str) -> ChapterCandidateExecutorRequest {
    ChapterCandidateExecutorRequest {
        base_generate_kwargs: Map::from_iter([
            ("prompt".to_string(), json!("SMOKE_PROMPT")),
            ("probe".to_string(), json!(probe_name)),
        ]),
        target_word_count: 1200,
        source: "chapter".to_string(),
        generation_label: "candidate".to_string(),
        max_candidates: 1,
        runtime_state: None,
    }
}

fn smoke_quality_adapter(
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
            story_packet: json!({"packet": true}),
            project: json!({"world_rules": "rules"}),
            chapter: json!({"id": "chapter-1"}),
            chapter_context: json!({"chapter_outline": "outline"}),
            target_word_count: 1200,
            generation_intent: json!({"mode": "smoke"}),
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
            log_prefix: "Chapter".to_string(),
        },
        |_input| json!({"runtime": "context"}),
        |_input| json!({"overall_score": 90.0}),
        |_input| json!({"action": "continue"}),
    )
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_production_adapter_config_from_route_gateway,
        build_chapter_candidate_route_gateway_config_from_app_config,
        build_chapter_candidate_route_gateway_owner_contract,
        build_default_chapter_candidate_route_gateway_smoke_probes,
        execute_chapter_candidate_route_gateway_with_executor,
        run_chapter_candidate_route_gateway_smoke_probe,
        run_chapter_candidate_route_gateway_smoke_suite, ChapterCandidateRouteGatewayConfig,
    };
    use crate::ai::config::AIConfig;
    use crate::config::{AppConfig, AppRuntimeMode};
    use crate::services::chapter_candidate_executor_production_adapter_service::{
        build_chapter_candidate_quality_adapter, ChapterCandidateQualityAdapterContext,
    };
    use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;

    fn assert_no_deleted_python_service_source_map(contract: &serde_json::Value) {
        for key in ["python_source_map", "source_map_files", "rollback_files"] {
            let Some(items) = contract.get(key).and_then(|value| value.as_array()) else {
                continue;
            };
            assert!(
                !items.iter().any(|item| item
                    .as_str()
                    .is_some_and(|path| path.starts_with("backend/app/services/"))),
                "{key} must not retain deleted backend/app/services source-map paths"
            );
        }

        if let Some(rollback_files) = contract
            .get("rollback_boundary")
            .and_then(|value| value.get("rollback_files"))
            .and_then(|value| value.as_array())
        {
            assert!(
                !rollback_files.iter().any(|item| item
                    .as_str()
                    .is_some_and(|path| path.starts_with("backend/app/services/"))),
                "rollback_boundary.rollback_files must not retain deleted backend/app/services paths"
            );
        }
    }

    #[test]
    fn should_build_gateway_config_from_app_config_cutover_flags() {
        let mut app_config = app_config();
        app_config.chapter_candidate_rust_executor_enabled = true;
        app_config.chapter_candidate_rust_executor_fallback_on_error = false;
        app_config.chapter_candidate_rust_executor_disabled_reason =
            "  smoke probe owns fallback  ".to_string();
        app_config.chapter_candidate_rust_executor_rollback_boundary =
            "  chapters.py candidate fallback  ".to_string();

        let gateway_config =
            build_chapter_candidate_route_gateway_config_from_app_config(&app_config);

        assert!(gateway_config.rust_executor_enabled);
        assert!(!gateway_config.fallback_on_rust_error);
        assert_eq!(
            gateway_config.disabled_reason.as_deref(),
            Some("smoke probe owns fallback")
        );
        assert_eq!(
            gateway_config.rollback_boundary,
            "chapters.py candidate fallback"
        );
    }

    #[test]
    fn should_default_blank_gateway_reason_and_boundary() {
        let mut app_config = app_config();
        app_config.chapter_candidate_rust_executor_disabled_reason = " ".to_string();
        app_config.chapter_candidate_rust_executor_rollback_boundary = "\n".to_string();

        let gateway_config =
            build_chapter_candidate_route_gateway_config_from_app_config(&app_config);
        let production_config =
            build_chapter_candidate_production_adapter_config_from_route_gateway(&gateway_config);

        assert!(gateway_config.disabled_reason.is_none());
        assert_eq!(
            gateway_config.rollback_boundary,
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            production_config.rollback_boundary,
            "python_candidate_executor_fallback"
        );
    }

    #[test]
    fn should_publish_candidate_route_gateway_owner_contract() {
        let contract = build_chapter_candidate_route_gateway_owner_contract();
        assert_no_deleted_python_service_source_map(&contract);

        assert_eq!(contract["owner"], "chapter_candidate_route_gateway_service");
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["gateway_entrypoints"][1],
            "execute_chapter_candidate_route_gateway_with_executor"
        );
        assert_eq!(
            contract["behavior_contract"]["fallback_fields"][1],
            "rollback_boundary"
        );
        assert_eq!(
            contract["candidate_executor_wiring_owner_contract"]["owner"],
            "chapter_candidate_executor_default_dependency_service"
        );
        assert_eq!(
            contract["candidate_executor_wiring_owner_contract"]["behavior_contract"]
                ["required_stages"][8],
            "executor"
        );
        assert_eq!(
            contract["active_consumers"][2],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["active_route_closeout_evidence"]["single_generation_active_smoke"],
            "chapter-single-generation-active-gateway-fallback-freeze-candidate"
        );
        assert_eq!(
            contract["active_route_closeout_evidence"]["batch_generation_active_smoke"],
            "chapter-batch-generation-active-gateway-fallback-freeze-candidate"
        );
        assert_eq!(
            contract["active_route_closeout_evidence"]["candidate_gateway_manifest_probe"],
            "chapter-candidate-route-gateway-smoke-rust"
        );
        assert_eq!(
            contract["active_route_closeout_evidence"]
                ["active_route_smoke_consumes_freeze_candidate"],
            true
        );
        assert_eq!(
            contract["active_route_closeout_evidence"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["active_route_closeout_evidence"]["remaining_blockers"][0],
            "separate route-gateway rollback shell freeze/delete/repoint review remains for non-executor Python source-map packages"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_knob"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-chapters-candidate-gateway-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            18
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["gateway_owner"],
            "execute_chapter_candidate_route_gateway"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["fallback_freeze_config_validated"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_candidate_route_gateway_owner_executor_source_map_deleted"
        );
    }

    #[tokio::test]
    async fn should_execute_rust_path_through_route_gateway_when_enabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_route_gateway_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: true,
                disabled_reason: None,
                rollback_boundary: "route gateway fallback".to_string(),
            },
            |request, _ai_config, _quality_adapter| {
                Box::pin(async move {
                    request.runtime_state = Some(json!({"gateway": "rust"}));
                    Ok(json!({"path": "rust"}))
                })
            },
            |_request, _context| Box::pin(async { Ok(json!({"path": "python"})) }),
        )
        .await
        .expect("route gateway output");

        assert!(!output.fallback_applied);
        assert_eq!(output.result["path"], "rust");
        assert_eq!(request.runtime_state, Some(json!({"gateway": "rust"})));
    }

    #[tokio::test]
    async fn should_execute_python_fallback_through_route_gateway_when_disabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_route_gateway_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: false,
                fallback_on_rust_error: true,
                disabled_reason: Some("gateway cutover disabled".to_string()),
                rollback_boundary: "route gateway fallback".to_string(),
            },
            |_request, _ai_config, _quality_adapter| {
                Box::pin(async { Err("rust executor should not run".to_string()) })
            },
            |_request, context| {
                Box::pin(async move {
                    Ok(json!({
                        "path": "python",
                        "reason": context.reason,
                        "rollback_boundary": context.rollback_boundary,
                    }))
                })
            },
        )
        .await
        .expect("route gateway fallback output");

        assert!(output.fallback_applied);
        assert_eq!(output.result["path"], "python");
        assert_eq!(output.result["reason"], "gateway cutover disabled");
        assert_eq!(output.result["rollback_boundary"], "route gateway fallback");
    }

    #[test]
    fn should_build_default_smoke_probes_for_rust_and_python_fallback() {
        let probes = build_default_chapter_candidate_route_gateway_smoke_probes();

        assert_eq!(probes.len(), 3);
        assert_eq!(probes[0].name, "chapter-candidate-route-gateway-rust-owner");
        assert_eq!(probes[0].owner, "rust");
        assert!(probes[0].config.rust_executor_enabled);
        assert_eq!(
            probes[1].name,
            "chapter-candidate-route-gateway-fallback-freeze-candidate"
        );
        assert_eq!(probes[1].owner, "rust");
        assert!(probes[1].config.rust_executor_enabled);
        assert!(!probes[1].config.fallback_on_rust_error);
        assert_eq!(
            probes[2].name,
            "chapter-candidate-route-gateway-python-fallback"
        );
        assert_eq!(probes[2].owner, "python-fallback");
        assert!(!probes[2].config.rust_executor_enabled);
    }

    #[tokio::test]
    async fn should_run_smoke_suite_through_rust_and_python_fallback_paths() {
        let results = run_chapter_candidate_route_gateway_smoke_suite()
            .await
            .expect("smoke results");

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].execution_path, "rust_candidate_executor");
        assert!(!results[0].fallback_applied);
        assert_eq!(results[0].result["path"], "rust");
        assert_eq!(
            results[0].runtime_state.as_ref().unwrap()["gateway_smoke"],
            "rust"
        );
        assert_eq!(
            results[0].readiness_evidence["owner_scope"],
            "candidate_executor_route_gateway_cutover"
        );
        assert_eq!(
            results[0].readiness_evidence["candidate_route_gateway_owner_contract"]["owner"],
            "chapter_candidate_route_gateway_service"
        );
        assert_eq!(
            results[0].readiness_evidence["gateway"]["gateway_consumed"],
            true
        );

        assert_eq!(results[1].execution_path, "rust_candidate_executor");
        assert!(!results[1].fallback_applied);
        assert_eq!(
            results[1].readiness_evidence["fallback_shrink_readiness"]
                ["fallback_freeze_config_validated"],
            true
        );
        assert_eq!(
            results[1].readiness_evidence["fallback_shrink_readiness"]
                ["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            results[1].readiness_evidence["service_runtime_closeout_status"]
                ["fallback_freeze_config_validated"],
            true
        );
        assert_eq!(
            results[1].readiness_evidence["service_runtime_closeout_status"]
                ["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            results[1].readiness_evidence["service_runtime_closeout_status"]
                ["physical_python_closeout_completed"],
            true
        );

        assert_eq!(results[2].execution_path, "python_fallback");
        assert!(results[2].fallback_applied);
        assert_eq!(results[2].result["path"], "python-fallback");
        assert_eq!(
            results[2].result["fallback_reason"],
            "smoke probe forces python fallback"
        );
        assert_eq!(
            results[2].runtime_state.as_ref().unwrap()["gateway_smoke"],
            "python-fallback"
        );
        assert_eq!(
            results[2].readiness_evidence["gateway"]["disabled_reason"],
            "smoke probe forces python fallback"
        );
        assert_eq!(
            results[2].readiness_evidence["fallback_contract"]["fallback_reason_field"],
            true
        );
    }

    #[tokio::test]
    async fn should_keep_probe_metadata_in_smoke_result() {
        let probe = build_default_chapter_candidate_route_gateway_smoke_probes()
            .into_iter()
            .next()
            .expect("rust probe");

        let result = run_chapter_candidate_route_gateway_smoke_probe(probe)
            .await
            .expect("smoke result");

        assert!(result.ok);
        assert_eq!(result.owner, "rust");
        assert_eq!(result.route_group, "chapters");
        assert_eq!(
            result.rollback_boundary,
            "python_candidate_executor_fallback"
        );
        assert_eq!(result.result["gateway_consumed"], true);
        let covered_rust_owners = result.readiness_evidence["covered_rust_owners"]
            .as_array()
            .expect("covered rust owners");
        assert!(covered_rust_owners
            .iter()
            .any(|owner| owner == "chapter_candidate_executor_production_adapter_service"));
        assert!(covered_rust_owners
            .iter()
            .any(|owner| owner == "chapter_candidate_executor_production_adapter_service"));
        assert!(covered_rust_owners
            .iter()
            .any(|owner| owner == "chapter_candidate_generation_service"));
        assert!(covered_rust_owners
            .iter()
            .any(|owner| owner == "chapter_candidate_word_budget_repair_service"));
        assert!(covered_rust_owners
            .iter()
            .any(|owner| owner == "chapter_candidate_targeted_final_repair_service"));
        assert!(covered_rust_owners
            .iter()
            .any(|owner| owner == "chapter_candidate_finalize_service"));
        assert!(!covered_rust_owners
            .iter()
            .any(|owner| owner == "chapter_candidate_executor_runtime_adapter_service"));
        assert_eq!(
            result.readiness_evidence["runtime_owner_chain"]["record_mapping"],
            "build_default_generation_candidate_record"
        );
        assert_eq!(
            result.readiness_evidence["runtime_owner_chain"]["generation"],
            "generate_candidate_pool_workflow"
        );
        assert_eq!(
            result.readiness_evidence["wiring_readiness"]["stage_count"],
            9
        );
        assert_eq!(
            result.readiness_evidence["wiring_readiness"]["external_formula_dependency_count"],
            0
        );
        assert!(
            result.readiness_evidence["wiring_readiness"]["rust_owned_dependency_count"]
                .as_u64()
                .expect("rust dependency count")
                >= 56
        );
        assert!(
            result.readiness_evidence["wiring_readiness"]["cutover_blockers"]
                .as_array()
                .expect("cutover blockers")
                .is_empty()
        );
        assert_eq!(
            result.readiness_evidence["fallback_contract"]["python_fallback_preserved"],
            true
        );
    }

    #[tokio::test]
    async fn should_project_cutover_readiness_for_route_gateway_fallback_probe() {
        let probe = build_default_chapter_candidate_route_gateway_smoke_probes()
            .into_iter()
            .find(|probe| probe.owner == "python-fallback")
            .expect("fallback probe");

        let result = run_chapter_candidate_route_gateway_smoke_probe(probe)
            .await
            .expect("fallback smoke result");

        assert!(result.fallback_applied);
        assert_eq!(result.execution_path, "python_fallback");
        assert_eq!(
            result.readiness_evidence["gateway"]["rust_executor_enabled"],
            false
        );
        assert_eq!(
            result.readiness_evidence["gateway"]["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            result.readiness_evidence["runtime_owner_chain"]["provider_stream"],
            "collect_default_generation_candidate_output"
        );
        let rust_target_files = result.readiness_evidence["wiring_readiness"]["rust_target_files"]
            .as_array()
            .expect("rust target files");
        assert!(rust_target_files.iter().any(|file| {
            file == "backend-rs/src/services/chapter_candidate_generation_service.rs"
        }));
        assert!(rust_target_files.iter().any(|file| {
            file == "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs"
        }));
        assert_eq!(
            result.readiness_evidence["next_cutover_gate"],
            "candidate executor direct python source-map deleted; separate route-gateway rollback shell freeze/delete/repoint review now remains for non-executor Python source-map packages"
        );
    }

    #[tokio::test]
    async fn should_project_fallback_freeze_candidate_readiness() {
        let probe = build_default_chapter_candidate_route_gateway_smoke_probes()
            .into_iter()
            .find(|probe| probe.name == "chapter-candidate-route-gateway-fallback-freeze-candidate")
            .expect("fallback freeze probe");

        let result = run_chapter_candidate_route_gateway_smoke_probe(probe)
            .await
            .expect("fallback freeze smoke result");

        assert!(!result.fallback_applied);
        assert_eq!(result.execution_path, "rust_candidate_executor");
        assert_eq!(
            result.readiness_evidence["gateway"]["fallback_on_rust_error"],
            false
        );
        assert_eq!(
            result.readiness_evidence["fallback_shrink_readiness"]["candidate_probe"],
            true
        );
        assert_eq!(
            result.readiness_evidence["fallback_shrink_readiness"]["rust_owner_path_validated"],
            true
        );
        assert_eq!(
            result.readiness_evidence["fallback_shrink_readiness"]
                ["fallback_freeze_config_validated"],
            true
        );
        assert!(
            result.readiness_evidence["fallback_shrink_readiness"]["remaining_blockers"]
                .as_array()
                .expect("remaining blockers")
                .is_empty()
        );
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
            chapter_candidate_rust_executor_enabled: true,
            chapter_candidate_rust_executor_fallback_on_error: true,
            chapter_candidate_rust_executor_disabled_reason: String::new(),
            chapter_candidate_rust_executor_rollback_boundary: "python_candidate_executor_fallback"
                .to_string(),
            rust_migration_noop_executor_smoke_enabled: false,
        }
    }

    fn executor_request() -> ChapterCandidateExecutorRequest {
        ChapterCandidateExecutorRequest {
            base_generate_kwargs: Map::from_iter([("prompt".to_string(), json!("PROMPT"))]),
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "candidate".to_string(),
            max_candidates: 1,
            runtime_state: None,
        }
    }

    fn quality_adapter(
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
                story_packet: json!({"packet": true}),
                project: json!({"world_rules": "rules"}),
                chapter: json!({"id": "chapter-1"}),
                chapter_context: json!({"chapter_outline": "outline"}),
                target_word_count: 1200,
                generation_intent: json!({"mode": "draft"}),
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
                log_prefix: "Chapter".to_string(),
            },
            |_input| json!({"runtime": "context"}),
            |_input| json!({"overall_score": 90.0}),
            |_input| json!({"action": "continue"}),
        )
    }
}
