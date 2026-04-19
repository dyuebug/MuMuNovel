"""章节批量生成 API。"""

from __future__ import annotations

from datetime import datetime

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Query, Request
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api import chapters as chapters_api
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
from app.services.batch_generation_orchestration_service import (
    orchestrate_batch_generation_create,
    orchestrate_batch_generation_resume,
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
from app.services.batch_generation_stream_service import (
    build_batch_generation_event_stream,
    validate_batch_generation_stream_access,
)
from app.utils.sse_response import create_sse_response

router = APIRouter(prefix="/chapters", tags=["章节管理"])
logger = get_logger(__name__)


@router.post("/project/{project_id}/batch-generate", response_model=BatchGenerateResponse, summary="按顺序批量生成章节")
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
        raise HTTPException(status_code=401, detail="未登录")

    project = await verify_project_access(project_id, user_id, db)
    response_payload = await orchestrate_batch_generation_create(
        db,
        project_id=project_id,
        project=project,
        user_id=user_id,
        batch_request=batch_request,
        background_tasks=background_tasks,
        ai_service=user_ai_service,
        check_prerequisites_fn=chapters_api.check_prerequisites,
        resolve_quality_profile_fn=chapters_api.resolve_chapter_quality_profile,
        build_story_packet_fn=chapters_api.build_story_generation_packet_with_project_continuity,
        resolve_story_repair_state_fn=chapters_api._resolve_generation_story_repair_state_for_batch,
        sync_task_story_repair_state_fn=chapters_api._sync_task_story_repair_state,
        execution_callable=chapters_api.execute_batch_generation_in_order,
    )
    return BatchGenerateResponse(**response_payload)


@router.get("/batch-generate/{batch_id}/status", response_model=BatchGenerateStatusResponse, summary="获取批量生成任务状态")
async def get_batch_generation_status(
    batch_id: str,
    db: AsyncSession = Depends(get_db),
):
    task_view = await load_batch_generation_task_view_context(
        db,
        batch_id=batch_id,
    )
    if task_view is None:
        raise HTTPException(status_code=404, detail="未找到批量生成任务")

    return build_batch_generation_status_response(
        task_view.task,
        quality_snapshot=task_view.quality_snapshot,
        workflow_snapshot=task_view.workflow_snapshot,
    )


@router.get("/batch-generate/{batch_id}/stream", summary="批量生成任务事件流")
async def stream_batch_generation_events(
    batch_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = getattr(request.state, 'user_id', None)
    await validate_batch_generation_stream_access(
        db,
        batch_id=batch_id,
        user_id=user_id,
    )
    return create_sse_response(
        build_batch_generation_event_stream(
            db,
            batch_id=batch_id,
        )
    )


@router.get("/project/{project_id}/batch-generate/active", summary="获取项目当前激活的批量生成任务")
async def get_active_batch_generation(
    project_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = getattr(request.state, 'user_id', None)
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


@router.get("/batch-generate/active-tasks", summary="获取当前用户的激活批量生成任务列表")
async def list_active_batch_generation_tasks(
    request: Request,
    db: AsyncSession = Depends(get_db),
    limit: int = Query(default=20, ge=1, le=100),
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

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


@router.post("/batch-generate/{batch_id}/cancel", summary="取消批量生成任务")
async def cancel_batch_generation(
    batch_id: str,
    db: AsyncSession = Depends(get_db),
):
    result = await db.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    task = result.scalar_one_or_none()

    if not task:
        raise HTTPException(status_code=404, detail="未找到批量生成任务")

    if task.status in ['completed', 'failed', 'cancelled']:
        raise HTTPException(status_code=400, detail=f"任务状态为 {task.status}，无法取消")

    task.status = 'cancelled'
    task.completed_at = datetime.now()
    await db.commit()

    logger.info(f"已取消批量生成任务: {batch_id}")
    return {
        "message": "批量生成任务已取消",
        "batch_id": batch_id,
        "completed_chapters": task.completed_chapters,
        "total_chapters": task.total_chapters,
    }


@router.post("/batch-generate/{batch_id}/resume", summary="恢复批量生成任务")
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

    return await orchestrate_batch_generation_resume(
        db,
        batch_id=batch_id,
        user_id=user_id,
        background_tasks=background_tasks,
        ai_service=user_ai_service,
        resolve_story_repair_state_for_batch=chapters_api._resolve_generation_story_repair_state_for_batch,
        check_prerequisites_fn=chapters_api.check_prerequisites,
        execution_callable=chapters_api.execute_batch_generation_in_order,
    )
