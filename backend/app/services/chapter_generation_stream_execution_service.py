from __future__ import annotations

import inspect
from typing import Any, Awaitable, Callable, Dict, List, Optional, Sequence

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.outline import Outline
from app.models.project import Project
from app.services.chapter_generation_stream_models import (
    ChapterGenerationStreamBuiltContext,
    ChapterGenerationStreamExecutionDependencies,
    ChapterGenerationStreamExecutionSetup,
    ChapterGenerationStreamPreparation,
    ChapterGenerationStreamPrompt,
    ChapterGenerationStreamRequestPayload,
    ChapterGenerationStreamRuntimeContext,
)
from app.services.chapter_quality_context_service import (
    build_story_generation_packet_with_project_continuity,
    resolve_chapter_quality_profile,
)

logger = get_logger(__name__)


async def _resolve_maybe_await(result: Any) -> Any:
    if inspect.isawaitable(result):
        return await result
    return result


def serialize_previous_chapters(previous_chapters: Sequence[Chapter]) -> List[dict[str, str | int | None]]:
    return [
        {
            "id": chapter.id,
            "chapter_number": chapter.chapter_number,
            "title": chapter.title,
            "content": chapter.content,
        }
        for chapter in previous_chapters
    ]


async def _load_generation_outline(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
) -> Optional[Outline]:
    if chapter.outline_id:
        outline_result = await db_session.execute(
            select(Outline)
            .where(Outline.id == chapter.outline_id)
            .execution_options(populate_existing=True)
        )
        return outline_result.scalar_one_or_none()

    outline_result = await db_session.execute(
        select(Outline)
        .where(Outline.project_id == chapter.project_id)
        .where(Outline.order_index == chapter.chapter_number)
        .execution_options(populate_existing=True)
    )
    return outline_result.scalar_one_or_none()


def _log_generation_context_stats(
    *,
    outline_mode: str,
    chapter: Chapter,
    chapter_context: Any,
) -> None:
    context_stats = getattr(chapter_context, "context_stats", {}) or {}
    chapter_number = getattr(chapter, "chapter_number", "?")
    if outline_mode == "one-to-one":
        logger.info("[1-1] Using OneToOneContextBuilder")
        logger.info(f"  - chapter: {chapter_number}")
        logger.info(f"  - outline length: {context_stats.get('outline_length', 0)}")
        logger.info(f"  - previous length: {context_stats.get('previous_content_length', 0)}")
        logger.info(f"  - characters length: {context_stats.get('characters_length', 0)}")
        logger.info(f"  - foreshadow length: {context_stats.get('foreshadow_length', 0)}")
        logger.info(f"  - memories length: {context_stats.get('memories_length', 0)}")
        logger.info(f"  - total length: {context_stats.get('total_length', 0)}")
        return

    logger.info("[1-N] Using OneToManyContextBuilder")
    logger.info(f"  - chapter: {chapter_number}")
    logger.info(f"  - continuation length: {context_stats.get('continuation_length', 0)}")
    logger.info(f"  - characters length: {context_stats.get('characters_length', 0)}")
    logger.info(f"  - memories length: {context_stats.get('memories_length', 0)}")
    logger.info(f"  - skeleton length: {context_stats.get('skeleton_length', 0)}")
    logger.info(f"  - foreshadow length: {context_stats.get('foreshadow_length', 0)}")
    logger.info(f"  - total length: {context_stats.get('total_length', 0)}")


def _resolve_chapter_perspective(
    *,
    project: Project,
    temp_narrative_perspective: Optional[str],
) -> str:
    return temp_narrative_perspective or project.narrative_perspective or "第三人称"


