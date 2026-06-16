// Rust owner for candidate executor orchestration originally mapped from
// Python chapter_candidate_executor_service.py. This module owns the
// generation -> word-budget repair -> targeted repair -> finalize sequence and
// keeps stage dependencies injectable for tests, default wiring, and route
// gateway rollback boundaries.

use std::{future::Future, pin::Pin};

use serde_json::{Map, Value};

use crate::services::chapter_candidate_finalize_service::{
    ChapterCandidateFinalizeRequest, ChapterCandidateFinalizeState,
};
use crate::services::chapter_candidate_generation_service::{
    ChapterCandidateGenerationRequest, ChapterCandidateGenerationResult,
};
use crate::services::chapter_candidate_targeted_final_repair_service::{
    ChapterCandidateTargetedFinalRepairRequest, ChapterCandidateTargetedFinalRepairResult,
};
use crate::services::chapter_candidate_word_budget_repair_service::{
    ChapterCandidateWordBudgetRepairRequest, ChapterCandidateWordBudgetRepairResult,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateExecutorRequest {
    pub(crate) base_generate_kwargs: Map<String, Value>,
    pub(crate) target_word_count: i64,
    pub(crate) source: String,
    pub(crate) generation_label: String,
    pub(crate) max_candidates: i64,
    pub(crate) runtime_state: Option<Value>,
}

#[cfg(test)]
pub(crate) struct ChapterCandidateExecutorDependencies<
    Generate,
    WordBudgetRepair,
    TargetedRepair,
    ResolveFinalize,
    FinalizeResult,
    ShouldTargetedRepair,
    ShouldFollowupTargetedRepair,
    SelectTargetedSeed,
> {
    pub(crate) generate_candidate_pool_fn: Generate,
    pub(crate) maybe_apply_word_budget_repair_fn: WordBudgetRepair,
    pub(crate) execute_targeted_final_repair_pass_fn: TargetedRepair,
    pub(crate) resolve_candidate_finalize_state_fn: ResolveFinalize,
    pub(crate) finalize_selected_candidate_result_fn: FinalizeResult,
    pub(crate) should_apply_targeted_final_repair_fn: ShouldTargetedRepair,
    pub(crate) should_apply_followup_targeted_final_repair_fn: ShouldFollowupTargetedRepair,
    pub(crate) select_targeted_final_repair_seed_candidate_fn: SelectTargetedSeed,
}

pub(crate) type ChapterCandidateGenerationStageFuture<'request> = Pin<
    Box<dyn Future<Output = Result<ChapterCandidateGenerationResult, String>> + Send + 'request>,
>;
pub(crate) type ChapterCandidateWordBudgetStageFuture<'request> =
    Pin<Box<dyn Future<Output = ChapterCandidateWordBudgetRepairResult> + Send + 'request>>;
pub(crate) type ChapterCandidateTargetedRepairStageFuture<'request> =
    Pin<Box<dyn Future<Output = ChapterCandidateTargetedFinalRepairResult> + Send + 'request>>;

pub(crate) struct ChapterCandidateExecutorBoxedDependencies<'deps> {
    pub(crate) generate_candidate_pool_fn: Box<
        dyn for<'request> FnMut(
                &'request mut ChapterCandidateGenerationRequest,
            ) -> ChapterCandidateGenerationStageFuture<'request>
            + Send
            + 'deps,
    >,
    pub(crate) maybe_apply_word_budget_repair_fn: Box<
        dyn for<'request> FnMut(
                &'request mut ChapterCandidateWordBudgetRepairRequest,
                Value,
                Vec<Value>,
            ) -> ChapterCandidateWordBudgetStageFuture<'request>
            + Send
            + 'deps,
    >,
    pub(crate) execute_targeted_final_repair_pass_fn: Box<
        dyn for<'request> FnMut(
                &'request mut ChapterCandidateTargetedFinalRepairRequest,
                Value,
                Vec<Value>,
            ) -> ChapterCandidateTargetedRepairStageFuture<'request>
            + Send
            + 'deps,
    >,
    pub(crate) resolve_candidate_finalize_state_fn: Box<
        dyn FnMut(ChapterCandidateExecutorFinalizeInput) -> ChapterCandidateFinalizeState
            + Send
            + 'deps,
    >,
    pub(crate) finalize_selected_candidate_result_fn: Box<
        dyn for<'request> FnMut(
                &'request mut ChapterCandidateFinalizeRequest,
                ChapterCandidateFinalizeState,
            ) -> Value
            + Send
            + 'deps,
    >,
    pub(crate) should_apply_targeted_final_repair_fn: Box<dyn FnMut(Value) -> bool + Send + 'deps>,
    pub(crate) should_apply_followup_targeted_final_repair_fn:
        Box<dyn FnMut(Value) -> bool + Send + 'deps>,
    pub(crate) select_targeted_final_repair_seed_candidate_fn:
        Box<dyn FnMut(Value, Vec<Value>) -> Option<Value> + Send + 'deps>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateExecutorFinalizeInput {
    pub(crate) request: ChapterCandidateFinalizeRequest,
    pub(crate) selected_candidate: Value,
    pub(crate) candidates: Vec<Value>,
    pub(crate) allow_word_budget_repair_promotion: bool,
}

