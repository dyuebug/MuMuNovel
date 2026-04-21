"""Batch generation chapter routes."""

from __future__ import annotations

from datetime import datetime

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Query, Request
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.common import verify_project_access
from app.api.settings import get_user_ai_service
from app.database import get_db
from app.logger import get_logger
from app.models.batch_generation_task import BatchGenerationTask
from app.schemas.chapter import (
    BatchGenerateRequest,
    BatchGenerateResponse,
    BatchGenerateStatusResponse,
)
from app.services.ai_service import AIService
from app.services.batch_generation_route_compat_service import (
    orchestrate_batch_generation_create_with_default_wiring,
    orchestrate_batch_generation_resume_with_default_wiring,
    stream_batch_generation_events_with_default_route_wiring,
)
from app.services.batch_generation_query_service import (
    load_active_project_batch_generation_task_view_context,
    load_active_user_batch_generation_task_view_contexts,
    load_batch_generation_task_view_context,
)
from app.services.batch_generation_status_service import (
    build_active_batch_generation_payload,
    build_batch_generation_status_response,
    build_batch_generation_task_list_item,
)

router = APIRouter(prefix="/chapters", tags=["chapter-batch-generation"])
logger = get_logger(__name__)


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
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
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
    db: AsyncSession = Depends(get_db),
):
    task_view = await load_batch_generation_task_view_context(
        db,
        batch_id=batch_id,
    )
    if task_view is None:
        raise HTTPException(status_code=404, detail="Batch generation task not found")

    return build_batch_generation_status_response(
        task_view.task,
        quality_snapshot=task_view.quality_snapshot,
        workflow_snapshot=task_view.workflow_snapshot,
    )


@router.get(
    "/batch-generate/{batch_id}/stream",
    summary="Stream batch generation events",
)
async def stream_batch_generation_events(
    batch_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    return await stream_batch_generation_events_with_default_route_wiring(
        db,
        batch_id=batch_id,
        request=request,
    )


@router.get(
    "/project/{project_id}/batch-generate/active",
    summary="Get active project batch generation",
)
async def get_active_batch_generation(
    project_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = getattr(request.state, "user_id", None)
    await verify_project_access(project_id, user_id, db)

    task_view = await load_active_project_batch_generation_task_view_context(
        db,
        project_id=project_id,
    )
    if task_view is None:
        return {
            "has_active_task": False,
            "task": None,
        }

    return build_active_batch_generation_payload(
        task_view.task,
        quality_snapshot=task_view.quality_snapshot,
        workflow_snapshot=task_view.workflow_snapshot,
    )


@router.get(
    "/batch-generate/active-tasks",
    summary="List active batch generation tasks",
)
async def list_active_batch_generation_tasks(
    request: Request,
    db: AsyncSession = Depends(get_db),
    limit: int = Query(default=20, ge=1, le=100),
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="Not logged in")

    task_views = await load_active_user_batch_generation_task_view_contexts(
        db,
        user_id=user_id,
        limit=limit,
    )
    items = [
        build_batch_generation_task_list_item(
            task_view.task,
            quality_snapshot=task_view.quality_snapshot,
            workflow_snapshot=task_view.workflow_snapshot,
        )
        for task_view in task_views
    ]
    return {
        "total": len(items),
        "items": items,
    }


@router.post(
    "/batch-generate/{batch_id}/cancel",
    summary="Cancel batch generation",
)
async def cancel_batch_generation(
    batch_id: str,
    db: AsyncSession = Depends(get_db),
):
    result = await db.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    task = result.scalar_one_or_none()

    if not task:
        raise HTTPException(status_code=404, detail="Batch generation task not found")

    if task.status in ["completed", "failed", "cancelled"]:
        raise HTTPException(
            status_code=400,
            detail=f"Cannot cancel task in status {task.status}",
        )

    task.status = "cancelled"
    task.completed_at = datetime.now()
    await db.commit()

    logger.info(f"Cancelled batch generation task {batch_id}")
    return {
        "message": "Batch generation cancelled",
        "batch_id": batch_id,
        "completed_chapters": task.completed_chapters,
        "total_chapters": task.total_chapters,
    }


@router.post(
    "/batch-generate/{batch_id}/resume",
    summary="Resume batch generation",
)
async def resume_batch_generation(
    batch_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
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
