from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, Optional

from app.services.chapter_candidate_view_service import snapshot_chapter_candidate


@dataclass(frozen=True)
class ChapterCandidateNormalizedResult:
    candidate_index: int
    candidate_count: int
    full_content: str
    candidate_word_count: int
    candidate_chunks: list[str]
    quality_metrics: Dict[str, Any]
    quality_gate_plan: Dict[str, Any]
    quality_gate_action: str
    quality_gate_snapshot: Optional[Dict[str, Any]]


def normalize_selected_candidate_result(
    *,
    selected_candidate: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any],
    include_quality_gate_snapshot_in_metrics: bool = False,
) -> ChapterCandidateNormalizedResult:
    candidate_view = snapshot_chapter_candidate(selected_candidate)
    quality_metrics = dict(candidate_view.quality_metrics)
    quality_gate_plan = dict(candidate_view.quality_gate_plan)
    quality_gate_action = str(quality_gate_plan.get("action") or "continue")
    quality_gate_snapshot = quality_gate_plan.get("quality_gate")
    if include_quality_gate_snapshot_in_metrics and isinstance(quality_gate_snapshot, dict):
        quality_metrics = {
            **quality_metrics,
            "quality_gate": quality_gate_snapshot,
        }
    quality_metrics = attach_story_runtime_contract_fn(quality_metrics, story_runtime_contract)
    return ChapterCandidateNormalizedResult(
        candidate_index=candidate_view.candidate_index,
        candidate_count=candidate_view.candidate_count,
        full_content=candidate_view.full_content,
        candidate_word_count=candidate_view.word_count,
        candidate_chunks=list(candidate_view.candidate_chunks),
        quality_metrics=quality_metrics if isinstance(quality_metrics, dict) else {},
        quality_gate_plan=quality_gate_plan,
        quality_gate_action=quality_gate_action,
        quality_gate_snapshot=quality_gate_snapshot if isinstance(quality_gate_snapshot, dict) else None,
    )
