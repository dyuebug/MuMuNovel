use crate::ai::config::AIConfig;
use crate::ai::service::AIService;
use crate::models::chapter;
use crate::services::chapter_candidate_executor_production_adapter_service::{
    build_chapter_candidate_quality_adapter, chapter_candidate_production_execution_path_name,
    ChapterCandidateProductionAdapterOutput, ChapterCandidateProductionFallbackContext,
    ChapterCandidateQualityAdapter, ChapterCandidateQualityAdapterContext,
};
use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
use crate::services::chapter_candidate_route_gateway_service::{
    execute_chapter_candidate_route_gateway, ChapterCandidateRouteGatewayConfig,
};
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
use serde_json::{json, Value};

const SINGLE_GENERATION_CANDIDATE_MAX_CANDIDATES: i64 = 1;

#[derive(Debug, Clone)]
struct SingleGenerationCandidateGatewayQualityContext {
    project_model: crate::models::project::Model,
    chapter_model: chapter::Model,
    previous_chapter_prompt_context:
        crate::services::chapter_generation_prompt_service::PreviousChapterPromptContext,
    story_packet: Value,
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
            generation_intent: json!({"mode": "single_generation_active_route"}),
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
) -> Result<Value, String> {
    let response = AIService::new(ai_config)
        .generate_text(&prompt, None, None)
        .await
        .map_err(|error| error.to_string())?;

    Ok(build_single_generation_direct_fallback_candidate_payload(
        response.content,
        context,
    ))
}

pub(crate) fn build_single_generation_runtime_prompt(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
) -> Result<String, String> {
    let (provider_payload, previous_chapter_prompt_context) = compact_generation_context(
        &context.project_model.outline_mode,
        target_word_count,
        provider_payload,
        context.previous_chapter_prompt_context.clone(),
    );
    build_prompt_with_provider_payload(
        &context.chapter_model,
        &context.project_model,
        previous_chapter_prompt_context,
        context.previous_chapter_exists,
        target_word_count,
        provider_payload,
        overrides,
    )
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

pub(crate) async fn execute_single_generation_candidate_runtime(
    context: &SingleGenerationCandidateRuntimeExecutionContext,
    ai_config: AIConfig,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<(String, GeneratedChapterResult), String> {
    let prompt = build_single_generation_runtime_prompt(
        context,
        target_word_count,
        provider_payload,
        overrides,
    )?;
    let mut request =
        build_single_generation_candidate_executor_request(&prompt, target_word_count, &ai_config);
    let fallback_prompt = prompt.clone();
    let fallback_ai_config = ai_config.clone();

    let output = execute_chapter_candidate_route_gateway(
        &mut request,
        ai_config,
        build_single_generation_candidate_quality_adapter(
            SingleGenerationCandidateGatewayQualityContext {
                project_model: context.project_model.clone(),
                chapter_model: context.chapter_model.clone(),
                previous_chapter_prompt_context: context.previous_chapter_prompt_context.clone(),
                story_packet: context.story_packet.clone(),
                overrides: overrides.clone(),
            },
            target_word_count,
        ),
        gateway_config,
        move |_request, fallback_context| {
            Box::pin(async move {
                generate_single_generation_direct_fallback_candidate(
                    fallback_ai_config,
                    fallback_prompt,
                    fallback_context,
                )
                .await
            })
        },
    )
    .await?;

    let mut result = build_single_generation_runtime_generated_result_from_candidate(
        &context.chapter_model,
        &output.result,
    )?;
    result.candidate_gateway_metadata =
        Some(build_single_generation_candidate_gateway_metadata(&output));

    Ok((prompt, result))
}
