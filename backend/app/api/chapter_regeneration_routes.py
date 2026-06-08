"""Chapter regeneration API routes."""

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Query, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.api.settings import get_user_ai_service
from app.database import get_db
from app.schemas.regeneration import ChapterRegenerateRequest
from app.services.ai_service import AIService
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
)
from app.services.chapter_regeneration_context_service import (
    prepare_chapter_regeneration_stream_context,
)
from app.services.chapter_regeneration_query_service import (
    load_regeneration_tasks_payload,
)
from app.services.chapter_regeneration_stream_service import (
    build_chapter_regeneration_event_stream,
)
from app.services.chapter_regenerator import ChapterRegenerator
from app.utils.sse_response import create_sse_response

router = APIRouter(prefix='/chapters', tags=['章节管理'])

# 保持 route 模块上的 patch surface 稳定，测试直接 monkeypatch 这里。
REGENERATOR_FACTORY = ChapterRegenerator


async def regenerate_chapter_stream_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    regenerate_request: ChapterRegenerateRequest,
    background_tasks: BackgroundTasks,
    db_session,
    user_ai_service: AIService,
):
    _ = background_tasks
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    try:
        regeneration_context = await prepare_chapter_regeneration_stream_context(
            db_session,
            chapter=chapter,
            regenerate_request=regenerate_request,
            user_id=user_id,
        )
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    except LookupError as exc:
        raise HTTPException(status_code=404, detail=str(exc)) from exc

    return create_sse_response(
        build_chapter_regeneration_event_stream(
            db_session_source=lambda: get_db(request),
            context=regeneration_context,
            user_ai_service=user_ai_service,
            regenerator_factory=lambda ai_service: REGENERATOR_FACTORY(ai_service),
            sanitize_generated_text=sanitize_generated_narrative_text,
            contains_workflow_meta_text=contains_chapter_workflow_meta_text,
        )
    )


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
