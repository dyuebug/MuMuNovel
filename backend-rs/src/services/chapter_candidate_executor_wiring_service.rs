// Staged Rust owner for Python chapter_candidate_executor_wiring_service.py.
// It captures the full executor dependency graph before the production path
// can consume Rust-owned candidate orchestration.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidateExecutorWiringOwner {
    RustOwner,
    ExternalFormulaCallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateExecutorWiringDependency {
    pub(crate) name: &'static str,
    pub(crate) owner: CandidateExecutorWiringOwner,
    pub(crate) target_owner: &'static str,
    pub(crate) required_for_cutover: bool,
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
            "backend-rs/src/services/chapter_candidate_executor_wiring_service.rs",
            "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs",
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_quality_adapter_service.rs",
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
    let dependencies = plan
        .stages
        .iter()
        .flat_map(|stage| stage.dependencies.iter());
    let mut rust_owned_dependency_count = 0_usize;
    let mut external_formula_dependency_count = 0_usize;
    let mut cutover_blockers = Vec::new();

    for dependency in dependencies {
        match dependency.owner {
            CandidateExecutorWiringOwner::RustOwner => rust_owned_dependency_count += 1,
            CandidateExecutorWiringOwner::ExternalFormulaCallback => {
                external_formula_dependency_count += 1;
                if dependency.required_for_cutover {
                    cutover_blockers.push(dependency.name);
                }
            }
        }
    }

    ChapterCandidateExecutorWiringReadiness {
        stage_count: plan.stages.len(),
        rust_owned_dependency_count,
        external_formula_dependency_count,
        cutover_blockers,
    }
}

pub(crate) fn validate_candidate_executor_wiring_plan(
    plan: &ChapterCandidateExecutorWiringPlan,
) -> Result<(), String> {
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
    for required_stage in required_stages {
        if !plan.stages.iter().any(|stage| stage.name == required_stage) {
            return Err(format!(
                "missing candidate executor wiring stage: {required_stage}"
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
        if stage.dependencies.is_empty() {
            return Err(format!(
                "candidate executor wiring stage has no dependencies: {}",
                stage.name
            ));
        }
    }

    Ok(())
}

fn route_gateway_smoke_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "route_gateway_smoke",
        owner_file: "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs",
        dependencies: vec![
            rust_dependency(
                "build_default_chapter_candidate_route_gateway_smoke_probes",
                "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs",
            ),
            rust_dependency(
                "run_chapter_candidate_route_gateway_smoke_suite",
                "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs",
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
                "backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs",
            ),
        ],
    }
}

fn quality_adapter_stage() -> CandidateExecutorWiringStage {
    CandidateExecutorWiringStage {
        name: "quality_adapter",
        owner_file: "backend-rs/src/services/chapter_candidate_quality_adapter_service.rs",
        dependencies: vec![
            rust_dependency(
                "build_chapter_candidate_quality_adapter",
                "backend-rs/src/services/chapter_candidate_quality_adapter_service.rs",
            ),
            rust_dependency(
                "evaluate_quality",
                "backend-rs/src/services/chapter_candidate_quality_adapter_service.rs",
            ),
            rust_dependency(
                "build_quality_gate_plan",
                "backend-rs/src/services/chapter_candidate_quality_adapter_service.rs",
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
                "generate_best_ranked_candidate_with_runtime_adapters",
                "backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs",
            ),
            rust_dependency(
                "generate_best_ranked_candidate_with_runtime_quality_adapters",
                "backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs",
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
    CandidateExecutorWiringDependency {
        name,
        owner: CandidateExecutorWiringOwner::RustOwner,
        target_owner,
        required_for_cutover: false,
    }
}

fn formula_dependency(name: &'static str) -> CandidateExecutorWiringDependency {
    CandidateExecutorWiringDependency {
        name,
        owner: CandidateExecutorWiringOwner::ExternalFormulaCallback,
        target_owner: "backend/app/services/chapter_candidate_rerank_service.py",
        required_for_cutover: true,
    }
}

fn rerank_dependency(name: &'static str) -> CandidateExecutorWiringDependency {
    CandidateExecutorWiringDependency {
        name,
        owner: CandidateExecutorWiringOwner::RustOwner,
        target_owner: "backend-rs/src/services/chapter_candidate_rerank_service.rs",
        required_for_cutover: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_default_chapter_candidate_executor_wiring_plan,
        resolve_candidate_executor_wiring_readiness, validate_candidate_executor_wiring_plan,
        CandidateExecutorWiringOwner,
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
        assert!(readiness.rust_owned_dependency_count >= 51);
        assert_eq!(readiness.external_formula_dependency_count, 0);
        assert!(readiness.cutover_blockers.is_empty());
    }

    #[test]
    fn should_validate_default_candidate_executor_wiring_plan() {
        let plan = build_default_chapter_candidate_executor_wiring_plan();

        validate_candidate_executor_wiring_plan(&plan).expect("default plan should be valid");
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
            output_dependency.owner,
            CandidateExecutorWiringOwner::RustOwner
        );
        assert!(!output_dependency.required_for_cutover);
    }
}
