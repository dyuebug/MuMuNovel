from __future__ import annotations

from datetime import datetime
from typing import TYPE_CHECKING, Optional

from app.schemas.regeneration import ChapterRegenerateRequest

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.chapter import Chapter
    from app.models.memory import PlotAnalysis
    from app.models.regeneration_task import RegenerationTask


REGENERATION_ANALYSIS_SOURCES = {"analysis_suggestions", "mixed"}


async def load_latest_regeneration_analysis(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    modification_source: str,
) -> Optional[PlotAnalysis]:
    from sqlalchemy import select

    from app.models.memory import PlotAnalysis

    if modification_source not in REGENERATION_ANALYSIS_SOURCES:
        return None

    analysis_result = await db_session.execute(
        select(PlotAnalysis)
        .where(PlotAnalysis.chapter_id == chapter_id)
        .order_by(PlotAnalysis.created_at.desc())
        .limit(1)
    )
    return analysis_result.scalar_one_or_none()


async def create_regeneration_task(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
    analysis: Optional[PlotAnalysis],
    user_id: str,
    regenerate_request: ChapterRegenerateRequest,
    style_id: Optional[int],
) -> RegenerationTask:
    from app.models.regeneration_task import RegenerationTask

    regeneration_task = RegenerationTask(
        chapter_id=chapter.id,
        analysis_id=analysis.id if analysis else None,
        user_id=user_id,
        project_id=chapter.project_id,
        modification_instructions="",
        original_suggestions=analysis.suggestions if analysis else None,
        selected_suggestion_indices=regenerate_request.selected_suggestion_indices,
        custom_instructions=regenerate_request.custom_instructions,
        style_id=style_id,
        target_word_count=regenerate_request.target_word_count,
        focus_areas=regenerate_request.focus_areas,
        preserve_elements=(
            regenerate_request.preserve_elements.model_dump()
            if regenerate_request.preserve_elements
            else None
        ),
        status="running",
        original_content=chapter.content,
        original_word_count=chapter.word_count or len(chapter.content or ""),
        version_note=regenerate_request.version_note,
        started_at=datetime.now(),
    )
    db_session.add(regeneration_task)
    await db_session.commit()
    await db_session.refresh(regeneration_task)
    return regeneration_task


async def mark_latest_regeneration_task_failed(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    error_message: str,
) -> Optional[RegenerationTask]:
    from sqlalchemy import select

    from app.models.regeneration_task import RegenerationTask

    task_result = await db_session.execute(
        select(RegenerationTask)
        .where(RegenerationTask.chapter_id == chapter_id)
        .order_by(RegenerationTask.created_at.desc())
        .limit(1)
    )
    regeneration_task = task_result.scalar_one_or_none()
    if regeneration_task is None:
        return None

    regeneration_task.status = "failed"
    regeneration_task.error_message = str(error_message)[:500]
    regeneration_task.completed_at = datetime.now()
    await db_session.commit()
    return regeneration_task
