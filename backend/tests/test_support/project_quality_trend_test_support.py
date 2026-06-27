from __future__ import annotations

import json
import re
from asyncio import Lock
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.retired_runtime_test_support import DATA_DIR
from migrator_app.models.chapter import Chapter
from tests.test_support.chapter_quality_metrics_query_test_support import (
    advance_quality_metrics_summary_state,
    build_quality_metrics_summary_from_state,
    build_quality_metrics_summary_state,
    load_latest_quality_metric_records_for_chapter_ids,
)

project_quality_trend_cache: dict[str, Dict[str, Any]] = {}
project_quality_trend_lock = Lock()
PROJECT_QUALITY_TREND_CACHE_MAX_SIZE = 128
PROJECT_QUALITY_TREND_SNAPSHOT_DIR = DATA_DIR / "project_quality_trend_snapshots"
PROJECT_QUALITY_TREND_SNAPSHOT_DIR.mkdir(parents=True, exist_ok=True)


@dataclass(frozen=True)
class ProjectQualityTrendQueryContext:
    chapters: List[Chapter]
    records_by_chapter: Dict[str, Dict[str, Any]]


def _normalize_json_payload(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, dict):
        return {str(key): _normalize_json_payload(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_normalize_json_payload(item) for item in value]
    if hasattr(value, "model_dump"):
        return _normalize_json_payload(value.model_dump())
    if hasattr(value, "dict"):
        return _normalize_json_payload(value.dict())
    return str(value)


async def load_project_quality_trend_query_context(
    db_session: AsyncSession,
    *,
    project_id: str,
    load_records_fn=load_latest_quality_metric_records_for_chapter_ids,
) -> ProjectQualityTrendQueryContext:
    chapters_result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == project_id)
        .order_by(Chapter.chapter_number)
    )
    chapters = chapters_result.scalars().all()
    chapter_ids = [chapter.id for chapter in chapters]
    records_by_chapter = await load_records_fn(db_session, chapter_ids)
    if not isinstance(records_by_chapter, dict):
        records_by_chapter = {}
    return ProjectQualityTrendQueryContext(
        chapters=chapters,
        records_by_chapter=records_by_chapter,
    )


def _build_project_quality_trend_cache_key(project_id: str, limit: int) -> str:
    return f"{project_id}:{limit}"


def _normalize_project_quality_trend_snapshot_file_stem(project_id: str, limit: int) -> str:
    normalized_project_id = re.sub(r"[^a-zA-Z0-9_-]+", "_", str(project_id or "").strip()) or "project"
    normalized_limit = max(int(limit or 0), 0)
    return f"{normalized_project_id}__{normalized_limit}"


def _project_quality_trend_snapshot_path(project_id: str, limit: int) -> Path:
    return PROJECT_QUALITY_TREND_SNAPSHOT_DIR / (
        f"{_normalize_project_quality_trend_snapshot_file_stem(project_id, limit)}.json"
    )


def load_project_quality_trend_snapshot(
    project_id: str,
    limit: int,
) -> Optional[Dict[str, Any]]:
    snapshot_path = _project_quality_trend_snapshot_path(project_id, limit)
    if not snapshot_path.exists():
        return None
    try:
        payload = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None


def persist_project_quality_trend_snapshot(
    project_id: str,
    limit: int,
    snapshot: Dict[str, Any],
) -> None:
    snapshot_path = _project_quality_trend_snapshot_path(project_id, limit)
    snapshot_path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = snapshot_path.with_suffix(".tmp")
    serialized = json.dumps(
        snapshot,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    )
    temp_path.write_text(serialized, encoding="utf-8")
    temp_path.replace(snapshot_path)


def delete_project_quality_trend_snapshot(project_id: str, limit: int) -> None:
    snapshot_path = _project_quality_trend_snapshot_path(project_id, limit)
    try:
        snapshot_path.unlink(missing_ok=True)
    except OSError:
        return


def _build_project_quality_trend_item_keys(items: List[Dict[str, Any]]) -> List[tuple[str, str]]:
    return [
        (
            str(item.get("chapter_id") or ""),
            str(item.get("history_id") or ""),
        )
        for item in items
        if item.get("chapter_id")
    ]


def _normalize_project_quality_trend_item_keys(value: Any) -> List[tuple[str, str]]:
    normalized: List[tuple[str, str]] = []
    for item in value or []:
        if not isinstance(item, (list, tuple)) or len(item) < 2:
            continue
        normalized.append((str(item[0] or ""), str(item[1] or "")))
    return normalized


def _decorate_project_quality_metrics_summary(
    summary: Optional[Dict[str, Any]],
    *,
    total_chapters: int,
    analyzed_chapters: int,
    last_generated_at: Optional[datetime],
) -> Optional[Dict[str, Any]]:
    if not isinstance(summary, dict):
        return None
    decorated = dict(summary)
    decorated["total_chapters"] = total_chapters
    decorated["analyzed_chapters"] = analyzed_chapters
    decorated["last_generated_at"] = last_generated_at.isoformat() if last_generated_at else None
    return decorated