async def prepare_chapter_generation_stream_request(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    check_prerequisites_fn: Callable[[AsyncSession, Chapter], Awaitable[tuple[bool, str, list[Chapter]]]],
) -> ChapterGenerationStreamPreparation:
    result = await db_session.execute(
        select(Chapter).where(Chapter.id == chapter_id)
    )
    chapter = result.scalar_one_or_none()
    if chapter is None:
        raise ValueError("章节不存在")

    can_generate, error_msg, previous_chapters = await check_prerequisites_fn(db_session, chapter)
    if not can_generate:
        raise RuntimeError(error_msg)

    return ChapterGenerationStreamPreparation(
        chapter=chapter,
        previous_chapters_data=serialize_previous_chapters(previous_chapters),
    )


async def load_chapter_generation_stream_runtime_context(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    user_id: str,
    generate_request: Any,
    style_id: Optional[int],
    resolve_story_repair_state_fn: Callable[..., Awaitable[Dict[str, Any]]],
    cancel_outline_postprocess_tasks_fn: Callable[[str], int],
    resolve_quality_profile_fn: Optional[Callable[..., Awaitable[Dict[str, Any]]]] = None,
    build_story_packet_fn: Optional[Callable[..., Awaitable[Any]]] = None,
) -> ChapterGenerationStreamRuntimeContext:
    chapter_result = await db_session.execute(
        select(Chapter).where(Chapter.id == chapter_id)
    )
    chapter = chapter_result.scalar_one_or_none()
    if chapter is None:
        raise ValueError("章节不存在")

    project_result = await db_session.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        raise ValueError("项目不存在")

    outline_mode = project.outline_mode or "one-to-many"
    cancelled_postprocess_tasks = cancel_outline_postprocess_tasks_fn(chapter.project_id)
    if cancelled_postprocess_tasks:
        logger.info(
            "Cancelled %s outline postprocess task(s) for project %s before chapter generation",
            cancelled_postprocess_tasks,
            chapter.project_id,
        )

    outline = await _load_generation_outline(db_session, chapter=chapter)
    quality_profile_resolver = resolve_quality_profile_fn or resolve_chapter_quality_profile
    story_packet_builder = build_story_packet_fn or build_story_generation_packet_with_project_continuity
    quality_profile = await quality_profile_resolver(
        db_session=db_session,
        user_id=user_id,
        project=project,
        style_id=style_id,
        enable_mcp=bool(getattr(generate_request, "enable_mcp", True)),
        prefer_project_default_style=not bool(style_id),
        log_prefix="chapter-generate",
    )
    story_packet = await story_packet_builder(
        db_session,
        project,
        source=generate_request,
        source_label="chapter-generate-request",
    )
    generation_guidance = story_packet.guidance
    story_repair_state = await resolve_story_repair_state_fn(
        db_session,
        chapter=chapter,
        story_repair_summary=getattr(generate_request, "story_repair_summary", None),
        story_repair_targets=getattr(generate_request, "story_repair_targets", None),
        story_preserve_strengths=getattr(generate_request, "story_preserve_strengths", None),
    )
    story_repair_payload = story_repair_state.get("payload")
    return ChapterGenerationStreamRuntimeContext(
        chapter=chapter,
        project=project,
        outline=outline,
        outline_mode=outline_mode,
        quality_profile=quality_profile,
        story_packet=story_packet,
        generation_guidance=generation_guidance,
        story_repair_state=story_repair_state,
        story_repair_payload=story_repair_payload,
        resolved_style_id=quality_profile.get("resolved_style_id"),
        style_content=quality_profile.get("style_content") or "",
        style_name=quality_profile.get("style_name") or "",
        style_preset_id=quality_profile.get("style_preset_id") or "",
    )


