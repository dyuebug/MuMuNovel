from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING, AsyncGenerator, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation status stream route and stream "
    "state contract; this Python stream module is kept only as frozen "
    "rollback/source-map material after its remaining callers were reduced to "
    "frozen shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs; "
    "backend-rs/src/api/health.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.batch_generation_task import BatchGenerationTask


STREAM_IDLE_TIMEOUT_SECONDS = 15


async def validate_batch_generation_stream_access(
    db_session: "AsyncSession",
    *,
    batch_id: str,
    user_id: Optional[str],
) -> "BatchGenerationTask":
    from fastapi import HTTPException
    from sqlalchemy import select
    from app.models.batch_generation_task import BatchGenerationTask

    if not user_id:
        raise HTTPException(status_code=401, detail='未登录')

    result = await db_session.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    task = result.scalar_one_or_none()
    if not task:
        raise HTTPException(status_code=404, detail='未找到批量生成任务')
    if task.user_id != user_id:
        raise HTTPException(status_code=403, detail='无权访问该任务')
    return task


async def build_batch_generation_event_stream(
    db_session: "AsyncSession",
    *,
    batch_id: str,
    idle_timeout_seconds: int = STREAM_IDLE_TIMEOUT_SECONDS,
) -> AsyncGenerator[str, None]:
    from sqlalchemy import select
    from app.models.batch_generation_task import BatchGenerationTask
    from app.services.task_workflow_runtime_service import (
        subscribe_task_stream,
        unsubscribe_task_stream,
    )
    from app.utils.sse_response import SSEResponse

    queue = await subscribe_task_stream(batch_id)
    try:
        yield await SSEResponse.send_progress('正在连接批量生成任务流', 0, 'processing')

        while True:
            try:
                event = await asyncio.wait_for(queue.get(), timeout=idle_timeout_seconds)
                yield SSEResponse.format_sse(event)

                if event.get('type') in {'done', 'error'}:
                    break
            except asyncio.TimeoutError:
                yield await SSEResponse.send_heartbeat()

                status_result = await db_session.execute(
                    select(BatchGenerationTask.status, BatchGenerationTask.error_message)
                    .where(BatchGenerationTask.id == batch_id)
                )
                row = status_result.first()
                if not row:
                    yield await SSEResponse.send_error('批量生成任务不存在', 404)
                    break

                status = row[0]
                error_message = row[1]
                if status in {'completed', 'cancelled'}:
                    yield await SSEResponse.send_done()
                    break
                if status == 'failed':
                    yield await SSEResponse.send_error(error_message or '批量生成任务执行失败', 500)
                    break
    finally:
        await unsubscribe_task_stream(batch_id, queue)
