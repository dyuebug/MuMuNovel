use std::sync::{Arc, Mutex};

use crate::ai::config::AIConfig;
use crate::ai::execution_trace::{
    AIExecutionFallbackKind, AIExecutionFallbackSummaryV1, AIExecutionTraceV1,
};
use crate::ai::service::AIService;
use crate::ai::types::AIRequestError;
use crate::models::chapter;
use crate::services::chapter_candidate_executor_production_adapter_service::{
    build_chapter_candidate_execution_trace_registry, build_chapter_candidate_quality_adapter,
    chapter_candidate_production_execution_path_name,
    generate_best_ranked_candidate_with_runtime_quality_adapters,
    generate_best_ranked_candidate_with_runtime_quality_adapters_tracked,
    take_chapter_candidate_execution_trace, ChapterCandidateProductionAdapterOutput,
    ChapterCandidateProductionFallbackContext, ChapterCandidateQualityAdapter,
    ChapterCandidateQualityAdapterContext,
};
use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
use crate::services::chapter_candidate_route_gateway_service::{
    execute_chapter_candidate_route_gateway_with_executor, ChapterCandidateRouteGatewayConfig,
};
use crate::services::chapter_generation_contract_prepare_service::build_chapter_repair_contract;
use crate::services::chapter_generation_prompt_service::{
    build_prompt_with_provider_payload, resolve_prompt_preference,
    ChapterGenerationPromptOverrides, PromptContextProviderPayload,
};
use crate::services::chapter_generation_runtime_service::{
    context_compaction_owner::compact_generation_context,
    single_generation_candidate_quality_owner, GeneratedChapterResult,
    SingleGenerationCandidateRuntimeExecutionContext,
};
use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};
use crate::services::chapter_single_generation_result_lifecycle_service::{
    apply_generated_result_lifecycle_view, apply_generated_result_quality_view,
    generated_result_lifecycle_view, generated_result_quality_view,
    single_generation_candidate_draft_lifecycle_view,
};
use crate::services::controlled_generation_guidance_service::append_controlled_generation_guidance;
use crate::services::generation_contract_service::{
    generation_contract_history_summary, generation_intent_to_legacy_value,
    story_packet_to_legacy_flat_value, GenerationContractHistorySummaryV1,
    GenerationContractSnapshotV1,
};
use serde_json::{json, Value};

const SINGLE_GENERATION_CANDIDATE_MAX_CANDIDATES: i64 = 1;

#[derive(Debug)]
pub(crate) enum ChapterCandidateRuntimeError {
    Provider(AIRequestError),
    Other(String),
}

