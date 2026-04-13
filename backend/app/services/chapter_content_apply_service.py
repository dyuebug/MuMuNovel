from __future__ import annotations

from dataclasses import dataclass

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.models.generation_history import GenerationHistory
from app.models.project import Project


@dataclass(frozen=True)
class ChapterContentApplyResult:
    old_word_count: int
    new_word_count: int


async def apply_chapter_content_update(
    db: AsyncSession,
    *,
    chapter: Chapter,
    content: str,
    history_entry: GenerationHistory | None = None,
    refresh_chapter: bool = True,
) -> ChapterContentApplyResult:
    old_word_count = chapter.word_count or len(chapter.content or "")
    new_word_count = len(content)

    chapter.content = content
    chapter.word_count = new_word_count

    project_result = await db.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project:
        current_words = project.current_words or 0
        project.current_words = max(0, current_words - old_word_count + new_word_count)

    if history_entry is not None:
        db.add(history_entry)

    await db.commit()
    if refresh_chapter:
        await db.refresh(chapter)

    return ChapterContentApplyResult(
        old_word_count=old_word_count,
        new_word_count=new_word_count,
    )