async def build_chapter_generation_stream_context(
    *,
    db_session: AsyncSession,
    runtime_context: ChapterGenerationStreamRuntimeContext,
    user_id: str,
    target_word_count: int,
    temp_narrative_perspective: Optional[str],
    memory_service: Any,
    foreshadow_service: Any,
    one_to_one_builder_cls: Any,
    one_to_many_builder_cls: Any,
    build_outline_structure_runtime_sources_fn: Callable[[Optional[Outline]], Any],
    build_generation_runtime_bundle_fn: Callable[..., Any],
) -> ChapterGenerationStreamBuiltContext:
    chapter = runtime_context.chapter
    project = runtime_context.project
    outline = runtime_context.outline
    outline_mode = runtime_context.outline_mode

    if outline_mode == "one-to-one":
        context_builder = one_to_one_builder_cls(
            memory_service=memory_service,
            foreshadow_service=foreshadow_service,
        )
        chapter_context = await context_builder.build(
            chapter=chapter,
            project=project,
            outline=outline,
            user_id=user_id,
            db=db_session,
            target_word_count=target_word_count,
        )
    else:
        context_builder = one_to_many_builder_cls(
            memory_service=memory_service,
            foreshadow_service=foreshadow_service,
        )
        chapter_context = await context_builder.build(
            chapter=chapter,
            project=project,
            outline=outline,
            user_id=user_id,
            db=db_session,
            target_word_count=target_word_count,
            style_content=runtime_context.style_content,
            temp_narrative_perspective=temp_narrative_perspective,
        )

    _log_generation_context_stats(
        outline_mode=outline_mode,
        chapter=chapter,
        chapter_context=chapter_context,
    )
    outline_runtime_sources = build_outline_structure_runtime_sources_fn(outline)
    generation_runtime_bundle = await _resolve_maybe_await(
        build_generation_runtime_bundle_fn(
            story_packet=runtime_context.story_packet,
            quality_profile=runtime_context.quality_profile,
            project=project,
            chapter=chapter,
            chapter_context=chapter_context,
            target_word_count=target_word_count,
            story_repair_state=runtime_context.story_repair_state,
            story_repair_payload=runtime_context.story_repair_payload,
            active_story_repair_payload=(
                runtime_context.story_repair_state.get("active_story_repair_payload")
                if isinstance(runtime_context.story_repair_state, dict)
                else None
            ),
            character_focus_source=outline_runtime_sources or None,
            character_state_source=outline_runtime_sources or None,
            organization_state_source=outline_runtime_sources or None,
        )
    )
    return ChapterGenerationStreamBuiltContext(
        chapter_context=chapter_context,
        generation_intent=generation_runtime_bundle.generation_intent,
        prompt_quality_kwargs=generation_runtime_bundle.prompt_quality_kwargs,
        story_runtime_contract=generation_runtime_bundle.story_runtime_contract,
    )


