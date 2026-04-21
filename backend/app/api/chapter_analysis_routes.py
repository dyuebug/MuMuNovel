"""章节分析 API。"""

from __future__ import annotations

from fastapi import APIRouter, Depends, Query, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.database import get_db
from app.services.chapter_analysis_route_compat_service import (
    get_chapter_analysis_with_default_route_wiring,
)

router = APIRouter(prefix="/chapters", tags=["章节管理"])


@router.get("/{chapter_id}/analysis", summary="获取章节分析")
async def get_chapter_analysis(
    chapter_id: str,
    request: Request,
    include_full_draft: bool = Query(False, description="是否包含完整草稿"),
    db: AsyncSession = Depends(get_db),
):
    """获取指定章节的分析结果。"""
    return await get_chapter_analysis_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        include_full_draft=include_full_draft,
        db_session=db,
    )
