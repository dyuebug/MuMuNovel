from __future__ import annotations

from datetime import datetime, timezone
from typing import Any, Dict, Optional


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
