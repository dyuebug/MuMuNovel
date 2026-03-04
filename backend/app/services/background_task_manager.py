"""In-memory background task manager for long-running generation jobs."""
from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, AsyncIterable, Awaitable, Dict, Iterable, Optional

from app.logger import get_logger

logger = get_logger(__name__)


@dataclass
class BackgroundTaskRecord:
    task_id: str
    task_type: str
    user_id: str
    project_id: str
    status: str = "pending"
    progress: int = 0
    message: str = "任务已创建"
    result: Optional[Dict[str, Any]] = None
    error: Optional[str] = None
    stage_code: Optional[str] = None
    execution_mode: str = "interactive"
    workflow_scope: Optional[str] = None
    checkpoint: Optional[Dict[str, Any]] = None
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    updated_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "task_id": self.task_id,
            "task_type": self.task_type,
            "project_id": self.project_id,
            "status": self.status,
            "progress": self.progress,
            "message": self.message,
            "result": self.result,
            "error": self.error,
            "stage_code": self.stage_code,
            "execution_mode": self.execution_mode,
            "workflow_scope": self.workflow_scope,
            "checkpoint": self.checkpoint,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
            "started_at": self.started_at.isoformat() if self.started_at else None,
            "completed_at": self.completed_at.isoformat() if self.completed_at else None,
        }

    def to_storage_dict(self) -> Dict[str, Any]:
        data = self.to_dict()
        data["user_id"] = self.user_id
        return data

    @classmethod
    def from_storage_dict(cls, payload: Dict[str, Any]) -> "BackgroundTaskRecord":
        def parse_dt(value: Any) -> Optional[datetime]:
            if not value or not isinstance(value, str):
                return None
            try:
                return datetime.fromisoformat(value)
            except ValueError:
                return None

        created_at = parse_dt(payload.get("created_at")) or datetime.now(timezone.utc)
        updated_at = parse_dt(payload.get("updated_at")) or created_at
        return cls(
            task_id=str(payload.get("task_id", "")),
            task_type=str(payload.get("task_type", "unknown")),
            user_id=str(payload.get("user_id", "")),
            project_id=str(payload.get("project_id", "")),
            status=str(payload.get("status", "pending")),
            progress=max(0, min(int(payload.get("progress", 0) or 0), 100)),
            message=str(payload.get("message", "任务已创建") or "任务已创建"),
            result=payload.get("result") if isinstance(payload.get("result"), dict) else None,
            error=payload.get("error"),
            stage_code=payload.get("stage_code"),
            execution_mode=str(payload.get("execution_mode", "interactive") or "interactive"),
            workflow_scope=payload.get("workflow_scope"),
            checkpoint=payload.get("checkpoint") if isinstance(payload.get("checkpoint"), dict) else None,
            created_at=created_at,
            updated_at=updated_at,
            started_at=parse_dt(payload.get("started_at")),
            completed_at=parse_dt(payload.get("completed_at")),
        )


