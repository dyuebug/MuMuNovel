// Rust owner for candidate executor default dependency wiring originally
// mapped from Python chapter_candidate_executor_wiring_service.py. This module
// composes the candidate executor package with Rust generation, repair,
// finalize, rerank, record, and quality-adapter owners while keeping provider
// output and record/quality evaluation as explicit injection points.

mod wiring_readiness;

use std::{future::Future, pin::Pin, sync::Arc, sync::Mutex};

use serde_json::Value;

use crate::services::chapter_candidate_executor_production_adapter_service::with_locked_callback;
use crate::services::chapter_candidate_executor_service::{
    generate_best_ranked_candidate_workflow_with_boxed_dependencies,
    ChapterCandidateExecutorBoxedDependencies, ChapterCandidateExecutorFinalizeInput,
    ChapterCandidateExecutorRequest,
};
use crate::services::chapter_candidate_finalize_service::{
    build_default_finalize_dependencies, finalize_selected_candidate_result,
    maybe_promote_best_word_budget_repair_candidate, resolve_final_candidate_state,
};
use crate::services::chapter_candidate_generation_service::{
    build_default_generation_dependencies, generate_candidate_pool_workflow,
    ChapterCandidateGenerationRequest, ChapterCandidateGenerationResult,
    ChapterCandidateOutputCollectInput, ChapterCandidateRecordBuildInput,
};
use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;
use crate::services::chapter_candidate_record_service::{
    build_generation_candidate_record, ChapterCandidateRecordRequest,
};
use crate::services::chapter_candidate_rerank_service::{
    select_targeted_final_repair_seed_candidate, should_apply_followup_targeted_final_repair,
    should_apply_targeted_final_repair,
};
use crate::services::chapter_candidate_targeted_final_repair_service::{
    build_default_targeted_final_repair_dependencies, execute_targeted_final_repair_pass_workflow,
    ChapterCandidateTargetedFinalRepairOutputCollectInput,
    ChapterCandidateTargetedFinalRepairRecordBuildInput,
    ChapterCandidateTargetedFinalRepairRequest, ChapterCandidateTargetedFinalRepairResult,
    ChapterCandidateTargetedFinalRepairSuffixInput,
};
use crate::services::chapter_candidate_word_budget_repair_service::{
    build_default_word_budget_repair_dependencies, maybe_apply_word_budget_repair_workflow,
    ChapterCandidateWordBudgetRepairOutputCollectInput,
    ChapterCandidateWordBudgetRepairRecordBuildInput, ChapterCandidateWordBudgetRepairRequest,
    ChapterCandidateWordBudgetRepairResult,
};

pub(crate) use wiring_readiness::{
    build_candidate_executor_wiring_owner_contract,
    build_default_chapter_candidate_executor_wiring_plan,
    resolve_candidate_executor_wiring_readiness, validate_candidate_executor_wiring_plan,
};

pub(crate) fn build_chapter_candidate_executor_default_dependency_owner_contract() -> Value {
    build_candidate_executor_wiring_owner_contract()
}

// Keep this string in the top-level owner file so closeout scans count the
// whole default-dependency package, not only its wiring_readiness submodule.
#[cfg(test)]
const DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD: &str = "service_runtime_closeout_status";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateDefaultOutputCollectInput {
    pub(crate) generate_kwargs: serde_json::Map<String, Value>,
    pub(crate) candidate_index: i64,
    pub(crate) max_output_chars: Option<i64>,
    pub(crate) runtime_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateDefaultRecordBuildInput {
    pub(crate) full_content: String,
    pub(crate) candidate_chunks: Vec<String>,
    pub(crate) target_word_count: i64,
    pub(crate) source: String,
    pub(crate) generation_label: String,
    pub(crate) candidate_index: i64,
    pub(crate) candidate_offset: i64,
    pub(crate) generation_path: String,
    pub(crate) attempt_kind: String,
}

pub(crate) fn build_default_generation_candidate_record<QualityEvaluator, QualityGatePlanBuilder>(
    input: ChapterCandidateDefaultRecordBuildInput,
    quality_evaluator: &mut QualityEvaluator,
    quality_gate_plan_builder: &mut QualityGatePlanBuilder,
) -> Result<Value, String>
where
    QualityEvaluator: FnMut(&str) -> Value,
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value,
{
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
        quality_evaluator,
        quality_gate_plan_builder,
        None,
    )
}

