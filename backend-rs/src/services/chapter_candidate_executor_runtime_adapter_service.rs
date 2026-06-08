// Staged runtime adapter for the Rust chapter candidate executor package.
// It replaces Python-side callback assembly for provider output and candidate
// records while keeping quality evaluation injectable until production cutover.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};

use crate::ai::config::AIConfig;
use crate::ai::service::AIService;
use crate::ai::types::ToolDef;
use crate::services::chapter_candidate_executor_default_dependency_service::{
    generate_best_ranked_candidate_with_default_dependency_wiring,
    ChapterCandidateDefaultOutputCollectInput, ChapterCandidateDefaultRecordBuildInput,
};
use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
use crate::services::chapter_candidate_output_service::{
    collect_generation_candidate_output, ChapterCandidateOutput, ChapterCandidateOutputRequest,
};
use crate::services::chapter_candidate_quality_adapter_service::{
    CandidateQualityGatePlanInput, CandidateQualityRuntimeContextBuildInput,
    CandidateStoryQualityMetricsInput, ChapterCandidateQualityAdapter,
};
use crate::services::chapter_candidate_record_service::{
    build_generation_candidate_record, ChapterCandidateRecordRequest,
};

#[derive(Debug, Clone)]
pub(crate) struct ChapterCandidateProviderStreamRequest {
    pub(crate) ai_config: AIConfig,
    pub(crate) prompt: String,
    pub(crate) system_prompt: Option<String>,
    pub(crate) tools: Option<Vec<ToolDef>>,
    pub(crate) candidate_index: i64,
    pub(crate) max_output_chars: Option<usize>,
}

pub(crate) async fn generate_best_ranked_candidate_with_runtime_quality_adapters<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_adapter: ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >,
) -> Result<Value, String>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
{
    let (quality_evaluator, quality_gate_plan_builder) =
        build_runtime_quality_adapter_callbacks(quality_adapter);
    generate_best_ranked_candidate_with_runtime_adapters(
        request,
        ai_config,
        quality_evaluator,
        quality_gate_plan_builder,
    )
    .await
}

pub(crate) fn build_runtime_quality_adapter_callbacks<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
>(
    quality_adapter: ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >,
) -> (
    impl FnMut(&str) -> Value + Send,
    impl FnMut(Value, i64) -> Value + Send,
)
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
{
    let quality_adapter = Arc::new(Mutex::new(quality_adapter));
    let evaluator_adapter = Arc::clone(&quality_adapter);
    let gate_builder_adapter = Arc::clone(&quality_adapter);

    (
        move |generated_content| {
            with_locked_callback(&evaluator_adapter, |adapter| {
                adapter.evaluate_quality(generated_content)
            })
        },
        move |metrics, attempt_offset| {
            with_locked_callback(&gate_builder_adapter, |adapter| {
                adapter.build_quality_gate_plan(metrics, attempt_offset)
            })
        },
    )
}

pub(crate) async fn generate_best_ranked_candidate_with_runtime_adapters<
    QualityEvaluator,
    QualityGatePlanBuilder,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_evaluator: QualityEvaluator,
    quality_gate_plan_builder: QualityGatePlanBuilder,
) -> Result<Value, String>
where
    QualityEvaluator: FnMut(&str) -> Value + Send + 'static,
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value + Send + 'static,
{
    let quality_evaluator = Arc::new(Mutex::new(quality_evaluator));
    let quality_gate_plan_builder = Arc::new(Mutex::new(quality_gate_plan_builder));
    let record_quality_evaluator = Arc::clone(&quality_evaluator);
    let record_quality_gate_plan_builder = Arc::clone(&quality_gate_plan_builder);
    let finalize_quality_gate_plan_builder = Arc::clone(&quality_gate_plan_builder);

    generate_best_ranked_candidate_with_default_dependency_wiring(
        request,
        move |input| {
            let ai_config = ai_config.clone();
            async move { collect_default_generation_candidate_output(ai_config, input).await }
        },
        move |input| {
            with_locked_callback(&record_quality_evaluator, |quality_evaluator| {
                with_locked_callback(
                    &record_quality_gate_plan_builder,
                    |quality_gate_plan_builder| {
                        build_default_generation_candidate_record(
                            input,
                            quality_evaluator,
                            quality_gate_plan_builder,
                        )
                    },
                )
            })
        },
        move |metrics, attempt_offset| {
            with_locked_callback(
                &finalize_quality_gate_plan_builder,
                |quality_gate_plan_builder| (quality_gate_plan_builder)(metrics, attempt_offset),
            )
        },
    )
    .await
}