async def build_chapter_generation_stream_prompt(
    *,
    db_session: AsyncSession,
    runtime_context: ChapterGenerationStreamRuntimeContext,
    built_context: ChapterGenerationStreamBuiltContext,
    current_user_id: str,
    target_word_count: int,
    temp_narrative_perspective: Optional[str],
    get_template_fn: Callable[..., Any],
    format_prompt_fn: Callable[..., str],
    apply_style_to_prompt_fn: Callable[..., str],
) -> ChapterGenerationStreamPrompt:
    chapter = runtime_context.chapter
    project = runtime_context.project
    chapter_context = built_context.chapter_context
    prompt_quality_kwargs = built_context.prompt_quality_kwargs
    chapter_perspective = _resolve_chapter_perspective(
        project=project,
        temp_narrative_perspective=temp_narrative_perspective,
    )

    common_kwargs = {
        "chapter_title": chapter.title,
        "chapter_number": chapter.chapter_number,
        "chapter_outline": chapter_context.chapter_outline,
        "target_word_count": target_word_count,
        "narrative_perspective": chapter_perspective,
        "world_time_period": project.world_time_period or "",
        "world_location": project.world_location or "",
        "world_atmosphere": project.world_atmosphere or "",
        "world_rules": project.world_rules or "",
        "characters_info": chapter_context.chapter_characters or "",
        "chapter_careers": chapter_context.chapter_careers or "",
        "foreshadow_reminders": chapter_context.foreshadow_reminders or "",
        **prompt_quality_kwargs,
    }

    outline_mode = runtime_context.outline_mode
    if outline_mode == "one-to-one":
        if chapter_context.continuation_point:
            template = await get_template_fn("CHAPTER_GENERATION_ONE_TO_ONE_NEXT", current_user_id, db_session)
            base_prompt = format_prompt_fn(
                template,
                previous_chapter_content=chapter_context.continuation_point,
                previous_chapter_summary=chapter_context.previous_chapter_summary or "",
                relevant_memories=chapter_context.relevant_memories or "",
                **common_kwargs,
            )
        else:
            template = await get_template_fn("CHAPTER_GENERATION_ONE_TO_ONE", current_user_id, db_session)
            base_prompt = format_prompt_fn(
                template,
                relevant_memories=chapter_context.relevant_memories or "",
                **common_kwargs,
            )
    else:
        if chapter_context.continuation_point:
            template = await get_template_fn("CHAPTER_GENERATION_ONE_TO_MANY_NEXT", current_user_id, db_session)
            base_prompt = format_prompt_fn(
                template,
                continuation_point=chapter_context.continuation_point,
                previous_chapter_summary=chapter_context.previous_chapter_summary or "",
                recent_chapters_context=chapter_context.recent_chapters_context or "",
                relevant_memories=chapter_context.relevant_memories or "",
                **common_kwargs,
            )
        else:
            template = await get_template_fn("CHAPTER_GENERATION_ONE_TO_MANY", current_user_id, db_session)
            base_prompt = format_prompt_fn(
                template,
                relevant_memories=chapter_context.relevant_memories or "",
                **common_kwargs,
            )

    prompt = (
        apply_style_to_prompt_fn(base_prompt, runtime_context.style_content)
        if runtime_context.style_content
        else base_prompt
    )
    return ChapterGenerationStreamPrompt(
        chapter_perspective=chapter_perspective,
        base_prompt=base_prompt,
        prompt=prompt,
    )


def build_chapter_generation_stream_request_payload(
    *,
    runtime_context: ChapterGenerationStreamRuntimeContext,
    built_context: ChapterGenerationStreamBuiltContext,
    stream_prompt: ChapterGenerationStreamPrompt,
    project: Project,
    target_word_count: int,
    enable_mcp: bool,
    custom_model: Optional[str],
    ai_service: Any,
    build_runtime_system_prompt_fn: Callable[..., str],
    calculate_max_tokens_fn: Callable[[int], int],
    build_request_options_fn: Callable[[Any], Optional[Dict[str, Any]]],
    detect_style_profile_fn: Callable[..., str],
    resolve_generation_temperature_fn: Callable[[str], float],
) -> ChapterGenerationStreamRequestPayload:
    chapter_context = built_context.chapter_context
    style_content = runtime_context.style_content
    style_name = runtime_context.style_name
    style_preset_id = runtime_context.style_preset_id
    story_runtime_contract = built_context.story_runtime_contract

    system_prompt = build_runtime_system_prompt_fn(
        project=project,
        style_content=style_content,
        chapter_outline=chapter_context.chapter_outline,
        previous_summary=chapter_context.previous_chapter_summary,
        style_name=style_name,
        style_preset_id=style_preset_id,
        target_word_count=target_word_count,
        story_runtime_contract=story_runtime_contract,
    )
    max_tokens = calculate_max_tokens_fn(target_word_count)
    style_profile = detect_style_profile_fn(
        style_name=style_name,
        style_preset_id=style_preset_id,
        style_content=style_content,
    )
    generate_kwargs: Dict[str, Any] = {
        "prompt": stream_prompt.prompt,
        "system_prompt": system_prompt,
        "tool_choice": "auto",
        "auto_mcp": enable_mcp,
        "max_tokens": max_tokens,
        "temperature": resolve_generation_temperature_fn(style_profile),
    }
    request_options = build_request_options_fn(ai_service)
    if request_options is not None:
        generate_kwargs["request_options"] = request_options
    if custom_model:
        generate_kwargs["model"] = custom_model

    return ChapterGenerationStreamRequestPayload(
        system_prompt=system_prompt,
        max_tokens=max_tokens,
        generate_kwargs=generate_kwargs,
    )


