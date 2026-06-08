from __future__ import annotations

from asyncio import Lock
from typing import Any, Callable, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.services.ai_service import AIService
from app.services.batch_generation_execution_service import (
    execute_batch_generation_generation_stage,
    execute_batch_generation_prompt_stage,
    resolve_batch_generation_chapter_runtime,
)
from app.services.batch_generation_single_chapter_service import (
    build_batch_generation_single_chapter_dependencies,
    build_batch_generation_single_chapter_request,
    generate_single_chapter_for_batch_workflow,
)
from app.services.chapter_context_service import OneToManyContextBuilder, OneToOneContextBuilder
from app.services.chapter_generation.runtime.prompt_service import (
    build_chapter_runtime_system_prompt,
    detect_style_profile,
    resolve_generation_temperature,
)
from app.services.chapter_generation.runtime.service import (
    build_chapter_generation_runtime_bundle,
    build_chapter_quality_runtime_context,
)
from app.services.chapter_generation.stream.request_policy_service import (
    _build_chapter_generation_request_options,
    _calculate_chapter_generation_max_tokens,
)
from app.services.chapter_quality_context_service import (
    StoryPacket,
    build_story_generation_packet_with_project_continuity,
    clone_chapter_quality_profile,
    resolve_chapter_quality_profile,
)
from app.services.chapter_web_research_service import (
    chapter_web_research_service as _chapter_web_research_service,
)
from app.services.foreshadow_service import foreshadow_service as _foreshadow_service
from app.services.memory_service import memory_service as _memory_service
from app.services.outline_runtime_source_service import build_outline_structure_runtime_sources
from app.services.prompt_service import PromptService, WritingStyleManager
from app.services.story_quality_feedback_service import compute_story_quality_metrics
from app.services.story_repair_payload_service import (
    StoryRepairPayload,
    resolve_quality_gate_execution_plan,
)
from app.services.story_runtime_serialization_service import attach_story_runtime_contract
from app.services.task_workflow_runtime_service import (
    publish_task_stream_event as _publish_task_stream_event,
)


def build_default_batch_generation_single_chapter_dependencies(
    *,
    candidate_generator_fn: Callable[..., Any],
    default_candidate_limit: int,
    heartbeat_interval_seconds: float,
    chapter_web_research_service: Any = _chapter_web_research_service,
    publish_task_stream_event_fn: Callable[..., Any] = _publish_task_stream_event,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    one_to_one_builder_cls: Any = OneToOneContextBuilder,
    one_to_many_builder_cls: Any = OneToManyContextBuilder,
    get_template_fn: Callable[..., Any] = PromptService.get_template,
    format_prompt_fn: Callable[..., Any] = PromptService.format_prompt,
    build_runtime_system_prompt_fn: Callable[..., Any] = build_chapter_runtime_system_prompt,
    compute_story_quality_metrics_fn: Callable[..., Any] = compute_story_quality_metrics,
    resolve_quality_gate_execution_plan_fn: Callable[..., Any] = resolve_quality_gate_execution_plan,
):
    return build_batch_generation_single_chapter_dependencies(
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
        resolve_batch_generation_chapter_runtime_fn=resolve_batch_generation_chapter_runtime,
        build_generation_runtime_bundle_fn=build_chapter_generation_runtime_bundle,
        build_story_packet_fn=build_story_generation_packet_with_project_continuity,
        clone_quality_profile_fn=clone_chapter_quality_profile,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources,
        execute_prompt_stage_fn=execute_batch_generation_prompt_stage,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=WritingStyleManager.apply_style_to_prompt,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=_calculate_chapter_generation_max_tokens,
        build_request_options_fn=_build_chapter_generation_request_options,
        detect_style_profile_fn=detect_style_profile,
        resolve_generation_temperature_fn=resolve_generation_temperature,
        execute_generation_stage_fn=execute_batch_generation_generation_stage,
        build_quality_runtime_context_fn=build_chapter_quality_runtime_context,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=candidate_generator_fn,
        attach_story_runtime_contract_fn=attach_story_runtime_contract,
        memory_service=_memory_service,
        foreshadow_service=_foreshadow_service,
        default_candidate_limit=default_candidate_limit,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
    )


async def generate_single_chapter_for_batch_with_default_wiring(
    *,
    db_session: AsyncSession,
    chapter: Chapter,
    user_id: str,
    style_id: Optional[int],
    target_word_count: int,
    ai_service: AIService,
    write_lock: Lock,
    story_packet: Optional[StoryPacket] = None,
    base_quality_profile: Optional[Dict[str, Any]] = None,
    custom_model: Optional[str] = None,
    previous_summary_context: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    enable_web_research: Optional[bool] = None,
    web_research_query: Optional[str] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    stream_task_id: Optional[str] = None,
    stream_chunks: bool = False,
    retry_count: int = 0,
    max_retries: int = 1,
    candidate_generator_fn: Callable[..., Any],
    default_candidate_limit: int,
    heartbeat_interval_seconds: float,
    chapter_web_research_service: Any = _chapter_web_research_service,
    publish_task_stream_event_fn: Callable[..., Any] = _publish_task_stream_event,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    one_to_one_builder_cls: Any = OneToOneContextBuilder,
    one_to_many_builder_cls: Any = OneToManyContextBuilder,
    get_template_fn: Callable[..., Any] = PromptService.get_template,
    format_prompt_fn: Callable[..., Any] = PromptService.format_prompt,
    build_runtime_system_prompt_fn: Callable[..., Any] = build_chapter_runtime_system_prompt,
    compute_story_quality_metrics_fn: Callable[..., Any] = compute_story_quality_metrics,
    resolve_quality_gate_execution_plan_fn: Callable[..., Any] = resolve_quality_gate_execution_plan,
) -> Dict[str, Any]:
    workflow_request = build_batch_generation_single_chapter_request(
        db_session=db_session,
        chapter=chapter,
        user_id=user_id,
        style_id=style_id,
        target_word_count=target_word_count,
        ai_service=ai_service,
        write_lock=write_lock,
        story_packet=story_packet,
        base_quality_profile=base_quality_profile,
        custom_model=custom_model,
        previous_summary_context=previous_summary_context,
        temp_narrative_perspective=temp_narrative_perspective,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        enable_web_research=enable_web_research,
        web_research_query=web_research_query,
        story_repair_summary=story_repair_summary,
        story_repair_targets=story_repair_targets,
        story_preserve_strengths=story_preserve_strengths,
        story_repair_payload=story_repair_payload,
        active_story_repair_snapshot=active_story_repair_snapshot,
        story_repair_state=story_repair_state,
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        retry_count=retry_count,
        max_retries=max_retries,
    )
    workflow_dependencies = build_default_batch_generation_single_chapter_dependencies(
        candidate_generator_fn=candidate_generator_fn,
        default_candidate_limit=default_candidate_limit,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
    )
    return await generate_single_chapter_for_batch_workflow(
        request=workflow_request,
        dependencies=workflow_dependencies,
    )
