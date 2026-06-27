from __future__ import annotations

import json
import asyncio
from asyncio import Queue
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Dict, List, Mapping, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.task_system.snapshot_runtime_persistence import (
    SNAPSHOT_UNSET,
    batch_task_exists,
    load_persisted_batch_generation_snapshot,
    normalize_runtime_payload,
    upsert_batch_generation_snapshot,
)
def _is_story_repair_payload_instance(value: Any) -> bool:
    if value is None:
        return False
    if not all(hasattr(value, attr) for attr in ("summary", "targets", "strengths")):
        return False
    return callable(getattr(value, "to_prompt_kwargs", None))


class TaskWorkflowRuntimeStateStore:
    def __init__(self) -> None:
        self._cache: dict[str, Dict[str, Any]] = {}
        self._lock = asyncio.Lock()

    @property
    def cache(self) -> dict[str, Dict[str, Any]]:
        return self._cache

    @property
    def lock(self) -> asyncio.Lock:
        return self._lock

    async def clear(self, task_id: str) -> None:
        async with self._lock:
            self._cache.pop(task_id, None)

    async def set(self, task_id: str, snapshot: Dict[str, Any]) -> None:
        async with self._lock:
            self._cache[task_id] = dict(snapshot or {})

    async def get(self, task_id: str) -> Dict[str, Any]:
        async with self._lock:
            return dict(self._cache.get(task_id) or {})

    async def update(
        self,
        task_id: str,
        updater: Callable[[Dict[str, Any]], Dict[str, Any]],
    ) -> Dict[str, Any]:
        async with self._lock:
            current = dict(self._cache.get(task_id) or {})
            next_snapshot = dict(updater(current) or {})
            self._cache[task_id] = next_snapshot
            return dict(next_snapshot)


workflow_runtime_state_store = TaskWorkflowRuntimeStateStore()


async def clear_task_workflow_runtime_cache(task_id: str) -> None:
    await workflow_runtime_state_store.clear(task_id)


async def set_task_workflow_runtime_snapshot(task_id: str, snapshot: Dict[str, Any]) -> None:
    await workflow_runtime_state_store.set(task_id, snapshot)


async def get_cached_task_workflow_runtime_snapshot(task_id: str) -> Dict[str, Any]:
    return await workflow_runtime_state_store.get(task_id)


