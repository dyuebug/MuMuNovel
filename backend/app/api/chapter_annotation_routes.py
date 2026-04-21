"""章节标注 API。"""

from fastapi import APIRouter, Depends, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.database import get_db
from app.services.chapter_annotation_route_compat_service import (
    get_chapter_annotations_with_default_route_wiring,
)

router = APIRouter(prefix="/chapters", tags=["章节管理"])


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
