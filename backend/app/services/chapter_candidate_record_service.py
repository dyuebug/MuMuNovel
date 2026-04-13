"""Chapter candidate record building service."""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional

from app.services.chapter_candidate_rerank_service import (
    attach_candidate_selection_metadata,
    build_candidate_selection_metadata,
    normalize_candidate_quality_gate_plan,
)
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
)


@dataclass(slots=True)
class ChapterCandidateRecordRequest:
    full_content: str
    candidate_chunks: List[str]
    target_word_count: int
    source: str
    generation_label: str
    candidate_index: int
    candidate_offset: int
    quality_evaluator: Callable[[str], Dict[str, Any]]
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]]
    generation_path: str
    attempt_kind: str


@dataclass(frozen=True, slots=True)
class ChapterCandidateRecordMetadataContext:
    word_count: int
    target_word_count: int
    candidate_index: int
    candidate_count: int
    source: str
    generation_path: str
    attempt_kind: str
    rerank_used: bool
    word_budget_repair_used: bool


def _build_generation_candidate_record_metadata_context(
    request: ChapterCandidateRecordRequest,
    *,
    word_count: int,
) -> ChapterCandidateRecordMetadataContext:
    attempt_kind = str(request.attempt_kind or '')
    return ChapterCandidateRecordMetadataContext(
        word_count=word_count,
        target_word_count=request.target_word_count,
        candidate_index=request.candidate_index,
        candidate_count=request.candidate_index,
        source=request.source,
        generation_path=request.generation_path,
        attempt_kind=attempt_kind,
        rerank_used=attempt_kind == 'rerank_candidate',
        word_budget_repair_used=attempt_kind == 'word_budget_repair',
    )


def _build_attached_generation_candidate_selection_metadata(
    *,
    quality_metrics: Dict[str, Any],
    quality_gate_plan: Dict[str, Any],
    metadata_context: ChapterCandidateRecordMetadataContext,
) -> tuple[Dict[str, Any], Dict[str, Any]]:
    selection_metadata = build_candidate_selection_metadata(
        quality_metrics,
        word_count=metadata_context.word_count,
        target_word_count=metadata_context.target_word_count,
        candidate_index=metadata_context.candidate_index,
        candidate_count=metadata_context.candidate_count,
        source=metadata_context.source,
        quality_gate_plan=quality_gate_plan,
        generation_path=metadata_context.generation_path,
        attempt_kind=metadata_context.attempt_kind,
        rerank_used=metadata_context.rerank_used,
        word_budget_repair_used=metadata_context.word_budget_repair_used,
    )
    attached_quality_metrics = attach_candidate_selection_metadata(
        quality_metrics,
        selection_metadata=selection_metadata,
    )
    return selection_metadata, attached_quality_metrics


def build_generation_candidate_record(
    *,
    request: ChapterCandidateRecordRequest,
    log_warning_fn: Optional[Callable[[str], None]] = None,
) -> Dict[str, Any]:
    full_content, removed_meta_lines = sanitize_generated_narrative_text(request.full_content)
    if removed_meta_lines > 0 and callable(log_warning_fn):
        log_warning_fn(
            f'Sanitized {removed_meta_lines} workflow/meta lines: '
            f'{request.generation_label}, candidate={request.candidate_index}'
        )
    if not full_content.strip():
        raise ValueError(
            f'{request.generation_label} generated empty narrative after sanitization'
        )
    if contains_chapter_workflow_meta_text(full_content):
        raise ValueError(f'{request.generation_label} generated workflow/meta text')

    candidate_word_count = len(full_content)
    metadata_context = _build_generation_candidate_record_metadata_context(
        request,
        word_count=candidate_word_count,
    )
    quality_metrics = dict(request.quality_evaluator(full_content) or {})
    initial_quality_gate_plan = request.quality_gate_plan_builder(
        quality_metrics,
        request.candidate_offset,
    )
    initial_quality_gate_plan = normalize_candidate_quality_gate_plan(
        initial_quality_gate_plan,
        word_count=candidate_word_count,
        target_word_count=request.target_word_count,
        quality_metrics=quality_metrics,
    )
    if isinstance(initial_quality_gate_plan.get('quality_gate'), dict):
        quality_metrics['quality_gate'] = initial_quality_gate_plan['quality_gate']
    _, enriched_quality_metrics = _build_attached_generation_candidate_selection_metadata(
        quality_metrics=quality_metrics,
        quality_gate_plan=initial_quality_gate_plan,
        metadata_context=metadata_context,
    )
    quality_gate_plan = request.quality_gate_plan_builder(
        enriched_quality_metrics,
        request.candidate_offset,
    )
    if not isinstance(quality_gate_plan, dict) or not quality_gate_plan:
        quality_gate_plan = initial_quality_gate_plan
    quality_gate_plan = normalize_candidate_quality_gate_plan(
        quality_gate_plan,
        word_count=candidate_word_count,
        target_word_count=request.target_word_count,
        quality_metrics=enriched_quality_metrics,
    )
    if isinstance(quality_gate_plan.get('quality_gate'), dict):
        enriched_quality_metrics['quality_gate'] = quality_gate_plan['quality_gate']
    elif isinstance(initial_quality_gate_plan.get('quality_gate'), dict):
        enriched_quality_metrics['quality_gate'] = initial_quality_gate_plan['quality_gate']
    selection_metadata, enriched_quality_metrics = _build_attached_generation_candidate_selection_metadata(
        quality_metrics=enriched_quality_metrics,
        quality_gate_plan=quality_gate_plan,
        metadata_context=metadata_context,
    )

    candidate_summary = full_content[:300].replace('\n', ' ') if full_content else ''
    return {
        'candidate_index': request.candidate_index,
        'full_content': full_content,
        'word_count': candidate_word_count,
        'summary_preview': candidate_summary,
        'quality_metrics': enriched_quality_metrics,
        'quality_gate_plan': quality_gate_plan,
        'candidate_chunks': list(request.candidate_chunks),
        **selection_metadata,
    }