pub(crate) async fn collect_default_generation_candidate_output(
    base_ai_config: AIConfig,
    input: ChapterCandidateDefaultOutputCollectInput,
) -> Result<ChapterCandidateOutput, String> {
    let provider_request =
        resolve_default_candidate_provider_stream_request(base_ai_config, input)?;
    collect_generation_candidate_output(
        ChapterCandidateOutputRequest {
            ai_service: AIService::new(provider_request.ai_config),
            prompt: provider_request.prompt,
            system_prompt: provider_request.system_prompt,
            tools: provider_request.tools,
            candidate_index: provider_request.candidate_index,
            max_output_chars: provider_request.max_output_chars,
            runtime_state: None,
        },
        |_chunk, _progress| async { Ok(()) },
    )
    .await
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

pub(crate) fn resolve_default_candidate_provider_stream_request(
    mut ai_config: AIConfig,
    input: ChapterCandidateDefaultOutputCollectInput,
) -> Result<ChapterCandidateProviderStreamRequest, String> {
    let generate_kwargs = input.generate_kwargs;
    apply_ai_config_overrides(&mut ai_config, &generate_kwargs)?;
    Ok(ChapterCandidateProviderStreamRequest {
        prompt: safe_string(generate_kwargs.get("prompt")).unwrap_or_default(),
        system_prompt: safe_string(generate_kwargs.get("system_prompt")),
        tools: parse_tools(generate_kwargs.get("tools"))?,
        ai_config,
        candidate_index: input.candidate_index,
        max_output_chars: input.max_output_chars.and_then(|value| {
            usize::try_from(value)
                .ok()
                .filter(|converted| *converted > 0)
        }),
    })
}

fn apply_ai_config_overrides(
    ai_config: &mut AIConfig,
    generate_kwargs: &Map<String, Value>,
) -> Result<(), String> {
    if let Some(temperature) = generate_kwargs.get("temperature").and_then(value_to_f64) {
        ai_config.temperature = temperature;
    }
    if let Some(max_tokens) = generate_kwargs.get("max_tokens") {
        let resolved = value_to_u32(max_tokens).ok_or_else(|| {
            "candidate provider max_tokens must be a positive integer".to_string()
        })?;
        ai_config.max_tokens = resolved;
    }
    Ok(())
}

fn parse_tools(value: Option<&Value>) -> Result<Option<Vec<ToolDef>>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value::<Vec<ToolDef>>(value.clone())
            .map(Some)
            .map_err(|error| format!("candidate provider tools payload is invalid: {error}")),
    }
}

fn safe_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.clone()),
        Some(value) if !value.is_null() => Some(value.to_string()),
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
}

fn value_to_u32(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .filter(|number| *number > 0)
        .or_else(|| {
            value
                .as_str()?
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|number| *number > 0)
        })
}