class BackgroundTaskManager:
    """Stores task state and updates task progress from SSE messages."""
    _PROGRESS_PHASE_ORDER: Dict[str, int] = {
        "init": 0,
        "loading": 1,
        "preparing": 2,
        "generating": 3,
        "parsing": 4,
        "saving": 5,
        "complete": 6,
    }
    _TASK_STAGE_ROOTS: Dict[str, str] = {
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
    _PHASE_KEYWORDS: Dict[str, tuple[str, ...]] = {
        "init": ("开始", "启动", "初始化", "start", "init"),
        "loading": ("加载", "读取", "获取", "检索", "loading", "load", "fetch"),
        "preparing": ("准备", "预处理", "提示词", "prompt", "prepare", "preparing"),
        "generating": ("生成", "创作", "推理", "草稿", "rewrite", "generate", "generating"),
        "parsing": ("解析", "校验", "提取", "parsing", "parse", "validate"),
        "saving": ("保存", "写入", "入库", "提交", "持久化", "saving", "save", "persist"),
        "complete": ("完成", "结束", "done", "complete", "success"),
    }

    def __init__(
        self,
        ttl_seconds: int = 7200,
        max_tasks: int = 2000,
        persistence_path: Optional[str] = None,
    ):
        self._tasks: Dict[str, BackgroundTaskRecord] = {}
        self._runner_tasks: Dict[str, asyncio.Task[None]] = {}
        self._lock = asyncio.Lock()
        self._ttl_seconds = ttl_seconds
        self._max_tasks = max_tasks
        self._persistence_path = Path(
            persistence_path or "data/runtime/background_tasks.json"
        )
        self._load_from_disk()

    @staticmethod
    def _touch_checkpoint(
        record: BackgroundTaskRecord,
        *,
        event: str,
        progress: Optional[int] = None,
        message: Optional[str] = None,
        extra: Optional[Dict[str, Any]] = None,
    ) -> None:
        snapshot: Dict[str, Any] = {}
        if isinstance(record.checkpoint, dict):
            snapshot.update(record.checkpoint)
        snapshot["event"] = event
        snapshot["updated_at"] = datetime.now(timezone.utc).isoformat()
        if progress is not None:
            snapshot["progress"] = progress
        if message is not None:
            snapshot["message"] = message
        if extra:
            snapshot.update(extra)
        record.checkpoint = snapshot

    @classmethod
    def _split_stage_code(cls, stage_code: Optional[str]) -> tuple[Optional[str], Optional[str]]:
        raw = (stage_code or "").strip()
        if not raw:
            return None, None
        base, sep, suffix = raw.rpartition(".")
        if sep and suffix in cls._PROGRESS_PHASE_ORDER:
            return base, suffix
        return raw, None

    @staticmethod
    def _contains_retry_hint(message: Optional[str]) -> bool:
        if not message:
            return False
        text = message.lower()
        return "重试" in text or "retry" in text

    @classmethod
    def _detect_phase_by_message(cls, message: Optional[str]) -> Optional[str]:
        if not message:
            return None
        text = message.strip().lower()
        if not text:
            return None
        for phase in ("complete", "saving", "parsing", "generating", "preparing", "loading", "init"):
            if any(keyword in text for keyword in cls._PHASE_KEYWORDS[phase]):
                return phase
        return None

    @staticmethod
    def _detect_phase_by_progress(progress: Optional[int]) -> Optional[str]:
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

    @classmethod
    def _resolve_progress_phase(
        cls,
        record: BackgroundTaskRecord,
        *,
        message: Optional[str],
        progress: Optional[int],
    ) -> Optional[str]:
        detected = cls._detect_phase_by_message(message) or cls._detect_phase_by_progress(progress)
        if not detected:
            return None

        _, current_phase = cls._split_stage_code(record.stage_code)
        if not current_phase:
            return detected

        # Keep stage progression monotonic unless retry clearly restarts an earlier step.
        if (
            cls._PROGRESS_PHASE_ORDER.get(detected, -1) < cls._PROGRESS_PHASE_ORDER.get(current_phase, -1)
            and not cls._contains_retry_hint(message)
        ):
            return current_phase
        return detected

    @classmethod
    def _resolve_stage_code_for_phase(
        cls,
        record: BackgroundTaskRecord,
        phase: Optional[str],
    ) -> Optional[str]:
        base, _ = cls._split_stage_code(record.stage_code)
        if not base:
            base = cls._TASK_STAGE_ROOTS.get(record.task_type)
        if not base:
            return record.stage_code
        if not phase or phase == "init":
            return base
        return f"{base}.{phase}"

    def _load_from_disk(self) -> None:
        """Load persisted task snapshots for cross-restart visibility."""
        try:
            if not self._persistence_path.exists():
                return
            payload = json.loads(self._persistence_path.read_text(encoding="utf-8"))
            items = payload.get("items") if isinstance(payload, dict) else None
            if not isinstance(items, list):
                return

            loaded: Dict[str, BackgroundTaskRecord] = {}
            for raw in items:
                if not isinstance(raw, dict):
                    continue
                record = BackgroundTaskRecord.from_storage_dict(raw)
                if not record.task_id or not record.user_id:
                    continue
                loaded[record.task_id] = record
            self._tasks = loaded
            self._recover_orphan_tasks_on_boot_locked()
            self._cleanup_locked()
            logger.info(
                "Loaded persisted background tasks: count=%s path=%s",
                len(self._tasks),
                self._persistence_path,
            )
        except Exception as exc:
            logger.warning(f"Failed to load background task persistence: {exc}")

    def _recover_orphan_tasks_on_boot_locked(self) -> None:
        """Mark pre-restart pending/running tasks as failed (worker context lost)."""
        now = datetime.now(timezone.utc)
        changed = False
        for record in self._tasks.values():
            if record.status not in {"pending", "running"}:
                continue
            record.status = "failed"
            record.error = "服务重启导致任务中断"
            record.message = "任务在服务重启后中断，已标记失败"
            if not record.started_at:
                record.started_at = record.updated_at or now
            record.completed_at = now
            record.updated_at = now
            changed = True
        if changed:
            self._persist_locked()

    def _persist_locked(self) -> None:
        """Persist task snapshots for recovery and cross-page sync."""
        try:
            self._persistence_path.parent.mkdir(parents=True, exist_ok=True)
            items = [
                record.to_storage_dict()
                for record in sorted(
                    self._tasks.values(),
                    key=lambda item: item.updated_at or item.created_at,
                    reverse=True,
                )
            ]
            payload = {
                "version": 1,
                "updated_at": datetime.now(timezone.utc).isoformat(),
                "items": items,
            }
            self._persistence_path.write_text(
                json.dumps(payload, ensure_ascii=False),
                encoding="utf-8",
            )
        except Exception as exc:
            logger.warning(f"Failed to persist background tasks: {exc}")

    async def create_task(
        self,
        task_id: str,
        task_type: str,
        user_id: str,
        project_id: str,
        message: str = "任务已创建",
        stage_code: Optional[str] = None,
        execution_mode: str = "interactive",
        workflow_scope: Optional[str] = None,
        checkpoint: Optional[Dict[str, Any]] = None,
    ) -> BackgroundTaskRecord:
        async with self._lock:
            self._cleanup_locked()
            if len(self._tasks) >= self._max_tasks:
                self._cleanup_locked(force=True)

            record = BackgroundTaskRecord(
                task_id=task_id,
                task_type=task_type,
                user_id=user_id,
                project_id=project_id,
                status="pending",
                progress=0,
                message=message,
                stage_code=stage_code,
                execution_mode=execution_mode,
                workflow_scope=workflow_scope,
                checkpoint=checkpoint,
            )
            self._tasks[task_id] = record
            self._persist_locked()
            return record

    async def get_task(self, task_id: str, user_id: str) -> Optional[BackgroundTaskRecord]:
        async with self._lock:
            self._cleanup_locked()
            record = self._tasks.get(task_id)
            if not record or record.user_id != user_id:
                return None
            return record

    async def list_tasks(
        self,
        *,
        user_id: str,
        project_id: Optional[str] = None,
        statuses: Optional[Iterable[str]] = None,
        limit: int = 20,
    ) -> list[BackgroundTaskRecord]:
        """List tasks for user with optional project/status filters."""
        normalized_limit = max(1, min(int(limit), 200))
        allowed_statuses = {str(item).strip() for item in (statuses or []) if str(item).strip()}

        async with self._lock:
            self._cleanup_locked()
            records = [
                record
                for record in self._tasks.values()
                if record.user_id == user_id
            ]
            if project_id:
                records = [record for record in records if record.project_id == project_id]
            if allowed_statuses:
                records = [record for record in records if record.status in allowed_statuses]

            records.sort(
                key=lambda item: item.updated_at or item.created_at,
                reverse=True,
            )
            return records[:normalized_limit]

    async def update_workflow_state(
        self,
        *,
        task_id: str,
        user_id: str,
        stage_code: Optional[str] = None,
        execution_mode: Optional[str] = None,
        workflow_scope: Optional[str] = None,
        checkpoint: Optional[Dict[str, Any]] = None,
        message: Optional[str] = None,
        progress: Optional[int] = None,
    ) -> Optional[BackgroundTaskRecord]:
        """Update workflow-oriented metadata for a task."""
        async with self._lock:
            record = self._tasks.get(task_id)
            if not record or record.user_id != user_id:
                return None

            if stage_code is not None:
                record.stage_code = stage_code
            if execution_mode is not None:
                record.execution_mode = execution_mode
            if workflow_scope is not None:
                record.workflow_scope = workflow_scope
            if checkpoint is not None:
                record.checkpoint = checkpoint
            if message is not None:
                record.message = message
            if progress is not None and record.status not in {"completed", "failed", "cancelled"}:
                record.progress = max(0, min(int(progress), 100))

            record.updated_at = datetime.now(timezone.utc)
            self._persist_locked()
            return record

    async def attach_runner(self, task_id: str, runner_task: asyncio.Task[None]) -> None:
        async with self._lock:
            self._runner_tasks[task_id] = runner_task

    async def cancel_task(self, task_id: str, user_id: str) -> Optional[BackgroundTaskRecord]:
        runner_task: Optional[asyncio.Task[None]] = None
        async with self._lock:
            self._cleanup_locked()
            record = self._tasks.get(task_id)
            if not record or record.user_id != user_id:
                return None

            if record.status in {"completed", "failed", "cancelled"}:
                return record

            now = datetime.now(timezone.utc)
            record.status = "cancelled"
            record.message = "任务已取消"
            if not record.started_at:
                record.started_at = now
            record.completed_at = now
            record.updated_at = now
            self._touch_checkpoint(
                record,
                event="cancelled",
                progress=record.progress,
                message=record.message,
            )

            runner_task = self._runner_tasks.get(task_id)
            self._persist_locked()

        if runner_task and not runner_task.done():
            runner_task.cancel()

        return record

    async def mark_running(self, task_id: str, message: Optional[str] = None) -> None:
        async with self._lock:
            record = self._tasks.get(task_id)
            if not record:
                return
            if record.status in {"failed", "completed", "cancelled"}:
                return
            record.status = "running"
            if message:
                record.message = message
            if not record.started_at:
                record.started_at = datetime.now(timezone.utc)
            record.updated_at = datetime.now(timezone.utc)
            self._touch_checkpoint(
                record,
                event="running",
                progress=record.progress,
                message=record.message,
            )
            self._persist_locked()

    async def update_progress(self, task_id: str, progress: Optional[int], message: Optional[str]) -> None:
        async with self._lock:
            record = self._tasks.get(task_id)
            if not record or record.status in {"failed", "completed", "cancelled"}:
                return
            record.status = "running"
            if progress is not None:
                record.progress = max(0, min(int(progress), 100))
            if message:
                record.message = message
            progress_phase = self._resolve_progress_phase(
                record,
                message=record.message,
                progress=record.progress,
            )
            next_stage_code = self._resolve_stage_code_for_phase(record, progress_phase)
            if next_stage_code is not None:
                record.stage_code = next_stage_code
            if not record.started_at:
                record.started_at = datetime.now(timezone.utc)
            record.updated_at = datetime.now(timezone.utc)
            checkpoint_extra: Dict[str, Any] = {}
            if progress_phase:
                checkpoint_extra["progress_phase"] = progress_phase
            if record.stage_code:
                checkpoint_extra["stage_code"] = record.stage_code
            self._touch_checkpoint(
                record,
                event="progress",
                progress=record.progress,
                message=record.message,
                extra=checkpoint_extra or None,
            )
            self._persist_locked()

    async def set_result(self, task_id: str, result: Dict[str, Any]) -> None:
        async with self._lock:
            record = self._tasks.get(task_id)
            if not record or record.status == "cancelled":
                return
            record.result = result
            record.updated_at = datetime.now(timezone.utc)
            self._touch_checkpoint(
                record,
                event="result",
                progress=record.progress,
                message=record.message,
                extra={"has_result": True},
            )
            self._persist_locked()

    async def mark_completed(self, task_id: str, message: Optional[str] = None) -> None:
        async with self._lock:
            record = self._tasks.get(task_id)
            if not record or record.status in {"failed", "cancelled"}:
                return
            record.status = "completed"
            record.progress = 100
            if message:
                record.message = message
            next_stage_code = self._resolve_stage_code_for_phase(record, "complete")
            if next_stage_code is not None:
                record.stage_code = next_stage_code
            if not record.started_at:
                record.started_at = datetime.now(timezone.utc)
            record.completed_at = datetime.now(timezone.utc)
            record.updated_at = datetime.now(timezone.utc)
            self._touch_checkpoint(
                record,
                event="completed",
                progress=record.progress,
                message=record.message,
                extra={"progress_phase": "complete", "stage_code": record.stage_code}
                if record.stage_code
                else {"progress_phase": "complete"},
            )
            self._persist_locked()

    async def mark_failed(self, task_id: str, error: str, message: Optional[str] = None) -> None:
        async with self._lock:
            record = self._tasks.get(task_id)
            if not record or record.status == "cancelled":
                return
            record.status = "failed"
            record.error = error
            record.message = message or "任务执行失败"
            if not record.started_at:
                record.started_at = datetime.now(timezone.utc)
            record.completed_at = datetime.now(timezone.utc)
            record.updated_at = datetime.now(timezone.utc)
            self._touch_checkpoint(
                record,
                event="failed",
                progress=record.progress,
                message=record.message,
                extra={"error": error, "stage_code": record.stage_code},
            )
            self._persist_locked()

    async def run_job(self, task_id: str, job: Awaitable[None]) -> None:
        await self.mark_running(task_id, "后台任务执行中")
        try:
            await job
            async with self._lock:
                record = self._tasks.get(task_id)
                if record and record.status in {"pending", "running"}:
                    record.status = "completed"
                    record.progress = 100
                    record.message = record.message or "任务已完成"
                    record.completed_at = datetime.now(timezone.utc)
                    record.updated_at = datetime.now(timezone.utc)
                    self._touch_checkpoint(
                        record,
                        event="completed",
                        progress=record.progress,
                        message=record.message,
                        extra={"progress_phase": "complete", "stage_code": record.stage_code}
                        if record.stage_code
                        else {"progress_phase": "complete"},
                    )
                    self._persist_locked()
        except asyncio.CancelledError:
            async with self._lock:
                record = self._tasks.get(task_id)
                if record and record.status not in {"completed", "failed", "cancelled"}:
                    now = datetime.now(timezone.utc)
                    record.status = "cancelled"
                    record.message = "任务已取消"
                    if not record.started_at:
                        record.started_at = now
                    record.completed_at = now
                    record.updated_at = now
                    self._persist_locked()
            return
        except Exception as exc:
            logger.error(f"Background task failed: task_id={task_id}, error={exc}")
            await self.mark_failed(task_id, str(exc))
        finally:
            async with self._lock:
                self._runner_tasks.pop(task_id, None)

    async def is_cancelled(self, task_id: str) -> bool:
        async with self._lock:
            record = self._tasks.get(task_id)
            return bool(record and record.status == "cancelled")

    async def consume_sse_stream(self, task_id: str, stream: AsyncIterable[Any]) -> None:
        for_done = False
        async for raw_chunk in stream:
            if await self.is_cancelled(task_id):
                return
            chunk = self._normalize_chunk(raw_chunk)
            if not chunk:
                continue

            for payload in self._extract_sse_payloads(chunk):
                if await self.is_cancelled(task_id):
                    return
                event_type = payload.get("type")
                if event_type == "progress":
                    await self.update_progress(
                        task_id=task_id,
                        progress=payload.get("progress"),
                        message=payload.get("message"),
                    )
                elif event_type == "result":
                    data = payload.get("data")
                    if isinstance(data, dict):
                        await self.set_result(task_id, data)
                elif event_type == "error":
                    err = payload.get("error") or payload.get("message") or "任务执行失败"
                    await self.mark_failed(task_id, str(err))
                    return
                elif event_type == "done":
                    for_done = True
                    await self.mark_completed(task_id)
                    return

        if not for_done:
            await self.mark_completed(task_id)

    def _cleanup_locked(self, force: bool = False) -> None:
        now = datetime.now(timezone.utc)
        ttl = timedelta(seconds=self._ttl_seconds)
        changed = False

        if force:
            removable = [
                task_id
                for task_id, record in self._tasks.items()
                if record.status in {"completed", "failed", "cancelled"}
            ]
            for task_id in removable[: max(0, len(removable) - self._max_tasks // 2)]:
                self._tasks.pop(task_id, None)
                self._runner_tasks.pop(task_id, None)
                changed = True

        removable = []
        for task_id, record in self._tasks.items():
            finished_at = record.completed_at or record.updated_at
            if record.status in {"completed", "failed", "cancelled"} and now - finished_at > ttl:
                removable.append(task_id)
        for task_id in removable:
            self._tasks.pop(task_id, None)
            self._runner_tasks.pop(task_id, None)
            changed = True

        if changed:
            self._persist_locked()

    @staticmethod
    def _normalize_chunk(raw_chunk: Any) -> str:
        if raw_chunk is None:
            return ""
        if isinstance(raw_chunk, bytes):
            return raw_chunk.decode("utf-8", errors="ignore")
        return str(raw_chunk)

    @staticmethod
    def _extract_sse_payloads(chunk: str) -> list[Dict[str, Any]]:
        payloads: list[Dict[str, Any]] = []
        for line in chunk.splitlines():
            if not line.startswith("data:"):
                continue
            data_part = line[5:].strip()
            if not data_part:
                continue
            try:
                payload = json.loads(data_part)
                if isinstance(payload, dict):
                    payloads.append(payload)
            except json.JSONDecodeError:
                continue
        return payloads


background_task_manager = BackgroundTaskManager()
