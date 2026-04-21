from __future__ import annotations

from typing import Any, Dict, Optional

from fastapi import APIRouter, Depends, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.api.common import verify_project_access
from app.database import get_db
from app.models.chapter import Chapter
from app.schemas.chapter import (
    ChapterCreate,
    ChapterListResponse,
    ChapterResponse,
    ChapterUpdate,
)
from app.services.chapter_crud_query_service import (
    load_chapter_navigation_payload,
    load_project_chapter_list_payload,
)
from app.services.foreshadow_service import foreshadow_service
from app.services.memory_service import memory_service
from app.services.chapter_crud_workflow_service import (
    create_chapter_record,
    delete_chapter_record,
    update_chapter_record,
)


router = APIRouter(prefix='/chapters', tags=['章节管理'])


@router.post('', response_model=ChapterResponse, summary='Create chapter')
async def create_chapter(
    chapter: ChapterCreate,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """Create a chapter and sync project word count."""
    user_id = require_authenticated_user_id(request)
    project = await verify_project_access(chapter.project_id, user_id, db)
    return await create_chapter_record(
        db_session=db,
        project=project,
        chapter_create=chapter,
    )


@router.get('/project/{project_id}', response_model=ChapterListResponse, summary='Get project chapters')
async def get_project_chapters(
    project_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """Return all chapters under a project with outline metadata."""
    user_id = require_authenticated_user_id(request)
    await verify_project_access(project_id, user_id, db)
    return await load_project_chapter_list_payload(
        db_session=db,
        project_id=project_id,
    )


@router.get('/{chapter_id}', response_model=ChapterResponse, summary='Get chapter detail')
async def get_chapter(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """Load one chapter by id after access control."""
    user_id = require_authenticated_user_id(request)
    return await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )


@router.put('/{chapter_id}', response_model=ChapterResponse, summary='Update chapter')
async def update_chapter(
    chapter_id: str,
    chapter_update: ChapterUpdate,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """Update chapter fields and sync project word count when content changes."""
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


@router.delete('/{chapter_id}', summary='Delete chapter')
async def delete_chapter(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """Delete a chapter and clean related side effects."""
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


@router.get('/{chapter_id}/navigation', summary='Get chapter navigation')
async def get_chapter_navigation(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """Return previous/current/next chapter navigation within the same project."""
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
