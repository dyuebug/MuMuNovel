// Staged executable Rust wiring for Python
// chapter_candidate_executor_wiring_service.py. It composes the Rust
// candidate executor package with default Rust rerank formulas while keeping
// provider output and record/quality evaluation as explicit injection points.
#![allow(dead_code)]

use std::{future::Future, sync::Arc, sync::Mutex};

use serde_json::{Map, Value};

use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
use crate::services::chapter_candidate_finalize_service::{
    finalize_selected_candidate_result, maybe_promote_best_word_budget_repair_candidate,
    resolve_final_candidate_state, ChapterCandidateFinalizeDependencies,
    ChapterCandidateFinalizeMetadataContext, ChapterCandidateFinalizeRequest,
    ChapterCandidateFinalizeState,
};
use crate::services::chapter_candidate_generation_service::{
    generate_candidate_pool_workflow, ChapterCandidateGenerationDependencies,
    ChapterCandidateGenerationRequest, ChapterCandidateOutputCollectInput,
    ChapterCandidateRecordBuildInput,
};
use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;
use crate::services::chapter_candidate_rerank_service::{
    attach_candidate_selection_metadata, build_candidate_pool_summary,
    build_candidate_retry_prompt_suffix, build_candidate_retry_strategy_suffix,
    build_candidate_selection_metadata, build_targeted_final_repair_suffix,
    build_word_budget_repair_suffix, normalize_candidate_quality_gate_plan,
    resolve_candidate_retry_temperature, resolve_targeted_final_repair_char_limit,
    resolve_targeted_final_repair_max_tokens, resolve_targeted_final_repair_temperature,
    resolve_word_budget_repair_char_limit, resolve_word_budget_repair_max_tokens,
    resolve_word_budget_repair_temperature, select_best_generation_candidate,
    select_targeted_final_repair_seed_candidate, should_adopt_targeted_final_repair_candidate,
    should_apply_followup_targeted_final_repair, should_apply_targeted_final_repair,
    should_apply_word_budget_repair, should_generate_additional_candidate,
    should_keep_targeted_final_repair_candidate, should_keep_word_budget_repair_candidate,
    should_prefer_targeted_final_repair_candidate, should_prefer_word_budget_repair_candidate,
    should_relax_word_budget_repair_limits, CandidateSelectionMetadataInput,
};
use crate::services::chapter_candidate_targeted_final_repair_service::{
    execute_targeted_final_repair_pass_workflow, ChapterCandidateTargetedFinalRepairDependencies,
    ChapterCandidateTargetedFinalRepairOutputCollectInput,
    ChapterCandidateTargetedFinalRepairRecordBuildInput,
    ChapterCandidateTargetedFinalRepairRequest, ChapterCandidateTargetedFinalRepairResult,
    ChapterCandidateTargetedFinalRepairSuffixInput,
};
use crate::services::chapter_candidate_word_budget_repair_service::{
    maybe_apply_word_budget_repair_workflow, ChapterCandidateWordBudgetRepairDependencies,
    ChapterCandidateWordBudgetRepairOutputCollectInput,
    ChapterCandidateWordBudgetRepairRecordBuildInput, ChapterCandidateWordBudgetRepairRequest,
    ChapterCandidateWordBudgetRepairSuffixInput,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateDefaultOutputCollectInput {
    pub(crate) generate_kwargs: serde_json::Map<String, Value>,
    pub(crate) candidate_index: i64,
    pub(crate) max_output_chars: Option<i64>,
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

    let base_prompt = request
        .base_generate_kwargs
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let base_temperature = resolve_base_temperature(&request.base_generate_kwargs);

    let mut generation_request = ChapterCandidateGenerationRequest {
        base_generate_kwargs: request.base_generate_kwargs.clone(),
        base_prompt: base_prompt.clone(),
        base_temperature,
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        generation_label: request.generation_label.clone(),
        max_candidates: request.max_candidates,
        runtime_state: request.runtime_state.take(),
    };
    let generation_result = {
        let collect_output = Arc::clone(&collect_output);
        let build_record = Arc::clone(&build_record);
        let mut generation_dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: move |input| {
                let collect_output = Arc::clone(&collect_output);
                async move {
                    let future = with_locked_callback(&collect_output, |collect_output| {
                        (collect_output)(default_collect_input_from_generation(input))
                    });
                    future.await
                }
            },
            build_generation_candidate_record_fn: move |input| {
                with_locked_callback(&build_record, |build_record| {
                    (build_record)(default_record_input_from_generation(input))
                })
            },
            should_generate_additional_candidate_fn: should_generate_additional_candidate,
            build_candidate_retry_prompt_suffix_fn: build_candidate_retry_prompt_suffix,
            build_candidate_retry_strategy_suffix_fn: build_candidate_retry_strategy_suffix,
            resolve_candidate_retry_temperature_fn: resolve_candidate_retry_temperature,
            select_best_generation_candidate_fn: select_best_generation_candidate,
        };
        generate_candidate_pool_workflow(&mut generation_request, &mut generation_dependencies)
            .await?
    };
    request.runtime_state = generation_request.runtime_state;
    let mut selected_candidate = generation_result.selected_candidate;
    let mut candidates = generation_result.candidates;

    let mut repair_request = ChapterCandidateWordBudgetRepairRequest {
        base_generate_kwargs: request.base_generate_kwargs.clone(),
        base_prompt: base_prompt.clone(),
        base_temperature,
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        generation_label: request.generation_label.clone(),
        runtime_state: request.runtime_state.take(),
    };
    let repair_result = {
        let collect_output = Arc::clone(&collect_output);
        let build_record = Arc::clone(&build_record);
        let mut repair_dependencies = ChapterCandidateWordBudgetRepairDependencies {
            should_apply_word_budget_repair_fn: should_apply_word_budget_repair,
            build_word_budget_repair_suffix_fn:
                |input: ChapterCandidateWordBudgetRepairSuffixInput| {
                    build_word_budget_repair_suffix(
                        input.quality_metrics,
                        input.quality_gate_plan,
                        input.current_content,
                        input.target_word_count,
                        input.attempt_index,
                        input.source,
                    )
                },
            should_relax_word_budget_repair_limits_fn: should_relax_word_budget_repair_limits,
            resolve_word_budget_repair_temperature_fn: resolve_word_budget_repair_temperature,
            resolve_word_budget_repair_max_tokens_fn: resolve_word_budget_repair_max_tokens,
            collect_generation_candidate_output_fn: move |input| {
                let collect_output = Arc::clone(&collect_output);
                async move {
                    let future = with_locked_callback(&collect_output, |collect_output| {
                        (collect_output)(default_collect_input_from_word_budget(input))
                    });
                    future.await
                }
            },
            resolve_word_budget_repair_char_limit_fn: resolve_word_budget_repair_char_limit,
            build_generation_candidate_record_fn: move |input| {
                with_locked_callback(&build_record, |build_record| {
                    (build_record)(default_record_input_from_word_budget(input))
                })
            },
            should_keep_word_budget_repair_candidate_fn: should_keep_word_budget_repair_candidate,
            select_best_generation_candidate_fn: select_best_generation_candidate,
            should_prefer_word_budget_repair_candidate_fn:
                should_prefer_word_budget_repair_candidate,
        };
        maybe_apply_word_budget_repair_workflow(
            &mut repair_request,
            selected_candidate,
            candidates,
            &mut repair_dependencies,
        )
        .await
    };
    request.runtime_state = repair_request.runtime_state;
    selected_candidate = repair_result.selected_candidate;
    candidates = repair_result.candidates;
    let mut deferred_followup_targeted_repair_seed_candidate = None;
    if should_apply_targeted_final_repair(selected_candidate.clone()) {
        let targeted_result = run_default_targeted_repair_stage(
            request,
            &base_prompt,
            base_temperature,
            selected_candidate.clone(),
            selected_candidate,
            candidates,
            "targeted-repair",
            true,
            {
                let collect_output = Arc::clone(&collect_output);
                collect_output
            },
            {
                let build_record = Arc::clone(&build_record);
                build_record
            },
        )
        .await;
        selected_candidate = targeted_result.selected_candidate;
        candidates = targeted_result.candidates;
        deferred_followup_targeted_repair_seed_candidate =
            targeted_result.deferred_followup_targeted_repair_seed_candidate;
    }

    let mut final_state = resolve_default_finalize_state(
        request,
        selected_candidate,
        candidates,
        true,
        &quality_gate_builder,
    )?;
    selected_candidate = final_state.selected_candidate.clone();
    candidates = final_state.candidates.clone();

    let targeted_seed = if should_apply_followup_targeted_final_repair(selected_candidate.clone()) {
        Some(selected_candidate.clone())
    } else if deferred_followup_targeted_repair_seed_candidate.is_some() {
        deferred_followup_targeted_repair_seed_candidate
    } else if is_targeted_quality_repair_candidate(&selected_candidate) {
        None
    } else {
        select_targeted_final_repair_seed_candidate(selected_candidate.clone(), candidates.clone())
    };
    if let Some(targeted_seed) = targeted_seed {
        let targeted_result = run_default_targeted_repair_stage(
            request,
            &base_prompt,
            base_temperature,
            targeted_seed,
            selected_candidate,
            candidates,
            "targeted-repair-post-finalize",
            false,
            {
                let collect_output = Arc::clone(&collect_output);
                collect_output
            },
            {
                let build_record = Arc::clone(&build_record);
                build_record
            },
        )
        .await;
        selected_candidate = targeted_result.selected_candidate;
        candidates = targeted_result.candidates;

        final_state = resolve_default_finalize_state(
            request,
            selected_candidate.clone(),
            candidates.clone(),
            false,
            &quality_gate_builder,
        )?;
        if should_apply_followup_targeted_final_repair(final_state.selected_candidate.clone()) {
            let targeted_result = run_default_targeted_repair_stage(
                request,
                &base_prompt,
                base_temperature,
                final_state.selected_candidate.clone(),
                final_state.selected_candidate,
                candidates,
                "targeted-repair-followup",
                false,
                {
                    let collect_output = Arc::clone(&collect_output);
                    collect_output
                },
                {
                    let build_record = Arc::clone(&build_record);
                    build_record
                },
            )
            .await;
            selected_candidate = targeted_result.selected_candidate;
            candidates = targeted_result.candidates;
        }
    }

    let final_state = resolve_default_finalize_state(
        request,
        selected_candidate,
        candidates,
        false,
        &quality_gate_builder,
    )?;
    let mut finalize_request = ChapterCandidateFinalizeRequest {
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        runtime_state: request.runtime_state.take(),
    };
    let mut finalize_dependencies = build_default_finalize_dependencies();
    let result = finalize_selected_candidate_result(
        &mut finalize_request,
        final_state,
        &mut finalize_dependencies,
    );
    request.runtime_state = finalize_request.runtime_state;
    Ok(result)
}

