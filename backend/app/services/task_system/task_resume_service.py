from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Dict, Optional


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
