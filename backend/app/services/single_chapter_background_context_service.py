"""??????????? helper?"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.models.project import Project
from app.services.chapter_quality_context_service import (
    StoryPacket,
    build_story_generation_packet_with_project_continuity,
    resolve_chapter_quality_profile,
)


@dataclass
class SingleChapterBackgroundExecutionContext:
    target_word_count: int
    enable_analysis: bool
    custom_model: Optional[str]
    temp_narrative_perspective: Optional[str]
    story_packet: StoryPacket
    quality_profile: Dict[str, Any]
    resolved_style_id: Optional[int]


async def build_single_chapter_background_execution_context(
    db_session: AsyncSession,
    *,
    user_id: str,
    project: Project,
    generate_request: Any,
) -> SingleChapterBackgroundExecutionContext:
    style_id = getattr(generate_request, "style_id", None)
    target_word_count = getattr(generate_request, "target_word_count", None) or 3000
    enable_analysis = bool(getattr(generate_request, "enable_analysis", False))
    custom_model = getattr(generate_request, "model", None)
    temp_narrative_perspective = getattr(generate_request, "narrative_perspective", None)
    creative_mode = getattr(generate_request, "creative_mode", None)
    story_focus = getattr(generate_request, "story_focus", None)
    plot_stage = getattr(generate_request, "plot_stage", None)

    story_packet = await build_story_generation_packet_with_project_continuity(
        db_session,
        project,
        source=generate_request,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        source_label="single-background-generate-request",
    )
    quality_profile = await resolve_chapter_quality_profile(
        db_session=db_session,
        user_id=user_id,
        project=project,
        style_id=style_id,
        enable_mcp=True,
        prefer_project_default_style=not bool(style_id),
        log_prefix="??????",
    )
    return SingleChapterBackgroundExecutionContext(
        target_word_count=target_word_count,
        enable_analysis=enable_analysis,
        custom_model=custom_model,
        temp_narrative_perspective=temp_narrative_perspective,
        story_packet=story_packet,
        quality_profile=quality_profile,
        resolved_style_id=quality_profile.get("resolved_style_id"),
    )