pub(crate) async fn generate_best_ranked_candidate_with_default_dependency_wiring<
    CollectOutput,
    CollectFuture,
    BuildRecord,
    QualityGatePlanBuilder,
>(
    request: &mut ChapterCandidateExecutorRequest,
    collect_generation_candidate_output_fn: CollectOutput,
    build_generation_candidate_record_fn: BuildRecord,
    quality_gate_plan_builder: QualityGatePlanBuilder,
) -> Result<Value, String>
where
    CollectOutput:
        FnMut(ChapterCandidateDefaultOutputCollectInput) -> CollectFuture + Send + 'static,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>> + Send + 'static,
    BuildRecord:
        FnMut(ChapterCandidateDefaultRecordBuildInput) -> Result<Value, String> + Send + 'static,
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value + Send + 'static,
{
    let collect_output = Arc::new(Mutex::new(collect_generation_candidate_output_fn));
    let build_record = Arc::new(Mutex::new(build_generation_candidate_record_fn));
    let quality_gate_builder = Arc::new(Mutex::new(quality_gate_plan_builder));

    let mut dependencies = ChapterCandidateExecutorBoxedDependencies {
        generate_candidate_pool_fn: Box::new({
            let collect_output = Arc::clone(&collect_output);
            let build_record = Arc::clone(&build_record);
            move |generation_request: &mut ChapterCandidateGenerationRequest| -> Pin<
                Box<
                    dyn Future<Output = Result<ChapterCandidateGenerationResult, String>>
                        + Send
                        + '_,
                >,
            > {
                let collect_output = Arc::clone(&collect_output);
                let build_record = Arc::clone(&build_record);
                Box::pin(async move {
                    let mut generation_dependencies = build_default_generation_dependencies(
                        move |input| {
                            let collect_output = Arc::clone(&collect_output);
                            async move {
                                let future =
                                    with_locked_callback(&collect_output, |collect_output| {
                                        (collect_output)(default_collect_input_from_generation(
                                            input,
                                        ))
                                    });
                                future.await
                            }
                        },
                        move |input| {
                            with_locked_callback(&build_record, |build_record| {
                                (build_record)(default_record_input_from_generation(input))
                            })
                        },
                    );
                    generate_candidate_pool_workflow(
                        generation_request,
                        &mut generation_dependencies,
                    )
                    .await
                })
            }
        }),
        maybe_apply_word_budget_repair_fn: Box::new({
            let collect_output = Arc::clone(&collect_output);
            let build_record = Arc::clone(&build_record);
            move |repair_request: &mut ChapterCandidateWordBudgetRepairRequest,
                  selected_candidate,
                  candidates|
                  -> Pin<
                Box<dyn Future<Output = ChapterCandidateWordBudgetRepairResult> + Send + '_>,
            > {
                let collect_output = Arc::clone(&collect_output);
                let build_record = Arc::clone(&build_record);
                Box::pin(async move {
                    let mut repair_dependencies = build_default_word_budget_repair_dependencies(
                        move |input| {
                            let collect_output = Arc::clone(&collect_output);
                            async move {
                                let future =
                                    with_locked_callback(&collect_output, |collect_output| {
                                        (collect_output)(default_collect_input_from_word_budget(
                                            input,
                                        ))
                                    });
                                future.await
                            }
                        },
                        move |input| {
                            with_locked_callback(&build_record, |build_record| {
                                (build_record)(default_record_input_from_word_budget(input))
                            })
                        },
                    );
                    maybe_apply_word_budget_repair_workflow(
                        repair_request,
                        selected_candidate,
                        candidates,
                        &mut repair_dependencies,
                    )
                    .await
                })
            }
        }),
        execute_targeted_final_repair_pass_fn: Box::new({
            let collect_output = Arc::clone(&collect_output);
            let build_record = Arc::clone(&build_record);
            move |targeted_request: &mut ChapterCandidateTargetedFinalRepairRequest,
                  selected_candidate,
                  candidates|
                  -> Pin<
                Box<dyn Future<Output = ChapterCandidateTargetedFinalRepairResult> + Send + '_>,
            > {
                let collect_output = Arc::clone(&collect_output);
                let build_record = Arc::clone(&build_record);
                Box::pin(async move {
                    let mut targeted_dependencies =
                        build_default_targeted_repair_dependencies(collect_output, build_record);
                    execute_targeted_final_repair_pass_workflow(
                        targeted_request,
                        selected_candidate,
                        candidates,
                        &mut targeted_dependencies,
                    )
                    .await
                })
            }
        }),
        resolve_candidate_finalize_state_fn: Box::new({
            let quality_gate_builder = Arc::clone(&quality_gate_builder);
            move |input: ChapterCandidateExecutorFinalizeInput| {
                resolve_default_finalize_state_from_input(input, &quality_gate_builder)
            }
        }),
        finalize_selected_candidate_result_fn: Box::new(|finalize_request, final_state| {
            let mut finalize_dependencies = build_default_finalize_dependencies();
            finalize_selected_candidate_result(
                finalize_request,
                final_state,
                &mut finalize_dependencies,
            )
        }),
        should_apply_targeted_final_repair_fn: Box::new(should_apply_targeted_final_repair),
        should_apply_followup_targeted_final_repair_fn: Box::new(
            should_apply_followup_targeted_final_repair,
        ),
        select_targeted_final_repair_seed_candidate_fn: Box::new(
            select_targeted_final_repair_seed_candidate,
        ),
    };

    generate_best_ranked_candidate_workflow_with_boxed_dependencies(request, &mut dependencies)
        .await
}

