from __future__ import annotations

import json
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Dict, Optional

from app.logger import get_logger
from app.schemas.regeneration import ChapterRegenerateRequest
from app.services.chapter_web_research_service import chapter_web_research_service
from app.services.chapter_generation.runtime.service import build_chapter_generation_runtime_bundle
from app.services.chapter_quality_context_service import (
    build_story_generation_packet_with_project_continuity,
    resolve_chapter_quality_profile,
)
from app.services.character_context_service import build_characters_info_with_careers
from app.services.outline_runtime_source_service import (
    build_outline_structure_runtime_sources as _build_outline_structure_runtime_sources,
)
from app.services.chapter_regeneration_stream_service import ChapterRegenerationStreamContext
from app.services.story_repair_payload_service import (
    resolve_generation_story_repair_state_for_chapter,
    story_repair_payload_to_prompt_kwargs,
)

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.chapter import Chapter
    from app.models.outline import Outline
    from app.models.project import Project

logger = get_logger(__name__)

@dataclass(frozen=True)
class ChapterRegenerationPreparation:
    effective_regenerate_request: ChapterRegenerateRequest
    style_content: str
    style_id: Optional[int]
    project_context: Dict[str, Any]
    story_runtime_contract: Optional[Dict[str, Any]]


async def _load_regeneration_outline(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
) -> Optional[Outline]:
    from sqlalchemy import select

    from app.models.outline import Outline

    if chapter.outline_id:
        outline_result = await db_session.execute(
            select(Outline).where(Outline.id == chapter.outline_id)
        )
        return outline_result.scalar_one_or_none()

    outline_result = await db_session.execute(
        select(Outline)
        .where(Outline.project_id == chapter.project_id)
        .where(Outline.order_index == chapter.chapter_number)
    )
    return outline_result.scalar_one_or_none()


def _resolve_regeneration_filter_character_names(
    *,
    chapter: Chapter,
    outline_mode: str,
    outline: Optional[Outline],
) -> Optional[list[str]]:
    if outline_mode == "one-to-one":
        structure_text = getattr(outline, "structure", None)
        if not structure_text:
            return None
        try:
            structure = json.loads(structure_text)
        except json.JSONDecodeError:
            logger.warning("章节重写 - outline.structure 不是合法 JSON")
            return None
        if not isinstance(structure, dict):
            return None
        filter_character_names = structure.get("characters", [])
        if filter_character_names:
            logger.info(f"章节重写 - 1-1 角色聚焦: {filter_character_names}")
            return filter_character_names
        return None

    if not chapter.expansion_plan:
        return None

    try:
        plan = json.loads(chapter.expansion_plan)
    except json.JSONDecodeError:
        logger.warning("章节重写 - expansion_plan 不是合法 JSON")
        return None
    if not isinstance(plan, dict):
        return None
    filter_character_names = plan.get("character_focus", [])
    if filter_character_names:
        logger.info(f"章节重写 - 1-N 角色聚焦: {filter_character_names}")
        return filter_character_names
    return None


