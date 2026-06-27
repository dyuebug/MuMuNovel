from __future__ import annotations

from contextlib import suppress
from functools import lru_cache
from importlib import import_module
from typing import TYPE_CHECKING, Any, Dict

from fastapi import APIRouter, Depends, Request
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.chapter_route_helpers_test_support import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from tests.test_support.api_common_test_support import verify_project_access
from tests.test_support.database_test_support import get_db
from tests.test_support.chapter_schema_test_support import (
    ChapterCreate,
    ChapterListResponse,
    ChapterResponse,
    ChapterUpdate,
)
from tests.test_support.chapter_query_test_support import (
    load_chapter_navigation_payload,
    load_project_chapter_list_payload,
)
from tests.test_support.foreshadow_test_support import foreshadow_service
from tests.test_support.memory_service_test_support import memory_service

if TYPE_CHECKING:
    from migrator_app.models.chapter import Chapter
    from migrator_app.models.project import Project

router = APIRouter(prefix="/chapters", tags=["章节管理"])


@lru_cache(maxsize=1)
def _chapter_crud_models() -> tuple[type[Any], type[Any]]:
    chapter_module = import_module("migrator_app.models.chapter")
    import_module("migrator_app.models.outline")
    project_module = import_module("migrator_app.models.project")
    return chapter_module.Chapter, project_module.Project


async def create_chapter_record(
    *,
    db_session: AsyncSession,
    project: Project,
    chapter_create: ChapterCreate,
) -> Chapter:
    Chapter, _ = _chapter_crud_models()
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
    _, Project = _chapter_crud_models()
    old_word_count = int(chapter.word_count or 0)
    update_data = chapter_update.model_dump(exclude_unset=True)
    new_content = update_data.get("content", chapter.content)
    new_word_count = len(new_content) if new_content else 0

    for field, value in update_data.items():
        setattr(chapter, field, value)

    if "content" in update_data:
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
) -> Dict[str, bool]:
    _, Project = _chapter_crud_models()
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
    return {"success": True}


@router.post("", response_model=ChapterResponse, summary="Create chapter")
async def create_chapter(
    chapter: ChapterCreate,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = require_authenticated_user_id(request)
    project = await verify_project_access(chapter.project_id, user_id, db)
    return await create_chapter_record(
        db_session=db,
        project=project,
        chapter_create=chapter,
    )


@router.get(
    "/project/{project_id}",
    response_model=ChapterListResponse,
    summary="Get project chapters",
)
async def get_project_chapters(
    project_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = require_authenticated_user_id(request)
    await verify_project_access(project_id, user_id, db)
    return await load_project_chapter_list_payload(
        db_session=db,
        project_id=project_id,
    )


@router.get("/{chapter_id}", response_model=ChapterResponse, summary="Get chapter detail")
async def get_chapter(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = require_authenticated_user_id(request)
    return await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )


@router.put("/{chapter_id}", response_model=ChapterResponse, summary="Update chapter")
async def update_chapter(
    chapter_id: str,
    chapter_update: ChapterUpdate,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    return await update_chapter_record(
        db_session=db,
        chapter=chapter,
        chapter_update=chapter_update,
    )


@router.delete("/{chapter_id}", summary="Delete chapter")
async def delete_chapter(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    return await delete_chapter_record(
        db_session=db,
        chapter=chapter,
        user_id=user_id,
    )


@router.get("/{chapter_id}/navigation", summary="Get chapter navigation")
async def get_chapter_navigation(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = require_authenticated_user_id(request)
    current_chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    return await load_chapter_navigation_payload(
        db_session=db,
        current_chapter=current_chapter,
    )


