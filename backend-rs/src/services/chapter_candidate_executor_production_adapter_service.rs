// Rust owner for the rollback-aware production adapter around the chapter
// candidate executor. This is the cutover boundary consumed by route and
// runtime code so Python-style provider, record, and quality closures stay
// behind an explicit fallback contract.

use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc, sync::Mutex};

use serde_json::{Map, Value};

use crate::ai::config::AIConfig;
use crate::ai::execution_trace::AIExecutionTraceV1;
use crate::ai::service::AIService;
use crate::ai::types::ToolDef;
use crate::services::chapter_candidate_executor_default_dependency_service::{
    build_default_generation_candidate_record,
    generate_best_ranked_candidate_with_default_dependency_wiring,
    ChapterCandidateDefaultOutputCollectInput,
};
use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
use crate::services::chapter_candidate_output_service::{
    collect_generation_candidate_output, collect_generation_candidate_output_tracked,
    ChapterCandidateOutput, ChapterCandidateOutputRequest,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateQualityAdapterContext {
    pub(crate) story_packet: Value,
    pub(crate) project: Value,
    pub(crate) chapter: Value,
    pub(crate) chapter_context: Value,
    pub(crate) target_word_count: i64,
    pub(crate) generation_intent: Value,
    pub(crate) creative_mode: String,
    pub(crate) story_focus: String,
    pub(crate) plot_stage: String,
    pub(crate) story_creation_brief: String,
    pub(crate) quality_preset: String,
    pub(crate) quality_notes: String,
    pub(crate) chapter_count: Option<i64>,
    pub(crate) current_chapter_number: Option<i64>,
    pub(crate) retry_count: i64,
    pub(crate) max_retries: i64,
    pub(crate) story_repair_summary: String,
    pub(crate) story_repair_targets: Vec<String>,
    pub(crate) story_preserve_strengths: Vec<String>,
    pub(crate) current_story_repair_payload: Option<Value>,
    pub(crate) scope: String,
    pub(crate) log_prefix: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateQualityRuntimeContextBuildInput {
    pub(crate) story_packet: Value,
    pub(crate) project: Value,
    pub(crate) chapter: Value,
    pub(crate) chapter_context: Value,
    pub(crate) target_word_count: i64,
    pub(crate) generation_intent: Value,
    pub(crate) creative_mode: String,
    pub(crate) story_focus: String,
    pub(crate) plot_stage: String,
    pub(crate) story_creation_brief: String,
    pub(crate) quality_preset: String,
    pub(crate) quality_notes: String,
    pub(crate) chapter_count: Option<i64>,
    pub(crate) current_chapter_number: Option<i64>,
    pub(crate) story_repair_summary: String,
    pub(crate) story_repair_targets: Vec<String>,
    pub(crate) story_preserve_strengths: Vec<String>,
    pub(crate) current_story_repair_payload: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateStoryQualityMetricsInput {
    pub(crate) content: String,
    pub(crate) chapter_outline: Value,
    pub(crate) world_rules: Value,
    pub(crate) quality_runtime_context: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateQualityGatePlanInput {
    pub(crate) candidate_metrics: Option<Value>,
    pub(crate) attempt_offset: i64,
    pub(crate) retry_count: i64,
    pub(crate) max_retries: i64,
    pub(crate) current_story_repair_payload: Option<Value>,
    pub(crate) scope: String,
}

pub(crate) struct ChapterCandidateQualityAdapter<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
> {
    context: ChapterCandidateQualityAdapterContext,
    build_quality_runtime_context_fn: BuildQualityRuntimeContext,
    compute_story_quality_metrics_fn: ComputeStoryQualityMetrics,
    resolve_quality_gate_execution_plan_fn: ResolveQualityGatePlan,
}

pub(crate) fn build_chapter_candidate_quality_adapter<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
>(
    context: ChapterCandidateQualityAdapterContext,
    build_quality_runtime_context_fn: BuildQualityRuntimeContext,
    compute_story_quality_metrics_fn: ComputeStoryQualityMetrics,
    resolve_quality_gate_execution_plan_fn: ResolveQualityGatePlan,
) -> ChapterCandidateQualityAdapter<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
>
where
    BuildQualityRuntimeContext: FnMut(CandidateQualityRuntimeContextBuildInput) -> Value,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value,
{
    ChapterCandidateQualityAdapter {
        context,
        build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn,
    }
}

impl<BuildQualityRuntimeContext, ComputeStoryQualityMetrics, ResolveQualityGatePlan>
    ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >
where
    BuildQualityRuntimeContext: FnMut(CandidateQualityRuntimeContextBuildInput) -> Value,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value,
{
    pub(crate) fn evaluate_quality(&mut self, generated_content: &str) -> Value {
        let quality_runtime_context =
            (self.build_quality_runtime_context_fn)(CandidateQualityRuntimeContextBuildInput {
                story_packet: self.context.story_packet.clone(),
                project: self.context.project.clone(),
                chapter: self.context.chapter.clone(),
                chapter_context: self.context.chapter_context.clone(),
                target_word_count: self.context.target_word_count,
                generation_intent: self.context.generation_intent.clone(),
                creative_mode: self.context.creative_mode.clone(),
                story_focus: self.context.story_focus.clone(),
                plot_stage: self.context.plot_stage.clone(),
                story_creation_brief: self.context.story_creation_brief.clone(),
                quality_preset: self.context.quality_preset.clone(),
                quality_notes: self.context.quality_notes.clone(),
                chapter_count: self.context.chapter_count,
                current_chapter_number: self.context.current_chapter_number,
                story_repair_summary: self.context.story_repair_summary.clone(),
                story_repair_targets: self.context.story_repair_targets.clone(),
                story_preserve_strengths: self.context.story_preserve_strengths.clone(),
                current_story_repair_payload: self.context.current_story_repair_payload.clone(),
            });

        (self.compute_story_quality_metrics_fn)(CandidateStoryQualityMetricsInput {
            content: generated_content.to_string(),
            chapter_outline: object_field(&self.context.chapter_context, "chapter_outline"),
            world_rules: object_field(&self.context.project, "world_rules"),
            quality_runtime_context,
        })
    }

    pub(crate) fn build_quality_gate_plan(
        &mut self,
        candidate_metrics: Value,
        attempt_offset: i64,
    ) -> Value {
        let candidate_metrics = candidate_metrics.is_object().then_some(candidate_metrics);
        (self.resolve_quality_gate_execution_plan_fn)(CandidateQualityGatePlanInput {
            candidate_metrics,
            attempt_offset,
            retry_count: self.context.retry_count,
            max_retries: self.context.max_retries,
            current_story_repair_payload: self.context.current_story_repair_payload.clone(),
            scope: self.context.scope.clone(),
        })
    }
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

pub(crate) fn with_locked_callback<T, R>(
    callback: &Mutex<T>,
    invoke: impl FnOnce(&mut T) -> R,
) -> R {
    let mut guard = callback
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    invoke(&mut *guard)
}

fn object_field(value: &Value, key: &str) -> Value {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .cloned()
        .unwrap_or(Value::Null)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateProductionAdapterConfig {
    pub(crate) rust_executor_enabled: bool,
    pub(crate) fallback_on_rust_error: bool,
    pub(crate) disabled_reason: Option<String>,
    pub(crate) rollback_boundary: String,
}

impl Default for ChapterCandidateProductionAdapterConfig {
    fn default() -> Self {
        Self {
            rust_executor_enabled: false,
            fallback_on_rust_error: true,
            disabled_reason: None,
            rollback_boundary: "python_candidate_executor_fallback".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChapterCandidateProductionExecutionPath {
    RustCandidateExecutor,
    PythonFallback,
}

pub(crate) fn chapter_candidate_production_execution_path_name(
    path: ChapterCandidateProductionExecutionPath,
) -> &'static str {
    match path {
        ChapterCandidateProductionExecutionPath::RustCandidateExecutor => "rust_candidate_executor",
        ChapterCandidateProductionExecutionPath::PythonFallback => "python_fallback",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateProductionAdapterDecision {
    pub(crate) path: ChapterCandidateProductionExecutionPath,
    pub(crate) reason: String,
    pub(crate) rollback_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateProductionFallbackContext {
    pub(crate) reason: String,
    pub(crate) rollback_boundary: String,
    pub(crate) rust_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateProductionAdapterOutput {
    pub(crate) result: Value,
    pub(crate) decision: ChapterCandidateProductionAdapterDecision,
    pub(crate) fallback_applied: bool,
    pub(crate) rust_error: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChapterCandidateProviderStreamRequest {
    pub(crate) ai_config: AIConfig,
    pub(crate) prompt: String,
    pub(crate) system_prompt: Option<String>,
    pub(crate) tools: Option<Vec<ToolDef>>,
    pub(crate) candidate_index: i64,
    pub(crate) max_output_chars: Option<usize>,
}

pub(crate) type ChapterCandidateExecutionTraceRegistry =
    Arc<Mutex<BTreeMap<i64, AIExecutionTraceV1>>>;

pub(crate) fn build_chapter_candidate_execution_trace_registry(
) -> ChapterCandidateExecutionTraceRegistry {
    Arc::new(Mutex::new(BTreeMap::new()))
}

pub(crate) fn take_chapter_candidate_execution_trace(
    registry: &ChapterCandidateExecutionTraceRegistry,
    candidate_index: i64,
) -> Result<Option<AIExecutionTraceV1>, String> {
    registry
        .lock()
        .map_err(|_| "chapter candidate execution trace registry lock poisoned".to_string())
        .map(|mut traces| traces.remove(&candidate_index))
}

pub(crate) fn resolve_chapter_candidate_production_adapter_decision(
    config: &ChapterCandidateProductionAdapterConfig,
) -> ChapterCandidateProductionAdapterDecision {
    if config.rust_executor_enabled {
        return ChapterCandidateProductionAdapterDecision {
            path: ChapterCandidateProductionExecutionPath::RustCandidateExecutor,
            reason: "rust candidate executor enabled by production adapter".to_string(),
            rollback_boundary: config.rollback_boundary.clone(),
        };
    }

    ChapterCandidateProductionAdapterDecision {
        path: ChapterCandidateProductionExecutionPath::PythonFallback,
        reason: config.disabled_reason.clone().unwrap_or_else(|| {
            "rust candidate executor disabled by production adapter".to_string()
        }),
        rollback_boundary: config.rollback_boundary.clone(),
    }
}

pub(crate) async fn collect_default_generation_candidate_output(
    base_ai_config: AIConfig,
    mut input: ChapterCandidateDefaultOutputCollectInput,
) -> Result<ChapterCandidateOutput, String> {
    let provider_request =
        resolve_default_candidate_provider_stream_request(base_ai_config, input.clone())?;
    collect_generation_candidate_output(
        ChapterCandidateOutputRequest {
            ai_service: AIService::new(provider_request.ai_config),
            prompt: provider_request.prompt,
            system_prompt: provider_request.system_prompt,
            tools: provider_request.tools,
            candidate_index: provider_request.candidate_index,
            max_output_chars: provider_request.max_output_chars,
            runtime_state: input.runtime_state.as_mut(),
        },
        |_chunk, _progress| async { Ok(()) },
    )
    .await
}

pub(crate) async fn collect_default_generation_candidate_output_tracked(
    base_ai_config: AIConfig,
    mut input: ChapterCandidateDefaultOutputCollectInput,
    allow_model_fallback: bool,
) -> Result<crate::services::chapter_candidate_output_service::TrackedChapterCandidateOutput, String>
{
    let provider_request =
        resolve_default_candidate_provider_stream_request(base_ai_config, input.clone())?;
    collect_generation_candidate_output_tracked(
        ChapterCandidateOutputRequest {
            ai_service: AIService::new(provider_request.ai_config),
            prompt: provider_request.prompt,
            system_prompt: provider_request.system_prompt,
            tools: provider_request.tools,
            candidate_index: provider_request.candidate_index,
            max_output_chars: provider_request.max_output_chars,
            runtime_state: input.runtime_state.as_mut(),
        },
        allow_model_fallback,
        |_chunk, _progress| async { Ok(()) },
    )
    .await
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
    if let Some(temperature) = generate_kwargs.get("temperature") {
        let resolved = value_to_finite_f64(temperature)
            .ok_or_else(|| "candidate provider temperature must be a finite number".to_string())?;
        ai_config.temperature = resolved;
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

fn value_to_finite_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        .filter(|number| number.is_finite())
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

pub(crate) async fn execute_chapter_candidate_production_adapter<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
    PythonFallback,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_adapter: ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >,
    config: ChapterCandidateProductionAdapterConfig,
    python_fallback_fn: PythonFallback,
) -> Result<ChapterCandidateProductionAdapterOutput, String>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
    PythonFallback: for<'request> FnOnce(
        &'request mut ChapterCandidateExecutorRequest,
        ChapterCandidateProductionFallbackContext,
    ) -> Pin<
        Box<dyn Future<Output = Result<Value, String>> + Send + 'request>,
    >,
{
    execute_chapter_candidate_production_adapter_with_executor(
        request,
        ai_config,
        quality_adapter,
        config,
        boxed_runtime_quality_adapter_executor::<
            BuildQualityRuntimeContext,
            ComputeStoryQualityMetrics,
            ResolveQualityGatePlan,
        >,
        python_fallback_fn,
    )
    .await
}

pub(crate) async fn execute_chapter_candidate_production_adapter_with_executor<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
    RustExecutor,
    PythonFallback,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_adapter: ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >,
    config: ChapterCandidateProductionAdapterConfig,
    rust_executor_fn: RustExecutor,
    python_fallback_fn: PythonFallback,
) -> Result<ChapterCandidateProductionAdapterOutput, String>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
    RustExecutor: for<'request> FnOnce(
        &'request mut ChapterCandidateExecutorRequest,
        AIConfig,
        ChapterCandidateQualityAdapter<
            BuildQualityRuntimeContext,
            ComputeStoryQualityMetrics,
            ResolveQualityGatePlan,
        >,
    ) -> Pin<
        Box<dyn Future<Output = Result<Value, String>> + Send + 'request>,
    >,
    PythonFallback: for<'request> FnOnce(
        &'request mut ChapterCandidateExecutorRequest,
        ChapterCandidateProductionFallbackContext,
    ) -> Pin<
        Box<dyn Future<Output = Result<Value, String>> + Send + 'request>,
    >,
{
    let decision = resolve_chapter_candidate_production_adapter_decision(&config);
    if decision.path == ChapterCandidateProductionExecutionPath::PythonFallback {
        let fallback_context = ChapterCandidateProductionFallbackContext {
            reason: decision.reason.clone(),
            rollback_boundary: decision.rollback_boundary.clone(),
            rust_error: None,
        };
        let result = python_fallback_fn(request, fallback_context).await?;
        return Ok(ChapterCandidateProductionAdapterOutput {
            result,
            decision,
            fallback_applied: true,
            rust_error: None,
        });
    }

    match rust_executor_fn(request, ai_config, quality_adapter).await {
        Ok(result) => Ok(ChapterCandidateProductionAdapterOutput {
            result,
            decision,
            fallback_applied: false,
            rust_error: None,
        }),
        Err(error) if config.fallback_on_rust_error => {
            let fallback_decision = ChapterCandidateProductionAdapterDecision {
                path: ChapterCandidateProductionExecutionPath::PythonFallback,
                reason: format!(
                    "rust candidate executor failed; python fallback selected: {error}"
                ),
                rollback_boundary: config.rollback_boundary,
            };
            let fallback_context = ChapterCandidateProductionFallbackContext {
                reason: fallback_decision.reason.clone(),
                rollback_boundary: fallback_decision.rollback_boundary.clone(),
                rust_error: Some(error.clone()),
            };
            let result = python_fallback_fn(request, fallback_context).await?;
            Ok(ChapterCandidateProductionAdapterOutput {
                result,
                decision: fallback_decision,
                fallback_applied: true,
                rust_error: Some(error),
            })
        }
        Err(error) => Err(error),
    }
}

fn boxed_runtime_quality_adapter_executor<
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
) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send + '_>>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
{
    Box::pin(
        generate_best_ranked_candidate_with_runtime_quality_adapters(
            request,
            ai_config,
            quality_adapter,
        ),
    )
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
    generate_best_ranked_candidate_with_runtime_adapters_and_collector(
        request,
        quality_evaluator,
        quality_gate_plan_builder,
        move |input| {
            let ai_config = ai_config.clone();
            async move { collect_default_generation_candidate_output(ai_config, input).await }
        },
    )
    .await
}

pub(crate) async fn generate_best_ranked_candidate_with_runtime_quality_adapters_tracked<
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
    allow_model_fallback: bool,
    execution_traces: ChapterCandidateExecutionTraceRegistry,
) -> Result<Value, String>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
{
    let (quality_evaluator, quality_gate_plan_builder) =
        build_runtime_quality_adapter_callbacks(quality_adapter);
    generate_best_ranked_candidate_with_runtime_adapters_tracked(
        request,
        ai_config,
        quality_evaluator,
        quality_gate_plan_builder,
        allow_model_fallback,
        execution_traces,
    )
    .await
}

pub(crate) async fn generate_best_ranked_candidate_with_runtime_adapters_tracked<
    QualityEvaluator,
    QualityGatePlanBuilder,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_evaluator: QualityEvaluator,
    quality_gate_plan_builder: QualityGatePlanBuilder,
    allow_model_fallback: bool,
    execution_traces: ChapterCandidateExecutionTraceRegistry,
) -> Result<Value, String>
where
    QualityEvaluator: FnMut(&str) -> Value + Send + 'static,
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value + Send + 'static,
{
    generate_best_ranked_candidate_with_runtime_adapters_and_collector(
        request,
        quality_evaluator,
        quality_gate_plan_builder,
        move |input| {
            let ai_config = ai_config.clone();
            let execution_traces = Arc::clone(&execution_traces);
            async move {
                let candidate_index = input.candidate_index;
                let tracked = collect_default_generation_candidate_output_tracked(
                    ai_config,
                    input,
                    allow_model_fallback,
                )
                .await?;
                execution_traces
                    .lock()
                    .map_err(|_| {
                        "chapter candidate execution trace registry lock poisoned".to_string()
                    })?
                    .insert(candidate_index, tracked.execution);
                Ok(tracked.output)
            }
        },
    )
    .await
}

async fn generate_best_ranked_candidate_with_runtime_adapters_and_collector<
    QualityEvaluator,
    QualityGatePlanBuilder,
    CollectOutput,
    CollectFuture,
>(
    request: &mut ChapterCandidateExecutorRequest,
    quality_evaluator: QualityEvaluator,
    quality_gate_plan_builder: QualityGatePlanBuilder,
    collect_output: CollectOutput,
) -> Result<Value, String>
where
    QualityEvaluator: FnMut(&str) -> Value + Send + 'static,
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value + Send + 'static,
    CollectOutput:
        FnMut(ChapterCandidateDefaultOutputCollectInput) -> CollectFuture + Send + 'static,
    CollectFuture: Future<Output = Result<ChapterCandidateOutput, String>> + Send + 'static,
{
    let quality_evaluator = Arc::new(Mutex::new(quality_evaluator));
    let quality_gate_plan_builder = Arc::new(Mutex::new(quality_gate_plan_builder));
    let record_quality_evaluator = Arc::clone(&quality_evaluator);
    let record_quality_gate_plan_builder = Arc::clone(&quality_gate_plan_builder);
    let finalize_quality_gate_plan_builder = Arc::clone(&quality_gate_plan_builder);

    generate_best_ranked_candidate_with_default_dependency_wiring(
        request,
        collect_output,
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

pub(crate) fn build_chapter_candidate_production_adapter_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_candidate_executor_production_adapter_service",
        "scope": "candidate_executor_production_cutover_and_rollback_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_record_service.rs",
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_route_gateway_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "resolve_chapter_candidate_production_adapter_decision",
                "execute_chapter_candidate_production_adapter",
                "execute_chapter_candidate_production_adapter_with_executor",
                "generate_best_ranked_candidate_with_runtime_quality_adapters",
                "generate_best_ranked_candidate_with_runtime_quality_adapters_tracked",
                "generate_best_ranked_candidate_with_runtime_adapters",
                "generate_best_ranked_candidate_with_runtime_adapters_tracked",
                "collect_default_generation_candidate_output",
                "collect_default_generation_candidate_output_tracked",
                "build_chapter_candidate_execution_trace_registry",
                "take_chapter_candidate_execution_trace",
                "resolve_default_candidate_provider_stream_request"
            ],
            "config_fields": [
                "rust_executor_enabled",
                "fallback_on_rust_error",
                "disabled_reason",
                "rollback_boundary"
            ],
            "decision_paths": [
                "rust_executor_enabled=true selects RustCandidateExecutor",
                "rust_executor_enabled=false selects PythonFallback without invoking Rust executor",
                "rust error with fallback_on_rust_error=true invokes PythonFallback with rust_error context",
                "rust error with fallback_on_rust_error=false propagates Rust error"
            ],
            "output_fields": [
                "result",
                "decision",
                "fallback_applied",
                "rust_error"
            ],
            "runtime_adapter_policy": [
                "quality adapter callbacks are converted to locked runtime callbacks",
                "provider request resolution and collection stay inside chapter_candidate_executor_production_adapter_service",
                "record building flows through default dependency record owner",
                "finalize quality gate plan callback is shared with default dependency wiring",
                "tracked runtime adapters register execution traces by candidate_index so consumers can select the final winner trace"
            ],
            "rollback_policy": [
                "default rollback boundary is python_candidate_executor_fallback",
                "disabled Rust cutover preserves configured disabled_reason",
                "fallback context carries reason, rollback_boundary, and optional rust_error",
                "python_fallback_removal_ready is true; remaining rollback semantics are governed by the explicit route-gateway rollback shell review"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_executor_production_adapter_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_route_gateway_service",
            "chapter_candidate_executor_default_dependency_service",
            "chapter_candidate_executor_production_adapter_service::quality_adapter_owner",
            "chapter_candidate_record_service",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter-candidate-route-gateway-smoke-rust"
        ],
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner",
                "phase5-chapters-candidate-gateway-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "chapter_candidate_gateway_manifest_probe_count": 1,
            "rust_manifest_probe_count": 18,
            "python_fallback_probe_count": 0,
            "decision_owner": "resolve_chapter_candidate_production_adapter_decision",
            "cutover_owner": "execute_chapter_candidate_production_adapter",
            "provider_request_owner": "resolve_default_candidate_provider_stream_request",
            "provider_collection_owner": "collect_default_generation_candidate_output",
            "quality_adapter_owner": "build_runtime_quality_adapter_callbacks",
            "runtime_executor_owner": "generate_best_ranked_candidate_with_runtime_quality_adapters",
            "rollback_knob": "ChapterCandidateProductionAdapterConfig",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "candidate executor production python source-map deleted; this owner is now Rust-only on the active path",
            "status": "rust_chapter_candidate_executor_production_adapter_owner_executor_source_map_deleted"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_executor_production_adapter_python_source_map",
            "runtime_rollback_knob": "ChapterCandidateProductionAdapterConfig",
            "python_fallback_removal_ready": true,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_execution_trace_registry,
        build_chapter_candidate_production_adapter_owner_contract,
        collect_default_generation_candidate_output,
        execute_chapter_candidate_production_adapter_with_executor,
        generate_best_ranked_candidate_with_runtime_adapters,
        generate_best_ranked_candidate_with_runtime_quality_adapters,
        resolve_chapter_candidate_production_adapter_decision,
        resolve_default_candidate_provider_stream_request, take_chapter_candidate_execution_trace,
        ChapterCandidateProductionAdapterConfig, ChapterCandidateProductionExecutionPath,
    };
    use crate::ai::config::AIConfig;
    use crate::ai::execution_trace::{
        AIExecutionOutcome, AIExecutionTraceV1, AI_EXECUTION_TRACE_SCHEMA_VERSION,
    };
    use crate::services::chapter_candidate_executor_default_dependency_service::ChapterCandidateDefaultOutputCollectInput;
    use crate::services::chapter_candidate_executor_production_adapter_service::{
        build_chapter_candidate_quality_adapter, ChapterCandidateQualityAdapterContext,
    };
    use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;

    fn execution_trace(actual_model: &str) -> AIExecutionTraceV1 {
        AIExecutionTraceV1 {
            schema_version: AI_EXECUTION_TRACE_SCHEMA_VERSION.to_string(),
            requested_provider: "openai".to_string(),
            requested_model: "gpt-primary".to_string(),
            actual_provider: "openai".to_string(),
            actual_model: actual_model.to_string(),
            outcome: AIExecutionOutcome::Succeeded,
            fallbacks: Vec::new(),
            endpoint_summary: None,
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
    fn should_resolve_python_fallback_when_rust_adapter_disabled() {
        let decision = resolve_chapter_candidate_production_adapter_decision(
            &ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: false,
                fallback_on_rust_error: true,
                disabled_reason: Some("cutover flag disabled".to_string()),
                rollback_boundary: "chapters.py candidate fallback".to_string(),
            },
        );

        assert_eq!(
            decision.path,
            ChapterCandidateProductionExecutionPath::PythonFallback
        );
        assert_eq!(decision.reason, "cutover flag disabled");
        assert_eq!(decision.rollback_boundary, "chapters.py candidate fallback");
    }

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
            runtime_state: None,
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
    fn should_resolve_string_max_tokens_and_temperature_overrides() {
        let input = ChapterCandidateDefaultOutputCollectInput {
            generate_kwargs: serde_json::Map::from_iter([
                ("temperature".to_string(), json!("0.66")),
                ("max_tokens".to_string(), json!("4096")),
            ]),
            candidate_index: 1,
            max_output_chars: None,
            runtime_state: None,
        };

        let request = resolve_default_candidate_provider_stream_request(AIConfig::default(), input)
            .expect("provider request");

        assert_eq!(request.ai_config.temperature, 0.66);
        assert_eq!(request.ai_config.max_tokens, 4096);
    }

    #[test]
    fn should_reject_non_finite_temperature_before_provider_call() {
        let input = ChapterCandidateDefaultOutputCollectInput {
            generate_kwargs: serde_json::Map::from_iter([(
                "temperature".to_string(),
                json!("NaN"),
            )]),
            candidate_index: 0,
            max_output_chars: None,
            runtime_state: None,
        };

        let error = resolve_default_candidate_provider_stream_request(AIConfig::default(), input)
            .expect_err("invalid temperature");

        assert_eq!(
            error,
            "candidate provider temperature must be a finite number"
        );
    }

    #[test]
    fn should_reject_invalid_max_tokens_before_provider_call() {
        let input = ChapterCandidateDefaultOutputCollectInput {
            generate_kwargs: serde_json::Map::from_iter([("max_tokens".to_string(), json!(0))]),
            candidate_index: 0,
            max_output_chars: None,
            runtime_state: None,
        };

        let error = resolve_default_candidate_provider_stream_request(AIConfig::default(), input)
            .expect_err("invalid max_tokens");

        assert_eq!(
            error,
            "candidate provider max_tokens must be a positive integer"
        );
    }

    #[test]
    fn should_reject_invalid_tools_payload_with_provider_error_prefix() {
        let input = ChapterCandidateDefaultOutputCollectInput {
            generate_kwargs: serde_json::Map::from_iter([("tools".to_string(), json!({}))]),
            candidate_index: 0,
            max_output_chars: None,
            runtime_state: None,
        };

        let error = resolve_default_candidate_provider_stream_request(AIConfig::default(), input)
            .expect_err("invalid tools");

        assert!(
            error.starts_with("candidate provider tools payload is invalid:"),
            "{error}"
        );
    }

    #[test]
    fn should_drop_non_positive_max_output_chars() {
        let input = ChapterCandidateDefaultOutputCollectInput {
            generate_kwargs: serde_json::Map::new(),
            candidate_index: 0,
            max_output_chars: Some(0),
            runtime_state: None,
        };

        let request = resolve_default_candidate_provider_stream_request(AIConfig::default(), input)
            .expect("provider request");

        assert_eq!(request.max_output_chars, None);
    }

    #[tokio::test]
    async fn should_execute_rust_candidate_executor_when_enabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_production_adapter_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: true,
                disabled_reason: None,
                rollback_boundary: "python fallback".to_string(),
            },
            |request, _ai_config, _quality_adapter| {
                Box::pin(async move {
                    request.runtime_state = Some(json!({"executor": "rust"}));
                    Ok(json!({"path": "rust", "content": "候选正文"}))
                })
            },
            |_request, _context| Box::pin(async { Ok(json!({"path": "python"})) }),
        )
        .await
        .expect("production adapter output");

        assert_eq!(
            output.decision.path,
            ChapterCandidateProductionExecutionPath::RustCandidateExecutor
        );
        assert!(!output.fallback_applied);
        assert_eq!(output.result["path"], "rust");
        assert_eq!(request.runtime_state, Some(json!({"executor": "rust"})));
    }

    #[tokio::test]
    async fn should_call_python_fallback_without_rust_execution_when_disabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_production_adapter_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: false,
                fallback_on_rust_error: true,
                disabled_reason: Some("smoke probe disabled rust cutover".to_string()),
                rollback_boundary: "compat service fallback".to_string(),
            },
            |_request, _ai_config, _quality_adapter| {
                Box::pin(async { Err("rust executor should not run".to_string()) })
            },
            |request, context| {
                Box::pin(async move {
                    request.runtime_state = Some(json!({"executor": "python"}));
                    Ok(json!({
                        "path": "python",
                        "fallback_reason": context.reason,
                        "rollback_boundary": context.rollback_boundary,
                    }))
                })
            },
        )
        .await
        .expect("fallback output");

        assert!(output.fallback_applied);
        assert_eq!(
            output.decision.path,
            ChapterCandidateProductionExecutionPath::PythonFallback
        );
        assert_eq!(output.result["path"], "python");
        assert_eq!(
            output.result["fallback_reason"],
            "smoke probe disabled rust cutover"
        );
        assert_eq!(request.runtime_state, Some(json!({"executor": "python"})));
    }

    #[tokio::test]
    async fn should_fallback_to_python_when_rust_executor_fails_and_rollback_enabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_production_adapter_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: true,
                disabled_reason: None,
                rollback_boundary: "route candidate fallback".to_string(),
            },
            |_request, _ai_config, _quality_adapter| {
                Box::pin(async { Err("provider timeout".to_string()) })
            },
            |_request, context| {
                Box::pin(async move {
                    Ok(json!({
                        "path": "python",
                        "rust_error": context.rust_error,
                        "fallback_reason": context.reason,
                    }))
                })
            },
        )
        .await
        .expect("fallback output");

        assert!(output.fallback_applied);
        assert_eq!(output.rust_error.as_deref(), Some("provider timeout"));
        assert_eq!(
            output.decision.path,
            ChapterCandidateProductionExecutionPath::PythonFallback
        );
        assert_eq!(output.result["rust_error"], "provider timeout");
        assert!(output.result["fallback_reason"]
            .as_str()
            .unwrap()
            .contains("python fallback selected"));
    }

    #[tokio::test]
    async fn should_propagate_rust_error_when_rollback_is_disabled() {
        let mut request = executor_request();

        let error = execute_chapter_candidate_production_adapter_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: false,
                disabled_reason: None,
                rollback_boundary: "no fallback".to_string(),
            },
            |_request, _ai_config, _quality_adapter| {
                Box::pin(async { Err("record build failed".to_string()) })
            },
            |_request, _context| Box::pin(async { Ok(json!({"path": "python"})) }),
        )
        .await
        .expect_err("rust error should be propagated");

        assert_eq!(error, "record build failed");
    }

    #[tokio::test]
    async fn should_route_runtime_quality_bridge_through_provider_request_owner() {
        let mut request = executor_request();
        request.base_generate_kwargs = Map::from_iter([
            ("prompt".to_string(), json!("PROMPT")),
            ("max_tokens".to_string(), json!(0)),
        ]);
        request.target_word_count = 1200;
        request.max_candidates = 1;

        let error = generate_best_ranked_candidate_with_runtime_quality_adapters(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
        )
        .await
        .expect_err("invalid provider request should stop before network");

        assert_eq!(
            error,
            "candidate provider max_tokens must be a positive integer"
        );
    }

    #[tokio::test]
    async fn should_stop_default_provider_collection_before_network_on_invalid_request() {
        let error = collect_default_generation_candidate_output(
            AIConfig::default(),
            ChapterCandidateDefaultOutputCollectInput {
                generate_kwargs: Map::from_iter([("max_tokens".to_string(), json!(0))]),
                candidate_index: 1,
                max_output_chars: None,
                runtime_state: None,
            },
        )
        .await
        .expect_err("invalid provider request should stop before network");

        assert_eq!(
            error,
            "candidate provider max_tokens must be a positive integer"
        );
    }

    #[tokio::test]
    async fn should_route_runtime_adapters_through_provider_request_owner() {
        let mut request = executor_request();
        request.base_generate_kwargs = Map::from_iter([
            ("prompt".to_string(), json!("PROMPT")),
            ("max_tokens".to_string(), json!(0)),
        ]);
        request.target_word_count = 1200;
        request.max_candidates = 1;

        let error = generate_best_ranked_candidate_with_runtime_adapters(
            &mut request,
            AIConfig::default(),
            |_content| json!({"overall_score": 88.0}),
            |metrics, attempt_offset| {
                json!({
                    "quality_gate": {
                        "decision": "passed",
                        "attempt_offset": attempt_offset,
                        "score": metrics["overall_score"],
                    }
                })
            },
        )
        .await
        .expect_err("invalid provider request should stop before network");

        assert_eq!(
            error,
            "candidate provider max_tokens must be a positive integer"
        );
    }

    fn executor_request() -> ChapterCandidateExecutorRequest {
        ChapterCandidateExecutorRequest {
            base_generate_kwargs: Map::from_iter([("prompt".to_string(), json!("PROMPT"))]),
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "candidate".to_string(),
            max_candidates: 1,
            runtime_state: None,
            repair_generation_contract: None,
        }
    }

    fn quality_adapter(
    ) -> crate::services::chapter_candidate_executor_production_adapter_service::ChapterCandidateQualityAdapter<
        impl FnMut(
            crate::services::chapter_candidate_executor_production_adapter_service::CandidateQualityRuntimeContextBuildInput,
        ) -> Value,
        impl FnMut(
            crate::services::chapter_candidate_executor_production_adapter_service::CandidateStoryQualityMetricsInput,
        ) -> Value,
        impl FnMut(
            crate::services::chapter_candidate_executor_production_adapter_service::CandidateQualityGatePlanInput,
        ) -> Value,
    >{
        build_chapter_candidate_quality_adapter(
            ChapterCandidateQualityAdapterContext {
                story_packet: json!({"packet": true}),
                project: json!({"world_rules": "rules"}),
                chapter: json!({"id": "chapter-1"}),
                chapter_context: json!({"chapter_outline": "outline"}),
                target_word_count: 1200,
                generation_intent: json!({"mode": "draft"}),
                creative_mode: String::new(),
                story_focus: String::new(),
                plot_stage: String::new(),
                story_creation_brief: String::new(),
                quality_preset: String::new(),
                quality_notes: String::new(),
                chapter_count: None,
                current_chapter_number: None,
                retry_count: 0,
                max_retries: 1,
                story_repair_summary: String::new(),
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
                current_story_repair_payload: None,
                scope: "chapter".to_string(),
                log_prefix: "Chapter".to_string(),
            },
            |_input| json!({"runtime": "context"}),
            |_input| json!({"overall_score": 90.0}),
            |_input| json!({"action": "continue"}),
        )
    }

    #[test]
    fn should_publish_chapter_candidate_production_adapter_owner_contract() {
        let contract = build_chapter_candidate_production_adapter_owner_contract();
        assert_no_deleted_python_service_source_map(&contract);
        let python_source_map = contract["python_source_map"]
            .as_array()
            .expect("python source map");

        assert_eq!(
            contract["owner"],
            "chapter_candidate_executor_production_adapter_service"
        );
        assert_eq!(
            contract["scope"],
            "candidate_executor_production_cutover_and_rollback_owner"
        );
        assert_eq!(python_source_map.len(), 0);
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][2],
            "execute_chapter_candidate_production_adapter_with_executor"
        );
        assert_eq!(
            contract["behavior_contract"]["decision_paths"][3],
            "rust error with fallback_on_rust_error=false propagates Rust error"
        );
        let entrypoints = contract["behavior_contract"]["entrypoints"]
            .as_array()
            .expect("entrypoints");
        assert!(entrypoints
            .iter()
            .any(|entrypoint| entrypoint == "collect_default_generation_candidate_output"));
        assert!(entrypoints.iter().any(|entrypoint| {
            entrypoint == "collect_default_generation_candidate_output_tracked"
        }));
        assert_eq!(
            contract["behavior_contract"]["runtime_adapter_policy"][1],
            "provider request resolution and collection stay inside chapter_candidate_executor_production_adapter_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_rollback_knob"],
            "ChapterCandidateProductionAdapterConfig"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            6
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["chapter_candidate_gateway_manifest_probe_count"],
            1
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
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_executor_production_adapter_owner_executor_source_map_deleted"
        );
    }
    #[test]
    fn should_take_only_winner_candidate_execution_trace_from_registry() {
        let registry = build_chapter_candidate_execution_trace_registry();
        {
            let mut traces = registry.lock().expect("trace registry");
            traces.insert(1, execution_trace("gpt-candidate-1"));
            traces.insert(3, execution_trace("gpt-repair-winner"));
        }

        let winner = take_chapter_candidate_execution_trace(&registry, 3)
            .expect("take winner trace")
            .expect("winner trace");

        assert_eq!(winner.actual_model, "gpt-repair-winner");
        assert!(registry.lock().expect("trace registry").contains_key(&1));
        assert!(take_chapter_candidate_execution_trace(&registry, 3)
            .expect("take removed winner")
            .is_none());
    }
}
