"""Chapter generation prerequisite helpers."""

from __future__ import annotations

from typing import TYPE_CHECKING

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active single-generation prerequisite contract; this "
    "Python module is kept only as frozen rollback/source-map material after "
    "explicit support-shell freeze approval."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/services/chapter_single_generation_prepare_service.rs"
SOURCE_MAP_ROLLBACK_FLAG = "legacy_single_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.chapter import Chapter


async def check_chapter_generation_prerequisites(
    db: AsyncSession,
    chapter: Chapter,
) -> tuple[bool, str, list[Chapter]]:
    """Check whether a chapter can start generation based on previous chapters."""
    from sqlalchemy import select

    from app.models.chapter import Chapter

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
        error_message = f"前置章节尚未完成: {', '.join(missing_numbers)} 章"
        return False, error_message, previous_chapters

    return True, "", previous_chapters