async fn run_default_targeted_repair_stage<CollectOutput, CollectFuture, BuildRecord>(
    request: &mut ChapterCandidateExecutorRequest,
    base_prompt: &str,
    base_temperature: f64,
    repair_seed_candidate: Value,
    selected_candidate: Value,
    candidates: Vec<Value>,
    generation_label_suffix: &str,
    allow_followup_seed_defer: bool,
    collect_output: Arc<Mutex<CollectOutput>>,
    build_record: Arc<Mutex<BuildRecord>>,
) -> ChapterCandidateTargetedFinalRepairResult
where
    CollectOutput: FnMut(ChapterCandidateDefaultOutputCollectInput) -> CollectFuture + Send,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>> + Send,
    BuildRecord: FnMut(ChapterCandidateDefaultRecordBuildInput) -> Result<Value, String> + Send,
{
    let mut targeted_request = ChapterCandidateTargetedFinalRepairRequest {
        base_generate_kwargs: request.base_generate_kwargs.clone(),
        base_prompt: base_prompt.to_string(),
        base_temperature,
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        generation_label: request.generation_label.clone(),
        generation_label_suffix: generation_label_suffix.to_string(),
        repair_seed_candidate,
        current_winner_candidate: selected_candidate.clone(),
        runtime_state: request.runtime_state.take(),
        allow_followup_seed_defer,
    };
    let mut targeted_dependencies = ChapterCandidateTargetedFinalRepairDependencies {
        build_targeted_final_repair_suffix_fn:
            |input: ChapterCandidateTargetedFinalRepairSuffixInput| {
                build_targeted_final_repair_suffix(
                    input.quality_metrics,
                    input.quality_gate_plan,
                    input.target_word_count,
                    input.attempt_index,
                    input.source,
                )
            },
        resolve_targeted_final_repair_temperature_fn: resolve_targeted_final_repair_temperature,
        resolve_targeted_final_repair_max_tokens_fn: resolve_targeted_final_repair_max_tokens,
        collect_generation_candidate_output_fn: move |input| {
            let collect_output = Arc::clone(&collect_output);
            async move {
                let future = with_locked_callback(&collect_output, |collect_output| {
                    (collect_output)(default_collect_input_from_targeted(input))
                });
                future.await
            }
        },
        resolve_targeted_final_repair_char_limit_fn: resolve_targeted_final_repair_char_limit,
        build_generation_candidate_record_fn: move |input| {
            with_locked_callback(&build_record, |build_record| {
                (build_record)(default_record_input_from_targeted(input))
            })
        },
        should_keep_targeted_final_repair_candidate_fn: should_keep_targeted_final_repair_candidate,
        should_adopt_targeted_final_repair_candidate_fn:
            should_adopt_targeted_final_repair_candidate,
        should_prefer_targeted_final_repair_candidate_fn:
            should_prefer_targeted_final_repair_candidate,
        should_apply_followup_targeted_final_repair_fn: should_apply_followup_targeted_final_repair,
    };
    let result = execute_targeted_final_repair_pass_workflow(
        &mut targeted_request,
        selected_candidate,
        candidates,
        &mut targeted_dependencies,
    )
    .await;
    request.runtime_state = targeted_request.runtime_state;
    result
}

