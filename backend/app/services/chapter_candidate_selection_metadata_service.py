from __future__ import annotations

from typing import Any, Dict


def attach_repair_seed_candidate_metadata(
    *,
    repair_candidate: Dict[str, Any],
    repair_seed_candidate: Dict[str, Any],
) -> Dict[str, Any]:
    quality_metrics = dict(repair_candidate.get("quality_metrics") or {})
    existing_selection = quality_metrics.get("candidate_selection")
    candidate_selection = (
        dict(existing_selection)
        if isinstance(existing_selection, dict)
        else {}
    )
    candidate_selection["repair_seed_candidate_index"] = max(
        int(repair_seed_candidate.get("candidate_index") or 1),
        1,
    )
    repair_seed_generation_path = str(repair_seed_candidate.get("generation_path") or "").strip()
    repair_seed_attempt_kind = str(repair_seed_candidate.get("attempt_kind") or "").strip()
    if repair_seed_generation_path:
        candidate_selection["repair_seed_generation_path"] = repair_seed_generation_path
    if repair_seed_attempt_kind:
        candidate_selection["repair_seed_attempt_kind"] = repair_seed_attempt_kind
    quality_metrics["candidate_selection"] = candidate_selection
    repair_candidate["quality_metrics"] = quality_metrics
    return repair_candidate