async def persist_task_workflow_runtime_snapshot(
    db_session: AsyncSession,
    task_id: str,
    runtime_snapshot: Dict[str, Any],
) -> None:
    await upsert_batch_generation_snapshot(
        db_session,
        task_id,
        workflow_runtime_state=normalize_runtime_payload(runtime_snapshot),
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

    runtime_snapshot = normalize_runtime_payload(persisted_snapshot.workflow_runtime_state)
    if not isinstance(runtime_snapshot, dict):
        return {}

    await set_task_workflow_runtime_snapshot(task_id, runtime_snapshot)
    return dict(runtime_snapshot)


async def update_task_workflow_runtime_state(
    task_id: str,
    event: Dict[str, Any],
    db_session: Optional[AsyncSession] = None,
) -> None:
    event_type = str(event.get("type") or "").strip().lower()
    progress_raw = event.get("progress")
    progress = progress_raw if isinstance(progress_raw, int) else None
    message = str(event.get("message")) if event.get("message") is not None else None

    def updater(current: Dict[str, Any]) -> Dict[str, Any]:
        explicit_phase = event.get("phase")
        if isinstance(explicit_phase, str) and explicit_phase.strip():
            phase = explicit_phase.strip().lower()
        else:
            phase = infer_workflow_phase(
                event_type=event_type,
                progress=progress,
                message=message,
            )
        previous_phase = str(current.get("phase") or "").strip().lower()
        if event_type == "done" and previous_phase in {"failed", "cancelled"} and not explicit_phase:
            phase = previous_phase

        snapshot = dict(current)
        snapshot["updated_at"] = datetime.now().isoformat()

        if event_type == "chapter_start":
            for field_name in (
                "pre_compaction_total_length",
                "context_budget_limit",
                "compaction_applied",
                "compaction_details",
            ):
                snapshot.pop(field_name, None)

        if event_type:
            snapshot["last_event"] = event_type
        if message is not None:
            snapshot["last_message"] = message
        if progress is not None:
            snapshot["progress"] = max(0, min(progress, 100))
        if isinstance(event.get("status"), str):
            snapshot["status"] = str(event.get("status"))
        if event.get("chapter_id") is not None:
            snapshot["current_chapter_id"] = event.get("chapter_id")
        if event.get("chapter_number") is not None:
            snapshot["current_chapter_number"] = event.get("chapter_number")
        if event.get("current_retry_count") is not None:
            snapshot["current_retry_count"] = event.get("current_retry_count")
        if event.get("max_retries") is not None:
            snapshot["max_retries"] = event.get("max_retries")

        for field_name, minimum in (
            ("candidate_index", 1),
            ("candidate_count", 1),
            ("word_count", 0),
            ("winner_candidate_index", 1),
        ):
            raw_value = event.get(field_name)
            if raw_value is None:
                continue
            try:
                snapshot[field_name] = max(int(raw_value), minimum)
            except (TypeError, ValueError):
                continue

        for field_name, minimum in (
            ("pre_compaction_total_length", 0),
            ("context_budget_limit", 0),
        ):
            raw_value = event.get(field_name)
            if raw_value is None:
                continue
            try:
                snapshot[field_name] = max(int(raw_value), minimum)
            except (TypeError, ValueError):
                continue

        for field_name in ("generation_path", "attempt_kind"):
            raw_value = event.get(field_name)
            if isinstance(raw_value, str) and raw_value.strip():
                snapshot[field_name] = raw_value.strip()

        for field_name in ("rerank_used", "word_budget_repair_used", "compaction_applied"):
            raw_value = event.get(field_name)
            if raw_value is not None:
                snapshot[field_name] = bool(raw_value)

        compaction_details = event.get("compaction_details")
        if isinstance(compaction_details, dict):
            snapshot["compaction_details"] = normalize_runtime_payload(compaction_details)

        if phase:
            snapshot["phase"] = phase

        return snapshot

    snapshot = await workflow_runtime_state_store.update(task_id, updater)

    if db_session is not None:
        await persist_task_workflow_runtime_snapshot(db_session, task_id, snapshot)


async def set_task_active_story_repair_payload(
    task_id: str,
    payload: Optional[Dict[str, Any]],
    db_session: Optional[AsyncSession] = None,
) -> None:
    def updater(current: Dict[str, Any]) -> Dict[str, Any]:
        current = dict(current or {})
        if isinstance(payload, dict):
            snapshot = dict(payload)
            snapshot["updated_at"] = str(snapshot.get("updated_at") or datetime.now().isoformat())
            current["active_story_repair_payload"] = snapshot
        else:
            current.pop("active_story_repair_payload", None)
        current["updated_at"] = datetime.now().isoformat()
        return current

    current = await workflow_runtime_state_store.update(task_id, updater)

    if db_session is not None:
        await persist_task_workflow_runtime_snapshot(db_session, task_id, current)


async def sync_task_story_repair_state(
    task_id: str,
    *,
    story_repair_state: Optional[Dict[str, Any]] = None,
    payload: Optional[Any] = None,
    active_story_repair_payload: Optional[Dict[str, Any]] = None,
    db_session: Optional[AsyncSession] = None,
) -> Dict[str, Any]:
    extra_state: Dict[str, Any] = {}
    if isinstance(story_repair_state, dict):
        candidate_payload = story_repair_state.get("payload")
        candidate_active_payload = story_repair_state.get("active_story_repair_payload")
        if _is_story_repair_payload_instance(candidate_payload) or candidate_payload is None:
            payload = candidate_payload
        if isinstance(candidate_active_payload, dict) or candidate_active_payload is None:
            active_story_repair_payload = candidate_active_payload
        extra_state = {
            key: value
            for key, value in story_repair_state.items()
            if key not in {"payload", "active_story_repair_payload"}
        }

    normalized_state = {
        "payload": payload if _is_story_repair_payload_instance(payload) else None,
        "active_story_repair_payload": (
            dict(active_story_repair_payload) if isinstance(active_story_repair_payload, dict) else None
        ),
        **extra_state,
    }
    await set_task_active_story_repair_payload(
        task_id,
        normalized_state.get("active_story_repair_payload"),
        db_session=db_session,
    )
    return normalized_state


PROGRESS_PHASE_ORDER: dict[str, int] = {
    "init": 0,
    "loading": 1,
    "preparing": 2,
    "generating": 3,
    "parsing": 4,
    "saving": 5,
    "complete": 6,
}

TASK_STAGE_ROOTS: dict[str, str] = {
    "wizard_world_building": "0.creative",
    "wizard_characters": "1.outline",
    "wizard_outline": "1.outline",
    "wizard_career_system": "1.outline",
    "world_regenerate": "0.creative",
    "outline_generate": "1.outline",
    "outline_expand": "4.group",
    "outline_batch_expand": "4.group",
    "careers_generate_system": "1.outline",
    "character_generate": "1.outline",
    "organization_generate": "1.outline",
    "chapters_batch_generate": "6.writing",
    "chapter_single_generate": "6.writing",
}

PHASE_KEYWORDS: dict[str, tuple[str, ...]] = {
    "init": ("开始", "启动", "初始化", "start", "init"),
    "loading": ("加载", "读取", "获取", "检索", "loading", "load", "fetch"),
    "preparing": ("准备", "预处理", "提示词", "prompt", "prepare", "preparing"),
    "generating": ("生成", "创作", "推理", "草稿", "rewrite", "generate", "generating"),
    "parsing": ("解析", "校验", "提取", "parsing", "parse", "validate"),
    "saving": ("保存", "写入", "入库", "提交", "持久化", "saving", "save", "persist"),
    "complete": ("完成", "结束", "done", "complete", "success"),
}


@dataclass(frozen=True)
class ActiveTaskQuery:
    user_id: str
    task_type: str
    project_id: str
    payload_fingerprint: Optional[str] = None


class BackgroundTaskRegistry:
    def __init__(self) -> None:
        self.tasks: Dict[str, Any] = {}
        self.runner_tasks: Dict[str, asyncio.Task[None]] = {}

    def set_tasks(self, tasks: Dict[str, Any], runner_tasks: Dict[str, asyncio.Task[None]]) -> None:
        self.tasks = tasks
        self.runner_tasks = runner_tasks

    def get_for_user(self, task_id: str, user_id: str) -> Optional[Any]:
        record = self.tasks.get(task_id)
        if not record or getattr(record, "user_id", None) != user_id:
            return None
        return record

    def list_for_user(self, user_id: str) -> list[Any]:
        return [record for record in self.tasks.values() if getattr(record, "user_id", None) == user_id]

    def find_active(self, query: ActiveTaskQuery) -> Optional[Any]:
        candidates = [
            record
            for record in self.tasks.values()
            if getattr(record, "user_id", None) == query.user_id
            and getattr(record, "task_type", None) == query.task_type
            and getattr(record, "project_id", None) == query.project_id
            and getattr(record, "status", None) in {"pending", "running"}
        ]

        if query.payload_fingerprint is not None:
            candidates = [
                record
                for record in candidates
                if getattr(record, "payload_fingerprint", None) == query.payload_fingerprint
            ]

        if not candidates:
            return None

        def sort_key(item: Any) -> datetime:
            return getattr(item, "updated_at", None) or getattr(item, "created_at", None)

        candidates.sort(key=sort_key, reverse=True)
        return candidates[0]


background_task_registry = BackgroundTaskRegistry()


def touch_checkpoint(
    checkpoint: Optional[Dict[str, Any]],
    *,
    event: str,
    progress: Optional[int] = None,
    message: Optional[str] = None,
    extra: Optional[Dict[str, Any]] = None,
    now: Optional[datetime] = None,
) -> Dict[str, Any]:
    snapshot: Dict[str, Any] = {}
    if isinstance(checkpoint, dict):
        snapshot.update(checkpoint)

    snapshot["event"] = event
    snapshot["updated_at"] = (now or datetime.now(timezone.utc)).isoformat()
    if progress is not None:
        snapshot["progress"] = progress
    if message is not None:
        snapshot["message"] = message
    if extra:
        snapshot.update(extra)
    return snapshot


def split_stage_code(
    stage_code: Optional[str],
    *,
    phase_order: Mapping[str, int] = PROGRESS_PHASE_ORDER,
) -> tuple[Optional[str], Optional[str]]:
    raw = (stage_code or "").strip()
    if not raw:
        return None, None
    base, sep, suffix = raw.rpartition(".")
    if sep and suffix in phase_order:
        return base, suffix
    return raw, None


def contains_retry_hint(message: Optional[str]) -> bool:
    if not message:
        return False
    text = message.lower()
    return "重试" in text or "retry" in text


def detect_phase_by_message(
    message: Optional[str],
    *,
    phase_keywords: Mapping[str, tuple[str, ...]] = PHASE_KEYWORDS,
) -> Optional[str]:
    if not message:
        return None
    text = message.strip().lower()
    if not text:
        return None

    for phase in (
        "complete",
        "saving",
        "parsing",
        "generating",
        "preparing",
        "loading",
        "init",
    ):
        if any(keyword in text for keyword in phase_keywords[phase]):
            return phase
    return None


def detect_phase_by_progress(progress: Optional[int]) -> Optional[str]:
    if progress is None:
        return None
    normalized = max(0, min(int(progress), 100))
    if normalized >= 100:
        return "complete"
    if normalized >= 93:
        return "saving"
    if normalized >= 86:
        return "parsing"
    if normalized >= 21:
        return "generating"
    if normalized >= 16:
        return "preparing"
    if normalized >= 6:
        return "loading"
    return "init"


def resolve_progress_phase(
    *,
    message: Optional[str],
    progress: Optional[int],
    stage_code: Optional[str],
    phase_order: Mapping[str, int] = PROGRESS_PHASE_ORDER,
) -> Optional[str]:
    detected = detect_phase_by_message(message) or detect_phase_by_progress(progress)
    if not detected:
        return None

    _, current_phase = split_stage_code(stage_code, phase_order=phase_order)
    if not current_phase:
        return detected

    if (
        phase_order.get(detected, -1) < phase_order.get(current_phase, -1)
        and not contains_retry_hint(message)
    ):
        return current_phase
    return detected


def resolve_stage_code_for_phase(
    *,
    task_type: str,
    stage_code: Optional[str],
    phase: Optional[str],
    stage_roots: Mapping[str, str] = TASK_STAGE_ROOTS,
    phase_order: Mapping[str, int] = PROGRESS_PHASE_ORDER,
) -> Optional[str]:
    base, _ = split_stage_code(stage_code, phase_order=phase_order)
    if not base:
        base = stage_roots.get(task_type)
    if not base:
        return stage_code
    if not phase or phase == "init":
        return base
    return f"{base}.{phase}"


def infer_workflow_phase(
    *,
    event_type: str,
    progress: Optional[int],
    message: Optional[str],
) -> Optional[str]:
    normalized_event = (event_type or "").strip().lower()
    text = (message or "").strip().lower()

    if normalized_event == "error":
        return "failed"
    if normalized_event == "done":
        return "complete"
    if normalized_event in {"chunk", "chapter_start"}:
        return "generating"
    if normalized_event == "analysis_started":
        return "parsing"

    if "取消" in text or "cancel" in text:
        return "cancelled"
    if "完成" in text or "complete" in text or "done" in text:
        return "complete"
    if "保存" in text or "save" in text:
        return "saving"
    if "分析" in text or "analysis" in text or "解析" in text or "parse" in text:
        return "parsing"
    if "重试" in text or "retry" in text:
        return "generating"
    if "生成" in text or "写作" in text or "generate" in text:
        return "generating"
    if "准备" in text or "prepare" in text:
        return "preparing"
    if "加载" in text or "load" in text:
        return "loading"

    if progress is None:
        return None
    if progress >= 100:
        return "complete"
    if progress >= 93:
        return "saving"
    if progress >= 85:
        return "parsing"
    if progress >= 20:
        return "generating"
    if progress >= 10:
        return "preparing"
    if progress > 0:
        return "loading"
    return "init"


def parse_dt(value: Any) -> Optional[datetime]:
    if not value or not isinstance(value, str):
        return None
    try:
        return datetime.fromisoformat(value)
    except ValueError:
        return None


def load_records_from_disk(
    persistence_path: Path,
    *,
    record_factory: Callable[[Dict[str, Any]], Any],
    record_filter: Optional[Callable[[Any], bool]] = None,
) -> Dict[str, Any]:
    if not persistence_path.exists():
        return {}

    payload = json.loads(persistence_path.read_text(encoding="utf-8"))
    items = payload.get("items") if isinstance(payload, dict) else None
    if not isinstance(items, list):
        return {}

    loaded: Dict[str, Any] = {}
    for raw in items:
        if not isinstance(raw, dict):
            continue
        record = record_factory(raw)
        if record_filter and not record_filter(record):
            continue
        record_id = str(getattr(record, "task_id", "") or "")
        if not record_id:
            continue
        loaded[record_id] = record

    return loaded


@dataclass(frozen=True)
class OrphanRecoveryResult:
    changed: bool


def recover_orphan_tasks_on_boot(
    tasks: Dict[str, Any],
    *,
    touch_checkpoint_fn: Callable[..., Any],
    now: Optional[datetime] = None,
) -> OrphanRecoveryResult:
    now = now or datetime.now(timezone.utc)
    changed = False

    for record in tasks.values():
        status = getattr(record, "status", None)
        if status not in {"pending", "running"}:
            continue

        record.status = "failed"
        record.error = "服务重启导致任务上下文丢失"
        record.message = "服务重启后未恢复执行上下文，请重新发起任务"
        if not getattr(record, "started_at", None):
            record.started_at = getattr(record, "updated_at", None) or now
        record.completed_at = now
        record.updated_at = now

        checkpoint_extra: Dict[str, Any] = {"error": record.error}
        stage_code = getattr(record, "stage_code", None)
        if stage_code:
            checkpoint_extra["stage_code"] = stage_code

        record.checkpoint = touch_checkpoint_fn(
            record.checkpoint,
            event="failed",
            progress=getattr(record, "progress", None),
            message=record.message,
            extra=checkpoint_extra,
            now=now,
        )
        changed = True

    return OrphanRecoveryResult(changed=changed)


@dataclass(frozen=True)
class TaskStreamFanoutResult:
    delivered: int
    dropped_full: int
    removed_stale: int


class TaskStreamHub:
    def __init__(self) -> None:
        self._subscribers: Dict[str, List[Any]] = {}
        self._lock = asyncio.Lock()

    @property
    def subscribers(self) -> Dict[str, List[Any]]:
        return self._subscribers

    @property
    def lock(self) -> asyncio.Lock:
        return self._lock

    async def subscribe(self, task_id: str, *, maxsize: int = 200) -> Queue:
        queue: Queue = Queue(maxsize=maxsize)
        async with self._lock:
            self._subscribers.setdefault(task_id, []).append(queue)
        return queue

    async def unsubscribe(self, task_id: str, queue: Queue) -> None:
        async with self._lock:
            queues = self._subscribers.get(task_id, [])
            if queue in queues:
                queues.remove(queue)
            if not queues and task_id in self._subscribers:
                del self._subscribers[task_id]

    async def fanout(self, task_id: str, event: Dict[str, Any]) -> TaskStreamFanoutResult:
        async with self._lock:
            subscribers = list(self._subscribers.get(task_id, []))

        if not subscribers:
            return TaskStreamFanoutResult(delivered=0, dropped_full=0, removed_stale=0)

        dropped_full = 0
        delivered = 0
        stale_queues: list[Any] = []
        for queue in subscribers:
            try:
                queue.put_nowait(event)
                delivered += 1
            except asyncio.QueueFull:
                dropped_full += 1
            except Exception:
                stale_queues.append(queue)

        if stale_queues:
            async with self._lock:
                queues = self._subscribers.get(task_id, [])
                for queue in stale_queues:
                    if queue in queues:
                        queues.remove(queue)
                if not queues and task_id in self._subscribers:
                    del self._subscribers[task_id]

        return TaskStreamFanoutResult(
            delivered=delivered,
            dropped_full=dropped_full,
            removed_stale=len(stale_queues),
        )


task_stream_hub = TaskStreamHub()


async def subscribe_task_stream(task_id: str) -> Queue:
    return await task_stream_hub.subscribe(task_id, maxsize=200)


async def unsubscribe_task_stream(task_id: str, queue: Queue) -> None:
    await task_stream_hub.unsubscribe(task_id, queue)


async def publish_task_stream_event(
    task_id: str,
    event: Dict[str, Any],
    db_session: Optional[AsyncSession] = None,
) -> None:
    from tests.test_support.retired_runtime_test_support import get_logger

    logger = get_logger(__name__)

    await update_task_workflow_runtime_state(task_id, event, db_session=db_session)

    fanout_result = await task_stream_hub.fanout(task_id, event)
    if fanout_result.dropped_full:
        logger.debug(
            f'Task stream queue is full, drop event: task={task_id}, type={event.get("type")}'
        )

__all__ = [
    "touch_checkpoint",
    "SNAPSHOT_UNSET",
    "PHASE_KEYWORDS",
    "PROGRESS_PHASE_ORDER",
    "TASK_STAGE_ROOTS",
    "contains_retry_hint",
    "detect_phase_by_message",
    "detect_phase_by_progress",
    "infer_workflow_phase",
    "resolve_progress_phase",
    "resolve_stage_code_for_phase",
    "split_stage_code",
    "ActiveTaskQuery",
    "BackgroundTaskRegistry",
    "background_task_registry",
    "OrphanRecoveryResult",
    "load_records_from_disk",
    "parse_dt",
    "recover_orphan_tasks_on_boot",
    "TaskWorkflowRuntimeStateStore",
    "workflow_runtime_state_store",
    "TaskStreamFanoutResult",
    "TaskStreamHub",
    "task_stream_hub",
    "normalize_runtime_payload",
    "subscribe_task_stream",
    "unsubscribe_task_stream",
    "clear_task_workflow_runtime_cache",
    "set_task_workflow_runtime_snapshot",
    "get_cached_task_workflow_runtime_snapshot",
    "upsert_batch_generation_snapshot",
    "load_persisted_batch_generation_snapshot",
    "persist_task_workflow_runtime_snapshot",
    "get_task_workflow_runtime_snapshot",
    "update_task_workflow_runtime_state",
    "set_task_active_story_repair_payload",
    "sync_task_story_repair_state",
    "publish_task_stream_event",
]