fn resolve_default_finalize_state<QualityGatePlanBuilder>(
    request: &ChapterCandidateExecutorRequest,
    selected_candidate: Value,
    candidates: Vec<Value>,
    allow_word_budget_repair_promotion: bool,
    quality_gate_builder: &Arc<Mutex<QualityGatePlanBuilder>>,
) -> Result<ChapterCandidateFinalizeState, String>
where
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value + Send,
{
    let finalize_request = ChapterCandidateFinalizeRequest {
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        runtime_state: request.runtime_state.clone(),
    };
    let mut finalize_dependencies = build_default_finalize_dependencies();
    let state = with_locked_callback(quality_gate_builder, |quality_gate_builder| {
        resolve_final_candidate_state(
            &finalize_request,
            selected_candidate,
            candidates,
            quality_gate_builder,
            &mut finalize_dependencies,
        )
    });
    if allow_word_budget_repair_promotion {
        return Ok(with_locked_callback(
            quality_gate_builder,
            |quality_gate_builder| {
                maybe_promote_best_word_budget_repair_candidate(
                    &finalize_request,
                    state,
                    quality_gate_builder,
                    &mut finalize_dependencies,
                )
            },
        ));
    }
    Ok(state)
}

fn with_locked_callback<T, R>(callback: &Mutex<T>, invoke: impl FnOnce(&mut T) -> R) -> R {
    let mut guard = callback
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    invoke(&mut *guard)
}

