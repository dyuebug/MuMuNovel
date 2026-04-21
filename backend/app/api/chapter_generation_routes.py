from __future__ import annotations

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.settings import get_user_ai_service
from app.database import get_db
from app.schemas.chapter import ChapterGenerateRequest
from app.services.ai_service import AIService
from app.services.chapter_generation_route_compat_service import (
    generate_chapter_content_background_with_default_route_wiring,
    generate_chapter_content_stream_with_default_route_wiring,
)

router = APIRouter(prefix="/chapters", tags=["chapters"])


@router.post("/{chapter_id}/generate-stream", summary="AI stream chapter generation")
async def generate_chapter_content_stream(
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest = ChapterGenerateRequest(),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    return await generate_chapter_content_stream_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        generate_request=generate_request,
        user_ai_service=user_ai_service,
    )


@router.post("/{chapter_id}/generate-background", summary="AI background chapter generation")
async def generate_chapter_content_background(
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest = ChapterGenerateRequest(),
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    return await generate_chapter_content_background_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        generate_request=generate_request,
        db_session=db,
        user_ai_service=user_ai_service,
    )
