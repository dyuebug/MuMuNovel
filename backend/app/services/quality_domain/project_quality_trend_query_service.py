from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, List

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.services.story_repair_payload_service import (
    load_latest_quality_metric_records_for_chapter_ids,
)


@dataclass(frozen=True)
class ProjectQualityTrendQueryContext:
    chapters: List[Chapter]
    records_by_chapter: Dict[str, Dict[str, Any]]


async def load_project_quality_trend_query_context(
    db_session: AsyncSession,
    *,
    project_id: str,
    load_records_fn: Callable[[AsyncSession, List[str]], Any] = load_latest_quality_metric_records_for_chapter_ids,
) -> ProjectQualityTrendQueryContext:
    chapters_result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == project_id)
        .order_by(Chapter.chapter_number)
    )
    chapters = chapters_result.scalars().all()
    chapter_ids = [chapter.id for chapter in chapters]
    records_by_chapter = await load_records_fn(db_session, chapter_ids)
    if not isinstance(records_by_chapter, dict):
        records_by_chapter = {}
    return ProjectQualityTrendQueryContext(
        chapters=chapters,
        records_by_chapter=records_by_chapter,
    )
