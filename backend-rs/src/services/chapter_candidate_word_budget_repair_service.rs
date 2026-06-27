// Rust owner for word-budget repair originally mapped from Python
// chapter_candidate_word_budget_repair_service.py. The default executor
// dependency owner now calls this workflow through real Rust rerank formulas.

use std::future::Future;

use serde_json::{Map, Value};

use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;
use crate::services::chapter_candidate_rerank_service::{
    build_word_budget_repair_suffix, resolve_word_budget_repair_char_limit,
    resolve_word_budget_repair_max_tokens, resolve_word_budget_repair_temperature,
    select_best_generation_candidate, should_apply_word_budget_repair,
    should_keep_word_budget_repair_candidate, should_prefer_word_budget_repair_candidate,
    should_relax_word_budget_repair_limits,
};
use crate::services::chapter_candidate_runtime_state_service::{
    resolve_generation_attempt_labels, sync_chapter_candidate_runtime_state,
    ChapterCandidateRuntimeStatePatch,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateWordBudgetRepairRequest {
    pub(crate) base_generate_kwargs: Map<String, Value>,
    pub(crate) base_prompt: String,
    pub(crate) base_temperature: f64,
    pub(crate) target_word_count: i64,
    pub(crate) source: String,
    pub(crate) generation_label: String,
    pub(crate) runtime_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateWordBudgetRepairOutputCollectInput {
    pub(crate) generate_kwargs: Map<String, Value>,
    pub(crate) candidate_index: i64,
    pub(crate) max_output_chars: Option<i64>,
    pub(crate) runtime_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateWordBudgetRepairRecordBuildInput {
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

pub(crate) struct ChapterCandidateWordBudgetRepairDependencies<
    ShouldApply,
    BuildSuffix,
    ShouldRelax,
    ResolveTemp,
    ResolveMaxTokens,
    CollectOutput,
    ResolveCharLimit,
    BuildRecord,
    ShouldKeep,
    SelectBest,
    ShouldPrefer,
> {
    pub(crate) should_apply_word_budget_repair_fn: ShouldApply,
    pub(crate) build_word_budget_repair_suffix_fn: BuildSuffix,
    pub(crate) should_relax_word_budget_repair_limits_fn: ShouldRelax,
    pub(crate) resolve_word_budget_repair_temperature_fn: ResolveTemp,
    pub(crate) resolve_word_budget_repair_max_tokens_fn: ResolveMaxTokens,
    pub(crate) collect_generation_candidate_output_fn: CollectOutput,
    pub(crate) resolve_word_budget_repair_char_limit_fn: ResolveCharLimit,
    pub(crate) build_generation_candidate_record_fn: BuildRecord,
    pub(crate) should_keep_word_budget_repair_candidate_fn: ShouldKeep,
    pub(crate) select_best_generation_candidate_fn: SelectBest,
    pub(crate) should_prefer_word_budget_repair_candidate_fn: ShouldPrefer,
}

pub(crate) fn build_default_word_budget_repair_dependencies<
    CollectOutput,
    CollectFuture,
    BuildRecord,
>(
    collect_generation_candidate_output_fn: CollectOutput,
    build_generation_candidate_record_fn: BuildRecord,
) -> ChapterCandidateWordBudgetRepairDependencies<
    impl FnMut(Value) -> bool,
    impl FnMut(ChapterCandidateWordBudgetRepairSuffixInput) -> Option<String>,
    impl FnMut(Option<Value>) -> bool,
    impl FnMut(f64, Option<Value>) -> f64,
    impl FnMut(i64, i64, bool) -> i64,
    CollectOutput,
    impl FnMut(i64, bool) -> Option<i64>,
    BuildRecord,
    impl FnMut(Value, Value) -> bool,
    impl FnMut(Vec<Value>) -> Option<Value>,
    impl FnMut(Value, Value) -> bool,
>
where
    CollectOutput: FnMut(ChapterCandidateWordBudgetRepairOutputCollectInput) -> CollectFuture,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>>,
    BuildRecord: FnMut(ChapterCandidateWordBudgetRepairRecordBuildInput) -> Result<Value, String>,
{
    ChapterCandidateWordBudgetRepairDependencies {
        should_apply_word_budget_repair_fn: should_apply_word_budget_repair,
        build_word_budget_repair_suffix_fn: |input: ChapterCandidateWordBudgetRepairSuffixInput| {
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
        collect_generation_candidate_output_fn,
        resolve_word_budget_repair_char_limit_fn: resolve_word_budget_repair_char_limit,
        build_generation_candidate_record_fn,
        should_keep_word_budget_repair_candidate_fn: should_keep_word_budget_repair_candidate,
        select_best_generation_candidate_fn: select_best_generation_candidate,
        should_prefer_word_budget_repair_candidate_fn: should_prefer_word_budget_repair_candidate,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateWordBudgetRepairResult {
    pub(crate) selected_candidate: Value,
    pub(crate) candidates: Vec<Value>,
    pub(crate) word_budget_repair_used: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct WordBudgetRepairSeedCandidateView {
    candidate_index: i64,
    generation_path: Option<String>,
    attempt_kind: Option<String>,
    quality_metrics: Option<Value>,
    quality_gate_plan: Option<Value>,
    word_count: i64,
    full_content: Option<String>,
}

pub(crate) async fn maybe_apply_word_budget_repair_workflow<
    ShouldApply,
    BuildSuffix,
    ShouldRelax,
    ResolveTemp,
    ResolveMaxTokens,
    CollectOutput,
    CollectFuture,
    ResolveCharLimit,
    BuildRecord,
    ShouldKeep,
    SelectBest,
    ShouldPrefer,
>(
    request: &mut ChapterCandidateWordBudgetRepairRequest,
    mut selected_candidate: Value,
    mut candidates: Vec<Value>,
    dependencies: &mut ChapterCandidateWordBudgetRepairDependencies<
        ShouldApply,
        BuildSuffix,
        ShouldRelax,
        ResolveTemp,
        ResolveMaxTokens,
        CollectOutput,
        ResolveCharLimit,
        BuildRecord,
        ShouldKeep,
        SelectBest,
        ShouldPrefer,
    >,
) -> ChapterCandidateWordBudgetRepairResult
where
    ShouldApply: FnMut(Value) -> bool,
    BuildSuffix: FnMut(ChapterCandidateWordBudgetRepairSuffixInput) -> Option<String>,
    ShouldRelax: FnMut(Option<Value>) -> bool,
    ResolveTemp: FnMut(f64, Option<Value>) -> f64,
    ResolveMaxTokens: FnMut(i64, i64, bool) -> i64,
    CollectOutput: FnMut(ChapterCandidateWordBudgetRepairOutputCollectInput) -> CollectFuture,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>>,
    ResolveCharLimit: FnMut(i64, bool) -> Option<i64>,
    BuildRecord: FnMut(ChapterCandidateWordBudgetRepairRecordBuildInput) -> Result<Value, String>,
    ShouldKeep: FnMut(Value, Value) -> bool,
    SelectBest: FnMut(Vec<Value>) -> Option<Value>,
    ShouldPrefer: FnMut(Value, Value) -> bool,
{
    if !(dependencies.should_apply_word_budget_repair_fn)(selected_candidate.clone()) {
        return ChapterCandidateWordBudgetRepairResult {
            selected_candidate,
            candidates,
            word_budget_repair_used: false,
        };
    }

    let original_selected_candidate = selected_candidate.clone();
    let repair_result = try_build_word_budget_repair_candidate(
        request,
        &selected_candidate,
        candidates.len() as i64 + 1,
        dependencies,
    )
    .await;

    let Ok(mut repair_candidate) = repair_result else {
        return ChapterCandidateWordBudgetRepairResult {
            selected_candidate,
            candidates,
            word_budget_repair_used: false,
        };
    };

    attach_repair_seed_candidate_metadata(&mut repair_candidate, &original_selected_candidate);

    if !(dependencies.should_keep_word_budget_repair_candidate_fn)(
        selected_candidate.clone(),
        repair_candidate.clone(),
    ) {
        return ChapterCandidateWordBudgetRepairResult {
            selected_candidate,
            candidates,
            word_budget_repair_used: false,
        };
    }

    candidates.push(repair_candidate.clone());
    let reranked_candidate = (dependencies.select_best_generation_candidate_fn)(candidates.clone())
        .unwrap_or_else(|| repair_candidate.clone());
    selected_candidate = if (dependencies.should_prefer_word_budget_repair_candidate_fn)(
        reranked_candidate.clone(),
        repair_candidate.clone(),
    ) {
        repair_candidate
    } else {
        reranked_candidate
    };

    ChapterCandidateWordBudgetRepairResult {
        selected_candidate,
        candidates,
        word_budget_repair_used: true,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateWordBudgetRepairSuffixInput {
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) quality_gate_plan: Option<Value>,
    pub(crate) current_content: Option<String>,
    pub(crate) target_word_count: i64,
    pub(crate) attempt_index: i64,
    pub(crate) source: String,
}

async fn try_build_word_budget_repair_candidate<
    ShouldApply,
    BuildSuffix,
    ShouldRelax,
    ResolveTemp,
    ResolveMaxTokens,
    CollectOutput,
    CollectFuture,
    ResolveCharLimit,
    BuildRecord,
    ShouldKeep,
    SelectBest,
    ShouldPrefer,
>(
    request: &mut ChapterCandidateWordBudgetRepairRequest,
    selected_candidate: &Value,
    repair_attempt_index: i64,
    dependencies: &mut ChapterCandidateWordBudgetRepairDependencies<
        ShouldApply,
        BuildSuffix,
        ShouldRelax,
        ResolveTemp,
        ResolveMaxTokens,
        CollectOutput,
        ResolveCharLimit,
        BuildRecord,
        ShouldKeep,
        SelectBest,
        ShouldPrefer,
    >,
) -> Result<Value, String>
where
    ShouldApply: FnMut(Value) -> bool,
    BuildSuffix: FnMut(ChapterCandidateWordBudgetRepairSuffixInput) -> Option<String>,
    ShouldRelax: FnMut(Option<Value>) -> bool,
    ResolveTemp: FnMut(f64, Option<Value>) -> f64,
    ResolveMaxTokens: FnMut(i64, i64, bool) -> i64,
    CollectOutput: FnMut(ChapterCandidateWordBudgetRepairOutputCollectInput) -> CollectFuture,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>>,
    ResolveCharLimit: FnMut(i64, bool) -> Option<i64>,
    BuildRecord: FnMut(ChapterCandidateWordBudgetRepairRecordBuildInput) -> Result<Value, String>,
    ShouldKeep: FnMut(Value, Value) -> bool,
    SelectBest: FnMut(Vec<Value>) -> Option<Value>,
    ShouldPrefer: FnMut(Value, Value) -> bool,
{
    let selected_candidate_view = word_budget_repair_seed_candidate_view(selected_candidate);
    let repair_suffix = (dependencies.build_word_budget_repair_suffix_fn)(
        ChapterCandidateWordBudgetRepairSuffixInput {
            quality_metrics: selected_candidate_view.quality_metrics.clone(),
            quality_gate_plan: selected_candidate_view.quality_gate_plan.clone(),
            current_content: selected_candidate_view.full_content.clone(),
            target_word_count: request.target_word_count,
            attempt_index: repair_attempt_index,
            source: request.source.clone(),
        },
    )
    .unwrap_or_default()
    .trim()
    .to_string();
    if repair_suffix.is_empty() {
        return Err("word-budget repair suffix is empty".to_string());
    }

    let source_word_count = selected_candidate_view.word_count;
    let relax_content_budget = (dependencies.should_relax_word_budget_repair_limits_fn)(
        selected_candidate_view.quality_gate_plan.clone(),
    );

    let mut repair_generate_kwargs = request.base_generate_kwargs.clone();
    repair_generate_kwargs.insert(
        "prompt".to_string(),
        Value::String(build_repair_prompt(
            &request.base_prompt,
            &repair_suffix,
            selected_candidate_view.full_content.as_deref(),
        )),
    );
    insert_f64(
        &mut repair_generate_kwargs,
        "temperature",
        (dependencies.resolve_word_budget_repair_temperature_fn)(
            request.base_temperature,
            selected_candidate_view.quality_metrics.clone(),
        ),
    );
    repair_generate_kwargs.insert(
        "max_tokens".to_string(),
        Value::Number(
            (dependencies.resolve_word_budget_repair_max_tokens_fn)(
                request.target_word_count,
                source_word_count,
                relax_content_budget,
            )
            .into(),
        ),
    );

    let labels = resolve_generation_attempt_labels(repair_attempt_index, true);
    sync_chapter_candidate_runtime_state(
        request.runtime_state.as_mut(),
        repair_attempt_index,
        repair_attempt_index,
        ChapterCandidateRuntimeStatePatch {
            current_chars: Some(0),
            chunk_count: Some(0),
            generation_path: Some(labels.generation_path.to_string()),
            attempt_kind: Some(labels.attempt_kind.to_string()),
            rerank_used: Some(false),
            word_budget_repair_used: Some(true),
            ..ChapterCandidateRuntimeStatePatch::default()
        },
    );

    let output = (dependencies.collect_generation_candidate_output_fn)(
        ChapterCandidateWordBudgetRepairOutputCollectInput {
            generate_kwargs: repair_generate_kwargs,
            candidate_index: repair_attempt_index,
            max_output_chars: (dependencies.resolve_word_budget_repair_char_limit_fn)(
                request.target_word_count,
                relax_content_budget,
            ),
            runtime_state: request.runtime_state.clone(),
        },
    )
    .await?;
    request.runtime_state = output
        .runtime_state
        .clone()
        .or_else(|| request.runtime_state.take());

    (dependencies.build_generation_candidate_record_fn)(
        ChapterCandidateWordBudgetRepairRecordBuildInput {
            full_content: output.full_content,
            candidate_chunks: output.chunks,
            target_word_count: request.target_word_count,
            source: request.source.clone(),
            generation_label: format!("{}-budget-repair", request.generation_label),
            candidate_index: repair_attempt_index,
            candidate_offset: repair_attempt_index - 1,
            generation_path: labels.generation_path.to_string(),
            attempt_kind: labels.attempt_kind.to_string(),
        },
    )
}

fn build_repair_prompt(
    base_prompt: &str,
    repair_suffix: &str,
    current_content: Option<&str>,
) -> String {
    [
        base_prompt,
        repair_suffix,
        "Previous draft to rewrite:\n<<<CHAPTER_DRAFT",
        current_content.unwrap_or_default(),
        "CHAPTER_DRAFT>>>",
    ]
    .into_iter()
    .map(str::trim)
    .filter(|section| !section.is_empty())
    .collect::<Vec<_>>()
    .join("\n\n")
}

fn attach_repair_seed_candidate_metadata(
    repair_candidate: &mut Value,
    repair_seed_candidate: &Value,
) {
    let repair_seed_candidate_view = word_budget_repair_seed_candidate_view(repair_seed_candidate);
    let Some(repair_candidate_map) = repair_candidate.as_object_mut() else {
        return;
    };
    let mut quality_metrics = repair_candidate_map
        .get("quality_metrics")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut candidate_selection = quality_metrics
        .get("candidate_selection")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let seed_index = repair_seed_candidate_view.candidate_index.max(1);
    candidate_selection.insert("repair_seed_candidate_index".to_string(), seed_index.into());
    if let Some(generation_path) = repair_seed_candidate_view.generation_path {
        candidate_selection.insert(
            "repair_seed_generation_path".to_string(),
            Value::String(generation_path),
        );
    }
    if let Some(attempt_kind) = repair_seed_candidate_view.attempt_kind {
        candidate_selection.insert(
            "repair_seed_attempt_kind".to_string(),
            Value::String(attempt_kind),
        );
    }

    quality_metrics.insert(
        "candidate_selection".to_string(),
        Value::Object(candidate_selection),
    );
    repair_candidate_map.insert(
        "quality_metrics".to_string(),
        Value::Object(quality_metrics),
    );
}

fn word_budget_repair_seed_candidate_view(
    repair_seed_candidate: &Value,
) -> WordBudgetRepairSeedCandidateView {
    WordBudgetRepairSeedCandidateView {
        candidate_index: repair_seed_candidate
            .get("candidate_index")
            .and_then(value_to_i64)
            .unwrap_or(1),
        generation_path: trimmed_string_field(repair_seed_candidate, "generation_path"),
        attempt_kind: trimmed_string_field(repair_seed_candidate, "attempt_kind"),
        quality_metrics: repair_seed_candidate.get("quality_metrics").cloned(),
        quality_gate_plan: repair_seed_candidate.get("quality_gate_plan").cloned(),
        word_count: repair_seed_candidate
            .get("word_count")
            .and_then(value_to_i64)
            .unwrap_or(0),
        full_content: repair_seed_candidate
            .get("full_content")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
}

fn trimmed_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

fn insert_f64(map: &mut Map<String, Value>, key: &str, value: f64) {
    if let Some(number) = serde_json::Number::from_f64(value) {
        map.insert(key.to_string(), Value::Number(number));
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| value.as_f64().map(|number| number as i64))
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
}

pub(crate) fn build_chapter_candidate_word_budget_repair_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_candidate_word_budget_repair_service",
        "scope": "candidate_word_budget_repair_prompt_runtime_record_and_selection_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_rerank_service.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/services/chapter_candidate_output_service.rs",
            "backend-rs/src/services/chapter_candidate_record_service.rs",
            "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_default_word_budget_repair_dependencies",
                "maybe_apply_word_budget_repair_workflow"
            ],
            "request_fields": [
                "base_generate_kwargs",
                "base_prompt",
                "base_temperature",
                "target_word_count",
                "source",
                "generation_label",
                "runtime_state"
            ],
            "repair_policy": [
                "workflow returns original selected candidate when should_apply is false",
                "repair suffix absence skips provider collection and keeps the original candidate",
                "provider or record-build failure keeps the original candidate",
                "repair candidate is appended only when keep policy accepts it",
                "final selected candidate follows prefer policy after rerank owner selection"
            ],
            "prompt_and_limits_policy": [
                "repair prompt preserves base prompt, repair suffix, and previous draft",
                "temperature derives from base temperature and selected quality metrics",
                "max_tokens derives from target, current word count, and relaxed-limit policy",
                "max_output_chars derives from target and relaxed-limit policy"
            ],
            "runtime_state_policy": [
                "runtime state sync records repair attempt index, total candidates, generation_path, attempt_kind, and word_budget_repair_used",
                "missing runtime_state is a no-op rather than an error"
            ],
            "record_policy": [
                "record builder receives repaired full content, chunks, target_word_count, source, generation label suffix, candidate index, offset, generation_path, and attempt_kind",
                "repair seed metadata records candidate index, generation_path, and attempt_kind when available"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_word_budget_repair_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_executor_service",
            "chapter_candidate_executor_default_dependency_service",
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
            "default_dependencies_owner": "build_default_word_budget_repair_dependencies",
            "workflow_owner": "maybe_apply_word_budget_repair_workflow",
            "repair_prompt_owner": "build_repair_prompt",
            "runtime_state_sync_owner": "sync_chapter_candidate_runtime_state",
            "output_collection_owner": "collect_generation_candidate_output_fn",
            "record_build_owner": "build_generation_candidate_record_fn",
            "rerank_formula_owner": "chapter_candidate_rerank_service",
            "candidate_record_owner": "chapter_candidate_record_service",
            "candidate_output_owner": "chapter_candidate_output_service",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "candidate word-budget-repair production python source-map deleted; this owner is now Rust-only on the active path",
            "status": "rust_chapter_candidate_word_budget_repair_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_word_budget_repair_python_source_map",
            "python_fallback_removal_ready": true,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_word_budget_repair_owner_contract,
        build_default_word_budget_repair_dependencies, maybe_apply_word_budget_repair_workflow,
        word_budget_repair_seed_candidate_view, ChapterCandidateWordBudgetRepairDependencies,
        ChapterCandidateWordBudgetRepairOutputCollectInput,
        ChapterCandidateWordBudgetRepairRecordBuildInput, ChapterCandidateWordBudgetRepairRequest,
        ChapterCandidateWordBudgetRepairSuffixInput,
    };
    use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;

    fn base_request() -> ChapterCandidateWordBudgetRepairRequest {
        let mut base_generate_kwargs = Map::new();
        base_generate_kwargs.insert("prompt".to_string(), Value::String("base".to_string()));
        ChapterCandidateWordBudgetRepairRequest {
            base_generate_kwargs,
            base_prompt: "Base prompt".to_string(),
            base_temperature: 0.8,
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "candidate".to_string(),
            runtime_state: Some(json!({})),
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
    fn should_publish_chapter_candidate_word_budget_repair_owner_contract() {
        let contract = build_chapter_candidate_word_budget_repair_owner_contract();
        assert_no_deleted_python_service_source_map(&contract);

        assert_eq!(
            contract["owner"],
            "chapter_candidate_word_budget_repair_service"
        );
        assert_eq!(
            contract["scope"],
            "candidate_word_budget_repair_prompt_runtime_record_and_selection_owner"
        );
        assert!(contract["rust_owner_map"]
            .as_array()
            .expect("rust owner map")
            .contains(&json!(
                "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs"
            )));
        assert!(contract["behavior_contract"]["entrypoints"]
            .as_array()
            .expect("entrypoints")
            .contains(&json!("maybe_apply_word_budget_repair_workflow")));
        assert!(contract["behavior_contract"]["repair_policy"]
            .as_array()
            .expect("repair policy")
            .iter()
            .any(|policy| policy.as_str().unwrap_or_default().contains("should_apply")));
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
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            json!(11)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["workflow_owner"],
            "maybe_apply_word_budget_repair_workflow"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["runtime_state_sync_owner"],
            "sync_chapter_candidate_runtime_state"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["record_build_owner"],
            "build_generation_candidate_record_fn"
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
            "candidate word-budget-repair production python source-map deleted; this owner is now Rust-only on the active path"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_word_budget_repair_owner_source_map_deleted"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
    }

    #[test]
    fn should_build_word_budget_repair_seed_candidate_view_from_selected_candidate() {
        let selected_candidate = json!({
            "candidate_index": 1,
            "word_count": 1800,
            "full_content": "Previous draft",
            "generation_path": "single_pass",
            "attempt_kind": "initial_candidate",
            "quality_metrics": {"overall_score": 80},
            "quality_gate_plan": {"quality_gate": {"decision": "auto_repair"}}
        });

        let view = word_budget_repair_seed_candidate_view(&selected_candidate);

        assert_eq!(view.candidate_index, 1);
        assert_eq!(view.word_count, 1800);
        assert_eq!(view.full_content.as_deref(), Some("Previous draft"));
        assert_eq!(view.generation_path.as_deref(), Some("single_pass"));
        assert_eq!(view.attempt_kind.as_deref(), Some("initial_candidate"));
        assert_eq!(view.quality_metrics.as_ref().unwrap()["overall_score"], 80);
        assert_eq!(
            view.quality_gate_plan.as_ref().unwrap()["quality_gate"]["decision"],
            "auto_repair"
        );
    }

    #[tokio::test]
    async fn should_skip_word_budget_repair_when_not_needed() {
        let mut request = base_request();
        let selected_candidate = json!({"candidate_index": 1, "word_count": 1180});
        let candidates = vec![selected_candidate.clone()];
        let mut dependencies = dependencies(
            false,
            |_input| Some("repair".to_string()),
            |_input| -> Result<ChapterCandidateOutput, String> {
                panic!("collector should not run when repair is skipped");
            },
        );

        let result = maybe_apply_word_budget_repair_workflow(
            &mut request,
            selected_candidate.clone(),
            candidates.clone(),
            &mut dependencies,
        )
        .await;

        assert_eq!(result.selected_candidate, selected_candidate);
        assert_eq!(result.candidates, candidates);
        assert!(!result.word_budget_repair_used);
    }

    #[tokio::test]
    async fn should_build_word_budget_repair_candidate_and_prompt() {
        let mut request = base_request();
        let selected_candidate = json!({
            "candidate_index": 1,
            "word_count": 1800,
            "full_content": "Previous draft",
            "generation_path": "single_pass",
            "attempt_kind": "initial_candidate",
            "quality_metrics": {"overall_score": 80},
            "quality_gate_plan": {"quality_gate": {"decision": "auto_repair"}}
        });
        let candidates = vec![selected_candidate.clone()];
        let mut dependencies = dependencies(
            true,
            |input: ChapterCandidateWordBudgetRepairSuffixInput| {
                assert_eq!(input.attempt_index, 2);
                assert_eq!(input.target_word_count, 1200);
                Some("Compress to target.".to_string())
            },
            |input: ChapterCandidateWordBudgetRepairOutputCollectInput| {
                assert_eq!(input.candidate_index, 2);
                assert_eq!(input.max_output_chars, Some(1800));
                assert!(input
                    .generate_kwargs
                    .get("prompt")
                    .and_then(Value::as_str)
                    .is_some_and(|prompt| prompt.contains("Previous draft to rewrite")));
                assert_eq!(input.generate_kwargs["temperature"], json!(0.55));
                assert_eq!(input.generate_kwargs["max_tokens"], json!(1600));
                Ok(ChapterCandidateOutput {
                    full_content: "Repaired content".to_string(),
                    chunks: vec!["Repaired".to_string(), " content".to_string()],
                    runtime_state: input.runtime_state.clone(),
                })
            },
        );

        let result = maybe_apply_word_budget_repair_workflow(
            &mut request,
            selected_candidate,
            candidates,
            &mut dependencies,
        )
        .await;

        assert!(result.word_budget_repair_used);
        assert_eq!(result.candidates.len(), 2);
        assert_eq!(result.selected_candidate["candidate_index"], 2);
        assert_eq!(
            result.selected_candidate["generation_path"],
            "word_budget_repair"
        );
        assert_eq!(
            result.selected_candidate["quality_metrics"]["candidate_selection"]
                ["repair_seed_candidate_index"],
            1
        );
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["generation_path"],
            "word_budget_repair"
        );
    }

    #[tokio::test]
    async fn should_keep_original_candidate_when_repair_collection_fails() {
        let mut request = base_request();
        let selected_candidate = json!({"candidate_index": 1, "word_count": 1800});
        let candidates = vec![selected_candidate.clone()];
        let mut dependencies = dependencies(
            true,
            |_input| Some("repair".to_string()),
            |_input| -> Result<ChapterCandidateOutput, String> {
                Err("provider failed".to_string())
            },
        );

        let result = maybe_apply_word_budget_repair_workflow(
            &mut request,
            selected_candidate.clone(),
            candidates.clone(),
            &mut dependencies,
        )
        .await;

        assert_eq!(result.selected_candidate, selected_candidate);
        assert_eq!(result.candidates, candidates);
        assert!(!result.word_budget_repair_used);
    }

    #[tokio::test]
    async fn should_build_default_word_budget_repair_dependencies_from_owner() {
        let mut request = base_request();
        let selected_candidate = json!({
            "candidate_index": 1,
            "target_word_count": 1200,
            "word_count": 1900,
            "full_content": "Previous oversized draft",
            "generation_path": "single_pass",
            "attempt_kind": "initial_candidate",
            "overall_score": 82.0,
            "selection_score": 80.0,
            "word_count_fit_score": 30.0,
            "quality_gate_decision": "auto_repair",
            "quality_gate_priority": 2,
            "quality_metrics": {
                "overall_score": 82.0,
                "candidate_selection": {"word_count": 1900},
                "quality_gate": {
                    "decision": "auto_repair",
                    "failed_metrics": [
                        {"label": "too long", "focus_area": "word_budget"}
                    ]
                }
            },
            "quality_gate_plan": {
                "quality_gate": {
                    "decision": "auto_repair",
                    "failed_metrics": [
                        {"label": "too long", "focus_area": "word_budget"}
                    ]
                },
                "active_story_repair_payload": {
                    "summary": "word budget pressure",
                    "repair_targets": ["compress middle"],
                    "focus_areas": ["word_budget"]
                }
            }
        });
        let built_records = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured_records = Arc::clone(&built_records);
        let mut dependencies = build_default_word_budget_repair_dependencies(
            |input: ChapterCandidateWordBudgetRepairOutputCollectInput| {
                assert_eq!(input.candidate_index, 2);
                assert!(input
                    .generate_kwargs
                    .get("prompt")
                    .and_then(Value::as_str)
                    .is_some_and(|prompt| {
                        prompt.contains("Previous draft to rewrite")
                            && prompt.contains("Previous oversized draft")
                    }));
                assert!(input
                    .generate_kwargs
                    .get("temperature")
                    .and_then(Value::as_f64)
                    .is_some_and(|temperature| temperature > 0.0));
                std::future::ready(Ok(ChapterCandidateOutput {
                    full_content: "Default word-budget repair".to_string(),
                    chunks: vec!["Default word-budget repair".to_string()],
                    runtime_state: input.runtime_state.clone(),
                }))
            },
            move |input: ChapterCandidateWordBudgetRepairRecordBuildInput| {
                let record = json!({
                    "full_content": input.full_content,
                    "candidate_chunks": input.candidate_chunks,
                    "target_word_count": input.target_word_count,
                    "source": input.source,
                    "generation_label": input.generation_label,
                    "candidate_index": input.candidate_index,
                    "candidate_offset": input.candidate_offset,
                    "generation_path": input.generation_path,
                    "attempt_kind": input.attempt_kind,
                    "word_count": 1210,
                    "overall_score": 90.0,
                    "selection_score": 96.0,
                    "word_count_fit_score": 99.0,
                    "quality_gate_decision": "allow_save",
                    "quality_gate_priority": 3,
                    "quality_metrics": {"overall_score": 90.0},
                    "quality_gate_plan": {"quality_gate": {"decision": "allow_save"}}
                });
                captured_records.lock().unwrap().push(record.clone());
                Ok(record)
            },
        );

        let result = maybe_apply_word_budget_repair_workflow(
            &mut request,
            selected_candidate,
            vec![json!({"candidate_index": 1})],
            &mut dependencies,
        )
        .await;

        assert!(result.word_budget_repair_used);
        assert_eq!(built_records.lock().unwrap().len(), 1);
        assert_eq!(result.selected_candidate["candidate_index"], 2);
        assert_eq!(
            result.selected_candidate["quality_metrics"]["candidate_selection"]
                ["repair_seed_candidate_index"],
            1
        );
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["generation_path"],
            "word_budget_repair"
        );
    }

    fn dependencies<SuffixFn, CollectFn, CollectResult>(
        should_apply: bool,
        build_suffix: SuffixFn,
        collect_output: CollectFn,
    ) -> ChapterCandidateWordBudgetRepairDependencies<
        impl FnMut(Value) -> bool,
        SuffixFn,
        impl FnMut(Option<Value>) -> bool,
        impl FnMut(f64, Option<Value>) -> f64,
        impl FnMut(i64, i64, bool) -> i64,
        impl FnMut(
            ChapterCandidateWordBudgetRepairOutputCollectInput,
        ) -> std::future::Ready<Result<ChapterCandidateOutput, String>>,
        impl FnMut(i64, bool) -> Option<i64>,
        impl FnMut(ChapterCandidateWordBudgetRepairRecordBuildInput) -> Result<Value, String>,
        impl FnMut(Value, Value) -> bool,
        impl FnMut(Vec<Value>) -> Option<Value>,
        impl FnMut(Value, Value) -> bool,
    >
    where
        SuffixFn: FnMut(ChapterCandidateWordBudgetRepairSuffixInput) -> Option<String>,
        CollectFn: FnMut(ChapterCandidateWordBudgetRepairOutputCollectInput) -> CollectResult,
        CollectResult: Into<Result<ChapterCandidateOutput, String>>,
    {
        let mut collect_output = collect_output;
        ChapterCandidateWordBudgetRepairDependencies {
            should_apply_word_budget_repair_fn: move |_candidate| should_apply,
            build_word_budget_repair_suffix_fn: build_suffix,
            should_relax_word_budget_repair_limits_fn: |_plan| false,
            resolve_word_budget_repair_temperature_fn: |_base, _metrics| 0.55,
            resolve_word_budget_repair_max_tokens_fn: |_target, _current, _relax| 1600,
            collect_generation_candidate_output_fn: move |input| {
                std::future::ready(collect_output(input).into())
            },
            resolve_word_budget_repair_char_limit_fn: |_target, _relax| Some(1800),
            build_generation_candidate_record_fn:
                |input: ChapterCandidateWordBudgetRepairRecordBuildInput| {
                    Ok(json!({
                        "full_content": input.full_content,
                        "candidate_chunks": input.candidate_chunks,
                        "target_word_count": input.target_word_count,
                        "source": input.source,
                        "generation_label": input.generation_label,
                        "candidate_index": input.candidate_index,
                        "candidate_offset": input.candidate_offset,
                        "generation_path": input.generation_path,
                        "attempt_kind": input.attempt_kind,
                        "word_count": 1190,
                        "quality_metrics": {"overall_score": 86},
                        "quality_gate_plan": {"quality_gate": {"decision": "allow_save"}}
                    }))
                },
            should_keep_word_budget_repair_candidate_fn: |_selected, _repair| true,
            select_best_generation_candidate_fn: |candidates: Vec<Value>| {
                candidates.last().cloned()
            },
            should_prefer_word_budget_repair_candidate_fn: |_reranked, _repair| true,
        }
    }
}
