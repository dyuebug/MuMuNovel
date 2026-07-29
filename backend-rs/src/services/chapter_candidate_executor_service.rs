// Rust owner for candidate executor orchestration originally mapped from
// Python chapter_candidate_executor_service.py. This module owns the
// generation -> word-budget repair -> targeted repair -> finalize sequence and
// keeps stage dependencies injectable for tests, default wiring, and route
// gateway rollback boundaries.

#[cfg(test)]
use serde_json::Value;

pub(crate) mod executor_owner;

#[cfg(test)]
pub(crate) use self::executor_owner::{
    generate_best_ranked_candidate_workflow, ChapterCandidateExecutorDependencies,
};
pub(crate) use self::executor_owner::{
    generate_best_ranked_candidate_workflow_with_boxed_dependencies,
    ChapterCandidateExecutorBoxedDependencies, ChapterCandidateExecutorFinalizeInput,
    ChapterCandidateExecutorRequest,
};

#[cfg(test)]
fn build_chapter_candidate_executor_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_candidate_executor_service",
        "scope": "candidate_generation_repair_finalize_and_post_finalize_orchestration_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_executor_service.rs",
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            "backend-rs/src/services/chapter_candidate_output_service.rs",
            "backend-rs/src/services/chapter_candidate_record_service.rs",
            "backend-rs/src/services/chapter_candidate_rerank_service.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "generate_best_ranked_candidate_workflow",
                "generate_best_ranked_candidate_workflow_with_boxed_dependencies"
            ],
            "stage_order": [
                "generation candidate pool",
                "word-budget repair",
                "optional pre-finalize targeted final repair",
                "finalize with word-budget repair promotion enabled",
                "optional post-finalize targeted final repair",
                "optional follow-up targeted final repair",
                "finalize with word-budget repair promotion disabled",
                "final result projection"
            ],
            "request_fields": [
                "base_generate_kwargs",
                "target_word_count",
                "source",
                "generation_label",
                "max_candidates",
                "runtime_state",
                "repair_generation_contract"
            ],
            "runtime_state_policy": [
                "runtime_state is moved through each stage request and restored to the executor request",
                "finalize receives the latest runtime_state and writes the final runtime_state back",
                "missing runtime_state is allowed by every stage owner"
            ],
            "targeted_repair_policy": [
                "pre-finalize targeted repair runs only when should_apply_targeted_final_repair accepts the selected candidate",
                "deferred targeted repair seed is preferred after finalize before selecting a new seed",
                "winner already marked targeted_quality_repair blocks selecting another post-finalize seed",
                "follow-up targeted repair runs only after post-finalize repair when follow-up policy accepts the finalized candidate"
            ],
            "finalize_policy": [
                "first finalize allows word-budget repair promotion",
                "post-targeted and final finalize disable word-budget repair promotion",
                "final result projection is delegated to the finalize owner"
            ],
            "dependency_policy": [
                "generic test dependencies and boxed production dependencies share the same orchestration order",
                "stage owners remain injectable for default wiring, production adapter, route gateway, and focused tests"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_executor_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_executor_default_dependency_service",
            "chapter_candidate_executor_production_adapter_service",
            "chapter_candidate_route_gateway_service",
            "chapter_batch_generation_active_gateway_smoke_service",
            "chapter_single_generation_active_gateway_smoke_service"
        ],
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner",
                "phase5-chapters-candidate-gateway-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "candidate_gateway_manifest_probe_count": 1,
            "rust_manifest_probe_count": 18,
            "python_fallback_probe_count": 0,
            "executor_stage_owner": "generate_best_ranked_candidate_workflow_with_boxed_dependencies",
            "generation_owner": "chapter_candidate_generation_service",
            "word_budget_repair_owner": "chapter_candidate_word_budget_repair_service",
            "targeted_final_repair_owner": "chapter_candidate_targeted_final_repair_service",
            "finalize_owner": "chapter_candidate_finalize_service",
            "production_adapter_owner": "chapter_candidate_executor_production_adapter_service",
            "route_gateway_owner": "chapter_candidate_route_gateway_service",
            "active_route_gateway_consumers": [
                "chapter-candidate-route-gateway-smoke-rust",
                "chapter-single-generation-active-gateway-smoke-rust",
                "chapter-batch-generation-active-gateway-smoke-rust"
            ],
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "candidate executor direct python source-map deleted; surviving Python closeout work is now limited to rerank compatibility packages and route-gateway rollback shells",
            "status": "rust_candidate_executor_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_executor_python_source_map",
            "python_fallback_removal_ready": true,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        }
    })
}

