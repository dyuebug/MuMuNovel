from __future__ import annotations

import asyncio
from typing import AsyncGenerator, Optional

from fastapi import HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.batch_generation_task import BatchGenerationTask
from app.services.task_workflow_runtime_service import subscribe_task_stream, unsubscribe_task_stream
from app.utils.sse_response import SSEResponse


STREAM_IDLE_TIMEOUT_SECONDS = 15


async def validate_batch_generation_stream_access(
    db_session: AsyncSession,
    *,
    batch_id: str,
    user_id: Optional[str],
) -> BatchGenerationTask:
    if not user_id:
        raise HTTPException(status_code=401, detail='???')

    result = await db_session.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    task = result.scalar_one_or_none()
    if not task:
        raise HTTPException(status_code=404, detail='?????????')
    if task.user_id != user_id:
        raise HTTPException(status_code=403, detail='???????')
    return task


async def build_batch_generation_event_stream(
    db_session: AsyncSession,
    *,
    batch_id: str,
    idle_timeout_seconds: int = STREAM_IDLE_TIMEOUT_SECONDS,
) -> AsyncGenerator[str, None]:
    queue = await subscribe_task_stream(batch_id)
    try:
        yield await SSEResponse.send_progress('????????', 0, 'processing')

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
                    yield await SSEResponse.send_error('?????', 404)
                    break

                status = row[0]
                error_message = row[1]
                if status in {'completed', 'cancelled'}:
                    yield await SSEResponse.send_done()
                    break
                if status == 'failed':
                    yield await SSEResponse.send_error(error_message or '????', 500)
                    break
    finally:
        await unsubscribe_task_stream(batch_id, queue)
