"""Test-only chapter candidate finalize support migrated out of app/services."""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, Iterable, List, Mapping, Optional

from tests.test_support.chapter_candidate_models_test_support import (
    ChapterCandidateWorkingSet,
)


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


@dataclass(slots=True)
class ChapterCandidateFinalizeRequest:
    target_word_count: int
    source: str
    runtime_state: Optional[Dict[str, Any]] = None


@dataclass(slots=True)
class ChapterCandidateFinalizeDependencies:
    resolve_generation_attempt_labels_fn: Callable[..., Any]
    build_candidate_selection_metadata_fn: Callable[..., Any]
    attach_candidate_selection_metadata_fn: Callable[..., Any]
    normalize_candidate_quality_gate_plan_fn: Callable[..., Any]
    build_candidate_pool_summary_fn: Callable[..., Any]
    sync_generation_runtime_state_fn: Callable[..., Any]
    select_best_generation_candidate_fn: Callable[..., Any]
    should_prefer_word_budget_repair_candidate_fn: Callable[..., Any]


@dataclass(frozen=True, slots=True)
class ChapterCandidateFinalizeMetadataContext:
    word_count: int
    target_word_count: int
    candidate_index: int
    candidate_count: int
    source: str
    generation_path: str
    attempt_kind: str
    rerank_used: bool
    word_budget_repair_used: bool
    winner_candidate_index: int


@dataclass(slots=True)
class ChapterCandidateFinalizeState(ChapterCandidateWorkingSet):
    winner_candidate_index: int
    final_attempt_kind: str
    final_generation_path: str
    final_quality_metrics: Dict[str, Any]
    final_quality_gate_plan: Dict[str, Any]
    rerank_used: bool
    word_budget_repair_used: bool


def _build_finalize_metadata_context(
    *,
    request: ChapterCandidateFinalizeRequest,
    candidate_count: int,
    winner_candidate_index: int,
    final_attempt_kind: str,
    final_generation_path: str,
    rerank_used: bool,
    word_budget_repair_used: bool,
    word_count: int,
) -> ChapterCandidateFinalizeMetadataContext:
    return ChapterCandidateFinalizeMetadataContext(
        word_count=word_count,
        target_word_count=request.target_word_count,
        candidate_index=winner_candidate_index,
        candidate_count=candidate_count,
        source=request.source,
        generation_path=final_generation_path,
        attempt_kind=final_attempt_kind,
        rerank_used=rerank_used,
        word_budget_repair_used=word_budget_repair_used,
        winner_candidate_index=winner_candidate_index,
    )


def _build_attached_final_selection_metadata(
    *,
    quality_metrics: Dict[str, Any],
    quality_gate_plan: Dict[str, Any],
    metadata_context: ChapterCandidateFinalizeMetadataContext,
    dependencies: ChapterCandidateFinalizeDependencies,
) -> tuple[Dict[str, Any], Dict[str, Any]]:
    selection_metadata = dependencies.build_candidate_selection_metadata_fn(
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
        winner_candidate_index=metadata_context.winner_candidate_index,
    )
    attached_quality_metrics = dependencies.attach_candidate_selection_metadata_fn(
        quality_metrics,
        selection_metadata=selection_metadata,
    )
    return selection_metadata, attached_quality_metrics


def build_chapter_candidate_finalize_dependencies(
    *,
    resolve_generation_attempt_labels_fn: Callable[..., Any],
    build_candidate_selection_metadata_fn: Callable[..., Any],
    attach_candidate_selection_metadata_fn: Callable[..., Any],
    normalize_candidate_quality_gate_plan_fn: Callable[..., Any],
    build_candidate_pool_summary_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    select_best_generation_candidate_fn: Callable[..., Any],
    should_prefer_word_budget_repair_candidate_fn: Callable[..., Any],
) -> ChapterCandidateFinalizeDependencies:
    return ChapterCandidateFinalizeDependencies(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        build_candidate_selection_metadata_fn=build_candidate_selection_metadata_fn,
        attach_candidate_selection_metadata_fn=attach_candidate_selection_metadata_fn,
        normalize_candidate_quality_gate_plan_fn=normalize_candidate_quality_gate_plan_fn,
        build_candidate_pool_summary_fn=build_candidate_pool_summary_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        select_best_generation_candidate_fn=select_best_generation_candidate_fn,
        should_prefer_word_budget_repair_candidate_fn=should_prefer_word_budget_repair_candidate_fn,
    )


