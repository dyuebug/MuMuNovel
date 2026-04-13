"""Compatibility helpers for chapter candidate executor entry points."""
from __future__ import annotations

from typing import Any, Callable, Dict, List, Optional

from app.services.ai_service import AIService
from app.services.chapter_candidate_output_service import (
    ChapterCandidateOutputRequest,
    collect_generation_candidate_output as _collect_generation_candidate_output_service,
)
from app.services.chapter_candidate_record_service import (
    ChapterCandidateRecordRequest,
    build_generation_candidate_record as _build_generation_candidate_record_service,
)
from app.services.chapter_candidate_runtime_state_service import (
    sync_chapter_candidate_runtime_state,
)
from app.services.chapter_candidate_executor_wiring_service import (
    build_default_chapter_candidate_executor_dependencies as _build_default_chapter_candidate_executor_dependencies_service,
)


async def collect_generation_candidate_output(
    ai_service: AIService,
    generate_kwargs: Dict[str, Any],
    *,
    candidate_index: int = 1,
    max_output_chars: Optional[int] = None,
    runtime_state: Optional[Dict[str, Any]] = None,
) -> tuple[str, List[str]]:
    return await _collect_generation_candidate_output_service(
        request=ChapterCandidateOutputRequest(
            ai_service=ai_service,
            generate_kwargs=generate_kwargs,
            candidate_index=candidate_index,
            max_output_chars=max_output_chars,
            runtime_state=runtime_state,
        ),
    )


def resolve_generation_attempt_labels(
    candidate_index: int,
    *,
    is_word_budget_repair: bool = False,
) -> tuple[str, str]:
    normalized_candidate_index = max(int(candidate_index or 1), 1)
    if is_word_budget_repair:
        return "word_budget_repair", "word_budget_repair"
    if normalized_candidate_index > 1:
        return "rerank_retry", "rerank_candidate"
    return "single_pass", "initial_candidate"


def sync_generation_runtime_state(
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
    sync_chapter_candidate_runtime_state(
        runtime_state,
        candidate_index=candidate_index,
        candidate_total=candidate_total,
        current_chars=current_chars,
        chunk_count=chunk_count,
        generation_path=generation_path,
        attempt_kind=attempt_kind,
        rerank_used=rerank_used,
        word_budget_repair_used=word_budget_repair_used,
        winner_candidate_index=winner_candidate_index,
    )


def build_generation_candidate_record(
    *,
    full_content: str,
    candidate_chunks: List[str],
    target_word_count: int,
    source: str,
    generation_label: str,
    candidate_index: int,
    candidate_offset: int,
    quality_evaluator: Callable[[str], Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    generation_path: str,
    attempt_kind: str,
    log_warning_fn: Callable[..., Any],
) -> Dict[str, Any]:
    return _build_generation_candidate_record_service(
        request=ChapterCandidateRecordRequest(
            full_content=full_content,
            candidate_chunks=candidate_chunks,
            target_word_count=target_word_count,
            source=source,
            generation_label=generation_label,
            candidate_index=candidate_index,
            candidate_offset=candidate_offset,
            quality_evaluator=quality_evaluator,
            quality_gate_plan_builder=quality_gate_plan_builder,
            generation_path=generation_path,
            attempt_kind=attempt_kind,
        ),
        log_warning_fn=log_warning_fn,
    )


def build_default_chapter_candidate_executor_dependencies(
    *,
    resolve_generation_attempt_labels_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    collect_generation_candidate_output_fn: Callable[..., Any],
    build_generation_candidate_record_fn: Callable[..., Any],
):
    return _build_default_chapter_candidate_executor_dependencies_service(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
    )
