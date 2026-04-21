"""Chapter regeneration API routes."""

from fastapi import APIRouter, BackgroundTasks, Depends, Query, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.api.settings import get_user_ai_service
from app.database import get_db
from app.schemas.regeneration import ChapterRegenerateRequest
from app.services.ai_service import AIService
from app.services.chapter_regeneration_query_service import (
    load_regeneration_tasks_payload,
)
from app.services.chapter_regeneration_route_compat_service import (
    regenerate_chapter_stream_with_default_route_wiring,
)

router = APIRouter(prefix='/chapters', tags=['章节管理'])


@router.post('/{chapter_id}/regenerate-stream', summary='Regenerate chapter stream')
async def regenerate_chapter_stream(
    chapter_id: str,
    request: Request,
    regenerate_request: ChapterRegenerateRequest,
    background_tasks: BackgroundTasks,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """Run regeneration with SSE streaming output."""
    return await regenerate_chapter_stream_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        regenerate_request=regenerate_request,
        background_tasks=background_tasks,
        db_session=db,
        user_ai_service=user_ai_service,
    )


@router.get('/{chapter_id}/regeneration/tasks', summary='Get regeneration task history')
async def get_regeneration_tasks(
    chapter_id: str,
    request: Request,
    limit: int = Query(10, ge=1, le=50),
    db: AsyncSession = Depends(get_db),
):
    """Return regeneration task history for one chapter."""
    user_id = require_authenticated_user_id(request)
    await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    return await load_regeneration_tasks_payload(
        db_session=db,
        chapter_id=chapter_id,
        limit=limit,
    )
