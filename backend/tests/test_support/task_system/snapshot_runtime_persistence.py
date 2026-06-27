from __future__ import annotations

from datetime import datetime
from typing import TYPE_CHECKING, Any, Optional

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.batch_generation_snapshot import BatchGenerationSnapshot


SNAPSHOT_UNSET = object()


def normalize_runtime_payload(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, dict):
        return {str(key): normalize_runtime_payload(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [normalize_runtime_payload(item) for item in value]
    if hasattr(value, "model_dump"):
        return normalize_runtime_payload(value.model_dump())
    if hasattr(value, "dict"):
        return normalize_runtime_payload(value.dict())
    return str(value)


async def batch_task_exists(db_session: "AsyncSession", task_id: str) -> bool:
    from sqlalchemy import select

    from migrator_app.models.batch_generation_task import BatchGenerationTask

    task_exists_result = await db_session.execute(
        select(BatchGenerationTask.id).where(BatchGenerationTask.id == task_id)
    )
    return task_exists_result.scalar_one_or_none() is not None


async def upsert_batch_generation_snapshot(
    db_session: "AsyncSession",
    task_id: str,
    *,
    latest_quality_metrics: Any = SNAPSHOT_UNSET,
    quality_metrics_history: Any = SNAPSHOT_UNSET,
    quality_metrics_summary: Any = SNAPSHOT_UNSET,
    workflow_runtime_state: Any = SNAPSHOT_UNSET,
    clear_runtime_cache_on_missing: bool = True,
) -> Optional["BatchGenerationSnapshot"]:
    from sqlalchemy import select
    from sqlalchemy.exc import IntegrityError

    from tests.test_support.retired_runtime_test_support import get_logger
    from migrator_app.models.batch_generation_snapshot import BatchGenerationSnapshot
    from tests.test_support.task_system import clear_task_workflow_runtime_cache

    logger = get_logger(__name__)

    if not await batch_task_exists(db_session, task_id):
        if clear_runtime_cache_on_missing:
            await clear_task_workflow_runtime_cache(task_id)
        logger.info(f"Skip batch snapshot persistence because task no longer exists: {task_id}")
        return None

    result = await db_session.execute(
        select(BatchGenerationSnapshot).where(BatchGenerationSnapshot.batch_task_id == task_id)
    )
    snapshot = result.scalar_one_or_none()
    did_change = snapshot is None
    if snapshot is None:
        snapshot = BatchGenerationSnapshot(batch_task_id=task_id)
        db_session.add(snapshot)

    if latest_quality_metrics is not SNAPSHOT_UNSET:
        normalized_latest_quality_metrics = normalize_runtime_payload(latest_quality_metrics)
        if snapshot.latest_quality_metrics != normalized_latest_quality_metrics:
            snapshot.latest_quality_metrics = normalized_latest_quality_metrics
            did_change = True
    if quality_metrics_history is not SNAPSHOT_UNSET:
        normalized_quality_metrics_history = normalize_runtime_payload(quality_metrics_history)
        if snapshot.quality_metrics_history != normalized_quality_metrics_history:
            snapshot.quality_metrics_history = normalized_quality_metrics_history
            did_change = True
    if quality_metrics_summary is not SNAPSHOT_UNSET:
        normalized_quality_metrics_summary = normalize_runtime_payload(quality_metrics_summary)
        if snapshot.quality_metrics_summary != normalized_quality_metrics_summary:
            snapshot.quality_metrics_summary = normalized_quality_metrics_summary
            did_change = True
    if workflow_runtime_state is not SNAPSHOT_UNSET:
        normalized_workflow_runtime_state = normalize_runtime_payload(workflow_runtime_state)
        if snapshot.workflow_runtime_state != normalized_workflow_runtime_state:
            snapshot.workflow_runtime_state = normalized_workflow_runtime_state
            did_change = True

    if did_change:
        try:
            await db_session.commit()
        except IntegrityError:
            await db_session.rollback()
            if clear_runtime_cache_on_missing:
                await clear_task_workflow_runtime_cache(task_id)
            logger.warning(
                f"Skip batch snapshot persistence because task disappeared during commit: {task_id}"
            )
            return None
    return snapshot


async def load_persisted_batch_generation_snapshot(
    db_session: "AsyncSession",
    task_id: str,
) -> Optional["BatchGenerationSnapshot"]:
    from sqlalchemy import select

    from migrator_app.models.batch_generation_snapshot import BatchGenerationSnapshot

    result = await db_session.execute(
        select(BatchGenerationSnapshot).where(BatchGenerationSnapshot.batch_task_id == task_id)
    )
    return result.scalar_one_or_none()



