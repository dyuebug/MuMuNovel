"""批量生成 route transport owner service。"""
from __future__ import annotations

from datetime import datetime
from typing import TYPE_CHECKING, Any

from fastapi import HTTPException, Request

from app.logger import get_logger

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation route transport and runtime chain; "
    "this Python route transport owner module is kept only as frozen "
    "rollback/source-map material after the remaining legacy route wiring "
    "owner was split into narrower shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs; "
    "backend-rs/src/api/health.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

logger = get_logger(__name__)

if TYPE_CHECKING:
    from app.models.batch_generation_task import BatchGenerationTask


async def stream_batch_generation_events_with_default_route_wiring(
    db_session: Any,
    *,
    batch_id: str,
    request: Request,
    validate_stream_access_fn,
    build_stream_fn,
):
    from app.utils.sse_response import create_sse_response

    user_id = getattr(request.state, "user_id", None)
    await validate_stream_access_fn(
        db_session,
        batch_id=batch_id,
        user_id=user_id,
    )
    return create_sse_response(
        build_stream_fn(
            db_session,
            batch_id=batch_id,
        )
    )


async def cancel_batch_generation_with_default_wiring(
    db_session: Any,
    *,
    batch_id: str,
):
    from sqlalchemy import select

    from app.models.batch_generation_task import BatchGenerationTask

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