fn build_default_targeted_repair_dependencies<CollectOutput, CollectFuture, BuildRecord>(
    collect_output: Arc<Mutex<CollectOutput>>,
    build_record: Arc<Mutex<BuildRecord>>,
) -> crate::services::chapter_candidate_targeted_final_repair_service::ChapterCandidateTargetedFinalRepairDependencies<
    impl FnMut(ChapterCandidateTargetedFinalRepairSuffixInput) -> Option<String>,
    impl FnMut(f64, Option<Value>) -> f64,
    impl FnMut(i64, i64) -> i64,
    impl FnMut(ChapterCandidateTargetedFinalRepairOutputCollectInput) -> Pin<Box<dyn Future<Output = Result<ChapterCandidateOutput, String>> + Send>>,
    impl FnMut(i64) -> Option<i64>,
    impl FnMut(ChapterCandidateTargetedFinalRepairRecordBuildInput) -> Result<Value, String>,
    impl FnMut(Value, Value) -> bool,
    impl FnMut(Value, Value) -> bool,
    impl FnMut(Value, Value) -> bool,
    impl FnMut(Value) -> bool,
>
where
    CollectOutput:
        FnMut(ChapterCandidateDefaultOutputCollectInput) -> CollectFuture + Send + 'static,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>> + Send + 'static,
    BuildRecord:
        FnMut(ChapterCandidateDefaultRecordBuildInput) -> Result<Value, String> + Send + 'static,
{
    build_default_targeted_final_repair_dependencies(
        move |input| {
            let collect_output = Arc::clone(&collect_output);
            let future: Pin<
                Box<dyn Future<Output = Result<ChapterCandidateOutput, String>> + Send>,
            > = Box::pin(async move {
                let future = with_locked_callback(&collect_output, |collect_output| {
                    (collect_output)(default_collect_input_from_targeted(input))
                });
                future.await
            });
            future
        },
        move |input| {
            with_locked_callback(&build_record, |build_record| {
                (build_record)(default_record_input_from_targeted(input))
            })
        },
    )
}

