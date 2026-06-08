"""章节标注 API。"""

from __future__ import annotations

from fastapi import APIRouter, Depends, Request
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.database import get_db
from app.models.memory import PlotAnalysis, StoryMemory
from app.services.chapter_annotation_service import (
    build_chapter_annotations_payload,
)

router = APIRouter(prefix="/chapters", tags=["章节管理"])


async def get_chapter_annotations_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    db_session: AsyncSession,
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    analysis_result = await db_session.execute(
        select(PlotAnalysis)
        .where(PlotAnalysis.chapter_id == chapter_id)
        .order_by(PlotAnalysis.created_at.desc())
        .limit(1)
    )
    analysis = analysis_result.scalar_one_or_none()

    memories_result = await db_session.execute(
        select(StoryMemory)
        .where(StoryMemory.chapter_id == chapter_id)
        .order_by(StoryMemory.importance_score.desc())
    )
    memories = memories_result.scalars().all()

    return build_chapter_annotations_payload(
        chapter=chapter,
        analysis=analysis,
        memories=memories,
    )


@router.get("/{chapter_id}/annotations", summary="获取章节标注")
async def get_chapter_annotations(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """获取章节的分析标注与记忆映射结果。"""
    return await get_chapter_annotations_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        db_session=db,
    )
