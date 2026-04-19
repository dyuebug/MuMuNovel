from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.models.project import Project
from app.services.analysis_task_service import create_analysis_task_safely
from app.services.chapter_quality_context_service import (
    StoryPacket,
    build_story_generation_packet_with_project_continuity,
    resolve_chapter_quality_profile,
)


@dataclass(frozen=True)
class ManualChapterAnalysisPreparation:
    task_id: str
    quality_profile: Dict[str, Any]
    story_packet: StoryPacket


async def prepare_manual_chapter_analysis(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
    project: Project,
    user_id: str,
) -> Optional[ManualChapterAnalysisPreparation]:
    analysis_quality_profile = await resolve_chapter_quality_profile(
        db_session=db_session,
        user_id=user_id,
        project=project,
        style_id=None,
        enable_mcp=True,
        prefer_project_default_style=True,
        log_prefix="章节分析",
    )
    analysis_story_packet = await build_story_generation_packet_with_project_continuity(
        db_session,
        project,
        source_label="manual-analysis-request",
    )
    analysis_task = await create_analysis_task_safely(
        db_session,
        chapter_id=chapter.id,
        user_id=user_id,
        project_id=project.id,
        log_context="manual-analysis",
    )
    if analysis_task is None:
        return None

    return ManualChapterAnalysisPreparation(
        task_id=analysis_task.id,
        quality_profile=analysis_quality_profile,
        story_packet=analysis_story_packet,
    )
