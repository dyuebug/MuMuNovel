from __future__ import annotations

import asyncio
from typing import Any, Callable, Dict, Optional


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
