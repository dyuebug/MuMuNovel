from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Dict, Optional

from app.config import DATA_DIR


PROJECT_QUALITY_TREND_SNAPSHOT_DIR = DATA_DIR / "project_quality_trend_snapshots"
PROJECT_QUALITY_TREND_SNAPSHOT_DIR.mkdir(parents=True, exist_ok=True)



def _normalize_snapshot_file_stem(project_id: str, limit: int) -> str:
    normalized_project_id = re.sub(r"[^a-zA-Z0-9_-]+", "_", str(project_id or "").strip()) or "project"
    normalized_limit = max(int(limit or 0), 0)
    return f"{normalized_project_id}__{normalized_limit}"



def _snapshot_path(project_id: str, limit: int) -> Path:
    return PROJECT_QUALITY_TREND_SNAPSHOT_DIR / f"{_normalize_snapshot_file_stem(project_id, limit)}.json"



def load_project_quality_trend_snapshot(project_id: str, limit: int) -> Optional[Dict[str, Any]]:
    snapshot_path = _snapshot_path(project_id, limit)
    if not snapshot_path.exists():
        return None
    try:
        payload = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None



def persist_project_quality_trend_snapshot(project_id: str, limit: int, snapshot: Dict[str, Any]) -> None:
    snapshot_path = _snapshot_path(project_id, limit)
    snapshot_path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = snapshot_path.with_suffix(".tmp")
    serialized = json.dumps(snapshot, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    temp_path.write_text(serialized, encoding="utf-8")
    temp_path.replace(snapshot_path)



def delete_project_quality_trend_snapshot(project_id: str, limit: int) -> None:
    snapshot_path = _snapshot_path(project_id, limit)
    try:
        snapshot_path.unlink(missing_ok=True)
    except OSError:
        return