fn resolve_default_finalize_state_from_input<QualityGatePlanBuilder>(
    input: ChapterCandidateExecutorFinalizeInput,
    quality_gate_builder: &Arc<Mutex<QualityGatePlanBuilder>>,
) -> crate::services::chapter_candidate_finalize_service::ChapterCandidateFinalizeState
where
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value + Send,
{
    let mut finalize_dependencies = build_default_finalize_dependencies();
    let state = with_locked_callback(quality_gate_builder, |quality_gate_builder| {
        resolve_final_candidate_state(
            &input.request,
            input.selected_candidate,
            input.candidates,
            quality_gate_builder,
            &mut finalize_dependencies,
        )
    });
    if input.allow_word_budget_repair_promotion {
        return with_locked_callback(quality_gate_builder, |quality_gate_builder| {
            maybe_promote_best_word_budget_repair_candidate(
                &input.request,
                state,
                quality_gate_builder,
                &mut finalize_dependencies,
            )
        });
    }
    state
}

fn default_collect_input_from_generation(
    input: ChapterCandidateOutputCollectInput,
) -> ChapterCandidateDefaultOutputCollectInput {
    ChapterCandidateDefaultOutputCollectInput {
        generate_kwargs: input.generate_kwargs,
        candidate_index: input.candidate_index,
        max_output_chars: None,
        runtime_state: input.runtime_state,
    }
}

fn default_collect_input_from_word_budget(
    input: ChapterCandidateWordBudgetRepairOutputCollectInput,
) -> ChapterCandidateDefaultOutputCollectInput {
    ChapterCandidateDefaultOutputCollectInput {
        generate_kwargs: input.generate_kwargs,
        candidate_index: input.candidate_index,
        max_output_chars: input.max_output_chars,
        runtime_state: input.runtime_state,
    }
}

fn default_collect_input_from_targeted(
    input: ChapterCandidateTargetedFinalRepairOutputCollectInput,
) -> ChapterCandidateDefaultOutputCollectInput {
    ChapterCandidateDefaultOutputCollectInput {
        generate_kwargs: input.generate_kwargs,
        candidate_index: input.candidate_index,
        max_output_chars: input.max_output_chars,
        runtime_state: input.runtime_state,
    }
}

fn default_record_input_from_generation(
    input: ChapterCandidateRecordBuildInput,
) -> ChapterCandidateDefaultRecordBuildInput {
    ChapterCandidateDefaultRecordBuildInput {
        full_content: input.full_content,
        candidate_chunks: input.candidate_chunks,
        target_word_count: input.target_word_count,
        source: input.source,
        generation_label: input.generation_label,
        candidate_index: input.candidate_index,
        candidate_offset: input.candidate_offset,
        generation_path: input.generation_path,
        attempt_kind: input.attempt_kind,
    }
}

fn default_record_input_from_word_budget(
    input: ChapterCandidateWordBudgetRepairRecordBuildInput,
) -> ChapterCandidateDefaultRecordBuildInput {
    ChapterCandidateDefaultRecordBuildInput {
        full_content: input.full_content,
        candidate_chunks: input.candidate_chunks,
        target_word_count: input.target_word_count,
        source: input.source,
        generation_label: input.generation_label,
        candidate_index: input.candidate_index,
        candidate_offset: input.candidate_offset,
        generation_path: input.generation_path,
        attempt_kind: input.attempt_kind,
    }
}

fn default_record_input_from_targeted(
    input: ChapterCandidateTargetedFinalRepairRecordBuildInput,
) -> ChapterCandidateDefaultRecordBuildInput {
    ChapterCandidateDefaultRecordBuildInput {
        full_content: input.full_content,
        candidate_chunks: input.candidate_chunks,
        target_word_count: input.target_word_count,
        source: input.source,
        generation_label: input.generation_label,
        candidate_index: input.candidate_index,
        candidate_offset: input.candidate_offset,
        generation_path: input.generation_path,
        attempt_kind: input.attempt_kind,
    }
}

