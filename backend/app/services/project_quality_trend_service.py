"""Project quality trend service public owner.

The heavy implementation lives in ``quality_domain``; this module keeps the
stable service import surface and owns the default route wiring that used to
live in a compat shell.
"""

from __future__ import annotations

from datetime import datetime
from typing import Any, Dict, List, Optional

from app.services.quality_domain.project_quality_trend_service import *  # noqa: F401,F403
from app.services.quality_domain.project_quality_trend_service import (
    get_project_quality_trend_snapshot as _get_project_quality_trend_snapshot_service,
)
from app.services.project_quality_trend_snapshot_store import (
    load_project_quality_trend_snapshot,
    persist_project_quality_trend_snapshot,
)
from app.services.story_quality_feedback_service import (
    advance_quality_metrics_summary_state,
    build_quality_metrics_summary_from_state,
    build_quality_metrics_summary_state,
)


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
    """Resolve a quality-trend snapshot with production route dependencies."""
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
