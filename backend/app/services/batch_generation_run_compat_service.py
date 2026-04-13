"""Compatibility helpers for batch generation run entry points."""
from __future__ import annotations

import asyncio
from contextlib import suppress

from sqlalchemy.ext.asyncio import AsyncSession

from app.models.batch_generation_task import BatchGenerationTask
from app.services.chapter_analysis_support_service import get_chapter_analysis_write_lock


async def get_db_write_lock(user_id: str):
    return await get_chapter_analysis_write_lock(user_id)


async def await_cancelable_batch_generation_result(
    *,
    generation_coro,
    task: BatchGenerationTask,
    db_session: AsyncSession,
    poll_interval_seconds: float,
):
    generation_task = asyncio.create_task(generation_coro)
    try:
        while True:
            try:
                return await asyncio.wait_for(
                    asyncio.shield(generation_task),
                    timeout=poll_interval_seconds,
                )
            except asyncio.TimeoutError:
                await db_session.refresh(task)
                if task.status == 'cancelled':
                    generation_task.cancel()
                    with suppress(asyncio.CancelledError):
                        await generation_task
                    raise asyncio.CancelledError()
    finally:
        if not generation_task.done():
            generation_task.cancel()
            with suppress(asyncio.CancelledError):
                await generation_task
