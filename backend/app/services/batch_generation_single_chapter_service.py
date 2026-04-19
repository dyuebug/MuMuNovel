"""批量生成单章 service。"""
from __future__ import annotations

from asyncio import Lock
from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.outline import Outline
from app.models.project import Project
from app.services.ai_service import AIService
from app.services.chapter_quality_context_service import StoryPacket
from app.services.story_repair_payload_service import StoryRepairPayload

logger = get_logger(__name__)


@dataclass(slots=True)
class BatchGenerationSingleChapterRequest:
    db_session: AsyncSession
    chapter: Chapter
    user_id: str
    style_id: Optional[int]
    target_word_count: int
    ai_service: AIService
    write_lock: Lock
    story_packet: Optional[StoryPacket] = None
    base_quality_profile: Optional[Dict[str, Any]] = None
    custom_model: Optional[str] = None
    previous_summary_context: Optional[str] = None
    temp_narrative_perspective: Optional[str] = None
    creative_mode: Optional[str] = None
    story_focus: Optional[str] = None
    plot_stage: Optional[str] = None
    story_creation_brief: Optional[str] = None
    quality_preset: Optional[str] = None
    quality_notes: Optional[str] = None
    enable_web_research: Optional[bool] = None
    web_research_query: Optional[str] = None
    story_repair_summary: Optional[str] = None
    story_repair_targets: Optional[list[str]] = None
    story_preserve_strengths: Optional[list[str]] = None
    story_repair_payload: Optional[StoryRepairPayload] = None
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None
    story_repair_state: Optional[Dict[str, Any]] = None
    stream_task_id: Optional[str] = None
    stream_chunks: bool = False
    retry_count: int = 0
    max_retries: int = 1


@dataclass(slots=True)
class BatchGenerationSingleChapterDependencies:
    chapter_web_research_service: Any
    publish_task_stream_event_fn: Callable[..., Any]
    resolve_batch_generation_chapter_runtime_fn: Callable[..., Any]
    build_generation_runtime_bundle_fn: Callable[..., Any]
    build_story_packet_fn: Callable[..., Any]
    clone_quality_profile_fn: Callable[..., Any]
    resolve_quality_profile_fn: Callable[..., Any]
    one_to_one_builder_cls: Any
    one_to_many_builder_cls: Any
    build_outline_structure_runtime_sources_fn: Callable[..., Any]
    execute_prompt_stage_fn: Callable[..., Any]
    get_template_fn: Callable[..., Any]
    format_prompt_fn: Callable[..., Any]
    apply_style_to_prompt_fn: Callable[..., Any]
    build_runtime_system_prompt_fn: Callable[..., Any]
    calculate_max_tokens_fn: Callable[..., Any]
    build_request_options_fn: Callable[..., Any]
    detect_style_profile_fn: Callable[..., Any]
    resolve_generation_temperature_fn: Callable[..., Any]
    execute_generation_stage_fn: Callable[..., Any]
    build_quality_runtime_context_fn: Callable[..., Any]
    compute_story_quality_metrics_fn: Callable[..., Any]
    resolve_quality_gate_execution_plan_fn: Callable[..., Any]
    candidate_generator_fn: Callable[..., Any]
    attach_story_runtime_contract_fn: Callable[..., Any]
    memory_service: Any
    foreshadow_service: Any
    default_candidate_limit: int
    heartbeat_interval_seconds: float


