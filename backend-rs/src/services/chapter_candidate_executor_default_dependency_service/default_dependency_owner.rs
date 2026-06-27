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
