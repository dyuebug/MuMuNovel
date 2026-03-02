"""In-memory background task manager for long-running generation jobs."""
from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from typing import Any, AsyncIterable, Awaitable, Dict, Optional

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
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
            "started_at": self.started_at.isoformat() if self.started_at else None,
            "completed_at": self.completed_at.isoformat() if self.completed_at else None,
        }


class BackgroundTaskManager:
    """Stores task state and updates task progress from SSE messages."""

    def __init__(self, ttl_seconds: int = 7200, max_tasks: int = 2000):
        self._tasks: Dict[str, BackgroundTaskRecord] = {}
        self._runner_tasks: Dict[str, asyncio.Task[None]] = {}
        self._lock = asyncio.Lock()
        self._ttl_seconds = ttl_seconds
        self._max_tasks = max_tasks

    async def create_task(
        self,
        task_id: str,
        task_type: str,
        user_id: str,
        project_id: str,
        message: str = "任务已创建",
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
            )
            self._tasks[task_id] = record
            return record

    async def get_task(self, task_id: str, user_id: str) -> Optional[BackgroundTaskRecord]:
        async with self._lock:
            self._cleanup_locked()
            record = self._tasks.get(task_id)
            if not record or record.user_id != user_id:
                return None
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

            runner_task = self._runner_tasks.get(task_id)

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
            if not record.started_at:
                record.started_at = datetime.now(timezone.utc)
            record.updated_at = datetime.now(timezone.utc)

    async def set_result(self, task_id: str, result: Dict[str, Any]) -> None:
        async with self._lock:
            record = self._tasks.get(task_id)
            if not record or record.status == "cancelled":
                return
            record.result = result
            record.updated_at = datetime.now(timezone.utc)

    async def mark_completed(self, task_id: str, message: Optional[str] = None) -> None:
        async with self._lock:
            record = self._tasks.get(task_id)
            if not record or record.status in {"failed", "cancelled"}:
                return
            record.status = "completed"
            record.progress = 100
            if message:
                record.message = message
            if not record.started_at:
                record.started_at = datetime.now(timezone.utc)
            record.completed_at = datetime.now(timezone.utc)
            record.updated_at = datetime.now(timezone.utc)

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

        if force:
            removable = [
                task_id
                for task_id, record in self._tasks.items()
                if record.status in {"completed", "failed", "cancelled"}
            ]
            for task_id in removable[: max(0, len(removable) - self._max_tasks // 2)]:
                self._tasks.pop(task_id, None)
                self._runner_tasks.pop(task_id, None)

        removable = []
        for task_id, record in self._tasks.items():
            finished_at = record.completed_at or record.updated_at
            if record.status in {"completed", "failed", "cancelled"} and now - finished_at > ttl:
                removable.append(task_id)
        for task_id in removable:
            self._tasks.pop(task_id, None)
            self._runner_tasks.pop(task_id, None)

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