async def prepare_chapter_generation_stream_execution(
    *,
    db_session: AsyncSession,
    chapter_id: str,
    current_user_id: str,
    generate_request: Any,
    user_ai_service: Any,
    target_word_count: int,
    custom_model: Optional[str],
    temp_narrative_perspective: Optional[str],
    style_id: Optional[int],
    dependencies: ChapterGenerationStreamExecutionDependencies,
    resolve_quality_profile_fn: Optional[Callable[..., Awaitable[Dict[str, Any]]]] = None,
    build_story_packet_fn: Optional[Callable[..., Awaitable[Any]]] = None,
) -> ChapterGenerationStreamExecutionSetup:
    stream_runtime_context = await load_chapter_generation_stream_runtime_context(
        db_session,
        chapter_id=chapter_id,
        user_id=current_user_id,
        generate_request=generate_request,
        style_id=style_id,
        resolve_story_repair_state_fn=dependencies.resolve_story_repair_state_fn,
        cancel_outline_postprocess_tasks_fn=dependencies.cancel_outline_postprocess_tasks_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        build_story_packet_fn=build_story_packet_fn,
    )
    current_chapter = stream_runtime_context.chapter
    project = stream_runtime_context.project

    built_stream_context = await build_chapter_generation_stream_context(
        db_session=db_session,
        runtime_context=stream_runtime_context,
        user_id=current_user_id,
        target_word_count=target_word_count,
        temp_narrative_perspective=temp_narrative_perspective,
        memory_service=dependencies.memory_service,
        foreshadow_service=dependencies.foreshadow_service,
        one_to_one_builder_cls=dependencies.one_to_one_builder_cls,
        one_to_many_builder_cls=dependencies.one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=dependencies.build_outline_structure_runtime_sources_fn,
        build_generation_runtime_bundle_fn=dependencies.build_generation_runtime_bundle_fn,
    )
    story_runtime_contract = built_stream_context.story_runtime_contract

    stream_prompt = await build_chapter_generation_stream_prompt(
        db_session=db_session,
        runtime_context=stream_runtime_context,
        built_context=built_stream_context,
        current_user_id=current_user_id,
        target_word_count=target_word_count,
        temp_narrative_perspective=temp_narrative_perspective,
        get_template_fn=dependencies.get_template_fn,
        format_prompt_fn=dependencies.format_prompt_fn,
        apply_style_to_prompt_fn=dependencies.apply_style_to_prompt_fn,
    )
    request_payload = build_chapter_generation_stream_request_payload(
        runtime_context=stream_runtime_context,
        built_context=built_stream_context,
        stream_prompt=stream_prompt,
        project=project,
        target_word_count=target_word_count,
        enable_mcp=generate_request.enable_mcp,
        custom_model=custom_model,
        ai_service=user_ai_service,
        build_runtime_system_prompt_fn=dependencies.build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=dependencies.calculate_max_tokens_fn,
        build_request_options_fn=dependencies.build_request_options_fn,
        detect_style_profile_fn=dependencies.detect_style_profile_fn,
        resolve_generation_temperature_fn=dependencies.resolve_generation_temperature_fn,
    )
    return ChapterGenerationStreamExecutionSetup(
        stream_runtime_context=stream_runtime_context,
        built_stream_context=built_stream_context,
        current_chapter=current_chapter,
        project=project,
        quality_profile=stream_runtime_context.quality_profile,
        story_packet=stream_runtime_context.story_packet,
        story_runtime_contract=story_runtime_contract,
        request_payload=request_payload,
    )
