"""Compatibility helpers for project quality trend route seams."""
from __future__ import annotations

from datetime import datetime
from typing import Any, Callable, Dict, List, Optional

from app.services.project_quality_trend_service import (
    get_project_quality_trend_snapshot as _get_project_quality_trend_snapshot_service,
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
    build_summary_state_fn: Callable[..., Optional[Dict[str, Any]]],
    advance_summary_state_fn: Callable[..., Optional[Dict[str, Any]]],
    summary_from_state_fn: Callable[..., Optional[Dict[str, Any]]],
    load_snapshot_fn: Callable[[str, int], Optional[Dict[str, Any]]],
    persist_snapshot_fn: Callable[[str, int, Dict[str, Any]], None],
) -> Dict[str, Any]:
    return await _get_project_quality_trend_snapshot_service(
        project_id=project_id,
        limit=limit,
        items=items,
        metrics_history=metrics_history,
        total_chapters=total_chapters,
        analyzed_chapters=analyzed_chapters,
        last_generated_at=last_generated_at,
        build_summary_state_fn=build_summary_state_fn,
        advance_summary_state_fn=advance_summary_state_fn,
        summary_from_state_fn=summary_from_state_fn,
        load_snapshot_fn=load_snapshot_fn,
        persist_snapshot_fn=persist_snapshot_fn,
    )
