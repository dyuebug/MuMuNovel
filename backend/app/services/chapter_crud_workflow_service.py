from __future__ import annotations

from contextlib import suppress

from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.models.project import Project
from app.schemas.chapter import ChapterCreate, ChapterUpdate
from app.services.foreshadow_service import foreshadow_service
from app.services.memory_service import memory_service


async def create_chapter_record(
    *,
    db_session: AsyncSession,
    project: Project,
    chapter_create: ChapterCreate,
) -> Chapter:
    word_count = len(chapter_create.content) if chapter_create.content else 0
    db_chapter = Chapter(
        **chapter_create.model_dump(),
        word_count=word_count,
    )
    db_session.add(db_chapter)
    project.current_words = int(project.current_words or 0) + word_count

    await db_session.commit()
    await db_session.refresh(db_chapter)
    return db_chapter


async def update_chapter_record(
    *,
    db_session: AsyncSession,
    chapter: Chapter,
    chapter_update: ChapterUpdate,
) -> Chapter:
    old_word_count = int(chapter.word_count or 0)
    update_data = chapter_update.model_dump(exclude_unset=True)
    new_content = update_data.get('content', chapter.content)
    new_word_count = len(new_content) if new_content else 0

    for field, value in update_data.items():
        setattr(chapter, field, value)

    if 'content' in update_data:
        chapter.word_count = new_word_count
        project = await db_session.get(Project, chapter.project_id)
        if project is not None:
            project.current_words = max(
                0,
                int(project.current_words or 0) - old_word_count + new_word_count,
            )

    await db_session.commit()
    await db_session.refresh(chapter)
    return chapter


async def delete_chapter_record(
    *,
    db_session: AsyncSession,
    chapter: Chapter,
    user_id: str,
) -> dict[str, bool]:
    project = await db_session.get(Project, chapter.project_id)
    if project is not None:
        project.current_words = max(
            0,
            int(project.current_words or 0) - int(chapter.word_count or 0),
        )

    with suppress(Exception):
        await memory_service.delete_chapter_memories(
            user_id=user_id,
            project_id=chapter.project_id,
            chapter_id=chapter.id,
        )
    with suppress(Exception):
        await foreshadow_service.delete_chapter_foreshadows(
            db=db_session,
            project_id=chapter.project_id,
            chapter_id=chapter.id,
            only_analysis_source=True,
        )

    await db_session.delete(chapter)
    await db_session.commit()
    return {'success': True}
