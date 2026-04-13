"""??????? API?"""

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Query, Request
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.api.settings import get_user_ai_service
from app.database import get_db
from app.models.chapter import Chapter
from app.models.regeneration_task import RegenerationTask
from app.schemas.regeneration import ChapterRegenerateRequest
from app.services.ai_service import AIService
from app.services.chapter_regeneration_context_service import (
    prepare_chapter_regeneration_stream_context,
)
from app.services.chapter_regeneration_stream_service import (
    build_chapter_regeneration_event_stream,
)
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
)
from app.services.chapter_regenerator import ChapterRegenerator
from app.utils.sse_response import create_sse_response

router = APIRouter(prefix="/chapters", tags=["????"])
REGENERATOR_FACTORY = ChapterRegenerator


@router.post("/{chapter_id}/regenerate-stream", summary="???????")
async def regenerate_chapter_stream(
    chapter_id: str,
    request: Request,
    regenerate_request: ChapterRegenerateRequest,
    background_tasks: BackgroundTasks,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """? SSE ?????????????????????"""
    _ = background_tasks
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    try:
        regeneration_context = await prepare_chapter_regeneration_stream_context(
            db,
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
            regenerator_factory=REGENERATOR_FACTORY,
            sanitize_generated_text=sanitize_generated_narrative_text,
            contains_workflow_meta_text=contains_chapter_workflow_meta_text,
        )
    )


@router.get("/{chapter_id}/regeneration/tasks", summary="???????????")
async def get_regeneration_tasks(
    chapter_id: str,
    request: Request,
    limit: int = Query(10, ge=1, le=50),
    db: AsyncSession = Depends(get_db),
):
    """???????????????"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    result = await db.execute(
        select(RegenerationTask)
        .where(RegenerationTask.chapter_id == chapter_id)
        .order_by(RegenerationTask.created_at.desc())
        .limit(limit)
    )
    tasks = result.scalars().all()

    return {
        "chapter_id": chapter_id,
        "total": len(tasks),
        "tasks": [
            {
                "task_id": task.id,
                "status": task.status,
                "version_number": task.version_number,
                "version_note": task.version_note,
                "original_word_count": task.original_word_count,
                "regenerated_word_count": task.regenerated_word_count,
                "created_at": task.created_at.isoformat() if task.created_at else None,
                "completed_at": task.completed_at.isoformat() if task.completed_at else None,
            }
            for task in tasks
        ],
    }