#[cfg(test)]
pub(crate) async fn generate_best_ranked_candidate_workflow<
    Generate,
    GenerateFuture,
    WordBudgetRepair,
    WordBudgetFuture,
    TargetedRepair,
    TargetedFuture,
    ResolveFinalize,
    FinalizeResult,
    ShouldTargetedRepair,
    ShouldFollowupTargetedRepair,
    SelectTargetedSeed,
>(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorDependencies<
        Generate,
        WordBudgetRepair,
        TargetedRepair,
        ResolveFinalize,
        FinalizeResult,
        ShouldTargetedRepair,
        ShouldFollowupTargetedRepair,
        SelectTargetedSeed,
    >,
) -> Result<Value, String>
where
    Generate: FnMut(&mut ChapterCandidateGenerationRequest) -> GenerateFuture,
    GenerateFuture: Future<Output = Result<ChapterCandidateGenerationResult, String>>,
    WordBudgetRepair:
        FnMut(&mut ChapterCandidateWordBudgetRepairRequest, Value, Vec<Value>) -> WordBudgetFuture,
    WordBudgetFuture: Future<Output = ChapterCandidateWordBudgetRepairResult>,
    TargetedRepair:
        FnMut(&mut ChapterCandidateTargetedFinalRepairRequest, Value, Vec<Value>) -> TargetedFuture,
    TargetedFuture: Future<Output = ChapterCandidateTargetedFinalRepairResult>,
    ResolveFinalize: FnMut(ChapterCandidateExecutorFinalizeInput) -> ChapterCandidateFinalizeState,
    FinalizeResult:
        FnMut(&mut ChapterCandidateFinalizeRequest, ChapterCandidateFinalizeState) -> Value,
    ShouldTargetedRepair: FnMut(Value) -> bool,
    ShouldFollowupTargetedRepair: FnMut(Value) -> bool,
    SelectTargetedSeed: FnMut(Value, Vec<Value>) -> Option<Value>,
{
    let base_prompt = request
        .base_generate_kwargs
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let base_temperature = resolve_base_temperature(&request.base_generate_kwargs);

    let generation_result =
        run_generation_stage(request, dependencies, &base_prompt, base_temperature).await?;
    let mut candidates = generation_result.candidates;
    let mut selected_candidate = generation_result.selected_candidate;

    let word_budget_result = run_word_budget_repair_stage(
        request,
        dependencies,
        &base_prompt,
        base_temperature,
        selected_candidate,
        candidates,
    )
    .await;
    selected_candidate = word_budget_result.selected_candidate;
    candidates = word_budget_result.candidates;

    let mut deferred_followup_targeted_repair_seed_candidate = None;
    if (dependencies.should_apply_targeted_final_repair_fn)(selected_candidate.clone()) {
        let targeted_result = run_targeted_repair_stage(
            request,
            dependencies,
            &base_prompt,
            base_temperature,
            selected_candidate.clone(),
            selected_candidate,
            candidates,
            "targeted-repair",
            true,
        )
        .await;
        selected_candidate = targeted_result.selected_candidate;
        candidates = targeted_result.candidates;
        deferred_followup_targeted_repair_seed_candidate =
            targeted_result.deferred_followup_targeted_repair_seed_candidate;
    }

    let mut final_state =
        resolve_finalize_state(request, dependencies, selected_candidate, candidates, true);
    selected_candidate = final_state.selected_candidate.clone();
    candidates = final_state.candidates.clone();

    let targeted_seed = select_post_finalize_targeted_repair_seed_candidate(
        selected_candidate.clone(),
        candidates.clone(),
        deferred_followup_targeted_repair_seed_candidate,
        dependencies,
    );
    if let Some(targeted_seed) = targeted_seed {
        let targeted_result = run_targeted_repair_stage(
            request,
            dependencies,
            &base_prompt,
            base_temperature,
            targeted_seed,
            selected_candidate,
            candidates,
            "targeted-repair-post-finalize",
            false,
        )
        .await;
        selected_candidate = targeted_result.selected_candidate;
        candidates = targeted_result.candidates;

        final_state = resolve_finalize_state(
            request,
            dependencies,
            selected_candidate.clone(),
            candidates.clone(),
            false,
        );
        if (dependencies.should_apply_followup_targeted_final_repair_fn)(
            final_state.selected_candidate.clone(),
        ) {
            let targeted_result = run_targeted_repair_stage(
                request,
                dependencies,
                &base_prompt,
                base_temperature,
                final_state.selected_candidate.clone(),
                final_state.selected_candidate,
                candidates,
                "targeted-repair-followup",
                false,
            )
            .await;
            selected_candidate = targeted_result.selected_candidate;
            candidates = targeted_result.candidates;
        }
    }

    let final_state =
        resolve_finalize_state(request, dependencies, selected_candidate, candidates, false);
    let mut finalize_request = ChapterCandidateFinalizeRequest {
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        runtime_state: request.runtime_state.take(),
    };
    let result =
        (dependencies.finalize_selected_candidate_result_fn)(&mut finalize_request, final_state);
    request.runtime_state = finalize_request.runtime_state;
    Ok(result)
}

