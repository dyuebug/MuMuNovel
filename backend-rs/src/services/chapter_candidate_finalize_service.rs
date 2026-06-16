// Rust owner for final-candidate orchestration originally mapped from Python
// chapter_candidate_finalize_service.py. The default executor dependency owner
// resolves and finalizes candidates through this module directly.

use serde_json::{json, Map, Value};

use crate::services::chapter_candidate_rerank_service::{
    attach_candidate_selection_metadata, build_candidate_pool_summary,
    build_candidate_selection_metadata, normalize_candidate_quality_gate_plan,
    select_best_generation_candidate, should_prefer_word_budget_repair_candidate,
    CandidateSelectionMetadataInput,
};
use crate::services::chapter_candidate_runtime_state_service::{
    resolve_generation_attempt_labels, sync_chapter_candidate_runtime_state,
    ChapterCandidateRuntimeStatePatch,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateFinalizeRequest {
    pub(crate) target_word_count: i64,
    pub(crate) source: String,
    pub(crate) runtime_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateFinalizeMetadataContext {
    pub(crate) word_count: i64,
    pub(crate) target_word_count: i64,
    pub(crate) candidate_index: i64,
    pub(crate) candidate_count: i64,
    pub(crate) source: String,
    pub(crate) generation_path: String,
    pub(crate) attempt_kind: String,
    pub(crate) rerank_used: bool,
    pub(crate) word_budget_repair_used: bool,
    pub(crate) winner_candidate_index: i64,
}

pub(crate) struct ChapterCandidateFinalizeDependencies<
    BuildSelection,
    AttachSelection,
    NormalizeGate,
    BuildPoolSummary,
    SelectBest,
    PreferWordBudgetRepair,
> {
    pub(crate) build_candidate_selection_metadata_fn: BuildSelection,
    pub(crate) attach_candidate_selection_metadata_fn: AttachSelection,
    pub(crate) normalize_candidate_quality_gate_plan_fn: NormalizeGate,
    pub(crate) build_candidate_pool_summary_fn: BuildPoolSummary,
    pub(crate) select_best_generation_candidate_fn: SelectBest,
    pub(crate) should_prefer_word_budget_repair_candidate_fn: PreferWordBudgetRepair,
}

pub(crate) fn build_default_finalize_dependencies() -> ChapterCandidateFinalizeDependencies<
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateFinalizeState {
    pub(crate) selected_candidate: Value,
    pub(crate) candidates: Vec<Value>,
    pub(crate) winner_candidate_index: i64,
    pub(crate) final_attempt_kind: String,
    pub(crate) final_generation_path: String,
    pub(crate) final_quality_metrics: Map<String, Value>,
    pub(crate) final_quality_gate_plan: Map<String, Value>,
    pub(crate) rerank_used: bool,
    pub(crate) word_budget_repair_used: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct ChapterCandidateFinalizeStateView {
    final_quality_gate_decision: String,
    repair_seed_candidate_index: Option<i64>,
}

impl ChapterCandidateFinalizeState {
    pub(crate) fn candidate_count(&self) -> i64 {
        self.candidates.len().max(1) as i64
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateRuntimeFinalizeSyncInput {
    pub(crate) candidate_index: i64,
    pub(crate) candidate_total: i64,
    pub(crate) current_chars: i64,
    pub(crate) chunk_count: i64,
    pub(crate) generation_path: String,
    pub(crate) attempt_kind: String,
    pub(crate) rerank_used: bool,
    pub(crate) word_budget_repair_used: bool,
    pub(crate) winner_candidate_index: i64,
}

pub(crate) fn resolve_final_candidate_state<
    QualityGatePlanBuilder,
    BuildSelection,
    AttachSelection,
    NormalizeGate,
    BuildPoolSummary,
    SelectBest,
    PreferWordBudgetRepair,
>(
    request: &ChapterCandidateFinalizeRequest,
    selected_candidate: Value,
    candidates: Vec<Value>,
    quality_gate_plan_builder: &mut QualityGatePlanBuilder,
    dependencies: &mut ChapterCandidateFinalizeDependencies<
        BuildSelection,
        AttachSelection,
        NormalizeGate,
        BuildPoolSummary,
        SelectBest,
        PreferWordBudgetRepair,
    >,
) -> ChapterCandidateFinalizeState
where
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value,
    BuildSelection: FnMut(Value, ChapterCandidateFinalizeMetadataContext) -> Value,
    AttachSelection: FnMut(Value, Value) -> Value,
    NormalizeGate: FnMut(Value, i64, i64, Value) -> Value,
    BuildPoolSummary: FnMut(Vec<Value>, i64, Option<i64>) -> Value,
    SelectBest: FnMut(Vec<Value>) -> Option<Value>,
    PreferWordBudgetRepair: FnMut(Value, Value) -> bool,
{
    let mut selected_candidate_map = object_from_value(selected_candidate);
    let candidate_view = snapshot_chapter_candidate(&selected_candidate_map);
    let winner_candidate_index = candidate_view.candidate_index;
    let word_budget_repair_used = is_word_budget_repair_candidate(&selected_candidate_map);
    let rerank_used = winner_candidate_index > 1 && !word_budget_repair_used;
    let final_attempt_kind = if candidate_view.attempt_kind.is_empty() {
        resolve_generation_attempt_labels(winner_candidate_index, word_budget_repair_used)
            .attempt_kind
            .to_string()
    } else {
        candidate_view.attempt_kind
    };
    let final_generation_path = if candidate_view.generation_path.is_empty() {
        if word_budget_repair_used {
            "word_budget_repair".to_string()
        } else if rerank_used {
            "rerank_retry".to_string()
        } else {
            "single_pass".to_string()
        }
    } else {
        candidate_view.generation_path
    };

    let mut final_quality_metrics = candidate_view.quality_metrics;
    let provisional_quality_gate_plan = candidate_view.quality_gate_plan;
    let metadata_context = build_finalize_metadata_context(
        request,
        candidates.len() as i64,
        winner_candidate_index,
        final_attempt_kind.clone(),
        final_generation_path.clone(),
        rerank_used,
        word_budget_repair_used,
        candidate_view.word_count,
    );
    let (_selection_metadata, attached_quality_metrics) = build_attached_final_selection_metadata(
        final_quality_metrics,
        &provisional_quality_gate_plan,
        metadata_context.clone(),
        dependencies,
    );
    final_quality_metrics = attached_quality_metrics;

    let final_quality_gate_plan =
        (quality_gate_plan_builder)(Value::Object(final_quality_metrics.clone()), 0);
    let final_quality_gate_plan = object_from_value((dependencies
        .normalize_candidate_quality_gate_plan_fn)(
        final_quality_gate_plan,
        candidate_view.word_count,
        request.target_word_count,
        Value::Object(final_quality_metrics.clone()),
    ));
    copy_quality_gate_into_metrics(&final_quality_gate_plan, &mut final_quality_metrics);
    let (final_selection_metadata, attached_quality_metrics) =
        build_attached_final_selection_metadata(
            final_quality_metrics,
            &final_quality_gate_plan,
            metadata_context,
            dependencies,
        );
    final_quality_metrics = attached_quality_metrics;

    for (key, value) in final_selection_metadata {
        selected_candidate_map.insert(key, value);
    }
    selected_candidate_map.insert(
        "quality_metrics".to_string(),
        Value::Object(final_quality_metrics.clone()),
    );
    selected_candidate_map.insert(
        "quality_gate_plan".to_string(),
        Value::Object(final_quality_gate_plan.clone()),
    );

    ChapterCandidateFinalizeState {
        selected_candidate: Value::Object(selected_candidate_map),
        candidates,
        winner_candidate_index,
        final_attempt_kind,
        final_generation_path,
        final_quality_metrics,
        final_quality_gate_plan,
        rerank_used,
        word_budget_repair_used,
    }
}

pub(crate) fn maybe_promote_best_word_budget_repair_candidate<
    QualityGatePlanBuilder,
    BuildSelection,
    AttachSelection,
    NormalizeGate,
    BuildPoolSummary,
    SelectBest,
    PreferWordBudgetRepair,
>(
    request: &ChapterCandidateFinalizeRequest,
    state: ChapterCandidateFinalizeState,
    quality_gate_plan_builder: &mut QualityGatePlanBuilder,
    dependencies: &mut ChapterCandidateFinalizeDependencies<
        BuildSelection,
        AttachSelection,
        NormalizeGate,
        BuildPoolSummary,
        SelectBest,
        PreferWordBudgetRepair,
    >,
) -> ChapterCandidateFinalizeState
where
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value,
    BuildSelection: FnMut(Value, ChapterCandidateFinalizeMetadataContext) -> Value,
    AttachSelection: FnMut(Value, Value) -> Value,
    NormalizeGate: FnMut(Value, i64, i64, Value) -> Value,
    BuildPoolSummary: FnMut(Vec<Value>, i64, Option<i64>) -> Value,
    SelectBest: FnMut(Vec<Value>) -> Option<Value>,
    PreferWordBudgetRepair: FnMut(Value, Value) -> bool,
{
    let state_view = snapshot_finalize_state(&state);
    if state_view.final_quality_gate_decision == "allow_save" {
        return state;
    }

    let repair_candidates = collect_word_budget_repair_candidates(&state.candidates);
    if repair_candidates.is_empty() {
        return state;
    }

    let best_repair_candidate =
        (dependencies.select_best_generation_candidate_fn)(repair_candidates.clone())
            .or_else(|| repair_candidates.last().cloned());
    let Some(best_repair_candidate) = best_repair_candidate else {
        return state;
    };
    if value_to_i64(best_repair_candidate.get("candidate_index")).unwrap_or(0)
        == state.winner_candidate_index
    {
        return state;
    }
    if !(dependencies.should_prefer_word_budget_repair_candidate_fn)(
        state.selected_candidate.clone(),
        best_repair_candidate.clone(),
    ) {
        return state;
    }

    resolve_final_candidate_state(
        request,
        best_repair_candidate,
        state.candidates,
        quality_gate_plan_builder,
        dependencies,
    )
}

pub(crate) fn finalize_selected_candidate_result<
    BuildSelection,
    AttachSelection,
    NormalizeGate,
    BuildPoolSummary,
    SelectBest,
    PreferWordBudgetRepair,
>(
    request: &mut ChapterCandidateFinalizeRequest,
    state: ChapterCandidateFinalizeState,
    dependencies: &mut ChapterCandidateFinalizeDependencies<
        BuildSelection,
        AttachSelection,
        NormalizeGate,
        BuildPoolSummary,
        SelectBest,
        PreferWordBudgetRepair,
    >,
) -> Value
where
    BuildSelection: FnMut(Value, ChapterCandidateFinalizeMetadataContext) -> Value,
    AttachSelection: FnMut(Value, Value) -> Value,
    NormalizeGate: FnMut(Value, i64, i64, Value) -> Value,
    BuildPoolSummary: FnMut(Vec<Value>, i64, Option<i64>) -> Value,
    SelectBest: FnMut(Vec<Value>) -> Option<Value>,
    PreferWordBudgetRepair: FnMut(Value, Value) -> bool,
{
    let candidate_count = state.candidate_count();
    let state_view = snapshot_finalize_state(&state);
    let mut selected_candidate = object_from_value(state.selected_candidate);
    let selected_candidate_view = snapshot_chapter_candidate(&selected_candidate);
    insert_i64(&mut selected_candidate, "candidate_count", candidate_count);
    insert_i64(&mut selected_candidate, "rerank_pool_size", candidate_count);
    let candidate_pool_summary = (dependencies.build_candidate_pool_summary_fn)(
        state.candidates.clone(),
        state.winner_candidate_index,
        state_view.repair_seed_candidate_index,
    );
    if candidate_pool_summary
        .as_array()
        .is_some_and(|items| !items.is_empty())
    {
        selected_candidate.insert(
            "candidate_pool_summary".to_string(),
            candidate_pool_summary.clone(),
        );
        let mut final_quality_metrics = selected_candidate_view.quality_metrics;
        final_quality_metrics.insert("candidate_pool_summary".to_string(), candidate_pool_summary);
        selected_candidate.insert(
            "quality_metrics".to_string(),
            Value::Object(final_quality_metrics),
        );
    }

    sync_finalize_runtime_state(
        request.runtime_state.as_mut(),
        ChapterCandidateRuntimeFinalizeSyncInput {
            candidate_index: state.winner_candidate_index,
            candidate_total: candidate_count,
            current_chars: selected_candidate_view.word_count,
            chunk_count: selected_candidate_view.candidate_chunks.len() as i64,
            generation_path: state.final_generation_path,
            attempt_kind: state.final_attempt_kind,
            rerank_used: state.rerank_used,
            word_budget_repair_used: state.word_budget_repair_used,
            winner_candidate_index: state.winner_candidate_index,
        },
    );
    Value::Object(selected_candidate)
}

fn build_finalize_metadata_context(
    request: &ChapterCandidateFinalizeRequest,
    candidate_count: i64,
    winner_candidate_index: i64,
    final_attempt_kind: String,
    final_generation_path: String,
    rerank_used: bool,
    word_budget_repair_used: bool,
    word_count: i64,
) -> ChapterCandidateFinalizeMetadataContext {
    ChapterCandidateFinalizeMetadataContext {
        word_count,
        target_word_count: request.target_word_count,
        candidate_index: winner_candidate_index,
        candidate_count,
        source: request.source.clone(),
        generation_path: final_generation_path,
        attempt_kind: final_attempt_kind,
        rerank_used,
        word_budget_repair_used,
        winner_candidate_index,
    }
}

fn build_attached_final_selection_metadata<
    BuildSelection,
    AttachSelection,
    NormalizeGate,
    BuildPoolSummary,
    SelectBest,
    PreferWordBudgetRepair,
>(
    quality_metrics: Map<String, Value>,
    _quality_gate_plan: &Map<String, Value>,
    metadata_context: ChapterCandidateFinalizeMetadataContext,
    dependencies: &mut ChapterCandidateFinalizeDependencies<
        BuildSelection,
        AttachSelection,
        NormalizeGate,
        BuildPoolSummary,
        SelectBest,
        PreferWordBudgetRepair,
    >,
) -> (Map<String, Value>, Map<String, Value>)
where
    BuildSelection: FnMut(Value, ChapterCandidateFinalizeMetadataContext) -> Value,
    AttachSelection: FnMut(Value, Value) -> Value,
{
    let selection_metadata = object_from_value((dependencies
        .build_candidate_selection_metadata_fn)(
        Value::Object(quality_metrics.clone()),
        metadata_context,
    ));
    let attached_quality_metrics = object_from_value((dependencies
        .attach_candidate_selection_metadata_fn)(
        Value::Object(quality_metrics),
        Value::Object(selection_metadata.clone()),
    ));
    (selection_metadata, attached_quality_metrics)
}

fn sync_finalize_runtime_state(
    runtime_state: Option<&mut Value>,
    input: ChapterCandidateRuntimeFinalizeSyncInput,
) {
    sync_chapter_candidate_runtime_state(
        runtime_state,
        input.candidate_index,
        input.candidate_total,
        ChapterCandidateRuntimeStatePatch {
            current_chars: Some(input.current_chars),
            chunk_count: Some(input.chunk_count),
            generation_path: Some(input.generation_path),
            attempt_kind: Some(input.attempt_kind),
            rerank_used: Some(input.rerank_used),
            word_budget_repair_used: Some(input.word_budget_repair_used),
            winner_candidate_index: Some(input.winner_candidate_index),
        },
    );
}

fn snapshot_finalize_state(
    state: &ChapterCandidateFinalizeState,
) -> ChapterCandidateFinalizeStateView {
    ChapterCandidateFinalizeStateView {
        final_quality_gate_decision: state
            .final_quality_gate_plan
            .get("quality_gate")
            .and_then(Value::as_object)
            .and_then(|gate| gate.get("decision"))
            .and_then(|value| safe_text_from_value(Some(value)))
            .unwrap_or_default(),
        repair_seed_candidate_index: state
            .final_quality_metrics
            .get("candidate_selection")
            .and_then(Value::as_object)
            .and_then(|selection| selection.get("repair_seed_candidate_index"))
            .and_then(|value| value_to_i64(Some(value)))
            .filter(|value| *value > 0),
    }
}

#[derive(Debug, Clone)]
struct ChapterCandidateView {
    candidate_index: i64,
    word_count: i64,
    generation_path: String,
    attempt_kind: String,
    candidate_chunks: Vec<String>,
    quality_metrics: Map<String, Value>,
    quality_gate_plan: Map<String, Value>,
}

fn snapshot_chapter_candidate(candidate: &Map<String, Value>) -> ChapterCandidateView {
    let full_content = candidate
        .get("full_content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let candidate_index = value_to_i64(candidate.get("candidate_index"))
        .unwrap_or(1)
        .max(1);
    let word_count = value_to_i64(candidate.get("word_count"))
        .unwrap_or_else(|| full_content.chars().count() as i64)
        .max(0);
    let candidate_chunks = candidate
        .get("candidate_chunks")
        .and_then(Value::as_array)
        .map(|chunks| {
            chunks
                .iter()
                .map(|chunk| {
                    chunk
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| chunk.to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    ChapterCandidateView {
        candidate_index,
        word_count,
        generation_path: trimmed_string(candidate.get("generation_path")),
        attempt_kind: trimmed_string(candidate.get("attempt_kind")),
        candidate_chunks,
        quality_metrics: candidate
            .get("quality_metrics")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
        quality_gate_plan: candidate
            .get("quality_gate_plan")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
    }
}

fn is_word_budget_repair_candidate(candidate: &Map<String, Value>) -> bool {
    let view = snapshot_chapter_candidate(candidate);
    view.attempt_kind == "word_budget_repair" || view.generation_path == "word_budget_repair"
}

fn collect_word_budget_repair_candidates(candidates: &[Value]) -> Vec<Value> {
    candidates
        .iter()
        .filter_map(|candidate| candidate.as_object())
        .filter(|candidate| is_word_budget_repair_candidate(candidate))
        .cloned()
        .map(Value::Object)
        .collect()
}

fn copy_quality_gate_into_metrics(
    quality_gate_plan: &Map<String, Value>,
    quality_metrics: &mut Map<String, Value>,
) {
    if let Some(quality_gate) = quality_gate_plan
        .get("quality_gate")
        .and_then(Value::as_object)
    {
        quality_metrics.insert(
            "quality_gate".to_string(),
            Value::Object(quality_gate.clone()),
        );
    }
}

fn object_from_value(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn trimmed_string(value: Option<&Value>) -> String {
    safe_text_from_value(value).unwrap_or_default()
}

fn safe_text_from_value(value: Option<&Value>) -> Option<String> {
    let text = match value {
        Some(Value::String(value)) => value.trim().to_string(),
        Some(Value::Null) | None => String::new(),
        Some(value) => value.to_string().trim_matches('"').trim().to_string(),
    };
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn value_to_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_i64).or_else(|| {
        value
            .and_then(Value::as_str)
            .and_then(|raw| raw.trim().parse::<i64>().ok())
    })
}

fn insert_i64(map: &mut Map<String, Value>, key: &str, value: i64) {
    map.insert(key.to_string(), json!(value));
}

pub(crate) fn build_chapter_candidate_finalize_owner_contract() -> Value {
    json!({
        "owner": "chapter_candidate_finalize_service",
        "scope": "candidate_final_selection_quality_gate_pool_summary_and_runtime_sync_owner",
        "python_source_map": [
            "backend/app/services/chapter_candidate_finalize_service.py",
            "backend/app/services/chapter_candidate_executor_service.py",
            "backend/app/services/chapter_candidate_executor_wiring_service.py",
            "backend/app/services/chapter_candidate_view_service.py",
            "backend/app/services/chapter_candidate_classification_service.py"
        ],
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
            "chapter_batch_generation_active_gateway_smoke_service",
            "chapter_single_generation_active_gateway_smoke_service"
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
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_chapter_candidate_finalize_owner_ready_for_source_map_closeout_review"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_finalize_python_source_map",
            "python_fallback_removal_ready": false,
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

    #[test]
    fn should_publish_chapter_candidate_finalize_owner_contract() {
        let contract = build_chapter_candidate_finalize_owner_contract();

        assert_eq!(contract["owner"], "chapter_candidate_finalize_service");
        assert_eq!(
            contract["scope"],
            "candidate_final_selection_quality_gate_pool_summary_and_runtime_sync_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/services/chapter_candidate_finalize_service.py"
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
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_finalize_owner_ready_for_source_map_closeout_review"
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
