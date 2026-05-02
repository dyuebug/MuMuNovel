from __future__ import annotations

import asyncio
from dataclasses import dataclass
from datetime import datetime
from typing import Any, Dict, Iterable, Optional


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