pub(crate) async fn generate_best_ranked_candidate_workflow_with_boxed_dependencies(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorBoxedDependencies<'_>,
) -> Result<Value, String> {
    let base_prompt = request
        .base_generate_kwargs
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let base_temperature = resolve_base_temperature(&request.base_generate_kwargs);

    let generation_result =
        run_generation_stage_boxed(request, dependencies, &base_prompt, base_temperature).await?;
    let mut candidates = generation_result.candidates;
    let mut selected_candidate = generation_result.selected_candidate;

    let word_budget_result = run_word_budget_repair_stage_boxed(
        request,
        dependencies,
        &base_prompt,
        base_temperature,
        selected_candidate,
        candidates,
    )
    .await;
    selected_candidate = word_budget_result.selected_candidate;
    candidates = word_budget_result.candidates;

    let mut deferred_followup_targeted_repair_seed_candidate = None;
    if (dependencies.should_apply_targeted_final_repair_fn)(selected_candidate.clone()) {
        let targeted_result = run_targeted_repair_stage_boxed(
            request,
            dependencies,
            &base_prompt,
            base_temperature,
            selected_candidate.clone(),
            selected_candidate,
            candidates,
            "targeted-repair",
            true,
        )
        .await;
        selected_candidate = targeted_result.selected_candidate;
        candidates = targeted_result.candidates;
        deferred_followup_targeted_repair_seed_candidate =
            targeted_result.deferred_followup_targeted_repair_seed_candidate;
    }

    let mut final_state =
        resolve_finalize_state_boxed(request, dependencies, selected_candidate, candidates, true);
    selected_candidate = final_state.selected_candidate.clone();
    candidates = final_state.candidates.clone();

    let targeted_seed = select_post_finalize_targeted_repair_seed_candidate_boxed(
        selected_candidate.clone(),
        candidates.clone(),
        deferred_followup_targeted_repair_seed_candidate,
        dependencies,
    );
    if let Some(targeted_seed) = targeted_seed {
        let targeted_result = run_targeted_repair_stage_boxed(
            request,
            dependencies,
            &base_prompt,
            base_temperature,
            targeted_seed,
            selected_candidate,
            candidates,
            "targeted-repair-post-finalize",
            false,
        )
        .await;
        selected_candidate = targeted_result.selected_candidate;
        candidates = targeted_result.candidates;

        final_state = resolve_finalize_state_boxed(
            request,
            dependencies,
            selected_candidate.clone(),
            candidates.clone(),
            false,
        );
        if (dependencies.should_apply_followup_targeted_final_repair_fn)(
            final_state.selected_candidate.clone(),
        ) {
            let targeted_result = run_targeted_repair_stage_boxed(
                request,
                dependencies,
                &base_prompt,
                base_temperature,
                final_state.selected_candidate.clone(),
                final_state.selected_candidate,
                candidates,
                "targeted-repair-followup",
                false,
            )
            .await;
            selected_candidate = targeted_result.selected_candidate;
            candidates = targeted_result.candidates;
        }
    }

    let final_state =
        resolve_finalize_state_boxed(request, dependencies, selected_candidate, candidates, false);
    let mut finalize_request = ChapterCandidateFinalizeRequest {
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        runtime_state: request.runtime_state.take(),
    };
    let result =
        (dependencies.finalize_selected_candidate_result_fn)(&mut finalize_request, final_state);
    request.runtime_state = finalize_request.runtime_state;
    Ok(result)
}

