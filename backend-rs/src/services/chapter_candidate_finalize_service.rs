// Rust owner for final-candidate orchestration originally mapped from Python
// chapter_candidate_finalize_service.py. The default executor dependency owner
// resolves and finalizes candidates through this module directly.

use serde_json::{json, Value};

pub(crate) mod finalize_owner;

pub(crate) use self::finalize_owner::{
    build_default_finalize_dependencies, finalize_selected_candidate_result,
    maybe_promote_best_word_budget_repair_candidate, resolve_final_candidate_state,
    ChapterCandidateFinalizeRequest, ChapterCandidateFinalizeState,
};
#[cfg(test)]
use self::finalize_owner::{
    snapshot_finalize_state, ChapterCandidateFinalizeDependencies,
    ChapterCandidateFinalizeMetadataContext,
};

pub(crate) fn build_chapter_candidate_finalize_owner_contract() -> Value {
    json!({
        "owner": "chapter_candidate_finalize_service",
        "scope": "candidate_final_selection_quality_gate_pool_summary_and_runtime_sync_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            "backend-rs/src/services/chapter_candidate_rerank_service.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/services/chapter_candidate_record_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_default_finalize_dependencies",
                "resolve_final_candidate_state",
                "maybe_promote_best_word_budget_repair_candidate",
                "finalize_selected_candidate_result"
            ],
            "metadata_policy": [
                "selection metadata is attached before and after quality-gate normalization",
                "candidate_count is normalized through the selected candidate set",
                "generation_path and attempt_kind fall back to runtime attempt labels when candidate fields are blank",
                "rerank_used is true only for non-repair winners beyond the first candidate",
                "word_budget_repair_used is derived from candidate generation_path or attempt_kind"
            ],
            "quality_gate_policy": [
                "quality gate builder consumes final quality metrics with attempt offset zero",
                "normalized quality_gate is copied back into quality_metrics",
                "quality-gate plan stays on the selected candidate result"
            ],
            "word_budget_repair_promotion_policy": [
                "allow_save keeps the selected candidate",
                "manual-review or retry final gate can promote the best word-budget repair candidate",
                "promotion is skipped when no repair candidate exists, the selected candidate is already repair, or preference returns false"
            ],
            "result_projection_policy": [
                "candidate_count and rerank_pool_size mirror the final candidate pool",
                "candidate_pool_summary is built by the rerank owner and copied into quality_metrics",
                "repair_seed_candidate_index is preserved when present in final selection metadata"
            ],
            "runtime_state_policy": [
                "runtime state sync records winner index, total candidates, current chars, chunk count, generation_path, attempt_kind, rerank flag, and repair flag",
                "missing runtime_state is a no-op rather than an error"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_finalize_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_executor_default_dependency_service",
            "chapter_candidate_executor_service",
            "chapter_candidate_executor_production_adapter_service",
            "chapter_candidate_route_gateway_service",
            "chapter-batch-generation-active-gateway-smoke-rust",
            "chapter-single-generation-active-gateway-smoke-rust"
        ],
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 17,
            "python_fallback_probe_count": 0,
            "default_dependencies_owner": "build_default_finalize_dependencies",
            "final_candidate_state_owner": "resolve_final_candidate_state",
            "word_budget_repair_promotion_owner": "maybe_promote_best_word_budget_repair_candidate",
            "runtime_state_sync_owner": "finalize_selected_candidate_result",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "candidate finalize production python source-map deleted; this owner is now Rust-only on the active path",
            "status": "rust_chapter_candidate_finalize_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_finalize_python_source_map",
            "python_fallback_removal_ready": true,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_chapter_candidate_finalize_owner_contract, build_default_finalize_dependencies,
        finalize_selected_candidate_result, maybe_promote_best_word_budget_repair_candidate,
        resolve_final_candidate_state, snapshot_finalize_state,
        ChapterCandidateFinalizeDependencies, ChapterCandidateFinalizeRequest,
        ChapterCandidateFinalizeState,
    };

    fn build_dependencies() -> ChapterCandidateFinalizeDependencies<
        impl FnMut(Value, super::ChapterCandidateFinalizeMetadataContext) -> Value,
        impl FnMut(Value, Value) -> Value,
        impl FnMut(Value, i64, i64, Value) -> Value,
        impl FnMut(Vec<Value>, i64, Option<i64>) -> Value,
        impl FnMut(Vec<Value>) -> Option<Value>,
        impl FnMut(Value, Value) -> bool,
    > {
        ChapterCandidateFinalizeDependencies {
            build_candidate_selection_metadata_fn:
                |_quality_metrics: Value,
                 context: super::ChapterCandidateFinalizeMetadataContext| {
                    json!({
                        "candidate_index": context.candidate_index,
                        "candidate_count": context.candidate_count,
                        "generation_path": context.generation_path,
                        "attempt_kind": context.attempt_kind,
                        "rerank_used": context.rerank_used,
                        "word_budget_repair_used": context.word_budget_repair_used,
                        "winner_candidate_index": context.winner_candidate_index,
                    })
                },
            attach_candidate_selection_metadata_fn:
                |quality_metrics: Value, selection_metadata: Value| {
                    let mut metrics = quality_metrics.as_object().cloned().unwrap_or_default();
                    metrics.insert("candidate_selection".to_string(), selection_metadata);
                    Value::Object(metrics)
                },
            normalize_candidate_quality_gate_plan_fn: |plan, _word_count, _target, _metrics| plan,
            build_candidate_pool_summary_fn:
                |candidates: Vec<Value>, winner_candidate_index: i64, _repair_seed: Option<i64>| {
                    Value::Array(
                        candidates
                            .into_iter()
                            .map(|candidate| {
                                let candidate_index =
                                    candidate["candidate_index"].as_i64().unwrap_or(0);
                                json!({
                                    "candidate_index": candidate_index,
                                    "is_winner": candidate_index == winner_candidate_index,
                                })
                            })
                            .collect(),
                    )
                },
            select_best_generation_candidate_fn: |candidates: Vec<Value>| {
                candidates.last().cloned()
            },
            should_prefer_word_budget_repair_candidate_fn: |_selected, _repair| true,
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
    fn should_publish_chapter_candidate_finalize_owner_contract() {
        let contract = build_chapter_candidate_finalize_owner_contract();
        assert_no_deleted_python_service_source_map(&contract);

        assert_eq!(contract["owner"], "chapter_candidate_finalize_service");
        assert_eq!(
            contract["scope"],
            "candidate_final_selection_quality_gate_pool_summary_and_runtime_sync_owner"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .map(|items| items.len()),
            Some(0)
        );
        assert!(contract["rust_owner_map"]
            .as_array()
            .expect("rust owner map")
            .contains(&json!(
                "backend-rs/src/services/chapter_candidate_finalize_service.rs"
            )));
        assert!(contract["behavior_contract"]["entrypoints"]
            .as_array()
            .expect("entrypoints")
            .contains(&json!("finalize_selected_candidate_result")));
        assert!(
            contract["behavior_contract"]["word_budget_repair_promotion_policy"]
                .as_array()
                .expect("word-budget policy")
                .iter()
                .any(|policy| policy.as_str().unwrap_or_default().contains("allow_save"))
        );
        assert!(contract["validation_boundary"]
            .as_array()
            .expect("validation boundary")
            .contains(&json!(
                "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
            )));
        assert_eq!(
            contract["active_consumers"][4],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["active_consumers"][5],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
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
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            json!(11)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            json!(17)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["default_dependencies_owner"],
            "build_default_finalize_dependencies"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["final_candidate_state_owner"],
            "resolve_final_candidate_state"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["word_budget_repair_promotion_owner"],
            "maybe_promote_best_word_budget_repair_candidate"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["runtime_state_sync_owner"],
            "finalize_selected_candidate_result"
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
            "rust_chapter_candidate_finalize_owner_source_map_deleted"
        );
    }

    #[test]
    fn should_snapshot_finalize_state_from_finalize_payloads() {
        let state = ChapterCandidateFinalizeState {
            selected_candidate: json!({"candidate_index": 2}),
            candidates: vec![json!({"candidate_index": 1}), json!({"candidate_index": 2})],
            winner_candidate_index: 2,
            final_attempt_kind: "word_budget_repair".to_string(),
            final_generation_path: "word_budget_repair".to_string(),
            final_quality_metrics: json!({
                "candidate_selection": {"repair_seed_candidate_index": 1}
            })
            .as_object()
            .cloned()
            .unwrap_or_default(),
            final_quality_gate_plan: json!({
                "quality_gate": {"decision": "manual_review"}
            })
            .as_object()
            .cloned()
            .unwrap_or_default(),
            rerank_used: false,
            word_budget_repair_used: true,
        };

        let view = snapshot_finalize_state(&state);

        assert_eq!(view.final_quality_gate_decision, "manual_review");
        assert_eq!(view.repair_seed_candidate_index, Some(1));
    }

    #[test]
    fn should_resolve_final_candidate_state_with_word_budget_repair_metadata() {
        let mut dependencies = build_dependencies();
        let request = ChapterCandidateFinalizeRequest {
            target_word_count: 1200,
            source: "chapter".to_string(),
            runtime_state: None,
        };
        let selected_candidate = json!({
            "candidate_index": 2,
            "attempt_kind": "word_budget_repair",
            "generation_path": "word_budget_repair",
            "word_count": 1260,
            "quality_metrics": {"overall_score": 88},
            "quality_gate_plan": {"action": "continue", "quality_gate": {"decision": "allow_save"}},
            "candidate_chunks": ["chunk-a"]
        });
        let candidates = vec![
            json!({"candidate_index": 1, "attempt_kind": "initial_candidate", "generation_path": "single_pass"}),
            selected_candidate.clone(),
        ];
        let mut quality_gate_plan_builder = |_metrics: Value, _attempt_offset: i64| json!({"action": "continue", "quality_gate": {"decision": "allow_save"}});

        let state = resolve_final_candidate_state(
            &request,
            selected_candidate,
            candidates,
            &mut quality_gate_plan_builder,
            &mut dependencies,
        );

        assert_eq!(state.winner_candidate_index, 2);
        assert_eq!(state.final_attempt_kind, "word_budget_repair");
        assert_eq!(state.final_generation_path, "word_budget_repair");
        assert!(state.word_budget_repair_used);
        assert!(!state.rerank_used);
        assert_eq!(state.selected_candidate["winner_candidate_index"], 2);
        assert_eq!(
            state.final_quality_metrics["candidate_selection"]["generation_path"],
            "word_budget_repair"
        );
    }

    #[test]
    fn should_finalize_selected_candidate_result_and_sync_runtime_state() {
        let mut dependencies = build_dependencies();
        let mut request = ChapterCandidateFinalizeRequest {
            target_word_count: 1200,
            source: "chapter".to_string(),
            runtime_state: Some(json!({})),
        };
        let selected_candidate = json!({
            "candidate_index": 2,
            "candidate_count": 2,
            "winner_candidate_index": 2,
            "word_count": 1260,
            "generation_path": "word_budget_repair",
            "attempt_kind": "word_budget_repair",
            "rerank_used": false,
            "word_budget_repair_used": true,
            "candidate_chunks": ["chunk-a", "chunk-b"],
            "quality_metrics": {"candidate_selection": {"repair_seed_candidate_index": 1}},
            "quality_gate_plan": {"action": "continue", "quality_gate": {"decision": "allow_save"}}
        });
        let mut quality_gate_plan_builder = |_metrics: Value, _attempt_offset: i64| json!({"action": "continue", "quality_gate": {"decision": "allow_save"}});
        let state = resolve_final_candidate_state(
            &request,
            selected_candidate.clone(),
            vec![json!({"candidate_index": 1}), selected_candidate],
            &mut quality_gate_plan_builder,
            &mut dependencies,
        );

        let result = finalize_selected_candidate_result(&mut request, state, &mut dependencies);

        assert_eq!(result["candidate_count"], 2);
        assert_eq!(result["rerank_pool_size"], 2);
        assert_eq!(
            result["quality_metrics"]["candidate_pool_summary"][1]["is_winner"],
            true
        );
        let runtime_state = request.runtime_state.as_ref().expect("runtime");
        assert_eq!(runtime_state["winner_candidate_index"], 2);
        assert_eq!(runtime_state["current_chars"], 1260);
        assert_eq!(runtime_state["chunk_count"], 2);
    }

    #[test]
    fn should_promote_preferred_word_budget_repair_candidate() {
        let mut dependencies = build_dependencies();
        let request = ChapterCandidateFinalizeRequest {
            target_word_count: 1200,
            source: "chapter".to_string(),
            runtime_state: None,
        };
        let candidates = vec![
            json!({
                "candidate_index": 1,
                "attempt_kind": "initial_candidate",
                "generation_path": "single_pass",
                "word_count": 1800,
                "quality_gate_plan": {"quality_gate": {"decision": "manual_review"}},
                "quality_metrics": {}
            }),
            json!({
                "candidate_index": 2,
                "attempt_kind": "word_budget_repair",
                "generation_path": "word_budget_repair",
                "word_count": 1260,
                "quality_gate_plan": {"quality_gate": {"decision": "allow_save"}},
                "quality_metrics": {}
            }),
        ];
        let mut quality_gate_plan_builder = |_metrics: Value, _attempt_offset: i64| json!({"quality_gate": {"decision": "manual_review"}});
        let state = resolve_final_candidate_state(
            &request,
            candidates[0].clone(),
            candidates,
            &mut quality_gate_plan_builder,
            &mut dependencies,
        );

        let promoted = maybe_promote_best_word_budget_repair_candidate(
            &request,
            state,
            &mut quality_gate_plan_builder,
            &mut dependencies,
        );

        assert_eq!(promoted.winner_candidate_index, 2);
        assert!(promoted.word_budget_repair_used);
    }

    #[test]
    fn should_build_default_finalize_dependencies_from_finalize_owner() {
        let mut dependencies = build_default_finalize_dependencies();
        let request = ChapterCandidateFinalizeRequest {
            target_word_count: 1200,
            source: "chapter".to_string(),
            runtime_state: None,
        };
        let selected_candidate = json!({
            "candidate_index": 1,
            "attempt_kind": "initial_candidate",
            "generation_path": "single_pass",
            "word_count": 1210,
            "quality_metrics": {
                "overall_score": 92.0,
                "quality_gate": {"decision": "allow_save", "status": "pass"}
            },
            "quality_gate_plan": {"quality_gate": {"decision": "allow_save", "status": "pass"}},
            "candidate_chunks": ["chunk-a"]
        });
        let mut quality_gate_plan_builder = |metrics: Value, _attempt_offset: i64| json!({"quality_gate": metrics["quality_gate"].clone()});

        let state = resolve_final_candidate_state(
            &request,
            selected_candidate,
            vec![json!({"candidate_index": 1})],
            &mut quality_gate_plan_builder,
            &mut dependencies,
        );

        assert_eq!(state.winner_candidate_index, 1);
        assert_eq!(
            state.final_quality_metrics["candidate_selection"]["generation_path"],
            "single_pass"
        );
        assert_eq!(
            state.final_quality_metrics["quality_gate"]["decision"],
            "allow_save"
        );
    }
}
