from __future__ import annotations

from typing import Any, Dict, Optional

from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.models.outline import Outline


async def load_project_chapter_list_payload(
    *,
    db_session: AsyncSession,
    project_id: str,
) -> dict[str, Any]:
    count_result = await db_session.execute(
        select(func.count(Chapter.id)).where(Chapter.project_id == project_id)
    )
    total = int(count_result.scalar_one() or 0)

    chapters_result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == project_id)
        .order_by(Chapter.chapter_number.asc())
    )
    chapters = list(chapters_result.scalars().all())

    outline_ids = [chapter.outline_id for chapter in chapters if chapter.outline_id]
    outlines_map: Dict[str, Outline] = {}
    if outline_ids:
        outlines_result = await db_session.execute(
            select(Outline).where(Outline.id.in_(outline_ids))
        )
        outlines_map = {outline.id: outline for outline in outlines_result.scalars().all()}

    items = []
    for chapter in chapters:
        outline = outlines_map.get(chapter.outline_id) if chapter.outline_id else None
        items.append({
            'id': chapter.id,
            'project_id': chapter.project_id,
            'title': chapter.title,
            'chapter_number': chapter.chapter_number,
            'content': chapter.content,
            'summary': chapter.summary,
            'word_count': chapter.word_count,
            'status': chapter.status,
            'outline_id': chapter.outline_id,
            'sub_index': chapter.sub_index,
            'expansion_plan': chapter.expansion_plan,
            'outline_title': getattr(outline, 'title', None),
            'outline_order': getattr(outline, 'outline_order', None),
            'created_at': chapter.created_at,
            'updated_at': chapter.updated_at,
        })

    return {
        'total': total,
        'items': items,
    }


def serialize_navigation_item(chapter: Chapter | None) -> Optional[Dict[str, Any]]:
    if chapter is None:
        return None
    return {
        'id': chapter.id,
        'chapter_number': chapter.chapter_number,
        'title': chapter.title,
    }


async def load_chapter_navigation_payload(
    *,
    db_session: AsyncSession,
    current_chapter: Chapter,
) -> dict[str, Any]:
    previous_result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == current_chapter.project_id)
        .where(Chapter.chapter_number < current_chapter.chapter_number)
        .order_by(Chapter.chapter_number.desc())
        .limit(1)
    )
    next_result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == current_chapter.project_id)
        .where(Chapter.chapter_number > current_chapter.chapter_number)
        .order_by(Chapter.chapter_number.asc())
        .limit(1)
    )

    previous_chapter = previous_result.scalar_one_or_none()
    next_chapter = next_result.scalar_one_or_none()
    return {
        'current': serialize_navigation_item(current_chapter),
        'previous': serialize_navigation_item(previous_chapter),
        'next': serialize_navigation_item(next_chapter),
    }