#[cfg(test)]
async fn run_generation_stage<
    Generate,
    GenerateFuture,
    WordBudgetRepair,
    TargetedRepair,
    ResolveFinalize,
    FinalizeResult,
    ShouldTargetedRepair,
    ShouldFollowupTargetedRepair,
    SelectTargetedSeed,
>(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorDependencies<
        Generate,
        WordBudgetRepair,
        TargetedRepair,
        ResolveFinalize,
        FinalizeResult,
        ShouldTargetedRepair,
        ShouldFollowupTargetedRepair,
        SelectTargetedSeed,
    >,
    base_prompt: &str,
    base_temperature: f64,
) -> Result<ChapterCandidateGenerationResult, String>
where
    Generate: FnMut(&mut ChapterCandidateGenerationRequest) -> GenerateFuture,
    GenerateFuture: Future<Output = Result<ChapterCandidateGenerationResult, String>>,
{
    let mut generation_request = ChapterCandidateGenerationRequest {
        base_generate_kwargs: request.base_generate_kwargs.clone(),
        base_prompt: base_prompt.to_string(),
        base_temperature,
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        generation_label: request.generation_label.clone(),
        max_candidates: request.max_candidates,
        runtime_state: request.runtime_state.take(),
    };
    let result = (dependencies.generate_candidate_pool_fn)(&mut generation_request).await;
    request.runtime_state = generation_request.runtime_state;
    result
}

#[cfg(test)]
async fn run_word_budget_repair_stage<
    Generate,
    WordBudgetRepair,
    WordBudgetFuture,
    TargetedRepair,
    ResolveFinalize,
    FinalizeResult,
    ShouldTargetedRepair,
    ShouldFollowupTargetedRepair,
    SelectTargetedSeed,
>(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorDependencies<
        Generate,
        WordBudgetRepair,
        TargetedRepair,
        ResolveFinalize,
        FinalizeResult,
        ShouldTargetedRepair,
        ShouldFollowupTargetedRepair,
        SelectTargetedSeed,
    >,
    base_prompt: &str,
    base_temperature: f64,
    selected_candidate: Value,
    candidates: Vec<Value>,
) -> ChapterCandidateWordBudgetRepairResult
where
    WordBudgetRepair:
        FnMut(&mut ChapterCandidateWordBudgetRepairRequest, Value, Vec<Value>) -> WordBudgetFuture,
    WordBudgetFuture: Future<Output = ChapterCandidateWordBudgetRepairResult>,
{
    let mut repair_request = ChapterCandidateWordBudgetRepairRequest {
        base_generate_kwargs: request.base_generate_kwargs.clone(),
        base_prompt: base_prompt.to_string(),
        base_temperature,
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        generation_label: request.generation_label.clone(),
        runtime_state: request.runtime_state.take(),
    };
    let result = (dependencies.maybe_apply_word_budget_repair_fn)(
        &mut repair_request,
        selected_candidate,
        candidates,
    )
    .await;
    request.runtime_state = repair_request.runtime_state;
    result
}

#[cfg(test)]
async fn run_targeted_repair_stage<
    Generate,
    WordBudgetRepair,
    TargetedRepair,
    TargetedFuture,
    ResolveFinalize,
    FinalizeResult,
    ShouldTargetedRepair,
    ShouldFollowupTargetedRepair,
    SelectTargetedSeed,
