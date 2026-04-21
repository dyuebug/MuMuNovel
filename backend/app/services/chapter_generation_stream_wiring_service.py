from __future__ import annotations

from typing import Any, Callable

from app.schemas.generation_payload import build_chapter_generation_stream_result_payload
from app.services.analysis_task_service import create_analysis_task_safely
from app.services.batch_generation_execution_service import build_batch_chapter_draft_attempt
from app.services.chapter_context_service import OneToManyContextBuilder, OneToOneContextBuilder
from app.services.chapter_generation_history_service import (
    _build_candidate_draft_payload,
    build_generation_history_payload,
)
from app.services.chapter_generation_runtime_prompt_service import (
    build_chapter_runtime_system_prompt,
    detect_style_profile,
    resolve_generation_temperature,
)
from app.services.chapter_generation_runtime_service import (
    build_chapter_generation_runtime_bundle,
    build_chapter_quality_runtime_context,
)
from app.services.chapter_generation_stream_models import (
    build_chapter_generation_stream_candidate_dependencies,
    build_chapter_generation_stream_dependencies,
    build_chapter_generation_stream_execution_dependencies,
    build_chapter_generation_stream_finalize_dependencies,
)
from app.services.chapter_generation_stream_request_policy_service import (
    _build_chapter_generation_request_options,
    _calculate_chapter_generation_max_tokens,
)
from app.services.foreshadow_service import foreshadow_service
from app.services.manual_chapter_analysis_execution_service import (
    execute_chapter_analysis_background,
)
from app.services.memory_service import memory_service
from app.services.outline_runtime_source_service import build_outline_structure_runtime_sources
from app.services.prompt_service import PromptService, WritingStyleManager
from app.services.story_quality_feedback_service import compute_story_quality_metrics
from app.services.story_repair_payload_service import (
    resolve_generation_story_repair_state_for_chapter,
    resolve_quality_gate_execution_plan,
)
from app.services.story_runtime_serialization_service import attach_story_runtime_contract


def _build_chapter_stream_draft_attempt(
    *,
    project_id: str,
    chapter_id: str,
    source: str,
    attempt_state: str,
    quality_gate_action: str | None,
    quality_gate_decision: str | None,
    full_content: str,
    summary_preview: str | None = None,
    quality_metrics: dict[str, Any] | None = None,
    repair_payload: dict[str, Any] | None = None,
    previous_content: str = '',
    previous_word_count: int = 0,
):
    normalized_repair_payload = dict(repair_payload or {}) if isinstance(repair_payload, dict) else {}
    normalized_repair_payload.setdefault('previous_content', previous_content)
    normalized_repair_payload.setdefault('previous_word_count', previous_word_count)
    return build_batch_chapter_draft_attempt(
        project_id=project_id,
        chapter_id=chapter_id,
        source=source,
        attempt_state=attempt_state,
        quality_gate_action=quality_gate_action,
        quality_gate_decision=quality_gate_decision,
        full_content=full_content,
        summary_preview=summary_preview,
        quality_metrics=quality_metrics,
        repair_payload=normalized_repair_payload,
    )


def build_default_chapter_generation_stream_dependencies(
    *,
    cancel_outline_postprocess_tasks_fn: Callable[..., Any],
    candidate_generator_fn: Callable[..., Any],
    candidate_rerank_limit: int,
    one_to_one_builder_cls: Any = OneToOneContextBuilder,
    one_to_many_builder_cls: Any = OneToManyContextBuilder,
    get_template_fn: Callable[..., Any] = PromptService.get_template,
    format_prompt_fn: Callable[..., str] = PromptService.format_prompt,
    apply_style_to_prompt_fn: Callable[..., str] = WritingStyleManager.apply_style_to_prompt,
    build_runtime_system_prompt_fn: Callable[..., Any] = build_chapter_runtime_system_prompt,
    detect_style_profile_fn: Callable[..., Any] = detect_style_profile,
    resolve_generation_temperature_fn: Callable[..., Any] = resolve_generation_temperature,
    compute_story_quality_metrics_fn: Callable[..., Any] = compute_story_quality_metrics,
    resolve_quality_gate_execution_plan_fn: Callable[..., Any] = resolve_quality_gate_execution_plan,
    analyze_chapter_background_fn: Callable[..., Any] = execute_chapter_analysis_background,
):
    execution_dependencies = build_chapter_generation_stream_execution_dependencies(
        resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_chapter,
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks_fn,
        memory_service=memory_service,
        foreshadow_service=foreshadow_service,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources,
        build_generation_runtime_bundle_fn=build_chapter_generation_runtime_bundle,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=_calculate_chapter_generation_max_tokens,
        build_request_options_fn=_build_chapter_generation_request_options,
        detect_style_profile_fn=detect_style_profile_fn,
        resolve_generation_temperature_fn=resolve_generation_temperature_fn,
    )
    candidate_dependencies = build_chapter_generation_stream_candidate_dependencies(
        build_quality_runtime_context_fn=build_chapter_quality_runtime_context,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_rerank_limit=candidate_rerank_limit,
        candidate_generator_fn=candidate_generator_fn,
        build_draft_attempt_fn=_build_chapter_stream_draft_attempt,
        attach_story_runtime_contract_fn=attach_story_runtime_contract,
    )
    finalize_dependencies = build_chapter_generation_stream_finalize_dependencies(
        foreshadow_service=foreshadow_service,
        build_generation_history_payload_fn=build_generation_history_payload,
        create_analysis_task_fn=create_analysis_task_safely,
        analyze_chapter_background_fn=analyze_chapter_background_fn,
        build_candidate_draft_payload_fn=_build_candidate_draft_payload,
        build_stream_result_payload_fn=build_chapter_generation_stream_result_payload,
    )
    return build_chapter_generation_stream_dependencies(
        execution=execution_dependencies,
        candidate=candidate_dependencies,
        finalize=finalize_dependencies,
    )
