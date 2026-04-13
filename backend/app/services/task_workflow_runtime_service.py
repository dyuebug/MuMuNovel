from __future__ import annotations

import asyncio
from asyncio import Lock, Queue
from datetime import datetime
from typing import Any, Dict, Optional

from sqlalchemy import select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.batch_generation_snapshot import BatchGenerationSnapshot
from app.models.batch_generation_task import BatchGenerationTask
from app.services.story_repair_payload_service import StoryRepairPayload

logger = get_logger(__name__)

_task_stream_subscribers: dict[str, list[Queue]] = {}
_task_stream_lock = Lock()
_task_workflow_state_cache: dict[str, Dict[str, Any]] = {}
_task_workflow_lock = Lock()
_SNAPSHOT_UNSET = object()
SNAPSHOT_UNSET = _SNAPSHOT_UNSET

task_stream_subscribers = _task_stream_subscribers
task_stream_lock = _task_stream_lock
task_workflow_state_cache = _task_workflow_state_cache
task_workflow_lock = _task_workflow_lock


def _normalize_runtime_payload(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, dict):
        return {str(key): _normalize_runtime_payload(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_normalize_runtime_payload(item) for item in value]
    if hasattr(value, 'model_dump'):
        return _normalize_runtime_payload(value.model_dump())
    if hasattr(value, 'dict'):
        return _normalize_runtime_payload(value.dict())
    return str(value)


async def subscribe_task_stream(task_id: str) -> Queue:
    queue: Queue = Queue(maxsize=200)
    async with _task_stream_lock:
        _task_stream_subscribers.setdefault(task_id, []).append(queue)
    return queue


async def unsubscribe_task_stream(task_id: str, queue: Queue) -> None:
    async with _task_stream_lock:
        queues = _task_stream_subscribers.get(task_id, [])
        if queue in queues:
            queues.remove(queue)
        if not queues and task_id in _task_stream_subscribers:
            del _task_stream_subscribers[task_id]


async def clear_task_workflow_runtime_cache(task_id: str) -> None:
    async with _task_workflow_lock:
        _task_workflow_state_cache.pop(task_id, None)


async def set_task_workflow_runtime_snapshot(task_id: str, snapshot: Dict[str, Any]) -> None:
    async with _task_workflow_lock:
        _task_workflow_state_cache[task_id] = dict(snapshot or {})


async def get_cached_task_workflow_runtime_snapshot(task_id: str) -> Dict[str, Any]:
    async with _task_workflow_lock:
        return dict(_task_workflow_state_cache.get(task_id) or {})


async def batch_task_exists(db_session: AsyncSession, task_id: str) -> bool:
    task_exists_result = await db_session.execute(
        select(BatchGenerationTask.id).where(BatchGenerationTask.id == task_id)
    )
    return task_exists_result.scalar_one_or_none() is not None


async def upsert_batch_generation_snapshot(
    db_session: AsyncSession,
    task_id: str,
    *,
    latest_quality_metrics: Any = _SNAPSHOT_UNSET,
    quality_metrics_history: Any = _SNAPSHOT_UNSET,
    quality_metrics_summary: Any = _SNAPSHOT_UNSET,
    workflow_runtime_state: Any = _SNAPSHOT_UNSET,
    clear_runtime_cache_on_missing: bool = True,
) -> Optional[BatchGenerationSnapshot]:
    if not await batch_task_exists(db_session, task_id):
        if clear_runtime_cache_on_missing:
            await clear_task_workflow_runtime_cache(task_id)
        logger.info(f'Skip batch snapshot persistence because task no longer exists: {task_id}')
        return None

    result = await db_session.execute(
        select(BatchGenerationSnapshot).where(BatchGenerationSnapshot.batch_task_id == task_id)
    )
    snapshot = result.scalar_one_or_none()
    did_change = snapshot is None
    if snapshot is None:
        snapshot = BatchGenerationSnapshot(batch_task_id=task_id)
        db_session.add(snapshot)

    if latest_quality_metrics is not _SNAPSHOT_UNSET:
        normalized_latest_quality_metrics = _normalize_runtime_payload(latest_quality_metrics)
        if snapshot.latest_quality_metrics != normalized_latest_quality_metrics:
            snapshot.latest_quality_metrics = normalized_latest_quality_metrics
            did_change = True
    if quality_metrics_history is not _SNAPSHOT_UNSET:
        normalized_quality_metrics_history = _normalize_runtime_payload(quality_metrics_history)
        if snapshot.quality_metrics_history != normalized_quality_metrics_history:
            snapshot.quality_metrics_history = normalized_quality_metrics_history
            did_change = True
    if quality_metrics_summary is not _SNAPSHOT_UNSET:
        normalized_quality_metrics_summary = _normalize_runtime_payload(quality_metrics_summary)
        if snapshot.quality_metrics_summary != normalized_quality_metrics_summary:
            snapshot.quality_metrics_summary = normalized_quality_metrics_summary
            did_change = True
    if workflow_runtime_state is not _SNAPSHOT_UNSET:
        normalized_workflow_runtime_state = _normalize_runtime_payload(workflow_runtime_state)
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
            logger.warning(f'Skip batch snapshot persistence because task disappeared during commit: {task_id}')
            return None
    return snapshot


async def load_persisted_batch_generation_snapshot(
    db_session: AsyncSession,
    task_id: str,
) -> Optional[BatchGenerationSnapshot]:
    result = await db_session.execute(
        select(BatchGenerationSnapshot).where(BatchGenerationSnapshot.batch_task_id == task_id)
    )
    return result.scalar_one_or_none()


async def persist_task_workflow_runtime_snapshot(
    db_session: AsyncSession,
    task_id: str,
    runtime_snapshot: Dict[str, Any],
) -> None:
    await upsert_batch_generation_snapshot(
        db_session,
        task_id,
        workflow_runtime_state=_normalize_runtime_payload(runtime_snapshot),
    )


async def get_task_workflow_runtime_snapshot(
    task_id: str,
    db_session: Optional[AsyncSession] = None,
) -> Dict[str, Any]:
    cached_runtime = await get_cached_task_workflow_runtime_snapshot(task_id)
    if cached_runtime:
        return cached_runtime

    if db_session is None:
        return {}

    persisted_snapshot = await load_persisted_batch_generation_snapshot(db_session, task_id)
    if persisted_snapshot is None:
        return {}

    runtime_snapshot = _normalize_runtime_payload(persisted_snapshot.workflow_runtime_state)
    if not isinstance(runtime_snapshot, dict):
        return {}

    await set_task_workflow_runtime_snapshot(task_id, runtime_snapshot)
    return dict(runtime_snapshot)


def _infer_batch_progress_phase(
    *,
    event_type: str,
    progress: Optional[int],
    message: Optional[str],
) -> Optional[str]:
    normalized_event = (event_type or '').strip().lower()
    text = (message or '').strip().lower()

    if normalized_event == 'error':
        return 'failed'
    if normalized_event == 'done':
        return 'complete'
    if normalized_event in {'chunk', 'chapter_start'}:
        return 'generating'
    if normalized_event == 'analysis_started':
        return 'parsing'

    if '??' in text or 'cancel' in text:
        return 'cancelled'
    if '??' in text or 'complete' in text or 'done' in text:
        return 'complete'
    if '??' in text or 'save' in text:
        return 'saving'
    if '??' in text or 'analysis' in text or '??' in text or 'parse' in text:
        return 'parsing'
    if '??' in text or 'retry' in text:
        return 'generating'
    if '??' in text or '??' in text or 'generate' in text:
        return 'generating'
    if '??' in text or 'prepare' in text:
        return 'preparing'
    if '??' in text or 'load' in text:
        return 'loading'

    if progress is None:
        return None
    if progress >= 100:
        return 'complete'
    if progress >= 93:
        return 'saving'
    if progress >= 85:
        return 'parsing'
    if progress >= 20:
        return 'generating'
    if progress >= 10:
        return 'preparing'
    if progress > 0:
        return 'loading'
    return 'init'


async def _update_task_workflow_runtime_state(
    task_id: str,
    event: Dict[str, Any],
    db_session: Optional[AsyncSession] = None,
) -> None:
    event_type = str(event.get('type') or '').strip().lower()
    progress_raw = event.get('progress')
    progress = progress_raw if isinstance(progress_raw, int) else None
    message = str(event.get('message')) if event.get('message') is not None else None

    async with _task_workflow_lock:
        current = _task_workflow_state_cache.get(task_id) or {}
        explicit_phase = event.get('phase')
        if isinstance(explicit_phase, str) and explicit_phase.strip():
            phase = explicit_phase.strip().lower()
        else:
            phase = _infer_batch_progress_phase(
                event_type=event_type,
                progress=progress,
                message=message,
            )
        previous_phase = str(current.get('phase') or '').strip().lower()
        if event_type == 'done' and previous_phase in {'failed', 'cancelled'} and not explicit_phase:
            phase = previous_phase

        snapshot = dict(current)
        snapshot['updated_at'] = datetime.now().isoformat()

        if event_type == 'chapter_start':
            for field_name in (
                'pre_compaction_total_length',
                'context_budget_limit',
                'compaction_applied',
                'compaction_details',
            ):
                snapshot.pop(field_name, None)

        if event_type:
            snapshot['last_event'] = event_type
        if message is not None:
            snapshot['last_message'] = message
        if progress is not None:
            snapshot['progress'] = max(0, min(progress, 100))
        if isinstance(event.get('status'), str):
            snapshot['status'] = str(event.get('status'))
        if event.get('chapter_id') is not None:
            snapshot['current_chapter_id'] = event.get('chapter_id')
        if event.get('chapter_number') is not None:
            snapshot['current_chapter_number'] = event.get('chapter_number')
        if event.get('current_retry_count') is not None:
            snapshot['current_retry_count'] = event.get('current_retry_count')
        if event.get('max_retries') is not None:
            snapshot['max_retries'] = event.get('max_retries')

        for field_name, minimum in (
            ('candidate_index', 1),
            ('candidate_count', 1),
            ('word_count', 0),
            ('winner_candidate_index', 1),
        ):
            raw_value = event.get(field_name)
            if raw_value is None:
                continue
            try:
                snapshot[field_name] = max(int(raw_value), minimum)
            except (TypeError, ValueError):
                continue

        for field_name, minimum in (
            ('pre_compaction_total_length', 0),
            ('context_budget_limit', 0),
        ):
            raw_value = event.get(field_name)
            if raw_value is None:
                continue
            try:
                snapshot[field_name] = max(int(raw_value), minimum)
            except (TypeError, ValueError):
                continue

        for field_name in ('generation_path', 'attempt_kind'):
            raw_value = event.get(field_name)
            if isinstance(raw_value, str) and raw_value.strip():
                snapshot[field_name] = raw_value.strip()

        for field_name in ('rerank_used', 'word_budget_repair_used', 'compaction_applied'):
            raw_value = event.get(field_name)
            if raw_value is not None:
                snapshot[field_name] = bool(raw_value)

        compaction_details = event.get('compaction_details')
        if isinstance(compaction_details, dict):
            snapshot['compaction_details'] = _normalize_runtime_payload(compaction_details)

        if phase:
            snapshot['phase'] = phase

        _task_workflow_state_cache[task_id] = snapshot

    if db_session is not None:
        await persist_task_workflow_runtime_snapshot(db_session, task_id, snapshot)


async def _set_task_active_story_repair_payload(
    task_id: str,
    payload: Optional[Dict[str, Any]],
    db_session: Optional[AsyncSession] = None,
) -> None:
    async with _task_workflow_lock:
        current = dict(_task_workflow_state_cache.get(task_id) or {})
        if isinstance(payload, dict):
            snapshot = dict(payload)
            snapshot['updated_at'] = str(snapshot.get('updated_at') or datetime.now().isoformat())
            current['active_story_repair_payload'] = snapshot
        else:
            current.pop('active_story_repair_payload', None)
        current['updated_at'] = datetime.now().isoformat()
        _task_workflow_state_cache[task_id] = current

    if db_session is not None:
        await persist_task_workflow_runtime_snapshot(db_session, task_id, current)


async def sync_task_story_repair_state(
    task_id: str,
    *,
    story_repair_state: Optional[Dict[str, Any]] = None,
    payload: Optional[StoryRepairPayload] = None,
    active_story_repair_payload: Optional[Dict[str, Any]] = None,
    db_session: Optional[AsyncSession] = None,
) -> Dict[str, Any]:
    extra_state: Dict[str, Any] = {}
    if isinstance(story_repair_state, dict):
        candidate_payload = story_repair_state.get('payload')
        candidate_active_payload = story_repair_state.get('active_story_repair_payload')
        if isinstance(candidate_payload, StoryRepairPayload) or candidate_payload is None:
            payload = candidate_payload
        if isinstance(candidate_active_payload, dict) or candidate_active_payload is None:
            active_story_repair_payload = candidate_active_payload
        extra_state = {
            key: value
            for key, value in story_repair_state.items()
            if key not in {'payload', 'active_story_repair_payload'}
        }

    normalized_state = {
        'payload': payload if isinstance(payload, StoryRepairPayload) else None,
        'active_story_repair_payload': dict(active_story_repair_payload) if isinstance(active_story_repair_payload, dict) else None,
        **extra_state,
    }
    await _set_task_active_story_repair_payload(
        task_id,
        normalized_state.get('active_story_repair_payload'),
        db_session=db_session,
    )
    return normalized_state


async def publish_task_stream_event(
    task_id: str,
    event: Dict[str, Any],
    db_session: Optional[AsyncSession] = None,
) -> None:
    await _update_task_workflow_runtime_state(task_id, event, db_session=db_session)

    async with _task_stream_lock:
        subscribers = list(_task_stream_subscribers.get(task_id, []))
    if not subscribers:
        return

    stale_queues: list[Queue] = []
    for queue in subscribers:
        try:
            queue.put_nowait(event)
        except asyncio.QueueFull:
            logger.debug(f'Task stream queue is full, drop event: task={task_id}, type={event.get("type")}')
        except Exception:
            stale_queues.append(queue)

    if stale_queues:
        async with _task_stream_lock:
            queues = _task_stream_subscribers.get(task_id, [])
            for queue in stale_queues:
                if queue in queues:
                    queues.remove(queue)
            if not queues and task_id in _task_stream_subscribers:
                del _task_stream_subscribers[task_id]
