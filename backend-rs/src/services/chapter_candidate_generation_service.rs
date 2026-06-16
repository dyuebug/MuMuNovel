// Rust owner for the candidate-pool generation workflow originally mapped
// from Python chapter_candidate_generation_service.py. Default executor
// dependencies call this owner directly.

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
struct CandidateRetryPayloadView {
    quality_metrics: Option<Value>,
    quality_gate_plan: Option<Value>,
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

fn candidate_retry_payload_view(candidate: &Value) -> CandidateRetryPayloadView {
    CandidateRetryPayloadView {
        quality_metrics: candidate.get("quality_metrics").cloned(),
        quality_gate_plan: candidate.get("quality_gate_plan").cloned(),
    }
}

pub(crate) fn build_chapter_candidate_generation_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_candidate_generation_service",
        "scope": "candidate_pool_generation_workflow_owner",
        "python_source_map": [
            "backend/app/services/chapter_candidate_generation_service.py",
            "backend/app/services/chapter_generation/stream/candidate_service.py",
            "backend/app/services/batch_generation_candidate_service.py",
            "backend/tests/test_services/test_chapter_candidate_generation_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_output_service.rs",
            "backend-rs/src/services/chapter_candidate_record_service.rs",
            "backend-rs/src/services/chapter_candidate_rerank_service.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_default_generation_dependencies",
                "generate_candidate_pool_workflow"
            ],
            "request_fields": [
                "base_generate_kwargs",
                "base_prompt",
                "base_temperature",
                "target_word_count",
                "source",
                "generation_label",
                "max_candidates",
                "runtime_state"
            ],
            "output_collect_fields": [
                "generate_kwargs",
                "candidate_index",
                "runtime_state"
            ],
            "record_build_fields": [
                "full_content",
                "candidate_chunks",
                "target_word_count",
                "source",
                "generation_label",
                "candidate_index",
                "candidate_offset",
                "generation_path",
                "attempt_kind"
            ],
            "workflow_policy": [
                "max_candidates is normalized to at least one",
                "initial attempt labels come from runtime-state owner",
                "retry prompt and strategy suffixes are appended to the base prompt when available",
                "retry temperature overrides generate kwargs only when finite JSON number conversion succeeds",
                "runtime_state is synced before each candidate collection and restored from output when present",
                "workflow stops when no additional candidate is required",
                "workflow stops when retry suffix is empty",
                "selected candidate comes from rerank owner or falls back to the last candidate",
                "workflow errors when no candidate is produced"
            ],
            "error_contract": [
                "candidate output collection error string is propagated",
                "candidate record build error string is propagated",
                "candidate generation produced no candidates"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_generation_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_executor_default_dependency_service",
            "chapter_candidate_executor_production_adapter_service",
            "chapter_candidate_executor_service",
            "chapter_candidate_route_gateway_service",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter-candidate-route-gateway-smoke-rust"
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
            "default_dependencies_owner": "build_default_generation_dependencies",
            "candidate_pool_workflow_owner": "generate_candidate_pool_workflow",
            "output_collection_owner": "collect_generation_candidate_output_fn",
            "record_build_owner": "build_generation_candidate_record_fn",
            "retry_prompt_owner": "build_candidate_retry_prompt_suffix_fn",
            "retry_strategy_owner": "build_candidate_retry_strategy_suffix_fn",
            "retry_temperature_owner": "resolve_candidate_retry_temperature_fn",
            "runtime_state_sync_owner": "sync_chapter_candidate_runtime_state",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_chapter_candidate_generation_owner_ready_for_source_map_closeout_review"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_generation_python_source_map",
            "python_fallback_removal_ready": false,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_generation_owner_contract, build_default_generation_dependencies,
        candidate_retry_payload_view, generate_candidate_pool_workflow,
        ChapterCandidateGenerationDependencies, ChapterCandidateGenerationRequest,
        ChapterCandidateOutputCollectInput, ChapterCandidateRecordBuildInput,
    };
    use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;
    use crate::services::chapter_candidate_record_service::{
        build_generation_candidate_record, ChapterCandidateRecordRequest,
    };

    fn base_request(max_candidates: i64) -> ChapterCandidateGenerationRequest {
        let mut base_generate_kwargs = Map::new();
        base_generate_kwargs.insert("prompt".to_string(), json!("BASE"));
        base_generate_kwargs.insert("temperature".to_string(), json!(0.7));

        ChapterCandidateGenerationRequest {
            base_generate_kwargs,
            base_prompt: "BASE".to_string(),
            base_temperature: 0.7,
            target_word_count: 1000,
            source: "single".to_string(),
            generation_label: "chapter".to_string(),
            max_candidates,
            runtime_state: Some(json!({})),
        }
    }

