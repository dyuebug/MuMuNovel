"""章节规划相关 API。"""

from fastapi import APIRouter, Depends, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.database import get_db
from app.schemas.chapter import ExpansionPlanUpdate
from app.services.chapter_expansion_plan_route_compat_service import (
    update_chapter_expansion_plan_with_default_route_wiring,
)

router = APIRouter(prefix="/chapters", tags=["章节管理"])


@router.put("/{chapter_id}/expansion-plan", response_model=dict, summary="更新章节规划信息")
async def update_chapter_expansion_plan(
    chapter_id: str,
    expansion_plan: ExpansionPlanUpdate,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """更新章节的展开规划信息和情节概要。"""
    return await update_chapter_expansion_plan_with_default_route_wiring(
        chapter_id=chapter_id,
        expansion_plan=expansion_plan,
        request=request,
        db_session=db,
    )