def build_batch_generation_single_chapter_request(
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
) -> BatchGenerationSingleChapterRequest:
    return BatchGenerationSingleChapterRequest(
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


def build_batch_generation_single_chapter_dependencies(
    *,
    chapter_web_research_service: Any,
    publish_task_stream_event_fn: Callable[..., Any],
    resolve_batch_generation_chapter_runtime_fn: Callable[..., Any],
    build_generation_runtime_bundle_fn: Callable[..., Any],
    build_story_packet_fn: Callable[..., Any],
    clone_quality_profile_fn: Callable[..., Any],
    resolve_quality_profile_fn: Callable[..., Any],
    one_to_one_builder_cls: Any,
    one_to_many_builder_cls: Any,
    build_outline_structure_runtime_sources_fn: Callable[..., Any],
    execute_prompt_stage_fn: Callable[..., Any],
    get_template_fn: Callable[..., Any],
    format_prompt_fn: Callable[..., Any],
    apply_style_to_prompt_fn: Callable[..., Any],
    build_runtime_system_prompt_fn: Callable[..., Any],
    calculate_max_tokens_fn: Callable[..., Any],
    build_request_options_fn: Callable[..., Any],
    detect_style_profile_fn: Callable[..., Any],
    resolve_generation_temperature_fn: Callable[..., Any],
    execute_generation_stage_fn: Callable[..., Any],
    build_quality_runtime_context_fn: Callable[..., Any],
    compute_story_quality_metrics_fn: Callable[..., Any],
    resolve_quality_gate_execution_plan_fn: Callable[..., Any],
    candidate_generator_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[..., Any],
    memory_service: Any,
    foreshadow_service: Any,
    default_candidate_limit: int,
    heartbeat_interval_seconds: float,
) -> BatchGenerationSingleChapterDependencies:
    return BatchGenerationSingleChapterDependencies(
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
        resolve_batch_generation_chapter_runtime_fn=resolve_batch_generation_chapter_runtime_fn,
        build_generation_runtime_bundle_fn=build_generation_runtime_bundle_fn,
        build_story_packet_fn=build_story_packet_fn,
        clone_quality_profile_fn=clone_quality_profile_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources_fn,
        execute_prompt_stage_fn=execute_prompt_stage_fn,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=calculate_max_tokens_fn,
        build_request_options_fn=build_request_options_fn,
        detect_style_profile_fn=detect_style_profile_fn,
        resolve_generation_temperature_fn=resolve_generation_temperature_fn,
        execute_generation_stage_fn=execute_generation_stage_fn,
        build_quality_runtime_context_fn=build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=candidate_generator_fn,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
        memory_service=memory_service,
        foreshadow_service=foreshadow_service,
        default_candidate_limit=default_candidate_limit,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
    )


async def _load_batch_generation_project_and_outline(
    request: BatchGenerationSingleChapterRequest,
) -> tuple[Project, Optional[Outline], str]:
    project_result = await request.db_session.execute(
        select(Project).where(Project.id == request.chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if not project:
        raise Exception("项目不存在")

    outline_mode = project.outline_mode if project else "one-to-many"
    logger.info(f"批量生成单章 - 大纲模式: {outline_mode}")

    if request.chapter.outline_id:
        outline_result = await request.db_session.execute(
            select(Outline).where(Outline.id == request.chapter.outline_id)
        )
    else:
        outline_result = await request.db_session.execute(
            select(Outline)
            .where(Outline.project_id == request.chapter.project_id)
            .where(Outline.order_index == request.chapter.chapter_number)
        )

    return project, outline_result.scalar_one_or_none(), outline_mode


async def _collect_batch_generation_research_assets(
    request: BatchGenerationSingleChapterRequest,
    dependencies: BatchGenerationSingleChapterDependencies,
    *,
    project: Project,
    outline: Optional[Outline],
) -> List[Dict[str, str]]:
    if not dependencies.chapter_web_research_service.is_enabled(request.enable_web_research):
        return []

    if request.stream_task_id:
        await dependencies.publish_task_stream_event_fn(
            request.stream_task_id,
            {
                "type": "progress",
                "message": f"第{request.chapter.chapter_number}章正在联网检索",
                "progress": 18,
                "status": "running",
                "phase": "researching",
            },
            db_session=request.db_session,
        )

    research_bundle = await dependencies.chapter_web_research_service.collect_for_chapter(
        project=project,
        chapter=request.chapter,
        outline=outline,
        story_creation_brief=request.story_creation_brief,
        enable_web_research=request.enable_web_research,
        web_research_query=request.web_research_query,
    )
    research_assets = list(research_bundle.get("assets") or [])
    research_query = str(research_bundle.get("query") or "")
    if not research_assets:
        return research_assets

    async with request.write_lock:
        saved_memory_ids = await dependencies.chapter_web_research_service.replace_chapter_memories(
            db_session=request.db_session,
            user_id=request.user_id,
            project=project,
            chapter=request.chapter,
            query=research_query,
            archive_path=str(research_bundle.get("archive_path") or ""),
            assets=research_assets,
        )

    logger.info(
        "联网检索 - 第%s章获得 %s 条资料，归档 %s 条记忆",
        request.chapter.chapter_number,
        len(research_assets),
        len(saved_memory_ids),
    )
    if request.stream_task_id:
        await dependencies.publish_task_stream_event_fn(
            request.stream_task_id,
            {
                "type": "progress",
                "message": f"第{request.chapter.chapter_number}章已检索到 {len(research_assets)} 条资料",
                "progress": 22,
                "status": "running",
                "phase": "researching",
            },
            db_session=request.db_session,
        )

    return research_assets


async def generate_single_chapter_for_batch_workflow(
    *,
    request: BatchGenerationSingleChapterRequest,
    dependencies: BatchGenerationSingleChapterDependencies,
) -> Dict[str, Any]:
    project, outline, outline_mode = await _load_batch_generation_project_and_outline(request)
    research_assets = await _collect_batch_generation_research_assets(
        request,
        dependencies,
        project=project,
        outline=outline,
    )

    resolved_chapter_runtime = await dependencies.resolve_batch_generation_chapter_runtime_fn(
        db_session=request.db_session,
        user_id=request.user_id,
        project=project,
        chapter=request.chapter,
        outline=outline,
        outline_mode=outline_mode,
        target_word_count=request.target_word_count,
        style_id=request.style_id,
        story_packet=request.story_packet,
        base_quality_profile=request.base_quality_profile,
        research_assets=research_assets,
        creative_mode=request.creative_mode,
        story_focus=request.story_focus,
        plot_stage=request.plot_stage,
        story_creation_brief=request.story_creation_brief,
        quality_preset=request.quality_preset,
        quality_notes=request.quality_notes,
        memory_service=dependencies.memory_service,
        foreshadow_service=dependencies.foreshadow_service,
        story_repair_state=request.story_repair_state,
        story_repair_payload=request.story_repair_payload,
        active_story_repair_snapshot=request.active_story_repair_snapshot,
        build_generation_runtime_bundle_fn=dependencies.build_generation_runtime_bundle_fn,
        build_story_packet_fn=dependencies.build_story_packet_fn,
        clone_quality_profile_fn=dependencies.clone_quality_profile_fn,
        resolve_quality_profile_fn=dependencies.resolve_quality_profile_fn,
        one_to_one_builder_cls=dependencies.one_to_one_builder_cls,
        one_to_many_builder_cls=dependencies.one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=dependencies.build_outline_structure_runtime_sources_fn,
    )
    effective_story_packet = resolved_chapter_runtime.effective_story_packet
    chapter_context = resolved_chapter_runtime.chapter_context
    generation_intent = resolved_chapter_runtime.generation_intent
    prompt_quality_kwargs = resolved_chapter_runtime.prompt_quality_kwargs
    story_runtime_contract = resolved_chapter_runtime.story_runtime_contract

    prompt_stage_result = await dependencies.execute_prompt_stage_fn(
        db_session=request.db_session,
        chapter=request.chapter,
        project=project,
        chapter_context=chapter_context,
        outline_mode=outline_mode,
        current_user_id=request.user_id,
        target_word_count=request.target_word_count,
        temp_narrative_perspective=request.temp_narrative_perspective,
        previous_summary_context=request.previous_summary_context,
        prompt_quality_kwargs=prompt_quality_kwargs,
        style_content=resolved_chapter_runtime.style_content,
        style_name=resolved_chapter_runtime.style_name,
        style_preset_id=resolved_chapter_runtime.style_preset_id,
        ai_service=request.ai_service,
        custom_model=request.custom_model,
        story_runtime_contract=story_runtime_contract,
        research_assets=research_assets,
        get_template_fn=dependencies.get_template_fn,
        format_prompt_fn=dependencies.format_prompt_fn,
        apply_style_to_prompt_fn=dependencies.apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=dependencies.build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=dependencies.calculate_max_tokens_fn,
        build_request_options_fn=dependencies.build_request_options_fn,
        detect_style_profile_fn=dependencies.detect_style_profile_fn,
        resolve_generation_temperature_fn=dependencies.resolve_generation_temperature_fn,
    )

    candidate_flow_result = await dependencies.execute_generation_stage_fn(
        stream_task_id=request.stream_task_id,
        stream_chunks=request.stream_chunks,
        chapter=request.chapter,
        effective_story_packet=effective_story_packet,
        project=project,
        chapter_context=chapter_context,
        target_word_count=request.target_word_count,
        generation_intent=generation_intent,
        current_story_repair_payload=request.story_repair_payload,
        retry_count=request.retry_count,
        max_retries=request.max_retries,
        default_candidate_limit=dependencies.default_candidate_limit,
        ai_service=request.ai_service,
        generate_kwargs=prompt_stage_result.generate_kwargs,
        story_runtime_contract=story_runtime_contract,
        db_session=request.db_session,
        heartbeat_interval_seconds=dependencies.heartbeat_interval_seconds,
        build_quality_runtime_context_fn=dependencies.build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=dependencies.compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=dependencies.resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=dependencies.candidate_generator_fn,
        attach_story_runtime_contract_fn=dependencies.attach_story_runtime_contract_fn,
    )

    return candidate_flow_result.selected_candidate_result