>(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorDependencies<
        Generate,
        WordBudgetRepair,
        TargetedRepair,
        ResolveFinalize,
        FinalizeResult,
        ShouldTargetedRepair,
        ShouldFollowupTargetedRepair,
        SelectTargetedSeed,
    >,
    base_prompt: &str,
    base_temperature: f64,
    repair_seed_candidate: Value,
    selected_candidate: Value,
    candidates: Vec<Value>,
    generation_label_suffix: &str,
    allow_followup_seed_defer: bool,
) -> ChapterCandidateTargetedFinalRepairResult
where
    TargetedRepair:
        FnMut(&mut ChapterCandidateTargetedFinalRepairRequest, Value, Vec<Value>) -> TargetedFuture,
    TargetedFuture: Future<Output = ChapterCandidateTargetedFinalRepairResult>,
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
    let result = (dependencies.execute_targeted_final_repair_pass_fn)(
        &mut targeted_request,
        selected_candidate,
        candidates,
    )
    .await;
    request.runtime_state = targeted_request.runtime_state;
    result
}

async fn run_generation_stage_boxed(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorBoxedDependencies<'_>,
    base_prompt: &str,
    base_temperature: f64,
) -> Result<ChapterCandidateGenerationResult, String> {
    let mut generation_request = ChapterCandidateGenerationRequest {
        base_generate_kwargs: request.base_generate_kwargs.clone(),
        base_prompt: base_prompt.to_string(),
        base_temperature,
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        generation_label: request.generation_label.clone(),
        max_candidates: request.max_candidates,
        runtime_state: request.runtime_state.take(),
    };
    let result = (dependencies.generate_candidate_pool_fn)(&mut generation_request).await;
    request.runtime_state = generation_request.runtime_state;
    result
}

async fn run_word_budget_repair_stage_boxed(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorBoxedDependencies<'_>,
    base_prompt: &str,
    base_temperature: f64,
    selected_candidate: Value,
    candidates: Vec<Value>,
) -> ChapterCandidateWordBudgetRepairResult {
    let mut repair_request = ChapterCandidateWordBudgetRepairRequest {
        base_generate_kwargs: request.base_generate_kwargs.clone(),
        base_prompt: base_prompt.to_string(),
        base_temperature,
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        generation_label: request.generation_label.clone(),
        runtime_state: request.runtime_state.take(),
    };
    let result = (dependencies.maybe_apply_word_budget_repair_fn)(
        &mut repair_request,
        selected_candidate,
        candidates,
    )
    .await;
    request.runtime_state = repair_request.runtime_state;
    result
}

async fn run_targeted_repair_stage_boxed(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorBoxedDependencies<'_>,
    base_prompt: &str,
    base_temperature: f64,
    repair_seed_candidate: Value,
    selected_candidate: Value,
    candidates: Vec<Value>,
    generation_label_suffix: &str,
    allow_followup_seed_defer: bool,
) -> ChapterCandidateTargetedFinalRepairResult {
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
    let result = (dependencies.execute_targeted_final_repair_pass_fn)(
        &mut targeted_request,
        selected_candidate,
        candidates,
    )
    .await;
    request.runtime_state = targeted_request.runtime_state;
    result
}

fn resolve_finalize_state_boxed(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorBoxedDependencies<'_>,
    selected_candidate: Value,
    candidates: Vec<Value>,
    allow_word_budget_repair_promotion: bool,
) -> ChapterCandidateFinalizeState {
    let finalize_request = ChapterCandidateFinalizeRequest {
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        runtime_state: request.runtime_state.clone(),
    };
    (dependencies.resolve_candidate_finalize_state_fn)(ChapterCandidateExecutorFinalizeInput {
        request: finalize_request,
        selected_candidate,
        candidates,
        allow_word_budget_repair_promotion,
    })
}

fn select_post_finalize_targeted_repair_seed_candidate_boxed(
    selected_candidate: Value,
    candidates: Vec<Value>,
    deferred_followup_targeted_repair_seed_candidate: Option<Value>,
    dependencies: &mut ChapterCandidateExecutorBoxedDependencies<'_>,
) -> Option<Value> {
    if (dependencies.should_apply_followup_targeted_final_repair_fn)(selected_candidate.clone()) {
        return Some(selected_candidate);
    }
    if deferred_followup_targeted_repair_seed_candidate.is_some() {
        return deferred_followup_targeted_repair_seed_candidate;
    }
    if is_targeted_quality_repair_candidate(&selected_candidate) {
        return None;
    }
    (dependencies.select_targeted_final_repair_seed_candidate_fn)(selected_candidate, candidates)
}

