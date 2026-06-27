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
