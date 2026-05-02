from __future__ import annotations

from fastapi import BackgroundTasks, HTTPException, Request

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
# NOTE: keep get_db patchable in tests; avoid importing get_db directly.
from app.schemas.regeneration import ChapterRegenerateRequest
from app.services.ai_service import AIService
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
)
from app.services.chapter_regeneration_context_service import (
    prepare_chapter_regeneration_stream_context,
)
from app.services.chapter_regeneration_stream_service import (
    build_chapter_regeneration_event_stream,
)
from app.services.chapter_regenerator import ChapterRegenerator
from app.utils.sse_response import create_sse_response

from app.database import get_db

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
            db_session_source=lambda: __import__(
                "app.services.chapter_regeneration_route_compat_service",
                fromlist=["get_db"],
            ).get_db(request),
            context=regeneration_context,
            user_ai_service=user_ai_service,
            regenerator_factory=lambda ai_service: __import__(
                "app.services.chapter_regeneration_route_compat_service",
                fromlist=["REGENERATOR_FACTORY"],
            ).REGENERATOR_FACTORY(ai_service),
            sanitize_generated_text=sanitize_generated_narrative_text,
            contains_workflow_meta_text=contains_chapter_workflow_meta_text,
        )
    )