async def prepare_chapter_regeneration_context(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
    regenerate_request: ChapterRegenerateRequest,
    user_id: str,
) -> ChapterRegenerationPreparation:
    from sqlalchemy import select

    from app.models.character import Character
    from app.models.project import Project

    project_result = await db_session.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        raise ValueError(f"Project not found for chapter regeneration: {chapter.project_id}")

    outline_mode = project.outline_mode or "one-to-many"
    outline = await _load_regeneration_outline(db_session, chapter=chapter)

    filter_character_names = _resolve_regeneration_filter_character_names(
        chapter=chapter,
        outline_mode=outline_mode,
        outline=outline,
    )

    characters_result = await db_session.execute(
        select(Character).where(Character.project_id == chapter.project_id)
    )
    characters = characters_result.scalars().all()
    characters_info_with_careers = await build_characters_info_with_careers(
        db_session,
        chapter.project_id,
        characters,
        filter_character_names,
    )

    quality_profile = await resolve_chapter_quality_profile(
        db_session=db_session,
        user_id=user_id,
        project=project,
        style_id=regenerate_request.style_id,
        enable_mcp=True,
        prefer_project_default_style=not bool(regenerate_request.style_id),
        log_prefix="章节重写",
    )
    story_repair_state = await resolve_generation_story_repair_state_for_chapter(
        db_session,
        chapter=chapter,
        story_repair_summary=getattr(regenerate_request, "story_repair_summary", None),
        story_repair_targets=getattr(regenerate_request, "story_repair_targets", None),
        story_preserve_strengths=getattr(regenerate_request, "story_preserve_strengths", None),
    )
    story_repair_payload = story_repair_state.get("payload")
    effective_regenerate_request = regenerate_request.model_copy(
        update=story_repair_payload_to_prompt_kwargs(story_repair_payload),
        deep=True,
    )
    regeneration_story_packet = await build_story_generation_packet_with_project_continuity(
        db_session,
        project,
        source=effective_regenerate_request,
        source_label="chapter-regenerate-request",
    )
    web_research_bundle = await chapter_web_research_service.collect_for_chapter(
        user_id=user_id,
        db_session=db_session,
        project=project,
        chapter=chapter,
        outline=outline,
        story_creation_brief=effective_regenerate_request.story_creation_brief,
        enable_web_research=effective_regenerate_request.enable_web_research,
        web_research_query=effective_regenerate_request.web_research_query,
    )
    web_research_assets = list(web_research_bundle.get("assets") or [])

    outline_runtime_sources = _build_outline_structure_runtime_sources(outline)
    generation_runtime = build_chapter_generation_runtime_bundle(
        story_packet=regeneration_story_packet,
        quality_profile=quality_profile,
        project=project,
        chapter=chapter,
        chapter_context=None,
        target_word_count=effective_regenerate_request.target_word_count,
        story_repair_state=story_repair_state,
        story_repair_payload=story_repair_payload,
        active_story_repair_payload=story_repair_state.get("active_story_repair_payload"),
        character_focus_source=outline_runtime_sources or None,
        character_state_source=(
            {**outline_runtime_sources, "chapter_characters": characters_info_with_careers}
            if outline_runtime_sources
            else characters_info_with_careers
        ),
        relationship_state_source=characters_info_with_careers,
        foreshadow_state_source=outline.content if outline else chapter.summary,
        organization_state_source=outline_runtime_sources or None,
    )

    style_content = quality_profile.get("style_content") or ""
    style_id = quality_profile.get("resolved_style_id")
    if style_id:
        logger.info(f"章节重写风格 ID: {style_id}")
    else:
        logger.info("章节重写未命中明确风格 ID")

    project_context = {
        "project_title": project.title if project else "未命名项目",
        "genre": project.genre if project else "未提供",
        "theme": project.theme if project else "未提供",
        "narrative_perspective": project.narrative_perspective if project else "第三人称",
        "time_period": project.world_time_period if project else "未提供",
        "location": project.world_location if project else "未提供",
        "atmosphere": project.world_atmosphere if project else "未提供",
        "characters_info": characters_info_with_careers,
        "chapter_outline": outline.content if outline else chapter.summary or "暂无大纲",
        "previous_context": "",
        "external_assets": web_research_assets,
        "reference_assets": web_research_assets,
        "prompt_quality_kwargs": generation_runtime.prompt_quality_kwargs,
    }

    return ChapterRegenerationPreparation(
        effective_regenerate_request=effective_regenerate_request,
        style_content=style_content,
        style_id=style_id,
        project_context=project_context,
        story_runtime_contract=generation_runtime.story_runtime_contract,
    )


async def prepare_chapter_regeneration_stream_context(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
    regenerate_request: ChapterRegenerateRequest,
    user_id: str,
) -> ChapterRegenerationStreamContext:
    from sqlalchemy import select

    from app.models.memory import PlotAnalysis

    if not chapter.content or not chapter.content.strip():
        raise ValueError("当前章节缺少可重写的原始内容")

    analysis = None
    if regenerate_request.modification_source in {"analysis_suggestions", "mixed"}:
        analysis_result = await db_session.execute(
            select(PlotAnalysis)
            .where(PlotAnalysis.chapter_id == chapter.id)
            .order_by(PlotAnalysis.created_at.desc())
            .limit(1)
        )
        analysis = analysis_result.scalar_one_or_none()
        if analysis is None:
            raise LookupError("未找到对应的章节分析")

    preparation = await prepare_chapter_regeneration_context(
        db_session,
        chapter=chapter,
        regenerate_request=regenerate_request,
        user_id=user_id,
    )
    return ChapterRegenerationStreamContext(
        chapter=chapter,
        analysis=analysis,
        user_id=user_id,
        regenerate_request=regenerate_request,
        effective_regenerate_request=preparation.effective_regenerate_request,
        project_context=preparation.project_context,
        style_content=preparation.style_content,
        style_id=preparation.style_id,
        story_runtime_contract=preparation.story_runtime_contract,
    )