def resolve_final_candidate_state(
    *,
    request: ChapterCandidateFinalizeRequest,
    selected_candidate: Dict[str, Any],
    candidates: List[Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    dependencies: ChapterCandidateFinalizeDependencies,
) -> ChapterCandidateFinalizeState:
    selected_candidate = dict(selected_candidate)
    candidate_view = snapshot_chapter_candidate(selected_candidate)
    winner_candidate_index = candidate_view.candidate_index
    selected_attempt_kind = candidate_view.attempt_kind
    selected_generation_path = candidate_view.generation_path
    word_budget_repair_used = is_word_budget_repair_candidate(selected_candidate)
    rerank_used = winner_candidate_index > 1 and not word_budget_repair_used
    final_attempt_kind = str(
        selected_attempt_kind
        or dependencies.resolve_generation_attempt_labels_fn(
            winner_candidate_index,
            is_word_budget_repair=word_budget_repair_used,
        )[1]
    )
    final_generation_path = (
        selected_generation_path
        or (
            "word_budget_repair"
            if word_budget_repair_used
            else "rerank_retry" if rerank_used else "single_pass"
        )
    )

    final_quality_metrics = dict(candidate_view.quality_metrics)
    provisional_quality_gate_plan = dict(candidate_view.quality_gate_plan)
    metadata_context = _build_finalize_metadata_context(
        request=request,
        candidate_count=len(candidates),
        winner_candidate_index=winner_candidate_index,
        final_attempt_kind=final_attempt_kind,
        final_generation_path=final_generation_path,
        rerank_used=rerank_used,
        word_budget_repair_used=word_budget_repair_used,
        word_count=candidate_view.word_count,
    )
    final_selection_metadata, final_quality_metrics = _build_attached_final_selection_metadata(
        quality_metrics=final_quality_metrics,
        quality_gate_plan=provisional_quality_gate_plan,
        metadata_context=metadata_context,
        dependencies=dependencies,
    )
    final_quality_gate_plan = quality_gate_plan_builder(final_quality_metrics, 0)
    final_quality_gate_plan = dependencies.normalize_candidate_quality_gate_plan_fn(
        final_quality_gate_plan,
        word_count=candidate_view.word_count,
        target_word_count=request.target_word_count,
        quality_metrics=final_quality_metrics,
    )
    if isinstance(final_quality_gate_plan.get("quality_gate"), dict):
        final_quality_metrics["quality_gate"] = final_quality_gate_plan["quality_gate"]
    final_selection_metadata, final_quality_metrics = _build_attached_final_selection_metadata(
        quality_metrics=final_quality_metrics,
        quality_gate_plan=final_quality_gate_plan,
        metadata_context=metadata_context,
        dependencies=dependencies,
    )

    selected_candidate.update(final_selection_metadata)
    selected_candidate["quality_metrics"] = final_quality_metrics
    selected_candidate["quality_gate_plan"] = final_quality_gate_plan

    return ChapterCandidateFinalizeState(
        selected_candidate=selected_candidate,
        candidates=candidates,
        winner_candidate_index=winner_candidate_index,
        final_attempt_kind=final_attempt_kind,
        final_generation_path=final_generation_path,
        final_quality_metrics=final_quality_metrics,
        final_quality_gate_plan=final_quality_gate_plan,
        rerank_used=rerank_used,
        word_budget_repair_used=word_budget_repair_used,
    )


def maybe_promote_best_word_budget_repair_candidate(
    *,
    request: ChapterCandidateFinalizeRequest,
    state: ChapterCandidateFinalizeState,
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    dependencies: ChapterCandidateFinalizeDependencies,
) -> ChapterCandidateFinalizeState:
    final_quality_gate = (
        state.final_quality_gate_plan.get("quality_gate")
        if isinstance(state.final_quality_gate_plan.get("quality_gate"), dict)
        else {}
    )
    if str(final_quality_gate.get("decision") or "").strip() == "allow_save":
        return state

    repair_candidates = collect_word_budget_repair_candidates(state.candidates)
    if not repair_candidates:
        return state

    best_word_budget_repair_candidate = (
        dependencies.select_best_generation_candidate_fn(repair_candidates)
        or dict(repair_candidates[-1])
    )
    if int(best_word_budget_repair_candidate.get("candidate_index") or 0) == state.winner_candidate_index:
        return state
    if not dependencies.should_prefer_word_budget_repair_candidate_fn(
        state.selected_candidate,
        best_word_budget_repair_candidate,
    ):
        return state

    return resolve_final_candidate_state(
        request=request,
        selected_candidate=dict(best_word_budget_repair_candidate),
        candidates=state.candidates,
        quality_gate_plan_builder=quality_gate_plan_builder,
        dependencies=dependencies,
    )


def finalize_selected_candidate_result(
    *,
    request: ChapterCandidateFinalizeRequest,
    state: ChapterCandidateFinalizeState,
    dependencies: ChapterCandidateFinalizeDependencies,
) -> Dict[str, Any]:
    selected_candidate = dict(state.selected_candidate)
    selected_candidate_view = snapshot_chapter_candidate(selected_candidate)
    final_quality_metrics = dict(state.final_quality_metrics)

    selected_candidate["candidate_count"] = state.candidate_count
    selected_candidate["rerank_pool_size"] = state.candidate_count
    final_candidate_selection = (
        dict(final_quality_metrics.get("candidate_selection") or {})
        if isinstance(final_quality_metrics.get("candidate_selection"), dict)
        else {}
    )
    candidate_pool_summary = dependencies.build_candidate_pool_summary_fn(
        state.candidates,
        winner_candidate_index=state.winner_candidate_index,
        repair_seed_candidate_index=int(final_candidate_selection.get("repair_seed_candidate_index") or 0) or None,
    )
    if candidate_pool_summary:
        selected_candidate["candidate_pool_summary"] = candidate_pool_summary
        final_quality_metrics = dict(selected_candidate_view.quality_metrics)
        final_quality_metrics["candidate_pool_summary"] = candidate_pool_summary
        selected_candidate["quality_metrics"] = final_quality_metrics

    dependencies.sync_generation_runtime_state_fn(
        request.runtime_state,
        candidate_index=state.winner_candidate_index,
        candidate_total=state.candidate_count,
        current_chars=selected_candidate_view.word_count,
        chunk_count=len(selected_candidate_view.candidate_chunks),
        generation_path=state.final_generation_path,
        attempt_kind=state.final_attempt_kind,
        rerank_used=state.rerank_used,
        word_budget_repair_used=state.word_budget_repair_used,
        winner_candidate_index=state.winner_candidate_index,
    )
    return selected_candidate
