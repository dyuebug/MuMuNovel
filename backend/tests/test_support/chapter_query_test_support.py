from __future__ import annotations

from functools import lru_cache
from importlib import import_module
from typing import TYPE_CHECKING, Any, Dict, Optional

from sqlalchemy import func, select

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.chapter import Chapter


@lru_cache(maxsize=1)
def _chapter_query_models() -> tuple[type[Any], type[Any]]:
    chapter_module = import_module("migrator_app.models.chapter")
    outline_module = import_module("migrator_app.models.outline")
    return chapter_module.Chapter, outline_module.Outline


async def check_chapter_generation_prerequisites(
    db_session: "AsyncSession",
    chapter: "Chapter",
) -> tuple[bool, str, list["Chapter"]]:
    Chapter, _ = _chapter_query_models()
    if chapter.chapter_number == 1:
        return True, "", []

    result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == chapter.project_id)
        .where(Chapter.chapter_number < chapter.chapter_number)
        .order_by(Chapter.chapter_number.asc())
    )
    previous_chapters = list(result.scalars().all())

    incomplete_chapters = [
        previous_chapter
        for previous_chapter in previous_chapters
        if not previous_chapter.content or previous_chapter.content.strip() == ""
    ]
    if incomplete_chapters:
        missing_numbers = [
            str(previous_chapter.chapter_number)
            for previous_chapter in incomplete_chapters
        ]
        return (
            False,
            f"前置章节尚未完成: {', '.join(missing_numbers)} 章",
            previous_chapters,
        )

    return True, "", previous_chapters


async def load_project_chapter_list_payload(
    *,
    db_session: "AsyncSession",
    project_id: str,
) -> dict[str, Any]:
    Chapter, Outline = _chapter_query_models()
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
    outlines_map: Dict[str, Any] = {}
    if outline_ids:
        outlines_result = await db_session.execute(
            select(Outline).where(Outline.id.in_(outline_ids))
        )
        outlines_map = {
            outline.id: outline
            for outline in outlines_result.scalars().all()
        }

    items = []
    for chapter in chapters:
        outline = outlines_map.get(chapter.outline_id) if chapter.outline_id else None
        items.append(
            {
                "id": chapter.id,
                "project_id": chapter.project_id,
                "title": chapter.title,
                "chapter_number": chapter.chapter_number,
                "content": chapter.content,
                "summary": chapter.summary,
                "word_count": chapter.word_count,
                "status": chapter.status,
                "outline_id": chapter.outline_id,
                "sub_index": chapter.sub_index,
                "expansion_plan": chapter.expansion_plan,
                "outline_title": getattr(outline, "title", None),
                "outline_order": getattr(outline, "order_index", None),
                "created_at": chapter.created_at,
                "updated_at": chapter.updated_at,
            }
        )

    return {
        "total": total,
        "items": items,
    }


def serialize_navigation_item(chapter: "Chapter" | None) -> Optional[Dict[str, Any]]:
    if chapter is None:
        return None
    return {
        "id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "title": chapter.title,
    }


async def load_chapter_navigation_payload(
    *,
    db_session: "AsyncSession",
    current_chapter: "Chapter",
) -> dict[str, Any]:
    Chapter, _ = _chapter_query_models()
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
        "current": serialize_navigation_item(current_chapter),
        "previous": serialize_navigation_item(previous_chapter),
        "next": serialize_navigation_item(next_chapter),
    }