fn with_locked_callback<T, R>(callback: &Mutex<T>, invoke: impl FnOnce(&mut T) -> R) -> R {
    let mut guard = callback
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    invoke(&mut *guard)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::{json, Value};

    use super::{
        build_default_generation_candidate_record, build_runtime_quality_adapter_callbacks,
        resolve_default_candidate_provider_stream_request,
    };
    use crate::ai::config::AIConfig;
    use crate::services::chapter_candidate_executor_default_dependency_service::{
        ChapterCandidateDefaultOutputCollectInput, ChapterCandidateDefaultRecordBuildInput,
    };
    use crate::services::chapter_candidate_quality_adapter_service::{
        build_chapter_candidate_quality_adapter, CandidateQualityGatePlanInput,
        CandidateQualityRuntimeContextBuildInput, CandidateStoryQualityMetricsInput,
        ChapterCandidateQualityAdapterContext,
    };

    #[test]
    fn should_resolve_provider_stream_request_from_generate_kwargs() {
        let input = ChapterCandidateDefaultOutputCollectInput {
            generate_kwargs: serde_json::Map::from_iter([
                ("prompt".to_string(), json!("PROMPT")),
                ("system_prompt".to_string(), json!("SYSTEM")),
                ("temperature".to_string(), json!("0.42")),
                ("max_tokens".to_string(), json!(2048)),
                (
                    "tools".to_string(),
                    json!([{
                        "type": "function",
                        "function": {
                            "name": "lookup",
                            "description": "Lookup context",
                            "parameters": {"type": "object"}
                        }
                    }]),
                ),
            ]),
            candidate_index: 2,
            max_output_chars: Some(1800),
        };

        let request = resolve_default_candidate_provider_stream_request(AIConfig::default(), input)
            .expect("provider request");

        assert_eq!(request.prompt, "PROMPT");
        assert_eq!(request.system_prompt.as_deref(), Some("SYSTEM"));
        assert_eq!(request.ai_config.temperature, 0.42);
        assert_eq!(request.ai_config.max_tokens, 2048);
        assert_eq!(request.candidate_index, 2);
        assert_eq!(request.max_output_chars, Some(1800));
        assert_eq!(request.tools.expect("tools").len(), 1);
    }

    #[test]
    fn should_build_default_candidate_record_with_rust_record_owner() {
        let mut quality_evaluator = |_content: &str| {
            json!({
                "overall_score": 91.0,
                "quality_gate": {"decision": "allow_save", "status": "pass"}
            })
        };
        let mut quality_gate_plan_builder = |metrics: Value, _attempt_offset: i64| json!({"quality_gate": metrics["quality_gate"].clone()});

        let record = build_default_generation_candidate_record(
            ChapterCandidateDefaultRecordBuildInput {
                full_content: "候选正文推进冲突。".to_string(),
                candidate_chunks: vec!["候选正文推进冲突。".to_string()],
                target_word_count: 1200,
                source: "chapter".to_string(),
                generation_label: "candidate".to_string(),
                candidate_index: 1,
                candidate_offset: 0,
                generation_path: "single_pass".to_string(),
                attempt_kind: "initial_candidate".to_string(),
            },
            &mut quality_evaluator,
            &mut quality_gate_plan_builder,
        )
        .expect("candidate record");

        assert_eq!(record["candidate_index"], 1);
        assert_eq!(record["generation_path"], "single_pass");
        assert_eq!(
            record["quality_metrics"]["candidate_selection"]["attempt_kind"],
            "initial_candidate"
        );
    }

    #[test]
    fn should_propagate_record_owner_errors() {
        let mut quality_evaluator = |_content: &str| json!({"overall_score": 50.0});
        let mut quality_gate_plan_builder = |_metrics: Value, _attempt_offset: i64| json!({"quality_gate": {"decision": "allow_save"}});

        let error = build_default_generation_candidate_record(
            ChapterCandidateDefaultRecordBuildInput {
                full_content: String::new(),
                candidate_chunks: vec![],
                target_word_count: 1200,
                source: "chapter".to_string(),
                generation_label: "candidate".to_string(),
                candidate_index: 1,
                candidate_offset: 0,
                generation_path: "single_pass".to_string(),
                attempt_kind: "initial_candidate".to_string(),
            },
            &mut quality_evaluator,
            &mut quality_gate_plan_builder,
        )
        .expect_err("record owner should reject meta-only content");

        assert!(error.contains("empty narrative"));
    }

    #[test]
    fn should_bridge_quality_adapter_callbacks_for_runtime_executor() {
        let runtime_inputs = Arc::new(Mutex::new(
            Vec::<CandidateQualityRuntimeContextBuildInput>::new(),
        ));
        let metrics_inputs = Arc::new(Mutex::new(Vec::<CandidateStoryQualityMetricsInput>::new()));
        let gate_inputs = Arc::new(Mutex::new(Vec::<CandidateQualityGatePlanInput>::new()));
        let captured_runtime_inputs = Arc::clone(&runtime_inputs);
        let captured_metrics_inputs = Arc::clone(&metrics_inputs);
        let captured_gate_inputs = Arc::clone(&gate_inputs);
        let quality_adapter = build_chapter_candidate_quality_adapter(
            quality_adapter_context(),
            move |input| {
                captured_runtime_inputs.lock().unwrap().push(input);
                json!({"runtime_context": "built"})
            },
            move |input| {
                captured_metrics_inputs.lock().unwrap().push(input);
                json!({
                    "overall_score": 77.0,
                    "quality_gate": {"decision": "allow_save"}
                })
            },
            move |input| {
                captured_gate_inputs.lock().unwrap().push(input);
                json!({"action": "continue", "quality_gate": {"decision": "allow_save"}})
            },
        );
        let (mut quality_evaluator, mut quality_gate_plan_builder) =
            build_runtime_quality_adapter_callbacks(quality_adapter);

        let metrics = quality_evaluator("draft text");
        let plan = quality_gate_plan_builder(metrics, 2);
        drop(quality_evaluator);
        drop(quality_gate_plan_builder);

        assert_eq!(plan["action"], "continue");
        let runtime_inputs = runtime_inputs.lock().unwrap();
        let metrics_inputs = metrics_inputs.lock().unwrap();
        let gate_inputs = gate_inputs.lock().unwrap();
        assert_eq!(runtime_inputs.len(), 1);
        assert_eq!(metrics_inputs.len(), 1);
        assert_eq!(metrics_inputs[0].content, "draft text");
        assert_eq!(gate_inputs.len(), 1);
        assert_eq!(
            gate_inputs[0].candidate_metrics.as_ref().unwrap()["overall_score"],
            77.0
        );
        assert_eq!(gate_inputs[0].scope, "chapter");
    }

    fn quality_adapter_context() -> ChapterCandidateQualityAdapterContext {
        ChapterCandidateQualityAdapterContext {
            story_packet: json!({"packet": true}),
            project: json!({"world_rules": "rules"}),
            chapter: json!({"id": "chapter-1"}),
            chapter_context: json!({"chapter_outline": "outline"}),
            target_word_count: 1200,
            generation_intent: json!({"mode": "draft"}),
            retry_count: 0,
            max_retries: 1,
            current_story_repair_payload: None,
            scope: "chapter".to_string(),
            log_prefix: "Chapter".to_string(),
        }
    }
}
