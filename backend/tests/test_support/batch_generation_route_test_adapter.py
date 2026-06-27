from __future__ import annotations

import asyncio
from datetime import datetime

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Query, Request

from tests.test_support.retired_runtime_test_support import get_logger
from tests.test_support.chapter_schema_test_support import (
    BatchGenerateRequest,
    BatchGenerateResponse,
    BatchGenerateStatusResponse,
)

logger = get_logger(__name__)
router = APIRouter(prefix="/chapters", tags=["chapter-batch-generation"])


async def get_db(request: Request):
    from tests.test_support.database_test_support import get_db as app_get_db

    async for session in app_get_db(request):
        yield session


async def get_user_ai_service(request: Request, db=Depends(get_db)):
    from tests.test_support.ai_dependencies_test_support import (
        get_user_ai_service as app_get_user_ai_service,
        require_login,
    )

    return await app_get_user_ai_service(user=require_login(request), db=db)


async def verify_project_access(*args, **kwargs):
    from tests.test_support.api_common_test_support import verify_project_access as verify_project_access_service

    return await verify_project_access_service(*args, **kwargs)


async def cancel_batch_generation_task(
    db_session,
    *,
    batch_id: str,
):
    from sqlalchemy import select
    from migrator_app.models.batch_generation_task import BatchGenerationTask

    result = await db_session.execute(
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
    await db_session.commit()

    logger.info(f"Cancelled batch generation task {batch_id}")
    return {
        "message": "Batch generation cancelled",
        "batch_id": batch_id,
        "completed_chapters": task.completed_chapters,
        "total_chapters": task.total_chapters,
    }


async def validate_batch_generation_stream_access(
    db_session,
    *,
    batch_id: str,
    user_id: str | None,
):
    from sqlalchemy import select
    from migrator_app.models.batch_generation_task import BatchGenerationTask

    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    result = await db_session.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    task = result.scalar_one_or_none()
    if not task:
        raise HTTPException(status_code=404, detail="未找到批量生成任务")
    if task.user_id != user_id:
        raise HTTPException(status_code=403, detail="无权访问该任务")
    return task


def build_batch_generation_event_stream(
    db_session,
    *,
    batch_id: str,
    idle_timeout_seconds: int = 15,
):
    from sqlalchemy import select
    from migrator_app.models.batch_generation_task import BatchGenerationTask
    from tests.test_support.task_system import (
        subscribe_task_stream,
        unsubscribe_task_stream,
    )
    from tests.test_support.utils.sse_response import SSEResponse

    async def _stream():
        queue = await subscribe_task_stream(batch_id)
        try:
            yield await SSEResponse.send_progress("正在连接批量生成任务流", 0, "processing")

            while True:
                try:
                    event = await asyncio.wait_for(queue.get(), timeout=idle_timeout_seconds)
                    yield SSEResponse.format_sse(event)

                    if event.get("type") in {"done", "error"}:
                        break
                except asyncio.TimeoutError:
                    yield await SSEResponse.send_heartbeat()

                    status_result = await db_session.execute(
                        select(
                            BatchGenerationTask.status,
                            BatchGenerationTask.error_message,
                        ).where(BatchGenerationTask.id == batch_id)
                    )
                    row = status_result.first()
                    if not row:
                        yield await SSEResponse.send_error("批量生成任务不存在", 404)
                        break

                    status = row[0]
                    error_message = row[1]
                    if status in {"completed", "cancelled"}:
                        yield await SSEResponse.send_done()
                        break
                    if status == "failed":
                        yield await SSEResponse.send_error(
                            error_message or "批量生成任务执行失败",
                            500,
                        )
                        break
        finally:
            await unsubscribe_task_stream(batch_id, queue)

    return _stream()


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

    from tests.test_support.batch_generation_orchestration_test_adapter import (
        orchestrate_batch_generation_create,
    )
    from tests.test_support.chapter_query_test_support import (
        check_chapter_generation_prerequisites,
    )
    from tests.test_support.story_continuity_ledger_test_support import (
        build_story_generation_packet_with_project_continuity,
    )
    from tests.test_support.chapter_prompt_quality_test_support import (
        resolve_chapter_quality_profile,
    )
    from tests.test_support.story_repair_payload_test_support import (
        resolve_generation_story_repair_state_for_batch,
    )
    from tests.test_support.task_system import (
        sync_task_story_repair_state,
    )

    project = await verify_project_access(project_id, user_id, db)
    response_payload = await orchestrate_batch_generation_create(
        db,
        project_id=project_id,
        project=project,
        user_id=user_id,
        batch_request=batch_request,
        background_tasks=background_tasks,
        ai_service=user_ai_service,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        resolve_quality_profile_fn=resolve_chapter_quality_profile,
        build_story_packet_fn=build_story_generation_packet_with_project_continuity,
        resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_batch,
        sync_task_story_repair_state_fn=sync_task_story_repair_state,
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
    from tests.test_support.batch_generation_status_read_owner_test_adapter import (
        build_batch_generation_status_response,
        load_batch_generation_task_view_context,
    )

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
    db=Depends(get_db),
):
    from tests.test_support.utils.sse_response import create_sse_response

    user_id = getattr(request.state, "user_id", None)
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
    from tests.test_support.batch_generation_status_read_owner_test_adapter import (
        build_active_batch_generation_payload,
        load_active_project_batch_generation_task_view_context,
    )

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
    db=Depends(get_db),
    limit: int = Query(default=20, ge=1, le=100),
):
    user_id = getattr(request.state, "user_id", None)
    from tests.test_support.batch_generation_status_read_owner_test_adapter import (
        build_batch_generation_task_list_item,
        load_active_user_batch_generation_task_view_contexts,
    )

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

    return {"total": len(items), "items": items}


@router.post(
    "/batch-generate/{batch_id}/cancel",
    summary="Cancel batch generation",
)
async def cancel_batch_generation(
    batch_id: str,
    db=Depends(get_db),
):
    return await cancel_batch_generation_task(
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

    from tests.test_support.batch_generation_orchestration_test_adapter import (
        orchestrate_batch_generation_resume,
    )
    from tests.test_support.batch_generation_status_read_owner_test_adapter import (
        build_batch_task_terminal_status,
    )
    from tests.test_support.chapter_query_test_support import (
        check_chapter_generation_prerequisites,
    )
    from tests.test_support.story_repair_payload_test_support import (
        resolve_generation_story_repair_state_for_batch,
    )

    return await orchestrate_batch_generation_resume(
        db,
        batch_id=batch_id,
        user_id=user_id,
        background_tasks=background_tasks,
        ai_service=user_ai_service,
        resolve_story_repair_state_for_batch=resolve_generation_story_repair_state_for_batch,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        build_batch_task_terminal_status_fn=build_batch_task_terminal_status,
    )





