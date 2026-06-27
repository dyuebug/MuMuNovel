from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Mapping, Optional


@dataclass(frozen=True)
class ChapterCandidateRuntimeStateSnapshot:
    candidate_total: int
    candidate_count: int
    candidate_index: int
    current_chars: int
    word_count: int
    chunk_count: int
    generation_path: str
    attempt_kind: str
    rerank_used: bool
    word_budget_repair_used: bool
    winner_candidate_index: Optional[int]


def build_chapter_candidate_runtime_state(*, max_candidates: int) -> Dict[str, Any]:
    normalized_max_candidates = max(1, int(max_candidates or 1))
    return {
        "candidate_total": normalized_max_candidates,
        "candidate_count": normalized_max_candidates,
        "candidate_index": 1,
        "current_chars": 0,
        "word_count": 0,
        "chunk_count": 0,
        "generation_path": "single_pass",
        "attempt_kind": "initial_candidate",
        "rerank_used": False,
        "word_budget_repair_used": False,
        "winner_candidate_index": None,
    }


def snapshot_chapter_candidate_runtime_state(
    runtime_state: Optional[Mapping[str, Any]],
    *,
    default_candidate_total: int = 1,
) -> ChapterCandidateRuntimeStateSnapshot:
    normalized_default_candidate_total = max(int(default_candidate_total or 1), 1)
    state = runtime_state or {}
    candidate_index = max(int(state.get("candidate_index") or 1), 1)
    candidate_total = max(
        int(state.get("candidate_total") or normalized_default_candidate_total),
        candidate_index,
    )
    candidate_count = max(int(state.get("candidate_count") or candidate_total), 1)
    current_chars = max(int(state.get("current_chars") or 0), 0)
    word_count = max(int(state.get("word_count") or current_chars), 0)
    chunk_count = max(int(state.get("chunk_count") or 0), 0)
    generation_path = str(state.get("generation_path") or "single_pass").strip() or "single_pass"
    attempt_kind = str(state.get("attempt_kind") or "initial_candidate").strip() or "initial_candidate"
    rerank_used = bool(state.get("rerank_used"))
    word_budget_repair_used = bool(state.get("word_budget_repair_used"))
    winner_candidate_index_value = state.get("winner_candidate_index")
    winner_candidate_index = None
    if winner_candidate_index_value is not None:
        winner_candidate_index = max(int(winner_candidate_index_value or 1), 1)
    return ChapterCandidateRuntimeStateSnapshot(
        candidate_total=candidate_total,
        candidate_count=candidate_count,
        candidate_index=candidate_index,
        current_chars=current_chars,
        word_count=word_count,
        chunk_count=chunk_count,
        generation_path=generation_path,
        attempt_kind=attempt_kind,
        rerank_used=rerank_used,
        word_budget_repair_used=word_budget_repair_used,
        winner_candidate_index=winner_candidate_index,
    )


def sync_chapter_candidate_runtime_state(
    runtime_state: Optional[Dict[str, Any]],
    *,
    candidate_index: int,
    candidate_total: int,
    current_chars: Optional[int] = None,
    chunk_count: Optional[int] = None,
    generation_path: Optional[str] = None,
    attempt_kind: Optional[str] = None,
    rerank_used: Optional[bool] = None,
    word_budget_repair_used: Optional[bool] = None,
    winner_candidate_index: Optional[int] = None,
) -> None:
    if runtime_state is None:
        return

    normalized_candidate_index = max(int(candidate_index or 1), 1)
    normalized_candidate_total = max(
        int(candidate_total or normalized_candidate_index),
        normalized_candidate_index,
    )
    runtime_state["candidate_index"] = normalized_candidate_index
    runtime_state["candidate_total"] = normalized_candidate_total
    runtime_state["candidate_count"] = normalized_candidate_total

    if current_chars is not None:
        normalized_chars = max(int(current_chars or 0), 0)
        runtime_state["current_chars"] = normalized_chars
        runtime_state["word_count"] = normalized_chars
    if chunk_count is not None:
        runtime_state["chunk_count"] = max(int(chunk_count or 0), 0)
    if isinstance(generation_path, str) and generation_path.strip():
        runtime_state["generation_path"] = generation_path.strip()
    if isinstance(attempt_kind, str) and attempt_kind.strip():
        runtime_state["attempt_kind"] = attempt_kind.strip()
    if rerank_used is not None:
        runtime_state["rerank_used"] = bool(rerank_used)
    if word_budget_repair_used is not None:
        runtime_state["word_budget_repair_used"] = bool(word_budget_repair_used)
    if winner_candidate_index is not None:
        runtime_state["winner_candidate_index"] = max(
            int(winner_candidate_index or 1),
            1,
        )
