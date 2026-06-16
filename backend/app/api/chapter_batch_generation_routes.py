"""Frozen batch-generation route source map.

This module is kept only for explicit Python rollback registration through
`legacy_batch_generation_python_routes_enabled`. The active route owner is
`backend-rs/src/api/chapter_batch_generation.rs`; do not add new business
logic here.
"""

from __future__ import annotations

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Query, Request

from app.logger import get_logger
from app.schemas.chapter import (
    BatchGenerateRequest,
    BatchGenerateResponse,
    BatchGenerateStatusResponse,
)

router = APIRouter(prefix="/chapters", tags=["chapter-batch-generation"])
logger = get_logger(__name__)

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the default batch-generation route path; this Python route "
    "shell remains available only behind the explicit legacy rollback flag."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/api/chapter_batch_generation.rs"
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"


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


async def verify_project_access(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        verify_project_access as verify_project_access_service,
    )

    return await verify_project_access_service(*args, **kwargs)


async def orchestrate_batch_generation_create_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation.route_wiring_service import (
        orchestrate_batch_generation_create_with_default_wiring as orchestrate_batch_generation_create_with_default_wiring_service,
    )

    return await orchestrate_batch_generation_create_with_default_wiring_service(
        *args,
        **kwargs,
    )


async def orchestrate_batch_generation_resume_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation.route_wiring_service import (
        orchestrate_batch_generation_resume_with_default_wiring as orchestrate_batch_generation_resume_with_default_wiring_service,
    )

    return await orchestrate_batch_generation_resume_with_default_wiring_service(
        *args,
        **kwargs,
    )


async def stream_batch_generation_events_with_default_route_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation.route_wiring_service import (
        stream_batch_generation_events_with_default_route_wiring as stream_batch_generation_events_with_default_route_wiring_service,
    )

    return await stream_batch_generation_events_with_default_route_wiring_service(
        *args,
        **kwargs,
    )


async def validate_batch_generation_stream_access(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        validate_batch_generation_stream_access as validate_batch_generation_stream_access_service,
    )

    return await validate_batch_generation_stream_access_service(*args, **kwargs)


def build_batch_generation_event_stream(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        build_batch_generation_event_stream as build_batch_generation_event_stream_service,
    )

    return build_batch_generation_event_stream_service(*args, **kwargs)


async def cancel_batch_generation_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation.route_wiring_service import (
        cancel_batch_generation_with_default_wiring as cancel_batch_generation_with_default_wiring_service,
    )

    return await cancel_batch_generation_with_default_wiring_service(*args, **kwargs)


async def load_batch_generation_status_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation.route_wiring_service import (
        load_batch_generation_status_with_default_wiring as load_batch_generation_status_with_default_wiring_service,
    )

    return await load_batch_generation_status_with_default_wiring_service(
        *args,
        **kwargs,
    )


async def load_active_project_batch_generation_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation.route_wiring_service import (
        load_active_project_batch_generation_with_default_wiring as load_active_project_batch_generation_with_default_wiring_service,
    )

    return await load_active_project_batch_generation_with_default_wiring_service(
        *args,
        **kwargs,
    )


async def load_active_batch_generation_task_list_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation.route_wiring_service import (
        load_active_batch_generation_task_list_with_default_wiring as load_active_batch_generation_task_list_with_default_wiring_service,
    )

    return await load_active_batch_generation_task_list_with_default_wiring_service(
        *args,
        **kwargs,
    )


@router.post(
    "/project/{project_id}/batch-generate",
    response_model=BatchGenerateResponse,
    summary="Create batch generation task",
)
async def batch_generate_chapters_in_order(
    project_id: str,
    batch_request: BatchGenerateRequest,
    request: Request,
    background_tasks: BackgroundTasks,
    db=Depends(get_db),
    user_ai_service=Depends(get_user_ai_service),
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="Not logged in")

    project = await verify_project_access(project_id, user_id, db)
    response_payload = await orchestrate_batch_generation_create_with_default_wiring(
        db,
        project_id=project_id,
        project=project,
        user_id=user_id,
        batch_request=batch_request,
        background_tasks=background_tasks,
        ai_service=user_ai_service,
    )
    return BatchGenerateResponse(**response_payload)


@router.get(
    "/batch-generate/{batch_id}/status",
    response_model=BatchGenerateStatusResponse,
    summary="Get batch generation status",
)
async def get_batch_generation_status(
    batch_id: str,
    db=Depends(get_db),
):
    return await load_batch_generation_status_with_default_wiring(
        db,
        batch_id=batch_id,
    )


@router.get(
    "/batch-generate/{batch_id}/stream",
    summary="Stream batch generation events",
)
async def stream_batch_generation_events(
    batch_id: str,
    request: Request,
    db=Depends(get_db),
):
    return await stream_batch_generation_events_with_default_route_wiring(
        db,
        batch_id=batch_id,
        request=request,
        validate_stream_access_fn=validate_batch_generation_stream_access,
        build_stream_fn=build_batch_generation_event_stream,
    )


@router.get(
    "/project/{project_id}/batch-generate/active",
    summary="Get active project batch generation",
)
async def get_active_batch_generation(
    project_id: str,
    request: Request,
    db=Depends(get_db),
):
    user_id = getattr(request.state, "user_id", None)
    return await load_active_project_batch_generation_with_default_wiring(
        db,
        project_id=project_id,
        user_id=user_id,
    )


@router.get(
    "/batch-generate/active-tasks",
    summary="List active batch generation tasks",
)
async def list_active_batch_generation_tasks(
    request: Request,
    db=Depends(get_db),
    limit: int = Query(default=20, ge=1, le=100),
):
    user_id = getattr(request.state, "user_id", None)
    return await load_active_batch_generation_task_list_with_default_wiring(
        db,
        user_id=user_id,
        limit=limit,
    )


@router.post(
    "/batch-generate/{batch_id}/cancel",
    summary="Cancel batch generation",
)
async def cancel_batch_generation(
    batch_id: str,
    db=Depends(get_db),
):
    return await cancel_batch_generation_with_default_wiring(
        db,
        batch_id=batch_id,
    )


@router.post(
    "/batch-generate/{batch_id}/resume",
    summary="Resume batch generation",
)
async def resume_batch_generation(
    batch_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    db=Depends(get_db),
    user_ai_service=Depends(get_user_ai_service),
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="Not logged in")

    return await orchestrate_batch_generation_resume_with_default_wiring(
        db,
        batch_id=batch_id,
        user_id=user_id,
        background_tasks=background_tasks,
        ai_service=user_ai_service,
    )
