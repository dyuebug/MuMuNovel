// Rust owner for the candidate-pool generation workflow originally mapped from
// the Python candidate-generation section that now lives in
// chapter_candidate_executor_service.py. Default executor dependencies call
// this owner directly.

use serde_json::Value;

pub(crate) mod generation_owner;

pub(crate) use self::generation_owner::{
    build_default_generation_dependencies, generate_candidate_pool_workflow,
    ChapterCandidateGenerationRequest, ChapterCandidateGenerationResult,
    ChapterCandidateOutputCollectInput, ChapterCandidateRecordBuildInput,
};
#[cfg(test)]
use self::generation_owner::{
    candidate_retry_payload_view, ChapterCandidateGenerationDependencies,
};

pub(crate) fn build_chapter_candidate_generation_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_candidate_generation_service",
        "scope": "candidate_pool_generation_workflow_owner",
        "python_source_map": [
            "backend/tests/test_services/test_chapter_candidate_generation_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_output_service.rs",
            "backend-rs/src/services/chapter_candidate_record_service.rs",
            "backend-rs/src/services/chapter_candidate_rerank_service.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_default_generation_dependencies",
                "generate_candidate_pool_workflow"
            ],
            "request_fields": [
                "base_generate_kwargs",
                "base_prompt",
                "base_temperature",
                "target_word_count",
                "source",
                "generation_label",
                "max_candidates",
                "runtime_state"
            ],
            "output_collect_fields": [
                "generate_kwargs",
                "candidate_index",
                "runtime_state"
            ],
            "record_build_fields": [
                "full_content",
                "candidate_chunks",
                "target_word_count",
                "source",
                "generation_label",
                "candidate_index",
                "candidate_offset",
                "generation_path",
                "attempt_kind"
            ],
            "workflow_policy": [
                "max_candidates is normalized to at least one",
                "initial attempt labels come from runtime-state owner",
                "retry prompt and strategy suffixes are appended to the base prompt when available",
                "retry temperature overrides generate kwargs only when finite JSON number conversion succeeds",
                "runtime_state is synced before each candidate collection and restored from output when present",
                "workflow stops when no additional candidate is required",
                "workflow stops when retry suffix is empty",
                "selected candidate comes from rerank owner or falls back to the last candidate",
                "workflow errors when no candidate is produced"
            ],
            "error_contract": [
                "candidate output collection error string is propagated",
                "candidate record build error string is propagated",
                "candidate generation produced no candidates"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_generation_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_executor_default_dependency_service",
            "chapter_candidate_executor_production_adapter_service",
            "chapter_candidate_executor_service",
            "chapter_candidate_route_gateway_service",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter-candidate-route-gateway-smoke-rust"
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
            "default_dependencies_owner": "build_default_generation_dependencies",
            "candidate_pool_workflow_owner": "generate_candidate_pool_workflow",
            "output_collection_owner": "collect_generation_candidate_output_fn",
            "record_build_owner": "build_generation_candidate_record_fn",
            "retry_prompt_owner": "build_candidate_retry_prompt_suffix_fn",
            "retry_strategy_owner": "build_candidate_retry_strategy_suffix_fn",
            "retry_temperature_owner": "resolve_candidate_retry_temperature_fn",
            "runtime_state_sync_owner": "sync_chapter_candidate_runtime_state",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "candidate generation direct python source-map deleted; surviving Python closeout work now lives in the broader candidate executor orchestration package",
            "status": "rust_chapter_candidate_generation_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_generation_python_source_map",
            "python_fallback_removal_ready": true,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_generation_owner_contract, build_default_generation_dependencies,
        candidate_retry_payload_view, generate_candidate_pool_workflow,
        ChapterCandidateGenerationDependencies, ChapterCandidateGenerationRequest,
        ChapterCandidateOutputCollectInput, ChapterCandidateRecordBuildInput,
    };
    use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;
    use crate::services::chapter_candidate_record_service::{
        build_generation_candidate_record, ChapterCandidateRecordRequest,
    };

    fn base_request(max_candidates: i64) -> ChapterCandidateGenerationRequest {
        let mut base_generate_kwargs = Map::new();
        base_generate_kwargs.insert("prompt".to_string(), json!("BASE"));
        base_generate_kwargs.insert("temperature".to_string(), json!(0.7));

        ChapterCandidateGenerationRequest {
            base_generate_kwargs,
            base_prompt: "BASE".to_string(),
            base_temperature: 0.7,
            target_word_count: 1000,
            source: "single".to_string(),
            generation_label: "chapter".to_string(),
            max_candidates,
            runtime_state: Some(json!({})),
        }
    }

    fn record_from_input(input: ChapterCandidateRecordBuildInput) -> Result<Value, String> {
        Ok(json!({
            "candidate_index": input.candidate_index,
            "candidate_offset": input.candidate_offset,
            "full_content": input.full_content,
            "candidate_chunks": input.candidate_chunks,
            "target_word_count": input.target_word_count,
            "source": input.source,
            "generation_label": input.generation_label,
            "generation_path": input.generation_path,
            "attempt_kind": input.attempt_kind,
            "quality_metrics": {"score": input.candidate_index},
            "quality_gate_plan": {"decision": if input.candidate_index == 1 { "retry" } else { "pass" }},
        }))
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
    fn should_build_candidate_retry_payload_view_from_candidate() {
        let candidate = json!({
            "candidate_index": 1,
            "quality_metrics": {"score": 1, "overall_score": 82},
            "quality_gate_plan": {"quality_gate": {"decision": "auto_repair"}}
        });

        let view = candidate_retry_payload_view(&candidate);

        assert_eq!(view.quality_metrics.as_ref().unwrap()["score"], 1);
        assert_eq!(
            view.quality_gate_plan.as_ref().unwrap()["quality_gate"]["decision"],
            "auto_repair"
        );
    }

    #[tokio::test]
    async fn should_generate_initial_candidate_like_python_workflow() {
        let mut request = base_request(1);
        let mut collect_calls = Vec::<ChapterCandidateOutputCollectInput>::new();
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| {
                collect_calls.push(input.clone());
                async move {
                    Ok(ChapterCandidateOutput {
                        full_content: format!("candidate-{}", input.candidate_index),
                        chunks: vec![format!("chunk-{}", input.candidate_index)],
                        runtime_state: input.runtime_state.clone(),
                    })
                }
            },
            build_generation_candidate_record_fn: record_from_input,
            should_generate_additional_candidate_fn: |_candidate, _produced, _max| false,
            build_candidate_retry_prompt_suffix_fn: |_plan, _attempt| {
                Some("retry prompt".to_string())
            },
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, _attempt, _source| {
                Some("retry strategy".to_string())
            },
            resolve_candidate_retry_temperature_fn: |_base, _metrics, _plan, _attempt| Some(0.9),
            select_best_generation_candidate_fn: |_candidates| None,
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(result.candidate_count(), 1);
        assert_eq!(result.selected_candidate["full_content"], "candidate-1");
        assert_eq!(collect_calls.len(), 1);
        assert_eq!(collect_calls[0].candidate_index, 1);
        assert_eq!(
            collect_calls[0]
                .runtime_state
                .as_ref()
                .expect("collect runtime")["generation_path"],
            "single_pass"
        );
        assert_eq!(collect_calls[0].generate_kwargs["prompt"], "BASE");
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["generation_path"],
            "single_pass"
        );
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["attempt_kind"],
            "initial_candidate"
        );
    }

    #[tokio::test]
    async fn should_generate_retry_candidate_with_prompt_suffix_and_temperature() {
        let mut request = base_request(2);
        let mut collect_calls = Vec::<ChapterCandidateOutputCollectInput>::new();
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| {
                collect_calls.push(input.clone());
                async move {
                    Ok(ChapterCandidateOutput {
                        full_content: format!("candidate-{}", input.candidate_index),
                        chunks: vec![format!("chunk-{}", input.candidate_index)],
                        runtime_state: input.runtime_state.clone(),
                    })
                }
            },
            build_generation_candidate_record_fn: record_from_input,
            should_generate_additional_candidate_fn: |_candidate, produced, max| {
                produced < max as usize
            },
            build_candidate_retry_prompt_suffix_fn: |_plan, attempt| {
                Some(format!("retry prompt {attempt}"))
            },
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, attempt, source| {
                Some(format!("retry strategy {attempt} {source}"))
            },
            resolve_candidate_retry_temperature_fn: |base, _metrics, _plan, attempt| {
                Some(base + attempt as f64 / 10.0)
            },
            select_best_generation_candidate_fn: |candidates: Vec<Value>| {
                candidates.last().cloned()
            },
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(result.candidate_count(), 2);
        assert_eq!(result.selected_candidate["candidate_index"], 2);
        assert_eq!(collect_calls.len(), 2);
        assert_eq!(
            collect_calls[1].generate_kwargs["prompt"],
            "BASE\n\nretry prompt 2\n\nretry strategy 2 single"
        );
        let retry_temperature = collect_calls[1].generate_kwargs["temperature"]
            .as_f64()
            .expect("retry temperature");
        assert!((retry_temperature - 0.9).abs() < f64::EPSILON);
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["generation_path"],
            "rerank_retry"
        );
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["attempt_kind"],
            "rerank_candidate"
        );
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["rerank_used"],
            true
        );
    }

    #[tokio::test]
    async fn should_build_default_generation_dependencies_from_owner() {
        let mut request = base_request(2);
        let mut collect_calls = Vec::<ChapterCandidateOutputCollectInput>::new();
        let mut dependencies = build_default_generation_dependencies(
            |input: ChapterCandidateOutputCollectInput| {
                collect_calls.push(input.clone());
                async move {
                    Ok(ChapterCandidateOutput {
                        full_content: format!("default-candidate-{}", input.candidate_index),
                        chunks: vec![format!("default-chunk-{}", input.candidate_index)],
                        runtime_state: input.runtime_state.clone(),
                    })
                }
            },
            |input: ChapterCandidateRecordBuildInput| {
                Ok(json!({
                    "candidate_index": input.candidate_index,
                    "candidate_offset": input.candidate_offset,
                    "full_content": input.full_content,
                    "candidate_chunks": input.candidate_chunks,
                    "target_word_count": input.target_word_count,
                    "word_count": if input.candidate_index == 1 { 400 } else { 1000 },
                    "source": input.source,
                    "generation_label": input.generation_label,
                    "generation_path": input.generation_path,
                    "attempt_kind": input.attempt_kind,
                    "quality_gate_decision": if input.candidate_index == 1 { "auto_repair" } else { "allow_save" },
                    "quality_gate_priority": if input.candidate_index == 1 { 2 } else { 3 },
                    "selection_score": if input.candidate_index == 1 { 70.0 } else { 96.0 },
                    "overall_score": if input.candidate_index == 1 { 68.0 } else { 92.0 },
                    "word_count_fit_score": if input.candidate_index == 1 { 40.0 } else { 99.0 },
                    "quality_metrics": {
                        "overall_score": if input.candidate_index == 1 { 68.0 } else { 92.0 },
                        "candidate_selection": {
                            "word_count": if input.candidate_index == 1 { 400 } else { 1000 },
                            "target_word_count": input.target_word_count
                        }
                    },
                    "quality_gate_plan": {
                        "quality_gate": {
                            "decision": if input.candidate_index == 1 { "auto_repair" } else { "allow_save" },
                            "failed_metrics": [
                                {"label": "conflict is thin", "focus_area": "conflict"}
                            ]
                        },
                        "active_story_repair_payload": {
                            "summary": "conflict needs a sharper turn",
                            "repair_targets": ["raise opposition"],
                            "focus_areas": ["conflict"]
                        }
                    }
                }))
            },
        );

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("default generation result");
        drop(dependencies);

        assert_eq!(result.candidate_count(), 2);
        assert_eq!(result.selected_candidate["candidate_index"], 2);
        assert_eq!(collect_calls.len(), 2);
        assert_eq!(
            collect_calls[0]
                .runtime_state
                .as_ref()
                .expect("collect runtime")["generation_path"],
            "single_pass"
        );
        assert_eq!(
            collect_calls[1]
                .runtime_state
                .as_ref()
                .expect("collect runtime")["generation_path"],
            "rerank_retry"
        );
        assert!(collect_calls[1]
            .generate_kwargs
            .get("prompt")
            .and_then(Value::as_str)
            .is_some_and(|prompt| {
                prompt.contains("Revision attempt #2")
                    && prompt.contains("Alternative candidate strategy #2")
            }));
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["generation_path"],
            "rerank_retry"
        );
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["attempt_kind"],
            "rerank_candidate"
        );
    }

    #[tokio::test]
    async fn should_stop_when_retry_suffix_is_empty() {
        let mut request = base_request(3);
        let mut collect_count = 0usize;
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| {
                collect_count += 1;
                async move {
                    Ok(ChapterCandidateOutput {
                        full_content: format!("candidate-{}", input.candidate_index),
                        chunks: vec![],
                        runtime_state: input.runtime_state.clone(),
                    })
                }
            },
            build_generation_candidate_record_fn: record_from_input,
            should_generate_additional_candidate_fn: |_candidate, _produced, _max| true,
            build_candidate_retry_prompt_suffix_fn: |_plan, _attempt| None,
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, _attempt, _source| None,
            resolve_candidate_retry_temperature_fn: |_base, _metrics, _plan, _attempt| Some(1.0),
            select_best_generation_candidate_fn: |_candidates| None,
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(collect_count, 1);
        assert_eq!(result.candidate_count(), 1);
        assert_eq!(result.selected_candidate["candidate_index"], 1);
    }

    #[tokio::test]
    async fn should_normalize_max_candidates_to_one() {
        let mut request = base_request(0);
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| async move {
                Ok(ChapterCandidateOutput {
                    full_content: format!("candidate-{}", input.candidate_index),
                    chunks: vec![],
                    runtime_state: input.runtime_state.clone(),
                })
            },
            build_generation_candidate_record_fn: record_from_input,
            should_generate_additional_candidate_fn: |_candidate, _produced, _max| true,
            build_candidate_retry_prompt_suffix_fn: |_plan, _attempt| Some("retry".to_string()),
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, _attempt, _source| None,
            resolve_candidate_retry_temperature_fn: |_base, _metrics, _plan, _attempt| None,
            select_best_generation_candidate_fn: |_candidates| None,
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(result.candidate_count(), 1);
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["candidate_total"],
            1
        );
    }

    #[tokio::test]
    async fn should_compose_with_rust_candidate_record_owner() {
        let mut request = base_request(1);
        let mut quality_evaluator = |_content: &str| {
            json!({
                "overall_score": 88.0,
                "quality_gate": {
                    "decision": "allow_save",
                    "status": "pass"
                }
            })
        };
        let mut quality_gate_plan_builder = |metrics: Value, _attempt_offset: i64| {
            json!({
                "action": "continue",
                "quality_gate": metrics["quality_gate"].clone()
            })
        };
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| async move {
                Ok(ChapterCandidateOutput {
                    full_content: format!("候选正文{}。", input.candidate_index),
                    chunks: vec![format!("候选正文{}。", input.candidate_index)],
                    runtime_state: input.runtime_state.clone(),
                })
            },
            build_generation_candidate_record_fn: |input: ChapterCandidateRecordBuildInput| {
                build_generation_candidate_record(
                    ChapterCandidateRecordRequest {
                        full_content: input.full_content,
                        candidate_chunks: input.candidate_chunks,
                        target_word_count: input.target_word_count,
                        source: input.source,
                        generation_label: input.generation_label,
                        candidate_index: input.candidate_index,
                        candidate_offset: input.candidate_offset,
                        generation_path: input.generation_path,
                        attempt_kind: input.attempt_kind,
                    },
                    &mut quality_evaluator,
                    &mut quality_gate_plan_builder,
                    None,
                )
            },
            should_generate_additional_candidate_fn: |_candidate, _produced, _max| false,
            build_candidate_retry_prompt_suffix_fn: |_plan, _attempt| None,
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, _attempt, _source| None,
            resolve_candidate_retry_temperature_fn: |_base, _metrics, _plan, _attempt| None,
            select_best_generation_candidate_fn: |_candidates| None,
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(result.candidate_count(), 1);
        assert_eq!(result.selected_candidate["candidate_index"], 1);
        assert_eq!(
            result.selected_candidate["quality_gate_decision"],
            "auto_repair"
        );
        assert_eq!(
            result.selected_candidate["quality_metrics"]["candidate_selection"]["attempt_kind"],
            "initial_candidate"
        );
    }

    #[test]
    fn should_publish_chapter_candidate_generation_owner_contract() {
        let contract = build_chapter_candidate_generation_owner_contract();
        assert_no_deleted_python_service_source_map(&contract);

        assert_eq!(contract["owner"], "chapter_candidate_generation_service");
        assert_eq!(
            contract["scope"],
            "candidate_pool_generation_workflow_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/tests/test_services/test_chapter_candidate_generation_service.py"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_candidate_generation_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][1],
            "generate_candidate_pool_workflow"
        );
        assert_eq!(
            contract["behavior_contract"]["request_fields"][7],
            "runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["workflow_policy"][0],
            "max_candidates is normalized to at least one"
        );
        assert_eq!(
            contract["behavior_contract"]["workflow_policy"][7],
            "selected candidate comes from rerank owner or falls back to the last candidate"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_candidate_executor_default_dependency_service"
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
            "build_default_generation_dependencies"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["candidate_pool_workflow_owner"],
            "generate_candidate_pool_workflow"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["runtime_state_sync_owner"],
            "sync_chapter_candidate_runtime_state"
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
            "candidate generation direct python source-map deleted; surviving Python closeout work now lives in the broader candidate executor orchestration package"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_generation_owner_source_map_deleted"
        );
    }
}
