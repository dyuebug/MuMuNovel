"""Chapter generation prerequisite helpers."""

from __future__ import annotations

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter


async def check_chapter_generation_prerequisites(
    db: AsyncSession,
    chapter: Chapter,
) -> tuple[bool, str, list[Chapter]]:
    """Check whether a chapter can start generation based on previous chapters."""
    if chapter.chapter_number == 1:
        return True, "", []

    result = await db.execute(
        select(Chapter)
        .where(Chapter.project_id == chapter.project_id)
        .where(Chapter.chapter_number < chapter.chapter_number)
        .order_by(Chapter.chapter_number)
    )
    previous_chapters = result.scalars().all()

    incomplete_chapters = [
        previous_chapter
        for previous_chapter in previous_chapters
        if not previous_chapter.content or previous_chapter.content.strip() == ""
    ]
    if incomplete_chapters:
        missing_numbers = [str(previous_chapter.chapter_number) for previous_chapter in incomplete_chapters]
        error_message = f"??????????? {', '.join(missing_numbers)} ?"
        return False, error_message, previous_chapters

    return True, "", previous_chapters