impl std::fmt::Display for ChapterCandidateRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider(error) => error.fmt(formatter),
            Self::Other(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ChapterCandidateRuntimeError {}

impl From<String> for ChapterCandidateRuntimeError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

#[derive(Debug, Clone)]
struct SingleGenerationCandidateGatewayQualityContext {
    project_model: crate::models::project::Model,
    chapter_model: chapter::Model,
    previous_chapter_prompt_context:
        crate::services::chapter_generation_prompt_service::PreviousChapterPromptContext,
    story_packet: Value,
    generation_intent: Value,
    overrides: ChapterGenerationPromptOverrides,
}

pub(crate) fn build_single_generation_candidate_executor_request(
    prompt: &str,
    target_word_count: i32,
    ai_config: &AIConfig,
) -> ChapterCandidateExecutorRequest {
    ChapterCandidateExecutorRequest {
        base_generate_kwargs: serde_json::Map::from_iter([
            ("prompt".to_string(), json!(prompt)),
            ("temperature".to_string(), json!(ai_config.temperature)),
            ("max_tokens".to_string(), json!(ai_config.max_tokens)),
        ]),
        target_word_count: i64::from(target_word_count),
        source: "chapter".to_string(),
        generation_label: "single_generation_candidate".to_string(),
        max_candidates: SINGLE_GENERATION_CANDIDATE_MAX_CANDIDATES,
        runtime_state: None,
        repair_generation_contract: None,
    }
}

pub(crate) fn build_single_generation_direct_fallback_candidate_payload(
    content: String,
    context: ChapterCandidateProductionFallbackContext,
) -> Value {
    json!({
        "full_content": content,
        "generation_path": "direct_generation_fallback",
        "fallback_reason": context.reason,
        "rollback_boundary": context.rollback_boundary,
        "rust_error": context.rust_error,
    })
}

pub(crate) fn build_single_generation_candidate_gateway_metadata(
    output: &ChapterCandidateProductionAdapterOutput,
) -> Value {
    json!({
        "execution_path": chapter_candidate_production_execution_path_name(output.decision.path),
        "fallback_applied": output.fallback_applied,
        "fallback_reason": output.decision.reason,
        "rollback_boundary": output.decision.rollback_boundary,
        "rust_error": output.rust_error,
    })
}

pub(crate) fn single_generation_candidate_gateway_content(
    candidate: &Value,
) -> Result<String, String> {
    candidate
        .get("full_content")
        .or_else(|| candidate.get("content"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| "candidate route gateway returned empty generated content".to_string())
}

fn build_single_generation_candidate_quality_adapter(
    context: SingleGenerationCandidateGatewayQualityContext,
    target_word_count: i32,
) -> ChapterCandidateQualityAdapter<
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
    let project_payload = json!({
        "id": context.project_model.id,
        "title": context.project_model.title,
        "world_rules": context.project_model.world_rules,
        "outline_mode": context.project_model.outline_mode,
    });
    let chapter_payload = json!({
        "id": context.chapter_model.id,
        "title": context.chapter_model.title,
        "chapter_number": context.chapter_model.chapter_number,
        "summary": context.chapter_model.summary,
        "expansion_plan": context.chapter_model.expansion_plan,
    });
    let chapter_context = json!({
        "chapter_outline": context.chapter_model.expansion_plan
            .as_deref()
            .or(context.chapter_model.summary.as_deref())
            .unwrap_or(""),
        "previous_chapter_continuation_point": context
            .previous_chapter_prompt_context
            .continuation_point,
        "previous_chapter_content": context
            .previous_chapter_prompt_context
            .previous_chapter_content,
    });
    let creative_mode = resolve_prompt_preference(
        context.overrides.creative_mode.as_deref(),
        context.project_model.default_creative_mode.as_deref(),
    );
    let story_focus = resolve_prompt_preference(
        context.overrides.story_focus.as_deref(),
        context.project_model.default_story_focus.as_deref(),
    );
    let plot_stage = resolve_prompt_preference(
        context.overrides.plot_stage.as_deref(),
        context.project_model.default_plot_stage.as_deref(),
    );
    let story_creation_brief = resolve_prompt_preference(
        context.overrides.story_creation_brief.as_deref(),
        context
            .project_model
            .default_story_creation_brief
            .as_deref(),
    );
    let quality_preset = resolve_prompt_preference(
        context.overrides.quality_preset.as_deref(),
        context.project_model.default_quality_preset.as_deref(),
    );
    let quality_notes = resolve_prompt_preference(
        context.overrides.quality_notes.as_deref(),
        context.project_model.default_quality_notes.as_deref(),
    );
    let story_repair_summary = context
        .overrides
        .story_repair_summary
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string();
    let story_repair_targets = context
        .overrides
        .story_repair_targets
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let story_preserve_strengths = context
        .overrides
        .story_preserve_strengths
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    build_chapter_candidate_quality_adapter(
        ChapterCandidateQualityAdapterContext {
            story_packet: context.story_packet.clone(),
            project: project_payload,
            chapter: chapter_payload,
            chapter_context,
            target_word_count: i64::from(target_word_count),
            generation_intent: context.generation_intent.clone(),
            creative_mode,
            story_focus,
            plot_stage,
            story_creation_brief,
            quality_preset,
            quality_notes,
            chapter_count: context.project_model.chapter_count.map(i64::from),
            current_chapter_number: Some(i64::from(context.chapter_model.chapter_number)),
            retry_count: 0,
            max_retries: 1,
            story_repair_summary,
            story_repair_targets,
            story_preserve_strengths,
            current_story_repair_payload: None,
            scope: "chapter".to_string(),
            log_prefix: "SingleGeneration".to_string(),
        },
        single_generation_candidate_quality_owner::build_single_generation_quality_runtime_context,
        single_generation_candidate_quality_owner::compute_single_generation_story_quality_metrics,
        single_generation_candidate_quality_owner::resolve_single_generation_quality_gate_plan,
    )
}

async fn generate_single_generation_direct_fallback_candidate(
    ai_config: AIConfig,
    prompt: String,
    context: ChapterCandidateProductionFallbackContext,
    allow_model_fallback: Option<bool>,
) -> Result<(Value, Option<AIExecutionTraceV1>), ChapterCandidateRuntimeError> {
    let (content, execution) = if let Some(allow_model_fallback) = allow_model_fallback {
        let tracked = AIService::new(ai_config)
            .generate_text_tracked(&prompt, None, None, allow_model_fallback)
            .await
            .map_err(|error| ChapterCandidateRuntimeError::Provider(error.error))?;
        (tracked.response.content, Some(tracked.execution))
    } else {
        let response = AIService::new(ai_config)
            .generate_text(&prompt, None, None)
            .await
            .map_err(ChapterCandidateRuntimeError::Other)?;
        (response.content, None)
    };

    Ok((
        build_single_generation_direct_fallback_candidate_payload(content, context),
        execution,
    ))
}

pub(crate) fn build_single_generation_runtime_prompt(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
) -> Result<String, String> {
    build_single_generation_runtime_prompt_with_guidance(
        context,
        target_word_count,
        provider_payload,
        overrides,
        None,
    )
}

pub(crate) fn build_single_generation_runtime_prompt_with_guidance(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    additional_guidance: Option<&str>,
) -> Result<String, String> {
    let (provider_payload, previous_chapter_prompt_context) = compact_generation_context(
        &context.project_model.outline_mode,
        target_word_count,
        provider_payload,
        context.previous_chapter_prompt_context.clone(),
    );
    let prompt = build_prompt_with_provider_payload(
        &context.chapter_model,
        &context.project_model,
        previous_chapter_prompt_context,
        context.previous_chapter_exists,
        target_word_count,
        provider_payload,
        overrides,
    )?;
    Ok(append_controlled_generation_guidance(
        prompt,
        additional_guidance,
    ))
}

pub(crate) fn build_single_generation_runtime_generated_result_from_content(
    chapter_model: &chapter::Model,
    content: String,
) -> Result<GeneratedChapterResult, String> {
    let (cleaned_content, _) = sanitize_generated_narrative_text(&content);
    if cleaned_content.trim().is_empty() {
        return Err("chapter generation produced empty narrative after sanitization".to_string());
    }
    if contains_chapter_workflow_meta_text(&cleaned_content) {
        return Err("chapter generation produced workflow/meta text".to_string());
    }
    let word_count = cleaned_content.chars().count() as i32;

    Ok(GeneratedChapterResult {
        chapter_id: chapter_model.id.clone(),
        chapter_number: chapter_model.chapter_number,
        title: chapter_model.title.clone(),
        content: cleaned_content,
        word_count,
        saved_word_count: word_count,
        chapter_status: "completed".to_string(),
        content_applied: true,
        provisional_draft_saved: false,
        attempt_state: "applied".to_string(),
        quality_metrics: None,
        quality_gate_action: Some("continue".to_string()),
        quality_gate_message: None,
        candidate_draft: None,
        candidate_gateway_metadata: None,
        selected_candidate_event_source: None,
    })
}

pub(crate) fn build_single_generation_runtime_generated_result_from_candidate(
    chapter_model: &chapter::Model,
    candidate: &Value,
) -> Result<GeneratedChapterResult, String> {
    let content = single_generation_candidate_gateway_content(candidate)?;
    let mut result =
        build_single_generation_runtime_generated_result_from_content(chapter_model, content)?;
    result.selected_candidate_event_source = Some(candidate.clone());
    let quality_view = generated_result_quality_view(candidate);
    let lifecycle_view = generated_result_lifecycle_view(
        &chapter_model.status,
        quality_view.quality_gate_action.as_deref(),
        "candidate",
    );

    apply_generated_result_quality_view(&mut result, &quality_view);
    apply_generated_result_lifecycle_view(&mut result, &lifecycle_view);

    if !lifecycle_view.content_applied {
        let draft_lifecycle_view = single_generation_candidate_draft_lifecycle_view(
            chapter_model,
            &result,
            chapter_model.content.as_deref().unwrap_or_default(),
            chapter_model.word_count,
        );
        result.candidate_draft = Some(draft_lifecycle_view.candidate_draft_payload);
    }

    Ok(result)
}

fn build_single_generation_repair_contract_summary(
    snapshot: Option<&GenerationContractSnapshotV1>,
) -> Result<Option<GenerationContractHistorySummaryV1>, String> {
    snapshot
        .map(|snapshot| {
            let repair_contract = build_chapter_repair_contract(snapshot.story_packet.clone())?;
            generation_contract_history_summary(&repair_contract).map_err(|error| error.to_string())
        })
        .transpose()
}

fn resolve_single_generation_candidate_quality_contract(
    snapshot: Option<&GenerationContractSnapshotV1>,
    legacy_story_packet: &Value,
) -> (Value, Value) {
    snapshot
        .map(|snapshot| {
            (
                story_packet_to_legacy_flat_value(&snapshot.story_packet),
                generation_intent_to_legacy_value(&snapshot.generation_intent),
            )
        })
        .unwrap_or_else(|| {
            (
                legacy_story_packet.clone(),
                json!({"mode": "single_generation_active_route"}),
            )
        })
}

pub(crate) async fn execute_single_generation_candidate_runtime(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    ai_config: AIConfig,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<(String, GeneratedChapterResult), String> {
    execute_single_generation_candidate_runtime_with_guidance(
        context,
        ai_config,
        target_word_count,
        provider_payload,
        overrides,
        None,
        gateway_config,
    )
    .await
}

pub(crate) async fn execute_single_generation_candidate_runtime_with_guidance(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    ai_config: AIConfig,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    additional_guidance: Option<&str>,
    gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<(String, GeneratedChapterResult), String> {
    let (prompt, result, _) = execute_single_generation_candidate_runtime_internal(
        context,
        ai_config,
        target_word_count,
        provider_payload,
        overrides,
        additional_guidance,
        gateway_config,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok((prompt, result))
}

fn append_candidate_executor_fallback(
    execution: &mut AIExecutionTraceV1,
    fallback_applied: bool,
    rust_error: Option<&str>,
) {
    if fallback_applied && rust_error.is_some() {
        execution.fallbacks.push(AIExecutionFallbackSummaryV1 {
            kind: AIExecutionFallbackKind::CandidateExecutorFallback,
            reason: "candidate_executor_failed".to_string(),
        });
    }
}

fn winner_candidate_index_from_result(result: &Value) -> Result<i64, String> {
    result
        .get("candidate_index")
        .and_then(Value::as_i64)
        .filter(|candidate_index| *candidate_index > 0)
        .ok_or_else(|| {
            "tracked chapter candidate result is missing winner candidate index".to_string()
        })
}

pub(crate) async fn execute_single_generation_candidate_runtime_tracked(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    ai_config: AIConfig,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    allow_model_fallback: bool,
) -> Result<(String, GeneratedChapterResult, Option<AIExecutionTraceV1>), String> {
    execute_single_generation_candidate_runtime_tracked_with_guidance(
        context,
        ai_config,
        target_word_count,
        provider_payload,
        overrides,
        None,
        gateway_config,
        allow_model_fallback,
    )
    .await
}

pub(crate) async fn execute_single_generation_candidate_runtime_tracked_with_guidance(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    ai_config: AIConfig,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    additional_guidance: Option<&str>,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    allow_model_fallback: bool,
) -> Result<(String, GeneratedChapterResult, Option<AIExecutionTraceV1>), String> {
    execute_single_generation_candidate_runtime_tracked_with_guidance_typed(
        context,
        ai_config,
        target_word_count,
        provider_payload,
        overrides,
        additional_guidance,
        gateway_config,
        allow_model_fallback,
    )
    .await
    .map_err(|error| error.to_string())
}

pub(crate) async fn execute_single_generation_candidate_runtime_tracked_with_guidance_typed(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    ai_config: AIConfig,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    additional_guidance: Option<&str>,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    allow_model_fallback: bool,
) -> Result<
    (String, GeneratedChapterResult, Option<AIExecutionTraceV1>),
    ChapterCandidateRuntimeError,
> {
    execute_single_generation_candidate_runtime_internal(
        context,
        ai_config,
        target_word_count,
        provider_payload,
        overrides,
        additional_guidance,
        gateway_config,
        Some(allow_model_fallback),
    )
    .await
}

async fn execute_single_generation_candidate_runtime_internal(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    ai_config: AIConfig,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    additional_guidance: Option<&str>,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    allow_model_fallback: Option<bool>,
) -> Result<
    (String, GeneratedChapterResult, Option<AIExecutionTraceV1>),
    ChapterCandidateRuntimeError,
> {
    let prompt = build_single_generation_runtime_prompt_with_guidance(
        context,
        target_word_count,
        provider_payload,
        overrides,
        additional_guidance,
    )?;
    let mut request =
        build_single_generation_candidate_executor_request(&prompt, target_word_count, &ai_config);
    request.repair_generation_contract = build_single_generation_repair_contract_summary(
        context.generation_contract_snapshot.as_ref(),
    )?;
    let fallback_prompt = prompt.clone();
    let fallback_ai_config = ai_config.clone();
    let execution_trace = Arc::new(Mutex::new(None));
    let fallback_execution_trace = Arc::clone(&execution_trace);
    let candidate_execution_traces = build_chapter_candidate_execution_trace_registry();
    let rust_candidate_execution_traces = Arc::clone(&candidate_execution_traces);
    let provider_error_slot = Arc::new(Mutex::new(None));
    let rust_provider_error_slot = Arc::clone(&provider_error_slot);
    let fallback_provider_error_slot = Arc::clone(&provider_error_slot);

    let (quality_story_packet, quality_generation_intent) =
        resolve_single_generation_candidate_quality_contract(
            context.generation_contract_snapshot.as_ref(),
            &context.story_packet,
        );

    let output = execute_chapter_candidate_route_gateway_with_executor(
        &mut request,
        ai_config,
        build_single_generation_candidate_quality_adapter(
            SingleGenerationCandidateGatewayQualityContext {
                project_model: context.project_model.clone(),
                chapter_model: context.chapter_model.clone(),
                previous_chapter_prompt_context: context.previous_chapter_prompt_context.clone(),
                story_packet: quality_story_packet,
                generation_intent: quality_generation_intent,
                overrides: overrides.clone(),
            },
            target_word_count,
        ),
        gateway_config,
        move |request, ai_config, quality_adapter| {
            let candidate_execution_traces = Arc::clone(&rust_candidate_execution_traces);
            let provider_error_slot = Arc::clone(&rust_provider_error_slot);
            Box::pin(async move {
                if let Some(allow_model_fallback) = allow_model_fallback {
                    generate_best_ranked_candidate_with_runtime_quality_adapters_tracked(
                        request,
                        ai_config,
                        quality_adapter,
                        allow_model_fallback,
                        candidate_execution_traces,
                        provider_error_slot,
                    )
                    .await
                } else {
                    generate_best_ranked_candidate_with_runtime_quality_adapters(
                        request,
                        ai_config,
                        quality_adapter,
                    )
                    .await
                }
            })
        },
        move |_request, fallback_context| {
            Box::pin(async move {
                let (payload, execution) =
                    match generate_single_generation_direct_fallback_candidate(
                        fallback_ai_config,
                        fallback_prompt,
                        fallback_context,
                        allow_model_fallback,
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(ChapterCandidateRuntimeError::Provider(error)) => {
                            let message = error.to_string();
                            *fallback_provider_error_slot.lock().map_err(|_| {
                                "single generation provider error slot lock poisoned".to_string()
                            })? = Some(error);
                            return Err(message);
                        }
                        Err(ChapterCandidateRuntimeError::Other(message)) => {
                            return Err(message);
                        }
                    };
                if let Some(execution) = execution {
                    *fallback_execution_trace.lock().map_err(|_| {
                        "single generation execution trace lock poisoned".to_string()
                    })? = Some(execution);
                }
                Ok(payload)
            })
        },
    )
    .await;
    let output = match output {
        Ok(output) => output,
        Err(message) => {
            let provider_error = provider_error_slot
                .lock()
                .map_err(|_| {
                    ChapterCandidateRuntimeError::Other(
                        "single generation provider error slot lock poisoned".to_string(),
                    )
                })?
                .take();
            return Err(provider_error
                .map(ChapterCandidateRuntimeError::Provider)
                .unwrap_or(ChapterCandidateRuntimeError::Other(message)));
        }
    };

    let mut result = build_single_generation_runtime_generated_result_from_candidate(
        &context.chapter_model,
        &output.result,
    )?;
    result.candidate_gateway_metadata =
        Some(build_single_generation_candidate_gateway_metadata(&output));
    let mut execution = execution_trace
        .lock()
        .map_err(|_| "single generation execution trace lock poisoned".to_string())?
        .take();
    if execution.is_none() && allow_model_fallback.is_some() && !output.fallback_applied {
        let winner_candidate_index = winner_candidate_index_from_result(&output.result)?;
        execution = take_chapter_candidate_execution_trace(
            &candidate_execution_traces,
            winner_candidate_index,
        )?;
        if execution.is_none() {
            return Err(ChapterCandidateRuntimeError::Other(format!(
                "tracked chapter candidate winner execution trace is missing: candidate_index={winner_candidate_index}"
            )));
        }
    }
    if let Some(execution) = execution.as_mut() {
        append_candidate_executor_fallback(
            execution,
            output.fallback_applied,
            output.rust_error.as_deref(),
        );
    }

    Ok((prompt, result, execution))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{
        append_candidate_executor_fallback, build_single_generation_repair_contract_summary,
        resolve_single_generation_candidate_quality_contract, winner_candidate_index_from_result,
    };
    use crate::ai::execution_trace::{
        AIExecutionFallbackKind, AIExecutionFallbackSummaryV1, AIExecutionOutcome,
        AIExecutionTraceV1, AI_EXECUTION_TRACE_SCHEMA_VERSION,
    };
    use crate::services::generation_contract_service::{
        build_generation_contract_snapshot, GenerationIntentKind, GenerationIntentV1,
        GenerationTarget, StoryLedgerEntry, StoryPacketV1,
    };

    fn execution_trace() -> AIExecutionTraceV1 {
        AIExecutionTraceV1 {
            schema_version: AI_EXECUTION_TRACE_SCHEMA_VERSION.to_string(),
            requested_provider: "openai".to_string(),
            requested_model: "gpt-primary".to_string(),
            actual_provider: "openai".to_string(),
            actual_model: "gpt-primary".to_string(),
            outcome: AIExecutionOutcome::Succeeded,
            fallbacks: Vec::new(),
            endpoint_summary: None,
        }
    }

    #[test]
    fn should_resolve_winner_candidate_index_from_final_candidate_result() {
        assert_eq!(
            winner_candidate_index_from_result(&json!({"candidate_index": 4}))
                .expect("winner candidate index"),
            4
        );
    }

    #[test]
    fn should_reject_missing_or_non_positive_winner_candidate_index() {
        for result in [
            json!({}),
            json!({"candidate_index": 0}),
            json!({"candidate_index": "4"}),
        ] {
            assert_eq!(
                winner_candidate_index_from_result(&result).expect_err("invalid winner index"),
                "tracked chapter candidate result is missing winner candidate index"
            );
        }
    }

    #[test]
    fn should_not_classify_config_disabled_direct_path_as_candidate_executor_failure() {
        let mut execution = execution_trace();

        append_candidate_executor_fallback(&mut execution, true, None);

        assert!(execution.fallbacks.is_empty());
    }

    #[test]
    fn should_not_append_candidate_executor_fallback_after_rust_success() {
        let mut execution = execution_trace();

        append_candidate_executor_fallback(&mut execution, false, None);

        assert!(execution.fallbacks.is_empty());
    }

    #[test]
    fn should_append_stable_candidate_executor_fallback_without_raw_rust_error() {
        let mut execution = execution_trace();
        execution.fallbacks.push(AIExecutionFallbackSummaryV1 {
            kind: AIExecutionFallbackKind::ModelFallback,
            reason: "model_not_found".to_string(),
        });
        let raw_error = "executor failed with prompt body and https://secret.example/v1";

        append_candidate_executor_fallback(&mut execution, true, Some(raw_error));

        assert_eq!(execution.fallbacks.len(), 2);
        assert_eq!(
            execution.fallbacks[0].kind,
            AIExecutionFallbackKind::ModelFallback
        );
        assert_eq!(
            execution.fallbacks[1].kind,
            AIExecutionFallbackKind::CandidateExecutorFallback
        );
        assert_eq!(execution.fallbacks[1].reason, "candidate_executor_failed");
        assert!(!serde_json::to_string(&execution)
            .expect("serialize execution trace")
            .contains(raw_error));
    }

    #[test]
    fn should_project_typed_contract_for_candidate_quality_without_exposing_schema_wrapper() {
        let target = GenerationTarget::chapter("project-1", "chapter-1");
        let mut packet = StoryPacketV1::new("project-1", target.clone());
        packet.compatibility_metadata = BTreeMap::from([(
            "legacy_source".to_owned(),
            json!("single_generation_active_route"),
        )]);
        packet.continuity.character_state_ledger = vec![StoryLedgerEntry {
            entity_type: "character".to_owned(),
            entity_id: "character-1".to_owned(),
            opaque_state: json!({"label": "沈砚", "summary": "情绪收紧"}),
        }];
        let mut intent = GenerationIntentV1::new(GenerationIntentKind::ChapterGenerate, target);
        intent.compatibility_metadata = BTreeMap::from([(
            "legacy_mode".to_owned(),
            json!("single_generation_active_route"),
        )]);
        let snapshot = build_generation_contract_snapshot(packet, intent).expect("snapshot");
        let repair_summary = build_single_generation_repair_contract_summary(Some(&snapshot))
            .expect("repair summary")
            .expect("typed snapshot should create repair summary");

        assert_eq!(
            repair_summary.intent_kind,
            GenerationIntentKind::ChapterRepair
        );
        assert_eq!(repair_summary.target, snapshot.generation_intent.target);
        assert_eq!(repair_summary.sources, snapshot.story_packet.sources);
        assert_ne!(repair_summary.input_digest, snapshot.input_digest);

        let (story_packet, generation_intent) =
            resolve_single_generation_candidate_quality_contract(
                Some(&snapshot),
                &json!({"legacy": "ignored"}),
            );

        assert_eq!(story_packet["source"], "single_generation_active_route");
        assert_eq!(story_packet["project_id"], "project-1");
        assert_eq!(
            story_packet["character_state_ledger"],
            json!([{"label": "沈砚", "summary": "情绪收紧"}])
        );
        assert!(story_packet.get("schema_version").is_none());
        assert_eq!(
            generation_intent,
            json!({"mode": "single_generation_active_route"})
        );
    }

    #[test]
    fn should_keep_legacy_candidate_quality_fallback_without_typed_snapshot() {
        let legacy_story_packet = json!({
            "source": "single_generation_active_route",
            "project_id": "project-1",
            "custom_fact": {"value": 1}
        });

        let repair_summary = build_single_generation_repair_contract_summary(None)
            .expect("legacy snapshot absence should stay compatible");
        let (story_packet, generation_intent) =
            resolve_single_generation_candidate_quality_contract(None, &legacy_story_packet);

        assert!(repair_summary.is_none());
        assert_eq!(story_packet, legacy_story_packet);
        assert_eq!(
            generation_intent,
            json!({"mode": "single_generation_active_route"})
        );
    }
}
