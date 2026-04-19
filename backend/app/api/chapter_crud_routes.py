from __future__ import annotations

from contextlib import suppress
from typing import Any, Dict, Optional

from fastapi import APIRouter, Depends, Request
from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.api.common import verify_project_access
from app.database import get_db
from app.models.chapter import Chapter
from app.models.outline import Outline
from app.models.project import Project
from app.schemas.chapter import (
    ChapterCreate,
    ChapterListResponse,
    ChapterResponse,
    ChapterUpdate,
)
from app.services.foreshadow_service import foreshadow_service
from app.services.memory_service import memory_service


router = APIRouter(prefix="/chapters", tags=["章节管理"])


@router.post("", response_model=ChapterResponse, summary="创建章节")
async def create_chapter(
    chapter: ChapterCreate,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """创建新章节并更新项目字数。"""
    user_id = require_authenticated_user_id(request)
    project = await verify_project_access(chapter.project_id, user_id, db)

    word_count = len(chapter.content) if chapter.content else 0
    db_chapter = Chapter(
        **chapter.model_dump(),
        word_count=word_count,
    )
    db.add(db_chapter)
    project.current_words = int(project.current_words or 0) + word_count

    await db.commit()
    await db.refresh(db_chapter)
    return db_chapter


@router.get("/project/{project_id}", response_model=ChapterListResponse, summary="获取项目章节列表")
async def get_project_chapters(
    project_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """获取项目下的全部章节及其关联大纲信息。"""
    user_id = require_authenticated_user_id(request)
    await verify_project_access(project_id, user_id, db)

    count_result = await db.execute(
        select(func.count(Chapter.id)).where(Chapter.project_id == project_id)
    )
    total = int(count_result.scalar_one() or 0)

    chapters_result = await db.execute(
        select(Chapter)
        .where(Chapter.project_id == project_id)
        .order_by(Chapter.chapter_number.asc())
    )
    chapters = list(chapters_result.scalars().all())

    outline_ids = [chapter.outline_id for chapter in chapters if chapter.outline_id]
    outlines_map: Dict[str, Outline] = {}
    if outline_ids:
        outlines_result = await db.execute(
            select(Outline).where(Outline.id.in_(outline_ids))
        )
        outlines_map = {outline.id: outline for outline in outlines_result.scalars().all()}

    items = []
    for chapter in chapters:
        outline = outlines_map.get(chapter.outline_id) if chapter.outline_id else None
        items.append({
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
            "outline_order": getattr(outline, "outline_order", None),
            "created_at": chapter.created_at,
            "updated_at": chapter.updated_at,
        })

    return {
        "total": total,
        "items": items,
    }


@router.get("/{chapter_id}", response_model=ChapterResponse, summary="获取章节详情")
async def get_chapter(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """根据 ID 获取单个章节详情。"""
    user_id = require_authenticated_user_id(request)
    return await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )


@router.put("/{chapter_id}", response_model=ChapterResponse, summary="更新章节")
async def update_chapter(
    chapter_id: str,
    chapter_update: ChapterUpdate,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """更新章节内容并同步项目字数变化。"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    old_word_count = int(chapter.word_count or 0)
    update_data = chapter_update.model_dump(exclude_unset=True)
    new_content = update_data.get("content", chapter.content)
    new_word_count = len(new_content) if new_content else 0

    for field, value in update_data.items():
        setattr(chapter, field, value)

    if "content" in update_data:
        chapter.word_count = new_word_count
        project = await db.get(Project, chapter.project_id)
        if project is not None:
            project.current_words = max(
                0,
                int(project.current_words or 0) - old_word_count + new_word_count,
            )

    await db.commit()
    await db.refresh(chapter)
    return chapter


@router.delete("/{chapter_id}", summary="删除章节")
async def delete_chapter(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """删除章节并回收对应项目字数。"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    project = await db.get(Project, chapter.project_id)
    if project is not None:
        project.current_words = max(
            0,
            int(project.current_words or 0) - int(chapter.word_count or 0),
        )

    with suppress(Exception):
        await memory_service.delete_chapter_memories(
            user_id=user_id,
            project_id=chapter.project_id,
            chapter_id=chapter_id,
        )
    with suppress(Exception):
        await foreshadow_service.delete_chapter_foreshadows(
            db=db,
            project_id=chapter.project_id,
            chapter_id=chapter_id,
            only_analysis_source=True,
        )

    await db.delete(chapter)
    await db.commit()
    return {"success": True}


@router.get("/{chapter_id}/navigation", summary="获取章节导航")
async def get_chapter_navigation(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """获取章节的上一章、下一章与目录导航信息。"""
    user_id = require_authenticated_user_id(request)
    current_chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    previous_result = await db.execute(
        select(Chapter)
        .where(Chapter.project_id == current_chapter.project_id)
        .where(Chapter.chapter_number < current_chapter.chapter_number)
        .order_by(Chapter.chapter_number.desc())
        .limit(1)
    )
    next_result = await db.execute(
        select(Chapter)
        .where(Chapter.project_id == current_chapter.project_id)
        .where(Chapter.chapter_number > current_chapter.chapter_number)
        .order_by(Chapter.chapter_number.asc())
        .limit(1)
    )

    previous_chapter = previous_result.scalar_one_or_none()
    next_chapter = next_result.scalar_one_or_none()

    def _serialize_navigation_item(chapter: Chapter | None) -> Optional[Dict[str, Any]]:
        if chapter is None:
            return None
        return {
            "id": chapter.id,
            "chapter_number": chapter.chapter_number,
            "title": chapter.title,
        }

    return {
        "current": _serialize_navigation_item(current_chapter),
        "previous": _serialize_navigation_item(previous_chapter),
        "next": _serialize_navigation_item(next_chapter),
    }
