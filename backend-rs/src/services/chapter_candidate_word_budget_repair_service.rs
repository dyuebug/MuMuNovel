// Staged Rust owner for Python chapter_candidate_word_budget_repair_service.py.
// It ports the repair workflow as a whole owner while rerank formula callbacks
// remain injectable until the candidate executor package cuts over.
#![allow(dead_code)]

use std::future::Future;

use serde_json::{Map, Value};

use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateWordBudgetRepairResult {
    pub(crate) selected_candidate: Value,
    pub(crate) candidates: Vec<Value>,
    pub(crate) word_budget_repair_used: bool,
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
    let repair_suffix = (dependencies.build_word_budget_repair_suffix_fn)(
        ChapterCandidateWordBudgetRepairSuffixInput {
            quality_metrics: selected_candidate.get("quality_metrics").cloned(),
            quality_gate_plan: selected_candidate.get("quality_gate_plan").cloned(),
            current_content: selected_candidate
                .get("full_content")
                .and_then(Value::as_str)
                .map(ToString::to_string),
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

    let source_word_count = selected_candidate
        .get("word_count")
        .and_then(value_to_i64)
        .unwrap_or(0);
    let quality_gate_plan = selected_candidate.get("quality_gate_plan").cloned();
    let relax_content_budget =
        (dependencies.should_relax_word_budget_repair_limits_fn)(quality_gate_plan);

    let mut repair_generate_kwargs = request.base_generate_kwargs.clone();
    repair_generate_kwargs.insert(
        "prompt".to_string(),
        Value::String(build_repair_prompt(
            &request.base_prompt,
            &repair_suffix,
            selected_candidate
                .get("full_content")
                .and_then(Value::as_str),
        )),
    );
    insert_f64(
        &mut repair_generate_kwargs,
        "temperature",
        (dependencies.resolve_word_budget_repair_temperature_fn)(
            request.base_temperature,
            selected_candidate.get("quality_metrics").cloned(),
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
        },
    )
    .await?;

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

    let seed_index = repair_seed_candidate
        .get("candidate_index")
        .and_then(value_to_i64)
        .unwrap_or(1)
        .max(1);
    candidate_selection.insert("repair_seed_candidate_index".to_string(), seed_index.into());
    if let Some(generation_path) = trimmed_string_field(repair_seed_candidate, "generation_path") {
        candidate_selection.insert(
            "repair_seed_generation_path".to_string(),
            Value::String(generation_path),
        );
    }
    if let Some(attempt_kind) = trimmed_string_field(repair_seed_candidate, "attempt_kind") {
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

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::{
        maybe_apply_word_budget_repair_workflow, ChapterCandidateWordBudgetRepairDependencies,
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