    fn record_from_input(input: ChapterCandidateRecordBuildInput) -> Result<Value, String> {
        Ok(json!({
            "candidate_index": input.candidate_index,
            "candidate_offset": input.candidate_offset,
            "full_content": input.full_content,
            "candidate_chunks": input.candidate_chunks,
            "target_word_count": input.target_word_count,
            "source": input.source,
            "generation_label": input.generation_label,
            "generation_path": input.generation_path,
            "attempt_kind": input.attempt_kind,
            "quality_metrics": {"score": input.candidate_index},
            "quality_gate_plan": {"decision": if input.candidate_index == 1 { "retry" } else { "pass" }},
        }))
    }

    #[test]
    fn should_build_candidate_retry_payload_view_from_candidate() {
        let candidate = json!({
            "candidate_index": 1,
            "quality_metrics": {"score": 1, "overall_score": 82},
            "quality_gate_plan": {"quality_gate": {"decision": "auto_repair"}}
        });

        let view = candidate_retry_payload_view(&candidate);

        assert_eq!(view.quality_metrics.as_ref().unwrap()["score"], 1);
        assert_eq!(
            view.quality_gate_plan.as_ref().unwrap()["quality_gate"]["decision"],
            "auto_repair"
        );
    }

    #[tokio::test]
    async fn should_generate_initial_candidate_like_python_workflow() {
        let mut request = base_request(1);
        let mut collect_calls = Vec::<ChapterCandidateOutputCollectInput>::new();
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| {
                collect_calls.push(input.clone());
                async move {
                    Ok(ChapterCandidateOutput {
                        full_content: format!("candidate-{}", input.candidate_index),
                        chunks: vec![format!("chunk-{}", input.candidate_index)],
                        runtime_state: input.runtime_state.clone(),
                    })
                }
            },
            build_generation_candidate_record_fn: record_from_input,
            should_generate_additional_candidate_fn: |_candidate, _produced, _max| false,
            build_candidate_retry_prompt_suffix_fn: |_plan, _attempt| {
                Some("retry prompt".to_string())
            },
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, _attempt, _source| {
                Some("retry strategy".to_string())
            },
            resolve_candidate_retry_temperature_fn: |_base, _metrics, _plan, _attempt| Some(0.9),
            select_best_generation_candidate_fn: |_candidates| None,
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(result.candidate_count(), 1);
        assert_eq!(result.selected_candidate["full_content"], "candidate-1");
        assert_eq!(collect_calls.len(), 1);
        assert_eq!(collect_calls[0].candidate_index, 1);
        assert_eq!(
            collect_calls[0]
                .runtime_state
                .as_ref()
                .expect("collect runtime")["generation_path"],
            "single_pass"
        );
        assert_eq!(collect_calls[0].generate_kwargs["prompt"], "BASE");
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["generation_path"],
            "single_pass"
        );
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["attempt_kind"],
            "initial_candidate"
        );
    }

    #[tokio::test]
    async fn should_generate_retry_candidate_with_prompt_suffix_and_temperature() {
        let mut request = base_request(2);
        let mut collect_calls = Vec::<ChapterCandidateOutputCollectInput>::new();
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| {
                collect_calls.push(input.clone());
                async move {
                    Ok(ChapterCandidateOutput {
                        full_content: format!("candidate-{}", input.candidate_index),
                        chunks: vec![format!("chunk-{}", input.candidate_index)],
                        runtime_state: input.runtime_state.clone(),
                    })
                }
            },
            build_generation_candidate_record_fn: record_from_input,
            should_generate_additional_candidate_fn: |_candidate, produced, max| {
                produced < max as usize
            },
            build_candidate_retry_prompt_suffix_fn: |_plan, attempt| {
                Some(format!("retry prompt {attempt}"))
            },
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, attempt, source| {
                Some(format!("retry strategy {attempt} {source}"))
            },
            resolve_candidate_retry_temperature_fn: |base, _metrics, _plan, attempt| {
                Some(base + attempt as f64 / 10.0)
            },
            select_best_generation_candidate_fn: |candidates: Vec<Value>| {
                candidates.last().cloned()
            },
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(result.candidate_count(), 2);
        assert_eq!(result.selected_candidate["candidate_index"], 2);
        assert_eq!(collect_calls.len(), 2);
        assert_eq!(
            collect_calls[1].generate_kwargs["prompt"],
            "BASE\n\nretry prompt 2\n\nretry strategy 2 single"
        );
        let retry_temperature = collect_calls[1].generate_kwargs["temperature"]
            .as_f64()
            .expect("retry temperature");
        assert!((retry_temperature - 0.9).abs() < f64::EPSILON);
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["generation_path"],
            "rerank_retry"
        );
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["attempt_kind"],
            "rerank_candidate"
        );
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["rerank_used"],
            true
        );
    }

    #[tokio::test]
    async fn should_build_default_generation_dependencies_from_owner() {
        let mut request = base_request(2);
        let mut collect_calls = Vec::<ChapterCandidateOutputCollectInput>::new();
        let mut dependencies = build_default_generation_dependencies(
            |input: ChapterCandidateOutputCollectInput| {
                collect_calls.push(input.clone());
                async move {
                    Ok(ChapterCandidateOutput {
                        full_content: format!("default-candidate-{}", input.candidate_index),
                        chunks: vec![format!("default-chunk-{}", input.candidate_index)],
                        runtime_state: input.runtime_state.clone(),
                    })
                }
            },
            |input: ChapterCandidateRecordBuildInput| {
                Ok(json!({
                    "candidate_index": input.candidate_index,
                    "candidate_offset": input.candidate_offset,
                    "full_content": input.full_content,
                    "candidate_chunks": input.candidate_chunks,
                    "target_word_count": input.target_word_count,
                    "word_count": if input.candidate_index == 1 { 400 } else { 1000 },
                    "source": input.source,
                    "generation_label": input.generation_label,
                    "generation_path": input.generation_path,
                    "attempt_kind": input.attempt_kind,
                    "quality_gate_decision": if input.candidate_index == 1 { "auto_repair" } else { "allow_save" },
                    "quality_gate_priority": if input.candidate_index == 1 { 2 } else { 3 },
                    "selection_score": if input.candidate_index == 1 { 70.0 } else { 96.0 },
                    "overall_score": if input.candidate_index == 1 { 68.0 } else { 92.0 },
                    "word_count_fit_score": if input.candidate_index == 1 { 40.0 } else { 99.0 },
                    "quality_metrics": {
                        "overall_score": if input.candidate_index == 1 { 68.0 } else { 92.0 },
                        "candidate_selection": {
                            "word_count": if input.candidate_index == 1 { 400 } else { 1000 },
                            "target_word_count": input.target_word_count
                        }
                    },
                    "quality_gate_plan": {
                        "quality_gate": {
                            "decision": if input.candidate_index == 1 { "auto_repair" } else { "allow_save" },
                            "failed_metrics": [
                                {"label": "conflict is thin", "focus_area": "conflict"}
                            ]
                        },
                        "active_story_repair_payload": {
                            "summary": "conflict needs a sharper turn",
                            "repair_targets": ["raise opposition"],
                            "focus_areas": ["conflict"]
                        }
                    }
                }))
            },
        );

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("default generation result");
        drop(dependencies);

        assert_eq!(result.candidate_count(), 2);
        assert_eq!(result.selected_candidate["candidate_index"], 2);
        assert_eq!(collect_calls.len(), 2);
        assert_eq!(
            collect_calls[0]
                .runtime_state
                .as_ref()
                .expect("collect runtime")["generation_path"],
            "single_pass"
        );
        assert_eq!(
            collect_calls[1]
                .runtime_state
                .as_ref()
                .expect("collect runtime")["generation_path"],
            "rerank_retry"
        );
        assert!(collect_calls[1]
            .generate_kwargs
            .get("prompt")
            .and_then(Value::as_str)
            .is_some_and(|prompt| {
                prompt.contains("Revision attempt #2")
                    && prompt.contains("Alternative candidate strategy #2")
            }));
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["generation_path"],
            "rerank_retry"
        );
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["attempt_kind"],
            "rerank_candidate"
        );
    }

    #[tokio::test]
    async fn should_stop_when_retry_suffix_is_empty() {
        let mut request = base_request(3);
        let mut collect_count = 0usize;
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| {
                collect_count += 1;
                async move {
                    Ok(ChapterCandidateOutput {
                        full_content: format!("candidate-{}", input.candidate_index),
                        chunks: vec![],
                        runtime_state: input.runtime_state.clone(),
                    })
                }
            },
            build_generation_candidate_record_fn: record_from_input,
            should_generate_additional_candidate_fn: |_candidate, _produced, _max| true,
            build_candidate_retry_prompt_suffix_fn: |_plan, _attempt| None,
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, _attempt, _source| None,
            resolve_candidate_retry_temperature_fn: |_base, _metrics, _plan, _attempt| Some(1.0),
            select_best_generation_candidate_fn: |_candidates| None,
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(collect_count, 1);
        assert_eq!(result.candidate_count(), 1);
        assert_eq!(result.selected_candidate["candidate_index"], 1);
    }

    #[tokio::test]
    async fn should_normalize_max_candidates_to_one() {
        let mut request = base_request(0);
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| async move {
                Ok(ChapterCandidateOutput {
                    full_content: format!("candidate-{}", input.candidate_index),
                    chunks: vec![],
                    runtime_state: input.runtime_state.clone(),
                })
            },
            build_generation_candidate_record_fn: record_from_input,
            should_generate_additional_candidate_fn: |_candidate, _produced, _max| true,
            build_candidate_retry_prompt_suffix_fn: |_plan, _attempt| Some("retry".to_string()),
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, _attempt, _source| None,
            resolve_candidate_retry_temperature_fn: |_base, _metrics, _plan, _attempt| None,
            select_best_generation_candidate_fn: |_candidates| None,
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(result.candidate_count(), 1);
        assert_eq!(
            request.runtime_state.as_ref().expect("runtime")["candidate_total"],
            1
        );
    }

    #[tokio::test]
    async fn should_compose_with_rust_candidate_record_owner() {
        let mut request = base_request(1);
        let mut quality_evaluator = |_content: &str| {
            json!({
                "overall_score": 88.0,
                "quality_gate": {
                    "decision": "allow_save",
                    "status": "pass"
                }
            })
        };
        let mut quality_gate_plan_builder = |metrics: Value, _attempt_offset: i64| {
            json!({
                "action": "continue",
                "quality_gate": metrics["quality_gate"].clone()
            })
        };
        let mut dependencies = ChapterCandidateGenerationDependencies {
            collect_generation_candidate_output_fn: |input: ChapterCandidateOutputCollectInput| async move {
                Ok(ChapterCandidateOutput {
                    full_content: format!("候选正文{}。", input.candidate_index),
                    chunks: vec![format!("候选正文{}。", input.candidate_index)],
                    runtime_state: input.runtime_state.clone(),
                })
            },
            build_generation_candidate_record_fn: |input: ChapterCandidateRecordBuildInput| {
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
                    &mut quality_evaluator,
                    &mut quality_gate_plan_builder,
                    None,
                )
            },
            should_generate_additional_candidate_fn: |_candidate, _produced, _max| false,
            build_candidate_retry_prompt_suffix_fn: |_plan, _attempt| None,
            build_candidate_retry_strategy_suffix_fn: |_plan, _metrics, _attempt, _source| None,
            resolve_candidate_retry_temperature_fn: |_base, _metrics, _plan, _attempt| None,
            select_best_generation_candidate_fn: |_candidates| None,
        };

        let result = generate_candidate_pool_workflow(&mut request, &mut dependencies)
            .await
            .expect("candidate generation result");

        assert_eq!(result.candidate_count(), 1);
        assert_eq!(result.selected_candidate["candidate_index"], 1);
        assert_eq!(
            result.selected_candidate["quality_gate_decision"],
            "auto_repair"
        );
        assert_eq!(
            result.selected_candidate["quality_metrics"]["candidate_selection"]["attempt_kind"],
            "initial_candidate"
        );
    }

    #[test]
    fn should_publish_chapter_candidate_generation_owner_contract() {
        let contract = build_chapter_candidate_generation_owner_contract();

        assert_eq!(contract["owner"], "chapter_candidate_generation_service");
        assert_eq!(
            contract["scope"],
            "candidate_pool_generation_workflow_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/services/chapter_candidate_generation_service.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_candidate_generation_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][1],
            "generate_candidate_pool_workflow"
        );
        assert_eq!(
            contract["behavior_contract"]["request_fields"][7],
            "runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["workflow_policy"][0],
            "max_candidates is normalized to at least one"
        );
        assert_eq!(
            contract["behavior_contract"]["workflow_policy"][7],
            "selected candidate comes from rerank owner or falls back to the last candidate"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_candidate_executor_default_dependency_service"
        );
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
            "build_default_generation_dependencies"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["candidate_pool_workflow_owner"],
            "generate_candidate_pool_workflow"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["runtime_state_sync_owner"],
            "sync_chapter_candidate_runtime_state"
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
            "rust_chapter_candidate_generation_owner_ready_for_source_map_closeout_review"
        );
    }
}
