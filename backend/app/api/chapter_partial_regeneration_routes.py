"""Partial chapter regeneration routes."""

from fastapi import APIRouter, Depends, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.settings import get_user_ai_service
from app.database import get_db
from app.schemas.chapter import PartialRegenerateRequest
from app.services.ai_service import AIService
from app.services.chapter_partial_regeneration_route_compat_service import (
    apply_partial_regenerate_with_default_route_wiring,
    partial_regenerate_stream_with_default_route_wiring,
)

router = APIRouter(prefix="/chapters", tags=["章节管理"])


@router.post("/{chapter_id}/partial-regenerate-stream", summary="局部重写章节片段")
async def partial_regenerate_stream(
    chapter_id: str,
    request: Request,
    partial_request: PartialRegenerateRequest,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """对章节选中片段进行局部重写并返回 SSE 流。"""
    return await partial_regenerate_stream_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        partial_request=partial_request,
        db_session=db,
        user_ai_service=user_ai_service,
    )


@router.post("/{chapter_id}/apply-partial-regenerate", summary="应用局部改写")
async def apply_partial_regenerate(
    chapter_id: str,
    request: Request,
    apply_request: dict,
    db: AsyncSession = Depends(get_db),
):
    """将局部重写结果写回到章节内容。"""
    return await apply_partial_regenerate_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        apply_request=apply_request,
        db_session=db,
    )
