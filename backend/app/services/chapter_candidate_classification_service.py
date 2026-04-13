from __future__ import annotations

from typing import Any, Dict, Iterable, List, Mapping, Optional

from app.services.chapter_candidate_view_service import snapshot_chapter_candidate


def is_word_budget_repair_candidate(candidate: Optional[Mapping[str, Any]]) -> bool:
    if not isinstance(candidate, Mapping):
        return False
    candidate_view = snapshot_chapter_candidate(candidate)
    return (
        candidate_view.attempt_kind == "word_budget_repair"
        or candidate_view.generation_path == "word_budget_repair"
    )


def is_targeted_quality_repair_candidate(candidate: Optional[Mapping[str, Any]]) -> bool:
    if not isinstance(candidate, Mapping):
        return False
    candidate_view = snapshot_chapter_candidate(candidate)
    return (
        candidate_view.attempt_kind == "targeted_quality_repair"
        or candidate_view.generation_path == "targeted_quality_repair"
    )


def collect_word_budget_repair_candidates(
    candidates: Iterable[Optional[Mapping[str, Any]]],
) -> List[Dict[str, Any]]:
    repair_candidates: List[Dict[str, Any]] = []
    for candidate in candidates:
        if isinstance(candidate, Mapping) and is_word_budget_repair_candidate(candidate):
            repair_candidates.append(dict(candidate))
    return repair_candidates