def _build_project_quality_trend_snapshot(
    *,
    items: List[Dict[str, Any]],
    metrics_history: List[Dict[str, Any]],
    total_chapters: int,
    analyzed_chapters: int,
    last_generated_at: Optional[datetime],
    build_summary_state_fn,
    summary_from_state_fn,
    summary_state: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    resolved_summary_state = summary_state
    if resolved_summary_state is None and metrics_history:
        resolved_summary_state = build_summary_state_fn(metrics_history, scope="batch")
    summary = summary_from_state_fn(resolved_summary_state, scope="batch")
    return {
        "item_keys": _build_project_quality_trend_item_keys(items),
        "items": _normalize_json_payload(items),
        "metrics_history": _normalize_json_payload(metrics_history),
        "_summary_state": _normalize_json_payload(resolved_summary_state),
        "summary": _decorate_project_quality_metrics_summary(
            _normalize_json_payload(summary),
            total_chapters=total_chapters,
            analyzed_chapters=analyzed_chapters,
            last_generated_at=last_generated_at,
        ),
    }


def _try_advance_project_quality_trend_snapshot(
    cached_snapshot: Optional[Dict[str, Any]],
    *,
    items: List[Dict[str, Any]],
    metrics_history: List[Dict[str, Any]],
    total_chapters: int,
    analyzed_chapters: int,
    last_generated_at: Optional[datetime],
    build_summary_state_fn,
    advance_summary_state_fn,
    summary_from_state_fn,
) -> Optional[Dict[str, Any]]:
    if not isinstance(cached_snapshot, dict):
        return None

    current_item_keys = _build_project_quality_trend_item_keys(items)
    cached_item_keys = _normalize_project_quality_trend_item_keys(cached_snapshot.get("item_keys"))
    cached_metrics_history = list(cached_snapshot.get("metrics_history") or [])
    cached_summary_state = cached_snapshot.get("_summary_state")

    if current_item_keys == cached_item_keys:
        return _build_project_quality_trend_snapshot(
            items=items,
            metrics_history=metrics_history,
            total_chapters=total_chapters,
            analyzed_chapters=analyzed_chapters,
            last_generated_at=last_generated_at,
            build_summary_state_fn=build_summary_state_fn,
            summary_from_state_fn=summary_from_state_fn,
            summary_state=cached_summary_state,
        )

    if (
        not current_item_keys
        or not cached_item_keys
        or not isinstance(cached_summary_state, dict)
        or len(cached_item_keys) != len(cached_metrics_history)
    ):
        return None

    overlap = 0
    max_overlap = min(len(cached_item_keys), len(current_item_keys))
    for size in range(max_overlap, 0, -1):
        if cached_item_keys[-size:] == current_item_keys[:size]:
            overlap = size
            break
    if overlap <= 0:
        return None

    dropped_count = len(cached_item_keys) - overlap
    appended_count = len(current_item_keys) - overlap
    if appended_count <= 0 or dropped_count > appended_count:
        return None

    working_history = list(cached_metrics_history)
    working_state: Optional[Dict[str, Any]] = dict(cached_summary_state)
    append_index = overlap

    for _ in range(dropped_count):
        if append_index >= len(metrics_history) or not working_history:
            return None
        dropped_event = working_history[0]
        appended_event = metrics_history[append_index]
        next_history = working_history[1:] + [appended_event]
        working_state = advance_summary_state_fn(
            working_state,
            appended_event=appended_event,
            current_history=next_history,
            dropped_event=dropped_event,
            scope="batch",
        )
        if working_state is None:
            return None
        working_history = next_history
        append_index += 1

    if append_index != len(metrics_history):
        return None

    return _build_project_quality_trend_snapshot(
        items=items,
        metrics_history=metrics_history,
        total_chapters=total_chapters,
        analyzed_chapters=analyzed_chapters,
        last_generated_at=last_generated_at,
        build_summary_state_fn=build_summary_state_fn,
        summary_from_state_fn=summary_from_state_fn,
        summary_state=working_state,
    )


async def get_project_quality_trend_snapshot(
    *,
    project_id: str,
    limit: int,
    items: List[Dict[str, Any]],
    metrics_history: List[Dict[str, Any]],
    total_chapters: int,
    analyzed_chapters: int,
    last_generated_at: Optional[datetime],
    build_summary_state_fn=build_quality_metrics_summary_state,
    advance_summary_state_fn=advance_quality_metrics_summary_state,
    summary_from_state_fn=build_quality_metrics_summary_from_state,
    load_snapshot_fn=load_project_quality_trend_snapshot,
    persist_snapshot_fn=persist_project_quality_trend_snapshot,
    max_cache_size: int = PROJECT_QUALITY_TREND_CACHE_MAX_SIZE,
) -> Dict[str, Any]:
    cache_key = _build_project_quality_trend_cache_key(project_id, limit)
    async with project_quality_trend_lock:
        cached_snapshot = project_quality_trend_cache.get(cache_key)
        if cached_snapshot is None:
            persisted_snapshot = load_snapshot_fn(project_id, limit)
            if isinstance(persisted_snapshot, dict):
                project_quality_trend_cache[cache_key] = persisted_snapshot
                cached_snapshot = persisted_snapshot

        snapshot = _try_advance_project_quality_trend_snapshot(
            cached_snapshot,
            items=items,
            metrics_history=metrics_history,
            total_chapters=total_chapters,
            analyzed_chapters=analyzed_chapters,
            last_generated_at=last_generated_at,
            build_summary_state_fn=build_summary_state_fn,
            advance_summary_state_fn=advance_summary_state_fn,
            summary_from_state_fn=summary_from_state_fn,
        )
        if snapshot is None:
            snapshot = _build_project_quality_trend_snapshot(
                items=items,
                metrics_history=metrics_history,
                total_chapters=total_chapters,
                analyzed_chapters=analyzed_chapters,
                last_generated_at=last_generated_at,
                build_summary_state_fn=build_summary_state_fn,
                summary_from_state_fn=summary_from_state_fn,
            )
        project_quality_trend_cache[cache_key] = snapshot
        persist_snapshot_fn(project_id, limit, snapshot)
        while len(project_quality_trend_cache) > max_cache_size:
            oldest_key = next(iter(project_quality_trend_cache))
            project_quality_trend_cache.pop(oldest_key, None)
        return {
            "items": list(snapshot.get("items") or []),
            "summary": snapshot.get("summary"),
        }


_get_project_quality_trend_snapshot_service = get_project_quality_trend_snapshot


async def get_project_quality_trend_snapshot_with_default_wiring(
    *,
    project_id: str,
    limit: int,
    items: List[Dict[str, Any]],
    metrics_history: List[Dict[str, Any]],
    total_chapters: int,
    analyzed_chapters: int,
    last_generated_at: Optional[datetime],
) -> Dict[str, Any]:
    return await _get_project_quality_trend_snapshot_service(
        project_id=project_id,
        limit=limit,
        items=items,
        metrics_history=metrics_history,
        total_chapters=total_chapters,
        analyzed_chapters=analyzed_chapters,
        last_generated_at=last_generated_at,
        build_summary_state_fn=build_quality_metrics_summary_state,
        advance_summary_state_fn=advance_quality_metrics_summary_state,
        summary_from_state_fn=build_quality_metrics_summary_from_state,
        load_snapshot_fn=load_project_quality_trend_snapshot,
        persist_snapshot_fn=persist_project_quality_trend_snapshot,
    )


async def build_project_quality_trend_response_payload(
    *,
    project_id: str,
    chapters: List[Chapter],
    records_by_chapter: Dict[str, Dict[str, Any]],
    limit: int,
    resolve_snapshot_fn,
) -> Dict[str, Any]:
    items: List[Dict[str, Any]] = []
    metrics_history: List[Dict[str, Any]] = []
    last_generated_at: Optional[datetime] = None

    for chapter in chapters:
        record = records_by_chapter.get(chapter.id)
        if not isinstance(record, dict):
            continue

        latest_quality_metrics = record.get("latest_quality_metrics")
        if isinstance(latest_quality_metrics, dict):
            metrics_history.append(latest_quality_metrics)

        generated_at_dt = record.get("generated_at_dt")
        if isinstance(generated_at_dt, datetime) and (
            last_generated_at is None or generated_at_dt > last_generated_at
        ):
            last_generated_at = generated_at_dt

        items.append(
            {
                "chapter_id": chapter.id,
                "chapter_number": chapter.chapter_number,
                "title": chapter.title,
                "status": chapter.status,
                "history_id": record.get("history_id"),
                "generated_at": record.get("generated_at"),
                "latest_quality_metrics": latest_quality_metrics,
            }
        )

    if limit > 0 and len(items) > limit:
        items = items[-limit:]
        metrics_history = metrics_history[-limit:]

    trend_snapshot = await resolve_snapshot_fn(
        project_id=project_id,
        limit=limit,
        items=items,
        metrics_history=metrics_history,
        total_chapters=len(chapters),
        analyzed_chapters=len(metrics_history),
        last_generated_at=last_generated_at,
    )

    return {
        "project_id": project_id,
        "has_metrics": bool(metrics_history),
        "total_chapters": len(chapters),
        "analyzed_chapters": len(metrics_history),
        "items": trend_snapshot.get("items") or items,
        "quality_metrics_summary": trend_snapshot.get("summary"),
    }



