use std::future::Future;

use serde_json::{Map, Value};

use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;
use crate::services::chapter_candidate_rerank_service::{
    build_candidate_retry_prompt_suffix, build_candidate_retry_strategy_suffix,
    resolve_candidate_retry_temperature, select_best_generation_candidate,
    should_generate_additional_candidate,
};
use crate::services::chapter_candidate_runtime_state_service::{
    resolve_generation_attempt_labels, sync_chapter_candidate_runtime_state,
    ChapterCandidateRuntimeStatePatch,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateGenerationRequest {
    pub(crate) base_generate_kwargs: Map<String, Value>,
    pub(crate) base_prompt: String,
    pub(crate) base_temperature: f64,
    pub(crate) target_word_count: i64,
    pub(crate) source: String,
    pub(crate) generation_label: String,
    pub(crate) max_candidates: i64,
    pub(crate) runtime_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateOutputCollectInput {
    pub(crate) generate_kwargs: Map<String, Value>,
    pub(crate) candidate_index: i64,
    pub(crate) runtime_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateRecordBuildInput {
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

pub(crate) struct ChapterCandidateGenerationDependencies<
    CollectOutput,
    BuildRecord,
    ShouldAdd,
    RetryPrompt,
    RetryStrategy,
    RetryTemp,
    SelectBest,
> {
    pub(crate) collect_generation_candidate_output_fn: CollectOutput,
    pub(crate) build_generation_candidate_record_fn: BuildRecord,
    pub(crate) should_generate_additional_candidate_fn: ShouldAdd,
    pub(crate) build_candidate_retry_prompt_suffix_fn: RetryPrompt,
    pub(crate) build_candidate_retry_strategy_suffix_fn: RetryStrategy,
    pub(crate) resolve_candidate_retry_temperature_fn: RetryTemp,
    pub(crate) select_best_generation_candidate_fn: SelectBest,
}

pub(crate) fn build_default_generation_dependencies<CollectOutput, CollectFuture, BuildRecord>(
    collect_generation_candidate_output_fn: CollectOutput,
    build_generation_candidate_record_fn: BuildRecord,
) -> ChapterCandidateGenerationDependencies<
    CollectOutput,
    BuildRecord,
    impl FnMut(Value, usize, i64) -> bool,
    impl FnMut(Option<Value>, i64) -> Option<String>,
    impl FnMut(Option<Value>, Option<Value>, i64, String) -> Option<String>,
    impl FnMut(f64, Option<Value>, Option<Value>, i64) -> Option<f64>,
    impl FnMut(Vec<Value>) -> Option<Value>,
>
where
    CollectOutput: FnMut(ChapterCandidateOutputCollectInput) -> CollectFuture,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>>,
    BuildRecord: FnMut(ChapterCandidateRecordBuildInput) -> Result<Value, String>,
{
    ChapterCandidateGenerationDependencies {
        collect_generation_candidate_output_fn,
        build_generation_candidate_record_fn,
        should_generate_additional_candidate_fn: should_generate_additional_candidate,
        build_candidate_retry_prompt_suffix_fn: build_candidate_retry_prompt_suffix,
        build_candidate_retry_strategy_suffix_fn: build_candidate_retry_strategy_suffix,
        resolve_candidate_retry_temperature_fn: resolve_candidate_retry_temperature,
        select_best_generation_candidate_fn: select_best_generation_candidate,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateGenerationResult {
    pub(crate) candidates: Vec<Value>,
    pub(crate) selected_candidate: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateRetryPayloadView {
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) quality_gate_plan: Option<Value>,
}

impl ChapterCandidateGenerationResult {
    #[cfg(test)]
    pub(crate) fn candidate_count(&self) -> usize {
        self.candidates.len()
    }
}

pub(crate) async fn generate_candidate_pool_workflow<
    CollectOutput,
    CollectFuture,
    BuildRecord,
    ShouldAdd,
    RetryPrompt,
    RetryStrategy,
    RetryTemp,
    SelectBest,
>(
    request: &mut ChapterCandidateGenerationRequest,
    dependencies: &mut ChapterCandidateGenerationDependencies<
        CollectOutput,
        BuildRecord,
        ShouldAdd,
        RetryPrompt,
        RetryStrategy,
        RetryTemp,
        SelectBest,
    >,
) -> Result<ChapterCandidateGenerationResult, String>
where
    CollectOutput: FnMut(ChapterCandidateOutputCollectInput) -> CollectFuture,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>>,
    BuildRecord: FnMut(ChapterCandidateRecordBuildInput) -> Result<Value, String>,
    ShouldAdd: FnMut(Value, usize, i64) -> bool,
    RetryPrompt: FnMut(Option<Value>, i64) -> Option<String>,
    RetryStrategy: FnMut(Option<Value>, Option<Value>, i64, String) -> Option<String>,
    RetryTemp: FnMut(f64, Option<Value>, Option<Value>, i64) -> Option<f64>,
    SelectBest: FnMut(Vec<Value>) -> Option<Value>,
{
    let resolved_max_candidates = request.max_candidates.max(1);
    let mut candidates = Vec::<Value>::new();
    let mut retry_suffix = String::new();
    let mut retry_temperature = None::<f64>;

    let initial_labels = resolve_generation_attempt_labels(1, false);
    sync_candidate_generation_runtime_state(
        request.runtime_state.as_mut(),
        1,
        resolved_max_candidates,
        &initial_labels.generation_path,
        &initial_labels.attempt_kind,
        false,
        false,
    );

    for candidate_offset in 0..resolved_max_candidates {
        let candidate_index = candidate_offset + 1;
        let labels = resolve_generation_attempt_labels(candidate_index, false);
        sync_candidate_generation_runtime_state(
            request.runtime_state.as_mut(),
            candidate_index,
            resolved_max_candidates,
            &labels.generation_path,
            &labels.attempt_kind,
            candidate_index > 1,
            false,
        );

        let mut current_generate_kwargs = request.base_generate_kwargs.clone();
        if !retry_suffix.is_empty() {
            current_generate_kwargs.insert(
                "prompt".to_string(),
                Value::String(
                    format!("{}\n\n{}", request.base_prompt, retry_suffix)
                        .trim()
                        .to_string(),
                ),
            );
        }
        if let Some(temperature) = retry_temperature {
            if let Some(number) = serde_json::Number::from_f64(temperature) {
                current_generate_kwargs.insert("temperature".to_string(), Value::Number(number));
            }
        }

        let output = (dependencies.collect_generation_candidate_output_fn)(
            ChapterCandidateOutputCollectInput {
                generate_kwargs: current_generate_kwargs,
                candidate_index,
                runtime_state: request.runtime_state.clone(),
            },
        )
        .await?;
        request.runtime_state = output
            .runtime_state
            .clone()
            .or_else(|| request.runtime_state.take());
        let candidate = (dependencies.build_generation_candidate_record_fn)(
            ChapterCandidateRecordBuildInput {
                full_content: output.full_content,
                candidate_chunks: output.chunks,
                target_word_count: request.target_word_count,
                source: request.source.clone(),
                generation_label: request.generation_label.clone(),
                candidate_index,
                candidate_offset,
                generation_path: labels.generation_path.to_string(),
                attempt_kind: labels.attempt_kind.to_string(),
            },
        )?;
        candidates.push(candidate);

        let latest_candidate = candidates
            .last()
            .cloned()
            .expect("candidate exists after push");
        if !(dependencies.should_generate_additional_candidate_fn)(
            latest_candidate.clone(),
            candidates.len(),
            resolved_max_candidates,
        ) {
            break;
        }

        let retry_payload_view = candidate_retry_payload_view(&latest_candidate);
        let attempt_index = candidate_index + 1;
        let retry_prompt_suffix = (dependencies.build_candidate_retry_prompt_suffix_fn)(
            retry_payload_view.quality_gate_plan.clone(),
            attempt_index,
        );
        let retry_strategy_suffix = (dependencies.build_candidate_retry_strategy_suffix_fn)(
            retry_payload_view.quality_gate_plan.clone(),
            retry_payload_view.quality_metrics.clone(),
            attempt_index,
            request.source.clone(),
        );
        retry_suffix = [retry_prompt_suffix, retry_strategy_suffix]
            .into_iter()
            .flatten()
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        retry_temperature = (dependencies.resolve_candidate_retry_temperature_fn)(
            request.base_temperature,
            retry_payload_view.quality_metrics,
            retry_payload_view.quality_gate_plan,
            attempt_index,
        );
        if retry_suffix.is_empty() {
            break;
        }
    }

    let selected_candidate = (dependencies.select_best_generation_candidate_fn)(candidates.clone())
        .or_else(|| candidates.last().cloned())
        .ok_or_else(|| "candidate generation produced no candidates".to_string())?;

    Ok(ChapterCandidateGenerationResult {
        candidates,
        selected_candidate,
    })
}

fn sync_candidate_generation_runtime_state(
    runtime_state: Option<&mut Value>,
    candidate_index: i64,
    candidate_total: i64,
    generation_path: &str,
    attempt_kind: &str,
    rerank_used: bool,
    word_budget_repair_used: bool,
) {
    sync_chapter_candidate_runtime_state(
        runtime_state,
        candidate_index,
        candidate_total,
        ChapterCandidateRuntimeStatePatch {
            current_chars: Some(0),
            chunk_count: Some(0),
            generation_path: Some(generation_path.to_string()),
            attempt_kind: Some(attempt_kind.to_string()),
            rerank_used: Some(rerank_used),
            word_budget_repair_used: Some(word_budget_repair_used),
            ..ChapterCandidateRuntimeStatePatch::default()
        },
    );
}

pub(crate) fn candidate_retry_payload_view(candidate: &Value) -> CandidateRetryPayloadView {
    CandidateRetryPayloadView {
        quality_metrics: candidate.get("quality_metrics").cloned(),
        quality_gate_plan: candidate.get("quality_gate_plan").cloned(),
    }
}
