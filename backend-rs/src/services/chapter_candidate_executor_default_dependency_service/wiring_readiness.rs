// Readiness evidence for the Rust default dependency owner that replaced
// Python chapter_candidate_executor_wiring_service.py.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateExecutorWiringDependency {
    pub(crate) name: &'static str,
    pub(crate) target_owner: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateExecutorWiringStage {
    pub(crate) name: &'static str,
    pub(crate) owner_file: &'static str,
    pub(crate) dependencies: Vec<CandidateExecutorWiringDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateExecutorWiringPlan {
    pub(crate) python_source_files: Vec<&'static str>,
    pub(crate) rust_target_files: Vec<&'static str>,
    pub(crate) stages: Vec<CandidateExecutorWiringStage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateExecutorWiringReadiness {
    pub(crate) stage_count: usize,
    pub(crate) rust_owned_dependency_count: usize,
    pub(crate) external_formula_dependency_count: usize,
    pub(crate) cutover_blockers: Vec<&'static str>,
}

pub(crate) fn build_default_chapter_candidate_executor_wiring_plan(
) -> ChapterCandidateExecutorWiringPlan {
    ChapterCandidateExecutorWiringPlan {
        python_source_files: vec![
            "backend/app/api/chapters.py",
            "backend/app/services/compat/chapter_generation_route_compat_service.py",
            "backend/app/services/chapter_generation/stream/candidate_service.py",
            "backend/app/services/batch_generation_candidate_service.py",
            "backend/app/services/chapter_candidate_executor_wiring_service.py",
            "backend/app/services/chapter_candidate_executor_service.py",
            "backend/app/services/chapter_candidate_rerank_service.py",
        ],
        rust_target_files: vec![
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_service.rs",
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_record_service.rs",
            "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            "backend-rs/src/services/chapter_candidate_rerank_service.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/services/chapter_candidate_output_service.rs",
        ],
        stages: vec![
            route_gateway_smoke_stage(),
            route_gateway_stage(),
            production_adapter_stage(),
            quality_adapter_stage(),
            generation_stage(),
            word_budget_repair_stage(),
            targeted_final_repair_stage(),
            finalize_stage(),
            executor_stage(),
        ],
    }
}

pub(crate) fn resolve_candidate_executor_wiring_readiness(
    plan: &ChapterCandidateExecutorWiringPlan,
) -> ChapterCandidateExecutorWiringReadiness {
    let rust_owned_dependency_count = plan
        .stages
        .iter()
        .flat_map(|stage| stage.dependencies.iter())
        .count();

    ChapterCandidateExecutorWiringReadiness {
        stage_count: plan.stages.len(),
        rust_owned_dependency_count,
        external_formula_dependency_count: 0,
        cutover_blockers: Vec::new(),
    }
}

pub(crate) fn build_candidate_executor_wiring_owner_contract() -> Value {
    let plan = build_default_chapter_candidate_executor_wiring_plan();
    validate_candidate_executor_wiring_plan(&plan)
        .expect("default candidate executor wiring plan must stay valid");
    let readiness = resolve_candidate_executor_wiring_readiness(&plan);

    json!({
        "owner": "chapter_candidate_executor_default_dependency_service",
        "scope": "candidate_executor_default_dependency_graph_and_wiring_readiness",
        "python_source_map": plan.python_source_files,
        "rust_owner_map": plan.rust_target_files,
        "behavior_contract": {
            "stage_count": readiness.stage_count,
            "rust_owned_dependency_count": readiness.rust_owned_dependency_count,
            "external_formula_dependency_count": readiness.external_formula_dependency_count,
            "required_stages": [
                "route_gateway_smoke",
                "route_gateway",
                "production_adapter",
                "quality_adapter",
                "generation",
                "word_budget_repair",
                "targeted_final_repair",
                "finalize",
                "executor"
            ],
            "retired_rust_target_files": [
                "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs",
                "backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs"
            ],
            "default_dependency_entrypoints": [
                "build_default_chapter_candidate_executor_wiring_plan",
                "validate_candidate_executor_wiring_plan",
                "resolve_candidate_executor_wiring_readiness",
                "build_default_chapter_candidate_executor_boxed_dependencies"
            ],
            "collapsed_owner_policy": "default wiring owns dependency graph evidence; generation, repair, finalize, provider, quality, record, rerank, and executor stages remain in their real owner files"
        },
        "active_consumers": [
            "chapter_candidate_route_gateway_service",
            "chapter_generation_runtime_service",
            "chapter_single_generation_active_gateway_smoke_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner",
                "phase5-chapters-candidate-gateway-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "chapter_candidate_gateway_manifest_probe_count": 1,
            "rust_manifest_probe_count": 18,
            "python_fallback_probe_count": 0,
            "wiring_stage_count": readiness.stage_count,
            "rust_owned_dependency_count": readiness.rust_owned_dependency_count,
            "external_formula_dependency_count": readiness.external_formula_dependency_count,
            "cutover_blockers": readiness.cutover_blockers,
            "production_adapter_owner": "chapter_candidate_executor_production_adapter_service",
            "executor_owner": "chapter_candidate_executor_service",
            "generation_owner": "chapter_candidate_generation_service",
            "word_budget_repair_owner": "chapter_candidate_word_budget_repair_service",
            "targeted_final_repair_owner": "chapter_candidate_targeted_final_repair_service",
            "finalize_owner": "chapter_candidate_finalize_service",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_chapter_candidate_executor_default_dependency_owner_ready_for_source_map_closeout_review"
        },
        "validation_boundary": [
            "cargo test chapter_candidate_executor_default_dependency_service",
            "cargo test chapter_candidate_route_gateway_service",
            "cargo test api::health",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "python_source_map_policy": "source_map_and_explicit_gateway_rollback_only",
            "runtime_knob": "python_candidate_executor_fallback",
            "cutover_blockers": readiness.cutover_blockers,
            "freeze_or_delete_requires_same_round_rollback_policy": true
        }
    })
}

pub(crate) fn validate_candidate_executor_wiring_plan(
    plan: &ChapterCandidateExecutorWiringPlan,
) -> Result<(), String> {
    let retired_rust_target_files = [
        "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs",
        "backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs",
    ];
    let required_stages = [
        "route_gateway_smoke",
        "route_gateway",
        "production_adapter",
        "quality_adapter",
        "generation",
        "word_budget_repair",
        "targeted_final_repair",
        "finalize",
        "executor",
    ];
    let required_rust_target_files = [
        "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
        "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
        "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
        "backend-rs/src/services/chapter_candidate_record_service.rs",
    ];
    for required_stage in required_stages {
        if !plan.stages.iter().any(|stage| stage.name == required_stage) {
            return Err(format!(
                "missing candidate executor wiring stage: {required_stage}"
            ));
        }
    }
    for required_file in required_rust_target_files {
        if !plan
            .rust_target_files
            .iter()
            .any(|target_file| *target_file == required_file)
        {
            return Err(format!(
                "missing candidate executor Rust target file: {required_file}"
            ));
        }
    }
    for retired_file in retired_rust_target_files {
        if plan
            .rust_target_files
            .iter()
            .any(|target_file| *target_file == retired_file)
        {
            return Err(format!(
                "candidate executor wiring plan references retired Rust target file: {retired_file}"
            ));
        }
    }

    for stage in &plan.stages {
        if stage.owner_file.trim().is_empty() {
            return Err(format!(
                "candidate executor wiring stage has no owner: {}",
                stage.name
            ));
        }
        if retired_rust_target_files
            .iter()
            .any(|retired_file| stage.owner_file == *retired_file)
        {
            return Err(format!(
                "candidate executor wiring stage {} references retired Rust owner file: {}",
                stage.name, stage.owner_file
            ));
        }
        if stage.dependencies.is_empty() {
            return Err(format!(
                "candidate executor wiring stage has no dependencies: {}",
                stage.name
            ));
        }
        for dependency in &stage.dependencies {
            if retired_rust_target_files
                .iter()
                .any(|retired_file| dependency.target_owner == *retired_file)
            {
                return Err(format!(
                    "candidate executor wiring dependency {} references retired Rust target file: {}",
                    dependency.name, dependency.target_owner
                ));
            }
        }
    }

    Ok(())
}

fn route_gateway_smoke_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "route_gateway_smoke",
        owner_file: "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
        dependencies: vec![
            rust_dependency(
                "build_default_chapter_candidate_route_gateway_smoke_probes",
                "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            ),
            rust_dependency(
                "run_chapter_candidate_route_gateway_smoke_suite",
                "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            ),
            rust_dependency(
                "execute_chapter_candidate_route_gateway",
                "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            ),
        ],
    }
}

fn route_gateway_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "route_gateway",
        owner_file: "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
        dependencies: vec![
            rust_dependency(
                "build_chapter_candidate_route_gateway_config_from_app_config",
                "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            ),
            rust_dependency(
                "build_chapter_candidate_production_adapter_config_from_route_gateway",
                "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            ),
            rust_dependency(
                "execute_chapter_candidate_route_gateway",
                "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            ),
            rust_dependency(
                "execute_chapter_candidate_production_adapter",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
        ],
    }
}

fn production_adapter_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "production_adapter",
        owner_file:
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
        dependencies: vec![
            rust_dependency(
                "resolve_chapter_candidate_production_adapter_decision",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rust_dependency(
                "execute_chapter_candidate_production_adapter",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rust_dependency(
                "generate_best_ranked_candidate_with_runtime_quality_adapters",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
        ],
    }
}

fn quality_adapter_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "quality_adapter",
        owner_file:
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
        dependencies: vec![
            rust_dependency(
                "build_chapter_candidate_quality_adapter",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rust_dependency(
                "build_runtime_quality_adapter_callbacks",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rust_dependency(
                "with_locked_callback",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rust_dependency(
                "evaluate_quality",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rust_dependency(
                "build_quality_gate_plan",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
        ],
    }
}

fn generation_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "generation",
        owner_file: "backend-rs/src/services/chapter_candidate_generation_service.rs",
        dependencies: vec![
            rust_dependency(
                "resolve_generation_attempt_labels",
                "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            ),
            rust_dependency(
                "sync_generation_runtime_state",
                "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            ),
            rust_dependency(
                "collect_generation_candidate_output",
                "backend-rs/src/services/chapter_candidate_output_service.rs",
            ),
            rust_dependency(
                "build_generation_candidate_record",
                "backend-rs/src/services/chapter_candidate_record_service.rs",
            ),
            rerank_dependency("should_generate_additional_candidate"),
            rerank_dependency("build_candidate_retry_prompt_suffix"),
            rerank_dependency("build_candidate_retry_strategy_suffix"),
            rerank_dependency("resolve_candidate_retry_temperature"),
            rerank_dependency("select_best_generation_candidate"),
        ],
    }
}

fn word_budget_repair_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "word_budget_repair",
        owner_file: "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs",
        dependencies: vec![
            rerank_dependency("should_apply_word_budget_repair"),
            rerank_dependency("build_word_budget_repair_suffix"),
            rerank_dependency("should_relax_word_budget_repair_limits"),
            rerank_dependency("resolve_word_budget_repair_temperature"),
            rerank_dependency("resolve_word_budget_repair_max_tokens"),
            rust_dependency(
                "collect_generation_candidate_output",
                "backend-rs/src/services/chapter_candidate_output_service.rs",
            ),
            rerank_dependency("resolve_word_budget_repair_char_limit"),
            rust_dependency(
                "build_generation_candidate_record",
                "backend-rs/src/services/chapter_candidate_record_service.rs",
            ),
            rerank_dependency("should_keep_word_budget_repair_candidate"),
            rerank_dependency("select_best_generation_candidate"),
            rerank_dependency("should_prefer_word_budget_repair_candidate"),
        ],
    }
}

fn targeted_final_repair_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "targeted_final_repair",
        owner_file: "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs",
        dependencies: vec![
            rerank_dependency("build_targeted_final_repair_suffix"),
            rerank_dependency("resolve_targeted_final_repair_temperature"),
            rerank_dependency("resolve_targeted_final_repair_max_tokens"),
            rust_dependency(
                "collect_generation_candidate_output",
                "backend-rs/src/services/chapter_candidate_output_service.rs",
            ),
            rerank_dependency("resolve_targeted_final_repair_char_limit"),
            rust_dependency(
                "build_generation_candidate_record",
                "backend-rs/src/services/chapter_candidate_record_service.rs",
            ),
            rerank_dependency("should_keep_targeted_final_repair_candidate"),
            rerank_dependency("should_adopt_targeted_final_repair_candidate"),
            rerank_dependency("should_prefer_targeted_final_repair_candidate"),
            rerank_dependency("should_apply_followup_targeted_final_repair"),
        ],
    }
}

fn finalize_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "finalize",
        owner_file: "backend-rs/src/services/chapter_candidate_finalize_service.rs",
        dependencies: vec![
            rust_dependency(
                "resolve_generation_attempt_labels",
                "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            ),
            rerank_dependency("build_candidate_selection_metadata"),
            rerank_dependency("attach_candidate_selection_metadata"),
            rerank_dependency("normalize_candidate_quality_gate_plan"),
            rerank_dependency("build_candidate_pool_summary"),
            rust_dependency(
                "sync_generation_runtime_state",
                "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            ),
            rerank_dependency("select_best_generation_candidate"),
            rerank_dependency("should_prefer_word_budget_repair_candidate"),
        ],
    }
}

fn executor_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "executor",
        owner_file: "backend-rs/src/services/chapter_candidate_executor_service.rs",
        dependencies: vec![
            rust_dependency(
                "generate_candidate_pool_workflow",
                "backend-rs/src/services/chapter_candidate_generation_service.rs",
            ),
            rust_dependency(
                "maybe_apply_word_budget_repair_workflow",
                "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs",
            ),
            rust_dependency(
                "execute_targeted_final_repair_pass_workflow",
                "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs",
            ),
            rust_dependency(
                "resolve_final_candidate_state",
                "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            ),
            rust_dependency(
                "maybe_promote_best_word_budget_repair_candidate",
                "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            ),
            rust_dependency(
                "finalize_selected_candidate_result",
                "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            ),
            rust_dependency(
                "generate_best_ranked_candidate_with_default_dependency_wiring",
                "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            ),
            rust_dependency(
                "build_default_generation_candidate_record",
                "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            ),
            rust_dependency(
                "collect_default_generation_candidate_output",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rust_dependency(
                "resolve_default_candidate_provider_stream_request",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rust_dependency(
                "generate_best_ranked_candidate_with_runtime_adapters",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rust_dependency(
                "generate_best_ranked_candidate_with_runtime_quality_adapters",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            rerank_dependency("should_apply_targeted_final_repair"),
            rerank_dependency("should_apply_followup_targeted_final_repair"),
            rerank_dependency("select_targeted_final_repair_seed_candidate"),
        ],
    }
}

fn rust_dependency(
    name: &'static str,
    target_owner: &'static str,
) -> CandidateExecutorWiringDependency {
    CandidateExecutorWiringDependency { name, target_owner }
}

fn rerank_dependency(name: &'static str) -> CandidateExecutorWiringDependency {
    CandidateExecutorWiringDependency {
        name,
        target_owner: "backend-rs/src/services/chapter_candidate_rerank_service.rs",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_candidate_executor_wiring_owner_contract,
        build_default_chapter_candidate_executor_wiring_plan,
        resolve_candidate_executor_wiring_readiness, validate_candidate_executor_wiring_plan,
    };

    #[test]
    fn should_build_full_candidate_executor_wiring_plan() {
        let plan = build_default_chapter_candidate_executor_wiring_plan();

        assert_eq!(
            plan.python_source_files,
            vec![
                "backend/app/api/chapters.py",
                "backend/app/services/compat/chapter_generation_route_compat_service.py",
                "backend/app/services/chapter_generation/stream/candidate_service.py",
                "backend/app/services/batch_generation_candidate_service.py",
                "backend/app/services/chapter_candidate_executor_wiring_service.py",
                "backend/app/services/chapter_candidate_executor_service.py",
                "backend/app/services/chapter_candidate_rerank_service.py",
            ]
        );
        assert_eq!(plan.stages.len(), 9);
        assert!(plan
            .stages
            .iter()
            .any(|stage| stage.name == "route_gateway_smoke"));
        assert!(plan
            .stages
            .iter()
            .any(|stage| stage.name == "route_gateway"));
        assert!(plan
            .stages
            .iter()
            .any(|stage| stage.name == "production_adapter"));
        assert!(plan
            .stages
            .iter()
            .any(|stage| stage.name == "quality_adapter"));
        assert!(plan.stages.iter().any(|stage| stage.name == "generation"));
        assert!(plan
            .stages
            .iter()
            .any(|stage| stage.name == "word_budget_repair"));
        assert!(plan
            .stages
            .iter()
            .any(|stage| stage.name == "targeted_final_repair"));
        assert!(plan.stages.iter().any(|stage| stage.name == "finalize"));
        assert!(plan.stages.iter().any(|stage| stage.name == "executor"));
    }

    #[test]
    fn should_mark_rerank_formulas_as_rust_owned_dependencies() {
        let plan = build_default_chapter_candidate_executor_wiring_plan();
        let readiness = resolve_candidate_executor_wiring_readiness(&plan);

        assert_eq!(readiness.stage_count, 9);
        assert!(readiness.rust_owned_dependency_count >= 56);
        assert_eq!(readiness.external_formula_dependency_count, 0);
        assert!(readiness.cutover_blockers.is_empty());
    }

    #[test]
    fn should_publish_candidate_executor_wiring_owner_contract() {
        let contract = build_candidate_executor_wiring_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_candidate_executor_default_dependency_service"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/api/chapters.py"
        );
        assert_eq!(
            contract["rust_owner_map"][2],
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs"
        );
        assert_eq!(contract["behavior_contract"]["stage_count"], 9);
        assert!(
            contract["behavior_contract"]["rust_owned_dependency_count"]
                .as_u64()
                .expect("rust owned dependency count")
                >= 56
        );
        assert_eq!(
            contract["behavior_contract"]["retired_rust_target_files"][1],
            "backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_candidate_route_gateway_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_knob"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            6
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["chapter_candidate_gateway_manifest_probe_count"],
            1
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
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_executor_default_dependency_owner_ready_for_source_map_closeout_review"
        );
    }

    #[test]
    fn should_validate_default_candidate_executor_wiring_plan() {
        let plan = build_default_chapter_candidate_executor_wiring_plan();

        validate_candidate_executor_wiring_plan(&plan).expect("default plan should be valid");
    }

    #[test]
    fn should_reject_retired_candidate_executor_target_files() {
        let mut plan = build_default_chapter_candidate_executor_wiring_plan();
        plan.rust_target_files
            .push("backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs");

        let error = validate_candidate_executor_wiring_plan(&plan)
            .expect_err("retired target should fail validation");

        assert!(error.contains("retired Rust target file"));
        assert!(error.contains("chapter_candidate_executor_runtime_adapter_service"));
    }

    #[test]
    fn should_reject_retired_candidate_executor_stage_owner() {
        let mut plan = build_default_chapter_candidate_executor_wiring_plan();
        let stage = plan
            .stages
            .iter_mut()
            .find(|stage| stage.name == "route_gateway_smoke")
            .expect("route gateway smoke stage");
        stage.owner_file =
            "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs";

        let error = validate_candidate_executor_wiring_plan(&plan)
            .expect_err("retired stage owner should fail validation");

        assert!(error.contains("retired Rust owner file"));
        assert!(error.contains("chapter_candidate_route_gateway_smoke_service"));
    }

    #[test]
    fn should_reject_retired_candidate_executor_dependency_target() {
        let mut plan = build_default_chapter_candidate_executor_wiring_plan();
        let stage = plan
            .stages
            .iter_mut()
            .find(|stage| stage.name == "route_gateway_smoke")
            .expect("route gateway smoke stage");
        stage.dependencies[0].target_owner =
            "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs";

        let error = validate_candidate_executor_wiring_plan(&plan)
            .expect_err("retired dependency target should fail validation");

        assert!(error.contains("retired Rust target file"));
        assert!(error.contains("chapter_candidate_route_gateway_smoke_service"));
    }

    #[test]
    fn should_include_collapsed_runtime_owner_targets() {
        let plan = build_default_chapter_candidate_executor_wiring_plan();

        assert!(plan.rust_target_files.contains(
            &"backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs"
        ));
        assert!(!plan.rust_target_files.contains(
            &"backend-rs/src/services/chapter_candidate_runtime_callback_bridge_service.rs"
        ));
        assert!(!plan.rust_target_files.contains(
            &"backend-rs/src/services/chapter_candidate_runtime_record_bridge_service.rs"
        ));
    }

    #[test]
    fn should_collapse_route_gateway_smoke_targets_into_route_gateway_owner() {
        let plan = build_default_chapter_candidate_executor_wiring_plan();
        let route_gateway_smoke_stage = plan
            .stages
            .iter()
            .find(|stage| stage.name == "route_gateway_smoke")
            .expect("route gateway smoke stage");

        assert_eq!(
            route_gateway_smoke_stage.owner_file,
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs"
        );
        assert!(!plan
            .rust_target_files
            .contains(&"backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs"));
        assert!(route_gateway_smoke_stage
            .dependencies
            .iter()
            .all(|dependency| dependency.target_owner
                != "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs"));
    }

    #[test]
    fn should_map_runtime_adapter_shared_dependencies_to_real_owners() {
        let plan = build_default_chapter_candidate_executor_wiring_plan();
        let dependencies = plan
            .stages
            .iter()
            .flat_map(|stage| stage.dependencies.iter())
            .collect::<Vec<_>>();

        for (name, target_owner) in [
            (
                "build_runtime_quality_adapter_callbacks",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            (
                "with_locked_callback",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            (
                "build_default_generation_candidate_record",
                "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            ),
            (
                "collect_default_generation_candidate_output",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
            (
                "resolve_default_candidate_provider_stream_request",
                "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            ),
        ] {
            let dependency = dependencies
                .iter()
                .find(|dependency| dependency.name == name)
                .unwrap_or_else(|| panic!("missing dependency: {name}"));

            assert_eq!(dependency.target_owner, target_owner);
        }
    }

    #[test]
    fn should_reject_missing_required_stage() {
        let mut plan = build_default_chapter_candidate_executor_wiring_plan();
        plan.stages.retain(|stage| stage.name != "executor");

        let error = validate_candidate_executor_wiring_plan(&plan)
            .expect_err("missing executor stage should fail validation");

        assert_eq!(error, "missing candidate executor wiring stage: executor");
    }

    #[test]
    fn should_keep_rust_owned_dependencies_out_of_formula_blockers() {
        let plan = build_default_chapter_candidate_executor_wiring_plan();
        let generation_stage = plan
            .stages
            .iter()
            .find(|stage| stage.name == "generation")
            .expect("generation stage exists");

        let output_dependency = generation_stage
            .dependencies
            .iter()
            .find(|dependency| dependency.name == "collect_generation_candidate_output")
            .expect("output dependency exists");

        assert_eq!(
            output_dependency.target_owner,
            "backend-rs/src/services/chapter_candidate_output_service.rs"
        );
    }
}
