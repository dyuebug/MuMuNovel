"""Frozen single-generation route source map.

This module is kept only for explicit Python rollback registration through
`legacy_single_generation_python_routes_enabled`. The active route owner is
`backend-rs/src/api/chapter_generation_routes.rs`; do not add new business
logic here.
"""

from __future__ import annotations

from fastapi import APIRouter, BackgroundTasks, Depends, Request

from app.schemas.chapter import ChapterGenerateRequest

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the default single-generation route path; this Python route "
    "shell remains available only behind the explicit legacy rollback flag."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/api/chapter_generation_routes.rs"
SOURCE_MAP_ROLLBACK_FLAG = "legacy_single_generation_python_routes_enabled"

router = APIRouter(prefix="/chapters", tags=["chapters"])


async def get_db(request: Request):
    from app.database import get_db as app_get_db

    async for session in app_get_db(request):
        yield session


async def get_user_ai_service(request: Request, db=Depends(get_db)):
    from app.api.settings import (
        get_user_ai_service as app_get_user_ai_service,
        require_login,
    )

    return await app_get_user_ai_service(user=require_login(request), db=db)


@router.post("/{chapter_id}/generate-stream", summary="AI stream chapter generation")
async def generate_chapter_content_stream(
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest = ChapterGenerateRequest(),
    user_ai_service=Depends(get_user_ai_service),
):
    from app.services.chapter_generation.route_wiring_service import (
        generate_chapter_content_stream_with_default_route_wiring,
    )

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
    db=Depends(get_db),
    user_ai_service=Depends(get_user_ai_service),
):
    from app.services.chapter_generation.route_wiring_service import (
        generate_chapter_content_background_with_default_route_wiring,
    )

    return await generate_chapter_content_background_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        generate_request=generate_request,
        db_session=db,
        user_ai_service=user_ai_service,
    )