#[cfg(test)]
mod tests {
    use std::future;

    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_executor_owner_contract, generate_best_ranked_candidate_workflow,
        ChapterCandidateExecutorDependencies, ChapterCandidateExecutorFinalizeInput,
        ChapterCandidateExecutorRequest,
    };
    use crate::services::chapter_candidate_finalize_service::{
        ChapterCandidateFinalizeRequest, ChapterCandidateFinalizeState,
    };
    use crate::services::chapter_candidate_generation_service::{
        ChapterCandidateGenerationRequest, ChapterCandidateGenerationResult,
    };
    use crate::services::chapter_candidate_targeted_final_repair_service::ChapterCandidateTargetedFinalRepairResult;
    use crate::services::chapter_candidate_word_budget_repair_service::{
        ChapterCandidateWordBudgetRepairRequest, ChapterCandidateWordBudgetRepairResult,
    };

    fn base_request() -> ChapterCandidateExecutorRequest {
        let mut base_generate_kwargs = Map::new();
        base_generate_kwargs.insert(
            "prompt".to_string(),
            Value::String("Base prompt".to_string()),
        );
        base_generate_kwargs.insert("temperature".to_string(), Value::String("0.62".to_string()));
        ChapterCandidateExecutorRequest {
            base_generate_kwargs,
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "candidate".to_string(),
            max_candidates: 2,
            runtime_state: Some(json!({"seed": true})),
            repair_generation_contract: None,
        }
    }

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
    fn should_publish_chapter_candidate_executor_owner_contract() {
        let contract = build_chapter_candidate_executor_owner_contract();
        assert_no_deleted_python_service_source_map(&contract);

        assert_eq!(contract["owner"], "chapter_candidate_executor_service");
        assert_eq!(
            contract["scope"],
            "candidate_generation_repair_finalize_and_post_finalize_orchestration_owner"
        );
        assert!(contract["rust_owner_map"]
            .as_array()
            .expect("rust owner map")
            .contains(&json!(
                "backend-rs/src/services/chapter_candidate_executor_service.rs"
            )));
        assert!(contract["behavior_contract"]["stage_order"]
            .as_array()
            .expect("stage order")
            .contains(&json!("optional follow-up targeted final repair")));
        assert!(contract["behavior_contract"]["entrypoints"]
            .as_array()
            .expect("entrypoints")
            .contains(&json!(
                "generate_best_ranked_candidate_workflow_with_boxed_dependencies"
            )));
        assert!(contract["validation_boundary"]
            .as_array()
            .expect("validation boundary")
            .contains(&json!(
                "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
            )));
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][1],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][2],
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
            contract["service_runtime_closeout_status"]["executor_stage_owner"],
            "generate_best_ranked_candidate_workflow_with_boxed_dependencies"
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
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "candidate executor direct python source-map deleted; surviving Python closeout work is now limited to rerank compatibility packages and route-gateway rollback shells"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_candidate_executor_owner_source_map_deleted"
        );
    }

    #[tokio::test]
    async fn should_run_candidate_executor_owner_chain() {
        let mut request = base_request();
        let mut dependencies = dependencies(true, true, false, None);

        let result = generate_best_ranked_candidate_workflow(&mut request, &mut dependencies)
            .await
            .expect("executor should finish");

        assert_eq!(result["finalized"], true);
        assert_eq!(result["candidate_index"], 5);
        assert_eq!(request.runtime_state.as_ref().unwrap()["finalized"], true);
    }

    #[tokio::test]
    async fn should_prefer_deferred_post_finalize_targeted_seed() {
        let mut request = base_request();
        let deferred_seed = json!({"candidate_index": 88, "attempt_kind": "deferred_seed"});
        let mut dependencies = dependencies(false, false, false, Some(deferred_seed.clone()));

        let result = generate_best_ranked_candidate_workflow(&mut request, &mut dependencies)
            .await
            .expect("executor should finish");

        assert_eq!(result["finalized"], true);
        assert_eq!(result["candidate_index"], 3);
    }

    #[tokio::test]
    async fn should_skip_new_post_finalize_seed_when_winner_is_targeted_repair() {
        let mut request = base_request();
        let mut dependencies = dependencies(false, false, true, None);

        let result = generate_best_ranked_candidate_workflow(&mut request, &mut dependencies)
            .await
            .expect("executor should finish");

        assert_eq!(result["finalized"], true);
        assert_eq!(result["candidate_index"], 2);
    }

    fn dependencies(
        apply_pre_targeted: bool,
        followup_after_finalize: bool,
        final_winner_is_targeted: bool,
        deferred_seed: Option<Value>,
    ) -> ChapterCandidateExecutorDependencies<
        impl FnMut(
            &mut crate::services::chapter_candidate_generation_service::ChapterCandidateGenerationRequest,
        ) -> future::Ready<Result<ChapterCandidateGenerationResult, String>>,
        impl FnMut(
            &mut crate::services::chapter_candidate_word_budget_repair_service::ChapterCandidateWordBudgetRepairRequest,
            Value,
            Vec<Value>,
        ) -> future::Ready<ChapterCandidateWordBudgetRepairResult>,
        impl FnMut(
            &mut crate::services::chapter_candidate_targeted_final_repair_service::ChapterCandidateTargetedFinalRepairRequest,
            Value,
            Vec<Value>,
        ) -> future::Ready<ChapterCandidateTargetedFinalRepairResult>,
        impl FnMut(ChapterCandidateExecutorFinalizeInput) -> ChapterCandidateFinalizeState,
        impl FnMut(
            &mut crate::services::chapter_candidate_finalize_service::ChapterCandidateFinalizeRequest,
            ChapterCandidateFinalizeState,
        ) -> Value,
        impl FnMut(Value) -> bool,
        impl FnMut(Value) -> bool,
        impl FnMut(Value, Vec<Value>) -> Option<Value>,
    >{
        let mut targeted_calls = 0_i64;
        ChapterCandidateExecutorDependencies {
            generate_candidate_pool_fn: |request: &mut ChapterCandidateGenerationRequest| {
                assert_eq!(request.base_prompt, "Base prompt");
                assert_eq!(request.base_temperature, 0.62);
                request.runtime_state = Some(json!({"generation": true}));
                future::ready(Ok(ChapterCandidateGenerationResult {
                    selected_candidate: json!({
                        "candidate_index": 1,
                        "attempt_kind": "initial_candidate",
                        "generation_path": "single_pass"
                    }),
                    candidates: vec![json!({"candidate_index": 1})],
                }))
            },
            maybe_apply_word_budget_repair_fn: move |
                request: &mut ChapterCandidateWordBudgetRepairRequest,
                selected: Value,
                mut candidates: Vec<Value>,
            | {
                assert_eq!(request.base_temperature, 0.62);
                request.runtime_state = Some(json!({"word_budget": true}));
                let repair = json!({
                    "candidate_index": 2,
                    "attempt_kind": "word_budget_repair",
                    "generation_path": "word_budget_repair"
                });
                candidates.push(repair.clone());
                future::ready(ChapterCandidateWordBudgetRepairResult {
                    selected_candidate: if apply_pre_targeted { selected } else { repair },
                    candidates,
                    word_budget_repair_used: true,
                })
            },
            execute_targeted_final_repair_pass_fn: move |
                request: &mut crate::services::chapter_candidate_targeted_final_repair_service::ChapterCandidateTargetedFinalRepairRequest,
                _selected: Value,
                mut candidates: Vec<Value>,
            | {
                targeted_calls += 1;
                request.runtime_state = Some(json!({"targeted": targeted_calls}));
                let candidate = json!({
                    "candidate_index": 2 + targeted_calls,
                    "attempt_kind": "targeted_quality_repair",
                    "generation_path": "targeted_quality_repair",
                    "label_suffix": request.generation_label_suffix,
                });
                candidates.push(candidate.clone());
                let deferred = if request.allow_followup_seed_defer {
                    deferred_seed.clone()
                } else {
                    None
                };
                future::ready(ChapterCandidateTargetedFinalRepairResult {
                    selected_candidate: candidate,
                    candidates,
                    deferred_followup_targeted_repair_seed_candidate: deferred,
                })
            },
            resolve_candidate_finalize_state_fn: move |input: ChapterCandidateExecutorFinalizeInput| {
                let mut selected = input.selected_candidate;
                if final_winner_is_targeted {
                    selected["attempt_kind"] = json!("targeted_quality_repair");
                    selected["generation_path"] = json!("targeted_quality_repair");
                }
                ChapterCandidateFinalizeState {
                    selected_candidate: selected,
                    candidates: input.candidates,
                    winner_candidate_index: 1,
                    final_attempt_kind: "initial_candidate".to_string(),
                    final_generation_path: "single_pass".to_string(),
                    final_quality_metrics: Map::new(),
                    final_quality_gate_plan: Map::new(),
                    rerank_used: false,
                    word_budget_repair_used: false,
                }
            },
            finalize_selected_candidate_result_fn:
                |request: &mut ChapterCandidateFinalizeRequest, state: ChapterCandidateFinalizeState| {
                request.runtime_state = Some(json!({"finalized": true}));
                let mut result = state.selected_candidate;
                result["finalized"] = json!(true);
                result
            },
            should_apply_targeted_final_repair_fn: move |_candidate| apply_pre_targeted,
            should_apply_followup_targeted_final_repair_fn: move |_candidate| {
                followup_after_finalize
            },
            select_targeted_final_repair_seed_candidate_fn: |_selected, _candidates| {
                Some(json!({"candidate_index": 77, "attempt_kind": "selected_seed"}))
            },
        }
    }
}
