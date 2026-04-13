from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Mapping, Optional


@dataclass(frozen=True)
class ChapterCandidateView:
    candidate_index: int
    candidate_count: int
    winner_candidate_index: int
    word_count: int
    generation_path: str
    attempt_kind: str
    rerank_used: bool
    word_budget_repair_used: bool
    full_content: str
    candidate_chunks: list[str]
    quality_metrics: Dict[str, Any]
    quality_gate_plan: Dict[str, Any]


def snapshot_chapter_candidate(candidate: Optional[Mapping[str, Any]]) -> ChapterCandidateView:
    source = candidate or {}
    full_content = str(source.get("full_content") or "")
    candidate_index = max(int(source.get("candidate_index") or 1), 1)
    candidate_count = max(int(source.get("candidate_count") or 1), 1)
    winner_candidate_index = max(int(source.get("winner_candidate_index") or candidate_index), 1)
    word_count = max(int(source.get("word_count") or len(full_content)), 0)
    generation_path = str(source.get("generation_path") or "").strip()
    attempt_kind = str(source.get("attempt_kind") or "").strip()
    rerank_used = bool(source.get("rerank_used"))
    word_budget_repair_used = bool(source.get("word_budget_repair_used"))
    candidate_chunks = [str(chunk) for chunk in (source.get("candidate_chunks") or [])]
    quality_metrics = dict(source.get("quality_metrics") or {})
    quality_gate_plan = (
        dict(source.get("quality_gate_plan") or {})
        if isinstance(source.get("quality_gate_plan"), dict)
        else {}
    )
    return ChapterCandidateView(
        candidate_index=candidate_index,
        candidate_count=candidate_count,
        winner_candidate_index=winner_candidate_index,
        word_count=word_count,
        generation_path=generation_path,
        attempt_kind=attempt_kind,
        rerank_used=rerank_used,
        word_budget_repair_used=word_budget_repair_used,
        full_content=full_content,
        candidate_chunks=candidate_chunks,
        quality_metrics=quality_metrics,
        quality_gate_plan=quality_gate_plan,
    )