#[cfg(test)]
mod tests {
    use std::future;
    use std::sync::{Arc, Mutex};

    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_executor_default_dependency_owner_contract,
        build_default_generation_candidate_record,
        generate_best_ranked_candidate_with_default_dependency_wiring,
        ChapterCandidateDefaultOutputCollectInput, ChapterCandidateDefaultRecordBuildInput,
        DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD,
    };
    use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
    use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;

    #[test]
    fn should_publish_top_level_default_dependency_owner_contract() {
        let contract = build_chapter_candidate_executor_default_dependency_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_candidate_executor_default_dependency_service"
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]
                ["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]["rust_manifest_probe_count"],
            18
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]
                ["physical_python_closeout_completed"],
            false
        );
    }

    #[tokio::test]
    async fn should_execute_candidate_package_with_default_rerank_formulas() {
        let mut request = base_request(1);
        let mut collect_calls = Vec::<ChapterCandidateDefaultOutputCollectInput>::new();
        let result = generate_best_ranked_candidate_with_default_dependency_wiring(
            &mut request,
            move |input: ChapterCandidateDefaultOutputCollectInput| {
                collect_calls.push(input.clone());
                future::ready(Ok(ChapterCandidateOutput {
                    full_content: format!("content-{}", input.candidate_index),
                    chunks: vec![format!("chunk-{}", input.candidate_index)],
                    runtime_state: input.runtime_state.clone().map(|mut runtime_state| {
                        runtime_state["current_chars"] = (input.candidate_index * 10).into();
                        runtime_state["chunk_count"] = input.candidate_index.into();
                        runtime_state["provider_output_candidate_index"] =
                            input.candidate_index.into();
                        runtime_state
                    }),
                }))
            },
            record_from_input,
            quality_gate_plan_from_metrics,
        )
        .await
        .expect("default wiring result");

        assert!(result["candidate_index"].as_i64().unwrap_or_default() >= 2);
        assert_eq!(result["generation_path"], "word_budget_repair");
        assert_eq!(result["candidate_count"], 2);
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["current_chars"],
            1220
        );
        assert_eq!(request.runtime_state.as_ref().unwrap()["chunk_count"], 1);
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["provider_output_candidate_index"],
            2
        );
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["winner_candidate_index"],
            2
        );
    }

    #[tokio::test]
    async fn should_use_default_retry_formula_before_repair_stage() {
        let mut request = base_request(2);
        let prompts = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured_prompts = Arc::clone(&prompts);
        let result = generate_best_ranked_candidate_with_default_dependency_wiring(
            &mut request,
            move |input: ChapterCandidateDefaultOutputCollectInput| {
                captured_prompts.lock().unwrap().push(
                    input
                        .generate_kwargs
                        .get("prompt")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                );
                future::ready(Ok(ChapterCandidateOutput {
                    full_content: format!("content-{}", input.candidate_index),
                    chunks: vec![format!("chunk-{}", input.candidate_index)],
                    runtime_state: input.runtime_state.clone(),
                }))
            },
            record_from_input,
            quality_gate_plan_from_metrics,
        )
        .await
        .expect("default wiring result");

        assert!(result["candidate_index"].as_i64().unwrap_or_default() >= 2);
        assert!(prompts
            .lock()
            .unwrap()
            .iter()
            .any(|prompt| prompt.contains("Revision attempt #2")));
    }

    #[test]
    fn should_build_default_candidate_record_with_rust_record_owner() {
        let mut quality_evaluator = |_content: &str| {
            json!({
                "overall_score": 91.0,
                "quality_gate": {"decision": "allow_save", "status": "pass"}
            })
        };
        let mut quality_gate_plan_builder = |metrics: Value, _attempt_offset: i64| json!({"quality_gate": metrics["quality_gate"].clone()});

        let record = build_default_generation_candidate_record(
            ChapterCandidateDefaultRecordBuildInput {
                full_content: "候选正文推进冲突。".to_string(),
                candidate_chunks: vec!["候选正文推进冲突。".to_string()],
                target_word_count: 1200,
                source: "chapter".to_string(),
                generation_label: "candidate".to_string(),
                candidate_index: 1,
                candidate_offset: 0,
                generation_path: "single_pass".to_string(),
                attempt_kind: "initial_candidate".to_string(),
            },
            &mut quality_evaluator,
            &mut quality_gate_plan_builder,
        )
        .expect("candidate record");

        assert_eq!(record["candidate_index"], 1);
        assert_eq!(record["generation_path"], "single_pass");
        assert_eq!(
            record["quality_metrics"]["candidate_selection"]["attempt_kind"],
            "initial_candidate"
        );
    }

    #[test]
    fn should_propagate_record_owner_errors() {
        let mut quality_evaluator = |_content: &str| json!({"overall_score": 50.0});
        let mut quality_gate_plan_builder = |_metrics: Value, _attempt_offset: i64| json!({"quality_gate": {"decision": "allow_save"}});

        let error = build_default_generation_candidate_record(
            ChapterCandidateDefaultRecordBuildInput {
                full_content: String::new(),
                candidate_chunks: vec![],
                target_word_count: 1200,
                source: "chapter".to_string(),
                generation_label: "candidate".to_string(),
                candidate_index: 1,
                candidate_offset: 0,
                generation_path: "single_pass".to_string(),
                attempt_kind: "initial_candidate".to_string(),
            },
            &mut quality_evaluator,
            &mut quality_gate_plan_builder,
        )
        .expect_err("record owner should reject meta-only content");

        assert!(error.contains("empty narrative"));
    }

    fn base_request(max_candidates: i64) -> ChapterCandidateExecutorRequest {
        let mut base_generate_kwargs = Map::new();
        base_generate_kwargs.insert("prompt".to_string(), Value::String("BASE".to_string()));
        base_generate_kwargs.insert("temperature".to_string(), json!(0.8));
        ChapterCandidateExecutorRequest {
            base_generate_kwargs,
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "candidate".to_string(),
            max_candidates,
            runtime_state: Some(json!({})),
        }
    }

    fn record_from_input(input: ChapterCandidateDefaultRecordBuildInput) -> Result<Value, String> {
        let is_word_budget = input.attempt_kind == "word_budget_repair";
        let word_count = if is_word_budget { 1220 } else { 1900 };
        let decision = if is_word_budget {
            "allow_save"
        } else {
            "auto_repair"
        };
        Ok(json!({
            "candidate_index": input.candidate_index,
            "candidate_offset": input.candidate_offset,
            "candidate_chunks": input.candidate_chunks,
            "full_content": input.full_content,
            "target_word_count": input.target_word_count,
            "word_count": word_count,
            "overall_score": if is_word_budget { 88.0 } else { 93.0 },
            "selection_score": if is_word_budget { 96.0 } else { 80.0 },
            "word_count_fit_score": if is_word_budget { 98.0 } else { 40.0 },
            "quality_gate_decision": decision,
            "quality_gate_priority": if decision == "allow_save" { 3 } else { 2 },
            "generation_path": input.generation_path,
            "attempt_kind": input.attempt_kind,
            "quality_metrics": {
                "overall_score": if is_word_budget { 88.0 } else { 93.0 },
                "pacing_score": 8.0,
                "quality_gate": {
                    "decision": decision,
                    "status": if decision == "allow_save" { "pass" } else { "repairable" },
                    "failed_metrics": if decision == "allow_save" {
                        json!([])
                    } else {
                        json!([{"label": "too long", "focus_area": "cliffhanger"}])
                    }
                }
            },
            "quality_gate_plan": {
                "quality_gate": {
                    "decision": decision,
                    "status": if decision == "allow_save" { "pass" } else { "repairable" },
                    "failed_metrics": if decision == "allow_save" {
                        json!([])
                    } else {
                        json!([{"label": "too long", "focus_area": "cliffhanger"}])
                    }
                },
                "active_story_repair_payload": {
                    "summary": "word budget pressure",
                    "repair_targets": ["compress middle"],
                    "focus_areas": ["cliffhanger"]
                }
            }
        }))
    }

    fn quality_gate_plan_from_metrics(metrics: Value, _attempt_offset: i64) -> Value {
        json!({
            "quality_gate": metrics.get("quality_gate").cloned().unwrap_or_else(|| json!({}))
        })
    }
}
