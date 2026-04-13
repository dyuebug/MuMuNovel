from __future__ import annotations

from asyncio import Lock
from datetime import datetime
from typing import Any, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.services.story_quality_feedback_service import (
    advance_quality_metrics_summary_state,
    build_quality_gate_decision,
    build_quality_metrics_summary_from_state,
    build_quality_metrics_summary_state,
    build_story_repair_guidance,
)
from app.services.task_workflow_runtime_service import (
    load_persisted_batch_generation_snapshot,
    upsert_batch_generation_snapshot,
)

_task_quality_metrics_cache: dict[str, Dict[str, Any]] = {}
_task_quality_lock = Lock()

task_quality_metrics_cache = _task_quality_metrics_cache
task_quality_lock = _task_quality_lock


def _normalize_quality_payload(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, dict):
        return {str(key): _normalize_quality_payload(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_normalize_quality_payload(item) for item in value]
    if hasattr(value, 'model_dump'):
        return _normalize_quality_payload(value.model_dump())
    if hasattr(value, 'dict'):
        return _normalize_quality_payload(value.dict())
    return str(value)


def _public_task_quality_snapshot(snapshot: Optional[Dict[str, Any]]) -> Dict[str, Any]:
    if not isinstance(snapshot, dict):
        return {}
    return {
        'latest': snapshot.get('latest'),
        'history': list(snapshot.get('history') or []),
        'summary': snapshot.get('summary'),
    }


async def clear_task_quality_metrics_cache(task_id: str) -> None:
    async with _task_quality_lock:
        _task_quality_metrics_cache.pop(task_id, None)


async def record_task_quality_metrics(
    task_id: str,
    metrics_event: Dict[str, Any],
    db_session: Optional[AsyncSession] = None,
) -> None:
    persisted_snapshot: Optional[Dict[str, Any]] = None
    async with _task_quality_lock:
        current = _task_quality_metrics_cache.get(task_id) or {
            'latest': None,
            'history': [],
            'summary': None,
            '_summary_state': None,
        }
        normalized_event = dict(metrics_event or {})
        if normalized_event and not isinstance(normalized_event.get('repair_guidance'), dict):
            normalized_event['repair_guidance'] = build_story_repair_guidance(normalized_event, scope='chapter')
        if normalized_event and not isinstance(normalized_event.get('quality_gate'), dict):
            normalized_event['quality_gate'] = build_quality_gate_decision(normalized_event, scope='chapter')

        normalized_event = _normalize_quality_payload(normalized_event)
        current['latest'] = normalized_event
        history = list(current.get('history') or [])
        dropped_event = history[0] if len(history) >= 20 else None
        history.append(normalized_event)
        if len(history) > 20:
            history = history[-20:]
        current['history'] = history
        summary_state = advance_quality_metrics_summary_state(
            current.get('_summary_state'),
            appended_event=normalized_event,
            current_history=history,
            dropped_event=dropped_event,
            scope='batch',
        )
        if summary_state is None and history:
            summary_state = build_quality_metrics_summary_state(history, scope='batch')
        current['_summary_state'] = summary_state
        current['summary'] = build_quality_metrics_summary_from_state(summary_state, scope='batch')
        _task_quality_metrics_cache[task_id] = current
        persisted_snapshot = _public_task_quality_snapshot(current)

    if db_session is not None and persisted_snapshot is not None:
        await upsert_batch_generation_snapshot(
            db_session,
            task_id,
            latest_quality_metrics=persisted_snapshot['latest'],
            quality_metrics_history=persisted_snapshot['history'],
            quality_metrics_summary=persisted_snapshot['summary'],
            clear_runtime_cache_on_missing=False,
        )


async def get_task_quality_metrics_snapshot(
    task_id: str,
    db_session: Optional[AsyncSession] = None,
) -> Dict[str, Any]:
    async with _task_quality_lock:
        cached_snapshot = _task_quality_metrics_cache.get(task_id)
    if cached_snapshot:
        return _public_task_quality_snapshot(cached_snapshot)

    if db_session is None:
        return {}

    persisted_snapshot = await load_persisted_batch_generation_snapshot(db_session, task_id)
    if persisted_snapshot is None:
        return {}

    recovered_history = _normalize_quality_payload(persisted_snapshot.quality_metrics_history) or []
    summary_state = build_quality_metrics_summary_state(recovered_history, scope='batch') if recovered_history else None
    recovered_snapshot = {
        'latest': _normalize_quality_payload(persisted_snapshot.latest_quality_metrics) or None,
        'history': recovered_history,
        'summary': _normalize_quality_payload(persisted_snapshot.quality_metrics_summary) or None,
        '_summary_state': summary_state,
    }
    if not recovered_snapshot['summary'] and summary_state is not None:
        recovered_snapshot['summary'] = build_quality_metrics_summary_from_state(summary_state, scope='batch')

    async with _task_quality_lock:
        _task_quality_metrics_cache[task_id] = recovered_snapshot
    return _public_task_quality_snapshot(recovered_snapshot)