fn is_targeted_quality_repair_candidate(candidate: &Value) -> bool {
    let attempt_kind = candidate
        .get("attempt_kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let generation_path = candidate
        .get("generation_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    attempt_kind == "targeted_quality_repair" || generation_path == "targeted_quality_repair"
}

fn resolve_base_temperature(base_generate_kwargs: &Map<String, Value>) -> f64 {
    base_generate_kwargs
        .get("temperature")
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        })
        .unwrap_or(0.8)
}

fn build_default_finalize_dependencies() -> ChapterCandidateFinalizeDependencies<
    impl FnMut(Value, ChapterCandidateFinalizeMetadataContext) -> Value,
    impl FnMut(Value, Value) -> Value,
    impl FnMut(Value, i64, i64, Value) -> Value,
    impl FnMut(Vec<Value>, i64, Option<i64>) -> Value,
    impl FnMut(Vec<Value>) -> Option<Value>,
    impl FnMut(Value, Value) -> bool,
> {
    ChapterCandidateFinalizeDependencies {
        build_candidate_selection_metadata_fn:
            |quality_metrics, context: ChapterCandidateFinalizeMetadataContext| {
                build_candidate_selection_metadata(CandidateSelectionMetadataInput {
                    quality_metrics: Some(quality_metrics),
                    word_count: context.word_count,
                    target_word_count: context.target_word_count,
                    candidate_index: context.candidate_index,
                    candidate_count: context.candidate_count,
                    source: context.source,
                    quality_gate_plan: None,
                    generation_path: Some(context.generation_path),
                    attempt_kind: Some(context.attempt_kind),
                    rerank_used: Some(context.rerank_used),
                    word_budget_repair_used: Some(context.word_budget_repair_used),
                    winner_candidate_index: Some(context.winner_candidate_index),
                    repair_seed_candidate_index: None,
                    repair_seed_generation_path: None,
                    repair_seed_attempt_kind: None,
                })
            },
        attach_candidate_selection_metadata_fn: attach_candidate_selection_metadata,
        normalize_candidate_quality_gate_plan_fn: normalize_candidate_quality_gate_plan,
        build_candidate_pool_summary_fn: |candidates, winner, repair_seed| {
            build_candidate_pool_summary(candidates, Some(winner), repair_seed)
        },
        select_best_generation_candidate_fn: select_best_generation_candidate,
        should_prefer_word_budget_repair_candidate_fn: should_prefer_word_budget_repair_candidate,
    }
}

