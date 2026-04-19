from __future__ import annotations

from typing import Any, Dict, Mapping

from app.models.chapter import Chapter
from app.services.chapter_candidate_runtime_state_service import ChapterCandidateRuntimeStateSnapshot
from app.services.chapter_candidate_view_service import ChapterCandidateView


def build_batch_generation_start_progress_event(*, chapter: Chapter) -> Dict[str, Any]:
    return {
        "type": "progress",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "message": f"Generating chapter {chapter.chapter_number}",
        "progress": 35,
        "status": "running",
        "phase": "generating",
    }


def build_batch_generation_candidate_progress_event(
    *,
    chapter: Chapter,
    runtime_snapshot: ChapterCandidateRuntimeStateSnapshot,
    target_word_count: int,
) -> Dict[str, Any]:
    progress = 35 + int(min(runtime_snapshot.current_chars / max(target_word_count, 1), 1.0) * 25)
    if runtime_snapshot.candidate_index > 1:
        progress = max(progress, 40 + (runtime_snapshot.candidate_index - 1) * 5)
    progress = min(progress, 70)
    return {
        "type": "progress",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "message": (
            f"Generating chapter {chapter.chapter_number} candidate "
            f"{runtime_snapshot.candidate_index}/{runtime_snapshot.candidate_total} "
            f"({runtime_snapshot.current_chars} chars)"
        ),
        "progress": progress,
        "status": "running",
        "phase": "generating",
        "candidate_index": runtime_snapshot.candidate_index,
        "candidate_count": runtime_snapshot.candidate_count,
        "word_count": runtime_snapshot.current_chars,
        "generation_path": runtime_snapshot.generation_path,
        "attempt_kind": runtime_snapshot.attempt_kind,
        "rerank_used": runtime_snapshot.rerank_used,
        "word_budget_repair_used": runtime_snapshot.word_budget_repair_used,
    }


def build_batch_generation_selected_candidate_progress_event(
    *,
    chapter: Chapter,
    selected_candidate_view: ChapterCandidateView,
    candidate_word_count: int,
    chapter_context_stats: Mapping[str, Any],
) -> Dict[str, Any]:
    winner_candidate_index = selected_candidate_view.winner_candidate_index
    return {
        "type": "progress",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "message": (
            f"Selected chapter {chapter.chapter_number} candidate "
            f"{winner_candidate_index}/{selected_candidate_view.candidate_count} "
            f"({candidate_word_count} chars)"
        ),
        "progress": 70,
        "status": "running",
        "phase": "generating",
        "candidate_index": selected_candidate_view.candidate_index,
        "candidate_count": selected_candidate_view.candidate_count,
        "word_count": candidate_word_count,
        "generation_path": selected_candidate_view.generation_path,
        "attempt_kind": selected_candidate_view.attempt_kind,
        "rerank_used": selected_candidate_view.rerank_used,
        "word_budget_repair_used": selected_candidate_view.word_budget_repair_used,
        "winner_candidate_index": winner_candidate_index,
        "pre_compaction_total_length": chapter_context_stats.get("pre_compaction_total_length"),
        "context_budget_limit": chapter_context_stats.get("context_budget_limit"),
        "compaction_applied": chapter_context_stats.get("compaction_applied"),
        "compaction_details": chapter_context_stats.get("compaction_details"),
    }


def build_batch_generation_chunk_event(*, chapter: Chapter, chunk: str) -> Dict[str, Any]:
    return {
        "type": "chunk",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "content": chunk,
    }


def build_chapter_generation_progress_kwargs(
    *,
    runtime_snapshot: ChapterCandidateRuntimeStateSnapshot,
    target_word_count: int,
) -> Dict[str, Any]:
    return {
        "current_chars": runtime_snapshot.current_chars,
        "estimated_total": target_word_count,
        "message": (
            f"候选草稿生成 {runtime_snapshot.candidate_index}/{runtime_snapshot.candidate_total} ... "
            f"({runtime_snapshot.current_chars}字)"
        ),
        "retry_count": max(runtime_snapshot.candidate_index - 1, 0),
        "max_retries": max(runtime_snapshot.candidate_total - 1, 1),
    }
