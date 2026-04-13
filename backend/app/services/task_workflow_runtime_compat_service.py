from __future__ import annotations

from typing import Any, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.models.batch_generation_snapshot import BatchGenerationSnapshot
from app.services.task_quality_snapshot_service import clear_task_quality_metrics_cache
from app.services.task_workflow_runtime_service import (
    SNAPSHOT_UNSET,
    batch_task_exists as _batch_task_exists_service,
    clear_task_workflow_runtime_cache,
    get_task_workflow_runtime_snapshot as _get_task_workflow_runtime_snapshot_service,
    load_persisted_batch_generation_snapshot as _load_persisted_batch_generation_snapshot_service,
    persist_task_workflow_runtime_snapshot as _persist_task_workflow_runtime_snapshot_service,
    upsert_batch_generation_snapshot as _upsert_batch_generation_snapshot_service,
)


async def clear_task_runtime_caches(task_id: str) -> None:
    await clear_task_quality_metrics_cache(task_id)
    await clear_task_workflow_runtime_cache(task_id)


async def batch_task_exists(db_session: AsyncSession, task_id: str) -> bool:
    return await _batch_task_exists_service(db_session, task_id)


async def upsert_batch_generation_snapshot(
    db_session: AsyncSession,
    task_id: str,
    *,
    latest_quality_metrics: Any = SNAPSHOT_UNSET,
    quality_metrics_history: Any = SNAPSHOT_UNSET,
    quality_metrics_summary: Any = SNAPSHOT_UNSET,
    workflow_runtime_state: Any = SNAPSHOT_UNSET,
) -> Optional[BatchGenerationSnapshot]:
    return await _upsert_batch_generation_snapshot_service(
        db_session,
        task_id,
        latest_quality_metrics=latest_quality_metrics,
        quality_metrics_history=quality_metrics_history,
        quality_metrics_summary=quality_metrics_summary,
        workflow_runtime_state=workflow_runtime_state,
    )


async def load_persisted_batch_generation_snapshot(
    db_session: AsyncSession,
    task_id: str,
) -> Optional[BatchGenerationSnapshot]:
    return await _load_persisted_batch_generation_snapshot_service(db_session, task_id)


async def persist_task_workflow_runtime_snapshot(
    db_session: AsyncSession,
    task_id: str,
    runtime_snapshot: Dict[str, Any],
) -> None:
    await _persist_task_workflow_runtime_snapshot_service(
        db_session,
        task_id,
        runtime_snapshot,
    )


async def get_task_workflow_runtime_snapshot(
    task_id: str,
    db_session: Optional[AsyncSession] = None,
) -> Dict[str, Any]:
    return await _get_task_workflow_runtime_snapshot_service(
        task_id,
        db_session,
    )