fn default_collect_input_from_generation(
    input: ChapterCandidateOutputCollectInput,
) -> ChapterCandidateDefaultOutputCollectInput {
    ChapterCandidateDefaultOutputCollectInput {
        generate_kwargs: input.generate_kwargs,
        candidate_index: input.candidate_index,
        max_output_chars: None,
    }
}

fn default_collect_input_from_word_budget(
    input: ChapterCandidateWordBudgetRepairOutputCollectInput,
) -> ChapterCandidateDefaultOutputCollectInput {
    ChapterCandidateDefaultOutputCollectInput {
        generate_kwargs: input.generate_kwargs,
        candidate_index: input.candidate_index,
        max_output_chars: input.max_output_chars,
    }
}

fn default_collect_input_from_targeted(
    input: ChapterCandidateTargetedFinalRepairOutputCollectInput,
) -> ChapterCandidateDefaultOutputCollectInput {
    ChapterCandidateDefaultOutputCollectInput {
        generate_kwargs: input.generate_kwargs,
        candidate_index: input.candidate_index,
        max_output_chars: input.max_output_chars,
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
        generate_best_ranked_candidate_with_default_dependency_wiring,
        ChapterCandidateDefaultOutputCollectInput, ChapterCandidateDefaultRecordBuildInput,
    };
    use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
    use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;

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
