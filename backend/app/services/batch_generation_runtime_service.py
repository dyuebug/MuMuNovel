"""批量生成 runtime 解析 helper。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Awaitable, Callable, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.project import Project
from app.services.chapter_quality_context_service import (
    StoryPacket,
    build_story_generation_packet_with_project_continuity,
    clone_chapter_quality_profile,
    resolve_chapter_quality_profile,
)
from app.services.story_repair_payload_service import StoryRepairPayload


logger = get_logger(__name__)


@dataclass(frozen=True)
class BatchGenerationRuntimePreparation:
    effective_story_packet: StoryPacket
    generation_guidance: Any
    quality_profile: Dict[str, Any]
    style_id: Any
    style_content: str
    style_name: str
    style_preset_id: Any
    generation_runtime: Optional[Any]


@dataclass(frozen=True)
class BatchGenerationResolvedRuntime:
    generation_runtime: Any
    generation_intent: Any
    prompt_quality_kwargs: Dict[str, Any]
    story_runtime_contract: Any


@dataclass(frozen=True)
class BatchGenerationBuiltContext:
    chapter_context: Any
    outline_runtime_sources: Any


@dataclass(frozen=True)
class BatchGenerationChapterRuntimeArtifacts:
    effective_story_packet: StoryPacket
    generation_guidance: Any
    quality_profile: Dict[str, Any]
    style_id: Any
    style_content: str
    style_name: str
    style_preset_id: Any
    chapter_context: Any
    outline_runtime_sources: Any
    generation_runtime: Any
    generation_intent: Any
    prompt_quality_kwargs: Dict[str, Any]
    story_runtime_contract: Any



async def prepare_batch_generation_runtime(
    *,
    db_session: AsyncSession,
    user_id: str,
    project: Project,
    chapter: Chapter,
    target_word_count: int,
    style_id: Optional[int],
    story_packet: Optional[StoryPacket],
    base_quality_profile: Optional[Dict[str, Any]],
    research_assets: list[Any],
    creative_mode: Optional[str],
    story_focus: Optional[str],
    plot_stage: Optional[str],
    story_creation_brief: Optional[str],
    quality_preset: Optional[str],
    quality_notes: Optional[str],
    chapter_context: Any = None,
    outline_runtime_sources: Any = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    build_story_packet_fn: Callable[..., Any] = build_story_generation_packet_with_project_continuity,
    clone_quality_profile_fn: Callable[..., Dict[str, Any]] = clone_chapter_quality_profile,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    build_generation_runtime_bundle_fn: Optional[Callable[..., Any]] = None,
) -> BatchGenerationRuntimePreparation:
    effective_story_packet = (
        story_packet
        if story_packet is not None
        else await build_story_packet_fn(
            db_session,
            project,
            creative_mode=creative_mode,
            story_focus=story_focus,
            plot_stage=plot_stage,
            story_creation_brief=story_creation_brief,
            quality_preset=quality_preset,
            quality_notes=quality_notes,
            source_label='batch-single-chapter-generate',
        )
    )
    generation_guidance = effective_story_packet.guidance

    if isinstance(base_quality_profile, dict) and base_quality_profile:
        quality_profile = clone_quality_profile_fn(
            base_quality_profile,
            external_assets=research_assets,
            reference_assets=research_assets,
        )
    else:
        quality_profile = await resolve_quality_profile_fn(
            db_session=db_session,
            user_id=user_id,
            project=project,
            style_id=style_id,
            enable_mcp=True,
            external_assets=research_assets,
            reference_assets=research_assets,
            prefer_project_default_style=not bool(style_id),
            log_prefix='批量生成',
        )

    resolved_style_id = quality_profile.get('resolved_style_id')
    style_content = quality_profile.get('style_content') or ''
    style_name = quality_profile.get('style_name') or ''
    style_preset_id = quality_profile.get('style_preset_id') or ''

    generation_runtime = None
    if build_generation_runtime_bundle_fn is not None and chapter_context is not None:
        generation_runtime = build_generation_runtime_bundle_fn(
            story_packet=effective_story_packet,
            quality_profile=quality_profile,
            project=project,
            chapter=chapter,
            chapter_context=chapter_context,
            target_word_count=target_word_count,
            story_repair_state=story_repair_state,
            story_repair_payload=story_repair_payload,
            active_story_repair_payload=active_story_repair_snapshot,
            character_focus_source=outline_runtime_sources or None,
            character_state_source=outline_runtime_sources or None,
            organization_state_source=outline_runtime_sources or None,
        )
    return BatchGenerationRuntimePreparation(
        effective_story_packet=effective_story_packet,
        generation_guidance=generation_guidance,
        quality_profile=(dict(quality_profile) if isinstance(quality_profile, dict) else {}),
        style_id=resolved_style_id,
        style_content=style_content,
        style_name=style_name,
        style_preset_id=style_preset_id,
        generation_runtime=generation_runtime,
    )


def finalize_batch_generation_runtime(
    *,
    runtime_preparation: BatchGenerationRuntimePreparation,
    project: Project,
    chapter: Chapter,
    chapter_context: Any,
    target_word_count: int,
    outline_runtime_sources: Any,
    story_repair_state: Optional[Dict[str, Any]],
    story_repair_payload: Optional[StoryRepairPayload],
    active_story_repair_snapshot: Optional[Dict[str, Any]],
    build_generation_runtime_bundle_fn: Callable[..., Any],
) -> BatchGenerationResolvedRuntime:
    generation_runtime = build_generation_runtime_bundle_fn(
        story_packet=runtime_preparation.effective_story_packet,
        quality_profile=runtime_preparation.quality_profile,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        story_repair_state=story_repair_state,
        story_repair_payload=story_repair_payload,
        active_story_repair_payload=active_story_repair_snapshot,
        character_focus_source=outline_runtime_sources or None,
        character_state_source=outline_runtime_sources or None,
        organization_state_source=outline_runtime_sources or None,
    )
    return BatchGenerationResolvedRuntime(
        generation_runtime=generation_runtime,
        generation_intent=generation_runtime.generation_intent,
        prompt_quality_kwargs=generation_runtime.prompt_quality_kwargs,
        story_runtime_contract=generation_runtime.story_runtime_contract,
    )


async def build_batch_generation_context(
    *,
    db_session: AsyncSession,
    chapter: Chapter,
    project: Project,
    outline: Any,
    outline_mode: str,
    user_id: str,
    target_word_count: int,
    style_content: str,
    memory_service: Any,
    foreshadow_service: Any,
    one_to_one_builder_cls: Callable[..., Any],
    one_to_many_builder_cls: Callable[..., Any],
    build_outline_structure_runtime_sources_fn: Callable[[Any], Any],
) -> BatchGenerationBuiltContext:
    if outline_mode == 'one-to-one':
        logger.info(f'构建上下文 - [1-1模式] 使用 {one_to_one_builder_cls.__name__}')
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
        logger.info(f'构建上下文 - [1-N模式] 使用 {one_to_many_builder_cls.__name__}')
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
            style_content=style_content,
            target_word_count=target_word_count,
        )

    context_stats = (
        dict(chapter_context.context_stats)
        if isinstance(getattr(chapter_context, 'context_stats', None), dict)
        else {}
    )
    logger.info('批量生成 - 上下文摘要')
    logger.info(f'  - 章节号: {chapter.chapter_number}')
    logger.info(f"  - 续写点长度: {len(getattr(chapter_context, 'continuation_point', '') or '')} 字")
    logger.info(f"  - 记忆数: {context_stats.get('memory_count', 0)} 条")
    logger.info(f"  - 上下文总长: {context_stats.get('total_length', 0)} 字")

    outline_runtime_sources = build_outline_structure_runtime_sources_fn(outline)
    return BatchGenerationBuiltContext(
        chapter_context=chapter_context,
        outline_runtime_sources=outline_runtime_sources,
    )



async def resolve_batch_generation_chapter_runtime(
    *,
    db_session: AsyncSession,
    user_id: str,
    project: Project,
    chapter: Chapter,
    outline: Any,
    outline_mode: str,
    target_word_count: int,
    style_id: Optional[int],
    story_packet: Optional[StoryPacket],
    base_quality_profile: Optional[Dict[str, Any]],
    research_assets: list[Any],
    creative_mode: Optional[str],
    story_focus: Optional[str],
    plot_stage: Optional[str],
    story_creation_brief: Optional[str],
    quality_preset: Optional[str],
    quality_notes: Optional[str],
    memory_service: Any,
    foreshadow_service: Any,
    story_repair_state: Optional[Dict[str, Any]],
    story_repair_payload: Optional[StoryRepairPayload],
    active_story_repair_snapshot: Optional[Dict[str, Any]],
    build_generation_runtime_bundle_fn: Callable[..., Any],
    build_story_packet_fn: Callable[..., Any] = build_story_generation_packet_with_project_continuity,
    clone_quality_profile_fn: Callable[..., Dict[str, Any]] = clone_chapter_quality_profile,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    one_to_one_builder_cls: Callable[..., Any],
    one_to_many_builder_cls: Callable[..., Any],
    build_outline_structure_runtime_sources_fn: Callable[[Any], Any],
    prepare_runtime_fn: Callable[..., Awaitable[BatchGenerationRuntimePreparation]] = prepare_batch_generation_runtime,
    build_context_fn: Callable[..., Awaitable[BatchGenerationBuiltContext]] = build_batch_generation_context,
    finalize_runtime_fn: Callable[..., BatchGenerationResolvedRuntime] = finalize_batch_generation_runtime,
) -> BatchGenerationChapterRuntimeArtifacts:
    runtime_preparation = await prepare_runtime_fn(
        db_session=db_session,
        user_id=user_id,
        project=project,
        chapter=chapter,
        target_word_count=target_word_count,
        style_id=style_id,
        story_packet=story_packet,
        base_quality_profile=base_quality_profile,
        research_assets=research_assets,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        build_story_packet_fn=build_story_packet_fn,
        clone_quality_profile_fn=clone_quality_profile_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
    )
    built_context = await build_context_fn(
        db_session=db_session,
        chapter=chapter,
        project=project,
        outline=outline,
        outline_mode=outline_mode,
        user_id=user_id,
        target_word_count=target_word_count,
        style_content=runtime_preparation.style_content,
        memory_service=memory_service,
        foreshadow_service=foreshadow_service,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources_fn,
    )
    resolved_runtime = finalize_runtime_fn(
        runtime_preparation=runtime_preparation,
        project=project,
        chapter=chapter,
        chapter_context=built_context.chapter_context,
        target_word_count=target_word_count,
        outline_runtime_sources=built_context.outline_runtime_sources,
        story_repair_state=story_repair_state,
        story_repair_payload=story_repair_payload,
        active_story_repair_snapshot=active_story_repair_snapshot,
        build_generation_runtime_bundle_fn=build_generation_runtime_bundle_fn,
    )
    return BatchGenerationChapterRuntimeArtifacts(
        effective_story_packet=runtime_preparation.effective_story_packet,
        generation_guidance=runtime_preparation.generation_guidance,
        quality_profile=runtime_preparation.quality_profile,
        style_id=runtime_preparation.style_id,
        style_content=runtime_preparation.style_content,
        style_name=runtime_preparation.style_name,
        style_preset_id=runtime_preparation.style_preset_id,
        chapter_context=built_context.chapter_context,
        outline_runtime_sources=built_context.outline_runtime_sources,
        generation_runtime=resolved_runtime.generation_runtime,
        generation_intent=resolved_runtime.generation_intent,
        prompt_quality_kwargs=resolved_runtime.prompt_quality_kwargs,
        story_runtime_contract=resolved_runtime.story_runtime_contract,
    )


