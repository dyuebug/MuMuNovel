from __future__ import annotations

import asyncio
from asyncio import Queue
from dataclasses import dataclass
from typing import Any, Dict, List, Optional


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