#[cfg(test)]
fn resolve_finalize_state<
    Generate,
    WordBudgetRepair,
    TargetedRepair,
    ResolveFinalize,
    FinalizeResult,
    ShouldTargetedRepair,
    ShouldFollowupTargetedRepair,
    SelectTargetedSeed,
>(
    request: &mut ChapterCandidateExecutorRequest,
    dependencies: &mut ChapterCandidateExecutorDependencies<
        Generate,
        WordBudgetRepair,
        TargetedRepair,
        ResolveFinalize,
        FinalizeResult,
        ShouldTargetedRepair,
        ShouldFollowupTargetedRepair,
        SelectTargetedSeed,
    >,
    selected_candidate: Value,
    candidates: Vec<Value>,
    allow_word_budget_repair_promotion: bool,
) -> ChapterCandidateFinalizeState
where
    ResolveFinalize: FnMut(ChapterCandidateExecutorFinalizeInput) -> ChapterCandidateFinalizeState,
{
    let finalize_request = ChapterCandidateFinalizeRequest {
        target_word_count: request.target_word_count,
        source: request.source.clone(),
        runtime_state: request.runtime_state.clone(),
    };
    (dependencies.resolve_candidate_finalize_state_fn)(ChapterCandidateExecutorFinalizeInput {
        request: finalize_request,
        selected_candidate,
        candidates,
        allow_word_budget_repair_promotion,
    })
}

#[cfg(test)]
fn select_post_finalize_targeted_repair_seed_candidate<
    Generate,
    WordBudgetRepair,
    TargetedRepair,
    ResolveFinalize,
    FinalizeResult,
    ShouldTargetedRepair,
    ShouldFollowupTargetedRepair,
    SelectTargetedSeed,
>(
    selected_candidate: Value,
    candidates: Vec<Value>,
    deferred_followup_targeted_repair_seed_candidate: Option<Value>,
    dependencies: &mut ChapterCandidateExecutorDependencies<
        Generate,
        WordBudgetRepair,
        TargetedRepair,
        ResolveFinalize,
        FinalizeResult,
        ShouldTargetedRepair,
        ShouldFollowupTargetedRepair,
        SelectTargetedSeed,
    >,
) -> Option<Value>
where
    ShouldFollowupTargetedRepair: FnMut(Value) -> bool,
    SelectTargetedSeed: FnMut(Value, Vec<Value>) -> Option<Value>,
{
    if (dependencies.should_apply_followup_targeted_final_repair_fn)(selected_candidate.clone()) {
        return Some(selected_candidate);
    }
    if deferred_followup_targeted_repair_seed_candidate.is_some() {
        return deferred_followup_targeted_repair_seed_candidate;
    }
    if is_targeted_quality_repair_candidate(&selected_candidate) {
        return None;
    }
    (dependencies.select_targeted_final_repair_seed_candidate_fn)(selected_candidate, candidates)
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

#[cfg(test)]
fn build_chapter_candidate_executor_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_candidate_executor_service",
        "scope": "candidate_generation_repair_finalize_and_post_finalize_orchestration_owner",
        "python_source_map": [
            "backend/app/services/chapter_candidate_executor_service.py",
            "backend/app/services/chapter_candidate_executor_wiring_service.py",
            "backend/app/services/chapter_candidate_generation_service.py",
            "backend/app/services/chapter_candidate_word_budget_repair_service.py",
            "backend/app/services/chapter_candidate_targeted_final_repair_service.py",
            "backend/app/services/chapter_candidate_finalize_service.py"
        ],
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
                "runtime_state"
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
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_candidate_executor_owner_ready_for_source_map_closeout_review"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_executor_python_source_map",
            "python_fallback_removal_ready": false,
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
        }
    }

    #[test]
    fn should_publish_chapter_candidate_executor_owner_contract() {
        let contract = build_chapter_candidate_executor_owner_contract();

        assert_eq!(contract["owner"], "chapter_candidate_executor_service");
        assert_eq!(
            contract["scope"],
            "candidate_generation_repair_finalize_and_post_finalize_orchestration_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/services/chapter_candidate_executor_service.py"
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
            false
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
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_candidate_executor_owner_ready_for_source_map_closeout_review"
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
