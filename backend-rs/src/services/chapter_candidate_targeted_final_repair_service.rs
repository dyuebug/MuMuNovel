// Rust owner for targeted final repair originally mapped from Python
// chapter_candidate_targeted_final_repair_service.py. The default executor
// dependency owner now calls this workflow through real Rust rerank formulas.

use std::future::Future;

use serde_json::{Map, Value};

use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;
use crate::services::chapter_candidate_rerank_service::{
    build_targeted_final_repair_suffix, resolve_targeted_final_repair_char_limit,
    resolve_targeted_final_repair_max_tokens, resolve_targeted_final_repair_temperature,
    should_adopt_targeted_final_repair_candidate, should_apply_followup_targeted_final_repair,
    should_keep_targeted_final_repair_candidate, should_prefer_targeted_final_repair_candidate,
};
use crate::services::chapter_candidate_runtime_state_service::{
    sync_chapter_candidate_runtime_state, ChapterCandidateRuntimeStatePatch,
};

const TARGETED_QUALITY_REPAIR: &str = "targeted_quality_repair";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateTargetedFinalRepairRequest {
    pub(crate) base_generate_kwargs: Map<String, Value>,
    pub(crate) base_prompt: String,
    pub(crate) base_temperature: f64,
    pub(crate) target_word_count: i64,
    pub(crate) source: String,
    pub(crate) generation_label: String,
    pub(crate) generation_label_suffix: String,
    pub(crate) repair_seed_candidate: Value,
    pub(crate) current_winner_candidate: Value,
    pub(crate) runtime_state: Option<Value>,
    pub(crate) allow_followup_seed_defer: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateTargetedFinalRepairOutputCollectInput {
    pub(crate) generate_kwargs: Map<String, Value>,
    pub(crate) candidate_index: i64,
    pub(crate) max_output_chars: Option<i64>,
    pub(crate) runtime_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateTargetedFinalRepairRecordBuildInput {
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateTargetedFinalRepairSuffixInput {
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) quality_gate_plan: Option<Value>,
    pub(crate) target_word_count: i64,
    pub(crate) attempt_index: i64,
    pub(crate) source: String,
}

pub(crate) struct ChapterCandidateTargetedFinalRepairDependencies<
    BuildSuffix,
    ResolveTemp,
    ResolveMaxTokens,
    CollectOutput,
    ResolveCharLimit,
    BuildRecord,
    ShouldKeep,
    ShouldAdopt,
    ShouldPrefer,
    ShouldFollowup,
> {
    pub(crate) build_targeted_final_repair_suffix_fn: BuildSuffix,
    pub(crate) resolve_targeted_final_repair_temperature_fn: ResolveTemp,
    pub(crate) resolve_targeted_final_repair_max_tokens_fn: ResolveMaxTokens,
    pub(crate) collect_generation_candidate_output_fn: CollectOutput,
    pub(crate) resolve_targeted_final_repair_char_limit_fn: ResolveCharLimit,
    pub(crate) build_generation_candidate_record_fn: BuildRecord,
    pub(crate) should_keep_targeted_final_repair_candidate_fn: ShouldKeep,
    pub(crate) should_adopt_targeted_final_repair_candidate_fn: ShouldAdopt,
    pub(crate) should_prefer_targeted_final_repair_candidate_fn: ShouldPrefer,
    pub(crate) should_apply_followup_targeted_final_repair_fn: ShouldFollowup,
}

pub(crate) fn build_default_targeted_final_repair_dependencies<
    CollectOutput,
    CollectFuture,
    BuildRecord,
>(
    collect_generation_candidate_output_fn: CollectOutput,
    build_generation_candidate_record_fn: BuildRecord,
) -> ChapterCandidateTargetedFinalRepairDependencies<
    impl FnMut(ChapterCandidateTargetedFinalRepairSuffixInput) -> Option<String>,
    impl FnMut(f64, Option<Value>) -> f64,
    impl FnMut(i64, i64) -> i64,
    CollectOutput,
    impl FnMut(i64) -> Option<i64>,
    BuildRecord,
    impl FnMut(Value, Value) -> bool,
    impl FnMut(Value, Value) -> bool,
    impl FnMut(Value, Value) -> bool,
    impl FnMut(Value) -> bool,
>
where
    CollectOutput: FnMut(ChapterCandidateTargetedFinalRepairOutputCollectInput) -> CollectFuture,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>>,
    BuildRecord:
        FnMut(ChapterCandidateTargetedFinalRepairRecordBuildInput) -> Result<Value, String>,
{
    ChapterCandidateTargetedFinalRepairDependencies {
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
        collect_generation_candidate_output_fn,
        resolve_targeted_final_repair_char_limit_fn: resolve_targeted_final_repair_char_limit,
        build_generation_candidate_record_fn,
        should_keep_targeted_final_repair_candidate_fn: should_keep_targeted_final_repair_candidate,
        should_adopt_targeted_final_repair_candidate_fn:
            should_adopt_targeted_final_repair_candidate,
        should_prefer_targeted_final_repair_candidate_fn:
            should_prefer_targeted_final_repair_candidate,
        should_apply_followup_targeted_final_repair_fn: should_apply_followup_targeted_final_repair,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateTargetedFinalRepairResult {
    pub(crate) selected_candidate: Value,
    pub(crate) candidates: Vec<Value>,
    pub(crate) deferred_followup_targeted_repair_seed_candidate: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
struct TargetedFinalRepairSeedCandidateView {
    candidate_index: i64,
    generation_path: Option<String>,
    attempt_kind: Option<String>,
    quality_metrics: Option<Value>,
    quality_gate_plan: Option<Value>,
    word_count: i64,
    full_content: Option<String>,
}

pub(crate) async fn execute_targeted_final_repair_pass_workflow<
    BuildSuffix,
    ResolveTemp,
    ResolveMaxTokens,
    CollectOutput,
    CollectFuture,
    ResolveCharLimit,
    BuildRecord,
    ShouldKeep,
    ShouldAdopt,
    ShouldPrefer,
    ShouldFollowup,
>(
    request: &mut ChapterCandidateTargetedFinalRepairRequest,
    mut selected_candidate: Value,
    mut candidates: Vec<Value>,
    dependencies: &mut ChapterCandidateTargetedFinalRepairDependencies<
        BuildSuffix,
        ResolveTemp,
        ResolveMaxTokens,
        CollectOutput,
        ResolveCharLimit,
        BuildRecord,
        ShouldKeep,
        ShouldAdopt,
        ShouldPrefer,
        ShouldFollowup,
    >,
) -> ChapterCandidateTargetedFinalRepairResult
where
    BuildSuffix: FnMut(ChapterCandidateTargetedFinalRepairSuffixInput) -> Option<String>,
    ResolveTemp: FnMut(f64, Option<Value>) -> f64,
    ResolveMaxTokens: FnMut(i64, i64) -> i64,
    CollectOutput: FnMut(ChapterCandidateTargetedFinalRepairOutputCollectInput) -> CollectFuture,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>>,
    ResolveCharLimit: FnMut(i64) -> Option<i64>,
    BuildRecord:
        FnMut(ChapterCandidateTargetedFinalRepairRecordBuildInput) -> Result<Value, String>,
    ShouldKeep: FnMut(Value, Value) -> bool,
    ShouldAdopt: FnMut(Value, Value) -> bool,
    ShouldPrefer: FnMut(Value, Value) -> bool,
    ShouldFollowup: FnMut(Value) -> bool,
{
    let original_selected_candidate = selected_candidate.clone();
    let repair_result = try_build_targeted_final_repair_candidate(
        request,
        candidates.len() as i64 + 1,
        dependencies,
    )
    .await;

    let Ok(mut final_repair_candidate) = repair_result else {
        return ChapterCandidateTargetedFinalRepairResult {
            selected_candidate: original_selected_candidate,
            candidates,
            deferred_followup_targeted_repair_seed_candidate: None,
        };
    };

    attach_repair_seed_candidate_metadata(
        &mut final_repair_candidate,
        &request.repair_seed_candidate,
    );

    let mut deferred_followup_targeted_repair_seed_candidate = None;
    if (dependencies.should_keep_targeted_final_repair_candidate_fn)(
        request.repair_seed_candidate.clone(),
        final_repair_candidate.clone(),
    ) {
        candidates.push(final_repair_candidate.clone());
        if (dependencies.should_adopt_targeted_final_repair_candidate_fn)(
            request.repair_seed_candidate.clone(),
            final_repair_candidate.clone(),
        ) && (dependencies.should_prefer_targeted_final_repair_candidate_fn)(
            request.current_winner_candidate.clone(),
            final_repair_candidate.clone(),
        ) {
            selected_candidate = final_repair_candidate;
        } else if request.allow_followup_seed_defer
            && (dependencies.should_apply_followup_targeted_final_repair_fn)(
                final_repair_candidate.clone(),
            )
        {
            deferred_followup_targeted_repair_seed_candidate = Some(final_repair_candidate);
        }
    }

    ChapterCandidateTargetedFinalRepairResult {
        selected_candidate,
        candidates,
        deferred_followup_targeted_repair_seed_candidate,
    }
}

async fn try_build_targeted_final_repair_candidate<
    BuildSuffix,
    ResolveTemp,
    ResolveMaxTokens,
    CollectOutput,
    CollectFuture,
    ResolveCharLimit,
    BuildRecord,
    ShouldKeep,
    ShouldAdopt,
    ShouldPrefer,
    ShouldFollowup,
>(
    request: &mut ChapterCandidateTargetedFinalRepairRequest,
    final_repair_attempt_index: i64,
    dependencies: &mut ChapterCandidateTargetedFinalRepairDependencies<
        BuildSuffix,
        ResolveTemp,
        ResolveMaxTokens,
        CollectOutput,
        ResolveCharLimit,
        BuildRecord,
        ShouldKeep,
        ShouldAdopt,
        ShouldPrefer,
        ShouldFollowup,
    >,
) -> Result<Value, String>
where
    BuildSuffix: FnMut(ChapterCandidateTargetedFinalRepairSuffixInput) -> Option<String>,
    ResolveTemp: FnMut(f64, Option<Value>) -> f64,
    ResolveMaxTokens: FnMut(i64, i64) -> i64,
    CollectOutput: FnMut(ChapterCandidateTargetedFinalRepairOutputCollectInput) -> CollectFuture,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>>,
    ResolveCharLimit: FnMut(i64) -> Option<i64>,
    BuildRecord:
        FnMut(ChapterCandidateTargetedFinalRepairRecordBuildInput) -> Result<Value, String>,
    ShouldKeep: FnMut(Value, Value) -> bool,
    ShouldAdopt: FnMut(Value, Value) -> bool,
    ShouldPrefer: FnMut(Value, Value) -> bool,
    ShouldFollowup: FnMut(Value) -> bool,
{
    let repair_seed_candidate_view =
        targeted_final_repair_seed_candidate_view(&request.repair_seed_candidate);
    let final_repair_suffix = (dependencies.build_targeted_final_repair_suffix_fn)(
        ChapterCandidateTargetedFinalRepairSuffixInput {
            quality_metrics: repair_seed_candidate_view.quality_metrics.clone(),
            quality_gate_plan: repair_seed_candidate_view.quality_gate_plan.clone(),
            target_word_count: request.target_word_count,
            attempt_index: final_repair_attempt_index,
            source: request.source.clone(),
        },
    )
    .unwrap_or_default()
    .trim()
    .to_string();
    if final_repair_suffix.is_empty() {
        return Err("targeted final repair suffix is empty".to_string());
    }

    let seed_word_count = repair_seed_candidate_view.word_count;
    let mut generate_kwargs = request.base_generate_kwargs.clone();
    generate_kwargs.insert(
        "prompt".to_string(),
        Value::String(build_repair_prompt(
            &request.base_prompt,
            &final_repair_suffix,
            repair_seed_candidate_view.full_content.as_deref(),
        )),
    );
    insert_f64(
        &mut generate_kwargs,
        "temperature",
        (dependencies.resolve_targeted_final_repair_temperature_fn)(
            request.base_temperature,
            repair_seed_candidate_view.quality_gate_plan.clone(),
        ),
    );
    generate_kwargs.insert(
        "max_tokens".to_string(),
        Value::Number(
            (dependencies.resolve_targeted_final_repair_max_tokens_fn)(
                request.target_word_count,
                seed_word_count,
            )
            .into(),
        ),
    );

    sync_chapter_candidate_runtime_state(
        request.runtime_state.as_mut(),
        final_repair_attempt_index,
        final_repair_attempt_index,
        ChapterCandidateRuntimeStatePatch {
            current_chars: Some(0),
            chunk_count: Some(0),
            generation_path: Some(TARGETED_QUALITY_REPAIR.to_string()),
            attempt_kind: Some(TARGETED_QUALITY_REPAIR.to_string()),
            rerank_used: Some(false),
            word_budget_repair_used: Some(false),
            ..ChapterCandidateRuntimeStatePatch::default()
        },
    );

    let output = (dependencies.collect_generation_candidate_output_fn)(
        ChapterCandidateTargetedFinalRepairOutputCollectInput {
            generate_kwargs,
            candidate_index: final_repair_attempt_index,
            max_output_chars: (dependencies.resolve_targeted_final_repair_char_limit_fn)(
                request.target_word_count,
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
        ChapterCandidateTargetedFinalRepairRecordBuildInput {
            full_content: output.full_content,
            candidate_chunks: output.chunks,
            target_word_count: request.target_word_count,
            source: request.source.clone(),
            generation_label: format!(
                "{}-{}",
                request.generation_label, request.generation_label_suffix
            ),
            candidate_index: final_repair_attempt_index,
            candidate_offset: final_repair_attempt_index - 1,
            generation_path: TARGETED_QUALITY_REPAIR.to_string(),
            attempt_kind: TARGETED_QUALITY_REPAIR.to_string(),
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
    let repair_seed_candidate_view =
        targeted_final_repair_seed_candidate_view(repair_seed_candidate);
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

fn targeted_final_repair_seed_candidate_view(
    repair_seed_candidate: &Value,
) -> TargetedFinalRepairSeedCandidateView {
    TargetedFinalRepairSeedCandidateView {
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

pub(crate) fn build_chapter_candidate_targeted_final_repair_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_candidate_targeted_final_repair_service",
        "scope": "candidate_targeted_final_repair_prompt_runtime_record_and_followup_owner",
        "python_source_map": [
            "backend/app/services/chapter_candidate_targeted_final_repair_service.py",
            "backend/app/services/chapter_candidate_executor_service.py",
            "backend/app/services/chapter_candidate_executor_wiring_service.py",
            "backend/app/services/chapter_candidate_selection_metadata_service.py",
            "backend/app/services/chapter_candidate_classification_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs",
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
                "build_default_targeted_final_repair_dependencies",
                "execute_targeted_final_repair_pass_workflow"
            ],
            "request_fields": [
                "base_generate_kwargs",
                "base_prompt",
                "base_temperature",
                "target_word_count",
                "source",
                "generation_label",
                "generation_label_suffix",
                "repair_seed_candidate",
                "current_winner_candidate",
                "runtime_state",
                "allow_followup_seed_defer"
            ],
            "repair_policy": [
                "suffix absence, provider failure, or record failure keeps the original selected candidate",
                "repair candidate is appended only when keep policy accepts it",
                "selected candidate is replaced only when adopt and prefer policies both accept it",
                "follow-up seed is deferred only when adoption is skipped, defer is allowed, and follow-up policy accepts the repair candidate"
            ],
            "prompt_and_limits_policy": [
                "repair prompt preserves base prompt, targeted suffix, and repair seed draft",
                "temperature derives from base temperature and repair seed quality gate plan",
                "max_tokens derives from target and repair seed word count",
                "max_output_chars derives from target word count"
            ],
            "runtime_state_policy": [
                "runtime state sync records repair attempt index, total candidates, current chars, generation_path, attempt_kind, targeted repair flag, and winner index",
                "missing runtime_state is a no-op rather than an error"
            ],
            "record_policy": [
                "record builder receives repaired full content, chunks, target_word_count, source, generation label suffix, candidate index, offset, generation_path, and attempt_kind",
                "repair seed metadata records candidate index, generation_path, and attempt_kind when available"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_targeted_final_repair_service",
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
            "default_dependencies_owner": "build_default_targeted_final_repair_dependencies",
            "workflow_owner": "execute_targeted_final_repair_pass_workflow",
            "repair_prompt_owner": "build_repair_prompt",
            "runtime_state_sync_owner": "sync_chapter_candidate_runtime_state",
            "output_collection_owner": "collect_generation_candidate_output_fn",
            "record_build_owner": "build_generation_candidate_record_fn",
            "followup_seed_owner": "deferred_followup_targeted_repair_seed_candidate",
            "rerank_formula_owner": "chapter_candidate_rerank_service",
            "candidate_record_owner": "chapter_candidate_record_service",
            "candidate_output_owner": "chapter_candidate_output_service",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_chapter_candidate_targeted_final_repair_owner_ready_for_source_map_closeout_review"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_targeted_final_repair_python_source_map",
            "python_fallback_removal_ready": false,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_targeted_final_repair_owner_contract,
        build_default_targeted_final_repair_dependencies,
        execute_targeted_final_repair_pass_workflow, targeted_final_repair_seed_candidate_view,
        ChapterCandidateTargetedFinalRepairDependencies,
        ChapterCandidateTargetedFinalRepairOutputCollectInput,
        ChapterCandidateTargetedFinalRepairRecordBuildInput,
        ChapterCandidateTargetedFinalRepairRequest, ChapterCandidateTargetedFinalRepairSuffixInput,
    };
    use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;

    fn base_request() -> ChapterCandidateTargetedFinalRepairRequest {
        let mut base_generate_kwargs = Map::new();
        base_generate_kwargs.insert("prompt".to_string(), Value::String("base".to_string()));
        ChapterCandidateTargetedFinalRepairRequest {
            base_generate_kwargs,
            base_prompt: "Base prompt".to_string(),
            base_temperature: 0.7,
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "candidate".to_string(),
            generation_label_suffix: "targeted-final-repair".to_string(),
            repair_seed_candidate: json!({
                "candidate_index": 2,
                "target_word_count": 1200,
                "word_count": 1420,
                "full_content": "Seed draft",
                "generation_path": "word_budget_repair",
                "attempt_kind": "word_budget_repair",
                "quality_metrics": {
                    "overall_score": 90,
                    "candidate_selection": {"word_count": 1420}
                },
                "quality_gate_plan": {
                    "quality_gate": {
                        "decision": "manual_review",
                        "failed_metrics": [
                            {"label": "weak ending pressure", "focus_area": "cliffhanger"}
                        ]
                    }
                }
            }),
            current_winner_candidate: json!({"candidate_index": 1, "overall_score": 88}),
            runtime_state: Some(json!({})),
            allow_followup_seed_defer: false,
        }
    }

    #[test]
    fn should_publish_chapter_candidate_targeted_final_repair_owner_contract() {
        let contract = build_chapter_candidate_targeted_final_repair_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_candidate_targeted_final_repair_service"
        );
        assert_eq!(
            contract["scope"],
            "candidate_targeted_final_repair_prompt_runtime_record_and_followup_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/services/chapter_candidate_targeted_final_repair_service.py"
        );
        assert!(contract["rust_owner_map"]
            .as_array()
            .expect("rust owner map")
            .contains(&json!(
                "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs"
            )));
        assert!(contract["behavior_contract"]["entrypoints"]
            .as_array()
            .expect("entrypoints")
            .contains(&json!("execute_targeted_final_repair_pass_workflow")));
        assert!(contract["behavior_contract"]["repair_policy"]
            .as_array()
            .expect("repair policy")
            .iter()
            .any(|policy| policy
                .as_str()
                .unwrap_or_default()
                .contains("follow-up seed")));
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
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["workflow_owner"],
            "execute_targeted_final_repair_pass_workflow"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["runtime_state_sync_owner"],
            "sync_chapter_candidate_runtime_state"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["followup_seed_owner"],
            "deferred_followup_targeted_repair_seed_candidate"
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
            "rust_chapter_candidate_targeted_final_repair_owner_ready_for_source_map_closeout_review"
        );
    }

    #[test]
    fn should_build_targeted_final_repair_seed_candidate_view_from_repair_seed_candidate() {
        let request = base_request();
        let view = targeted_final_repair_seed_candidate_view(&request.repair_seed_candidate);

        assert_eq!(view.candidate_index, 2);
        assert_eq!(view.generation_path.as_deref(), Some("word_budget_repair"));
        assert_eq!(view.attempt_kind.as_deref(), Some("word_budget_repair"));
        assert_eq!(view.word_count, 1420);
        assert_eq!(view.full_content.as_deref(), Some("Seed draft"));
        assert_eq!(
            view.quality_metrics.as_ref().unwrap()["candidate_selection"]["word_count"],
            1420
        );
        assert_eq!(
            view.quality_gate_plan.as_ref().unwrap()["quality_gate"]["decision"],
            "manual_review"
        );
    }

    #[tokio::test]
    async fn should_adopt_preferred_targeted_final_repair_candidate() {
        let mut request = base_request();
        let selected_candidate = request.current_winner_candidate.clone();
        let candidates = vec![
            selected_candidate.clone(),
            request.repair_seed_candidate.clone(),
        ];
        let mut dependencies = dependencies(
            |_input| Some("Fix tail quality gaps.".to_string()),
            |input| {
                Ok(ChapterCandidateOutput {
                    full_content: "Targeted repair".to_string(),
                    chunks: vec!["Targeted".to_string(), " repair".to_string()],
                    runtime_state: input.runtime_state.clone(),
                })
            },
            true,
            true,
            true,
            false,
        );

        let result = execute_targeted_final_repair_pass_workflow(
            &mut request,
            selected_candidate,
            candidates,
            &mut dependencies,
        )
        .await;

        assert_eq!(result.candidates.len(), 3);
        assert_eq!(result.selected_candidate["candidate_index"], 3);
        assert_eq!(
            result.selected_candidate["generation_path"],
            "targeted_quality_repair"
        );
        assert_eq!(
            result.selected_candidate["quality_metrics"]["candidate_selection"]
                ["repair_seed_candidate_index"],
            2
        );
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["attempt_kind"],
            "targeted_quality_repair"
        );
    }

    #[tokio::test]
    async fn should_defer_followup_seed_when_not_adopted() {
        let mut request = base_request();
        request.allow_followup_seed_defer = true;
        let selected_candidate = request.current_winner_candidate.clone();
        let candidates = vec![
            selected_candidate.clone(),
            request.repair_seed_candidate.clone(),
        ];
        let mut dependencies = dependencies(
            |_input| Some("Fix one remaining gap.".to_string()),
            |input| {
                Ok(ChapterCandidateOutput {
                    full_content: "Followup seed".to_string(),
                    chunks: vec!["Followup seed".to_string()],
                    runtime_state: input.runtime_state.clone(),
                })
            },
            true,
            false,
            false,
            true,
        );

        let result = execute_targeted_final_repair_pass_workflow(
            &mut request,
            selected_candidate.clone(),
            candidates,
            &mut dependencies,
        )
        .await;

        assert_eq!(result.selected_candidate, selected_candidate);
        assert_eq!(
            result
                .deferred_followup_targeted_repair_seed_candidate
                .as_ref()
                .unwrap()["candidate_index"],
            3
        );
    }

    #[tokio::test]
    async fn should_keep_original_candidate_when_targeted_repair_fails() {
        let mut request = base_request();
        let selected_candidate = request.current_winner_candidate.clone();
        let candidates = vec![
            selected_candidate.clone(),
            request.repair_seed_candidate.clone(),
        ];
        let mut dependencies = dependencies(
            |_input| Some("repair".to_string()),
            |_input| -> Result<ChapterCandidateOutput, String> {
                Err("provider failed".to_string())
            },
            true,
            true,
            true,
            false,
        );

        let result = execute_targeted_final_repair_pass_workflow(
            &mut request,
            selected_candidate.clone(),
            candidates.clone(),
            &mut dependencies,
        )
        .await;

        assert_eq!(result.selected_candidate, selected_candidate);
        assert_eq!(result.candidates, candidates);
        assert!(result
            .deferred_followup_targeted_repair_seed_candidate
            .is_none());
    }

    #[tokio::test]
    async fn should_build_default_targeted_repair_dependencies_from_owner() {
        let mut request = base_request();
        let selected_candidate = request.current_winner_candidate.clone();
        let candidates = vec![
            selected_candidate.clone(),
            request.repair_seed_candidate.clone(),
        ];
        let built_records = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured_records = Arc::clone(&built_records);
        let mut dependencies = build_default_targeted_final_repair_dependencies(
            |input: ChapterCandidateTargetedFinalRepairOutputCollectInput| {
                assert_eq!(input.candidate_index, 3);
                assert!(input
                    .generate_kwargs
                    .get("prompt")
                    .and_then(Value::as_str)
                    .is_some_and(|prompt| {
                        prompt.contains("Previous draft to rewrite")
                            && prompt.contains("Seed draft")
                    }));
                assert!(input
                    .generate_kwargs
                    .get("temperature")
                    .and_then(Value::as_f64)
                    .is_some_and(|temperature| temperature > 0.0));
                std::future::ready(Ok(ChapterCandidateOutput {
                    full_content: "Default targeted repair".to_string(),
                    chunks: vec!["Default targeted repair".to_string()],
                    runtime_state: input.runtime_state.clone(),
                }))
            },
            move |input: ChapterCandidateTargetedFinalRepairRecordBuildInput| {
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
                    "selection_score": 99.0,
                    "word_count_fit_score": 99.0,
                    "quality_gate_decision": "allow_save",
                    "quality_gate_priority": 3,
                    "quality_metrics": {"overall_score": 95},
                    "quality_gate_plan": {"quality_gate": {"decision": "allow_save"}}
                });
                captured_records.lock().unwrap().push(record.clone());
                Ok(record)
            },
        );

        let result = execute_targeted_final_repair_pass_workflow(
            &mut request,
            selected_candidate,
            candidates,
            &mut dependencies,
        )
        .await;

        assert_eq!(built_records.lock().unwrap().len(), 1);
        let targeted_candidate = result.candidates.last().expect("targeted candidate");
        assert_eq!(targeted_candidate["candidate_index"], 3);
        assert_eq!(
            targeted_candidate["attempt_kind"],
            "targeted_quality_repair"
        );
        assert_eq!(
            targeted_candidate["quality_metrics"]["candidate_selection"]
                ["repair_seed_attempt_kind"],
            "word_budget_repair"
        );
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["generation_path"],
            "targeted_quality_repair"
        );
    }

    fn dependencies<SuffixFn, CollectFn>(
        build_suffix: SuffixFn,
        collect_output: CollectFn,
        should_keep: bool,
        should_adopt: bool,
        should_prefer: bool,
        should_followup: bool,
    ) -> ChapterCandidateTargetedFinalRepairDependencies<
        SuffixFn,
        impl FnMut(f64, Option<Value>) -> f64,
        impl FnMut(i64, i64) -> i64,
        impl FnMut(
            ChapterCandidateTargetedFinalRepairOutputCollectInput,
        ) -> std::future::Ready<Result<ChapterCandidateOutput, String>>,
        impl FnMut(i64) -> Option<i64>,
        impl FnMut(ChapterCandidateTargetedFinalRepairRecordBuildInput) -> Result<Value, String>,
        impl FnMut(Value, Value) -> bool,
        impl FnMut(Value, Value) -> bool,
        impl FnMut(Value, Value) -> bool,
        impl FnMut(Value) -> bool,
    >
    where
        SuffixFn: FnMut(ChapterCandidateTargetedFinalRepairSuffixInput) -> Option<String>,
        CollectFn: FnMut(
            ChapterCandidateTargetedFinalRepairOutputCollectInput,
        ) -> Result<ChapterCandidateOutput, String>,
    {
        let mut collect_output = collect_output;
        ChapterCandidateTargetedFinalRepairDependencies {
            build_targeted_final_repair_suffix_fn: build_suffix,
            resolve_targeted_final_repair_temperature_fn: |_base, _plan| 0.52,
            resolve_targeted_final_repair_max_tokens_fn: |_target, _current| 1500,
            collect_generation_candidate_output_fn:
                move |input: ChapterCandidateTargetedFinalRepairOutputCollectInput| {
                    assert_eq!(input.candidate_index, 3);
                    assert_eq!(input.max_output_chars, Some(1700));
                    assert!(input
                        .generate_kwargs
                        .get("prompt")
                        .and_then(Value::as_str)
                        .is_some_and(|prompt: &str| prompt.contains("Previous draft to rewrite")));
                    assert_eq!(input.generate_kwargs["temperature"], json!(0.52));
                    assert_eq!(input.generate_kwargs["max_tokens"], json!(1500));
                    std::future::ready(collect_output(input))
                },
            resolve_targeted_final_repair_char_limit_fn: |_target| Some(1700),
            build_generation_candidate_record_fn:
                |input: ChapterCandidateTargetedFinalRepairRecordBuildInput| {
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
                        "word_count": 1210,
                        "quality_metrics": {"overall_score": 92},
                        "quality_gate_plan": {"quality_gate": {"decision": "allow_save"}}
                    }))
                },
            should_keep_targeted_final_repair_candidate_fn: move |_seed, _repair| should_keep,
            should_adopt_targeted_final_repair_candidate_fn: move |_seed, _repair| should_adopt,
            should_prefer_targeted_final_repair_candidate_fn: move |_winner, _repair| should_prefer,
            should_apply_followup_targeted_final_repair_fn: move |_repair| should_followup,
        }
    }
}
