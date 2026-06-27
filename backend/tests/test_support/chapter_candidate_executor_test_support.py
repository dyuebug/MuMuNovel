"""Test-only chapter candidate executor support migrated out of app/services."""
from __future__ import annotations

import asyncio
from dataclasses import dataclass
from functools import lru_cache
from typing import Any, Callable, Dict, List, Optional

from tests.test_support.retired_runtime_test_support import get_logger
from tests.test_support.ai_gateway.ai_service import AIService
from tests.test_support.chapter_candidate_models_test_support import (
    ChapterCandidateWorkingSet,
)
from tests.test_support.chapter_candidate_word_budget_repair_test_support import (
    ChapterCandidateWordBudgetRepairDependencies,
    ChapterCandidateWordBudgetRepairRequest,
    build_chapter_candidate_word_budget_repair_dependencies,
    maybe_apply_word_budget_repair_workflow,
)
from tests.test_support.chapter_candidate_targeted_final_repair_test_support import (
    ChapterCandidateTargetedFinalRepairDependencies,
    ChapterCandidateTargetedFinalRepairRequest,
    build_chapter_candidate_targeted_final_repair_dependencies,
    execute_targeted_final_repair_pass_workflow,
)
from tests.test_support.chapter_candidate_finalize_test_support import (
    ChapterCandidateFinalizeDependencies,
    ChapterCandidateFinalizeRequest,
    ChapterCandidateFinalizeState,
    build_chapter_candidate_finalize_dependencies,
    finalize_selected_candidate_result,
    is_targeted_quality_repair_candidate,
    maybe_promote_best_word_budget_repair_candidate,
    resolve_final_candidate_state,
)
from tests.test_support.chapter_candidate_rerank_test_support import (
    attach_candidate_selection_metadata,
    build_candidate_pool_summary,
    build_candidate_retry_prompt_suffix,
    build_candidate_retry_strategy_suffix,
    build_candidate_selection_metadata,
    build_targeted_final_repair_suffix,
    build_word_budget_repair_suffix,
    normalize_candidate_quality_gate_plan,
    resolve_candidate_retry_temperature,
    resolve_targeted_final_repair_char_limit,
    resolve_targeted_final_repair_max_tokens,
    resolve_targeted_final_repair_temperature,
    resolve_word_budget_repair_char_limit,
    resolve_word_budget_repair_max_tokens,
    resolve_word_budget_repair_temperature,
    select_best_generation_candidate,
    select_targeted_final_repair_seed_candidate,
    should_adopt_targeted_final_repair_candidate,
    should_apply_followup_targeted_final_repair,
    should_apply_targeted_final_repair,
    should_apply_word_budget_repair,
    should_generate_additional_candidate,
    should_keep_targeted_final_repair_candidate,
    should_keep_word_budget_repair_candidate,
    should_prefer_targeted_final_repair_candidate,
    should_prefer_word_budget_repair_candidate,
    should_relax_word_budget_repair_limits,
)
from tests.test_support.chapter_candidate_runtime_state_test_support import (
    snapshot_chapter_candidate_runtime_state,
    sync_chapter_candidate_runtime_state,
)
from tests.test_support.chapter_generated_text_test_support import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
    trim_text_to_sentence_boundary,
)


logger = get_logger(__name__)


@dataclass(slots=True)
class ChapterCandidateGenerationRequest:
    ai_service: AIService
    base_generate_kwargs: Dict[str, Any]
    base_prompt: str
    base_temperature: float
    target_word_count: int
    source: str
    generation_label: str
    quality_evaluator: Callable[[str], Dict[str, Any]]
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]]
    max_candidates: int
    runtime_state: Optional[Dict[str, Any]] = None


@dataclass(slots=True)
class ChapterCandidateGenerationDependencies:
    resolve_generation_attempt_labels_fn: Callable[..., Any]
    sync_generation_runtime_state_fn: Callable[..., Any]
    collect_generation_candidate_output_fn: Callable[..., Any]
    build_generation_candidate_record_fn: Callable[..., Any]
    should_generate_additional_candidate_fn: Callable[..., Any]
    build_candidate_retry_prompt_suffix_fn: Callable[..., Any]
    build_candidate_retry_strategy_suffix_fn: Callable[..., Any]
    resolve_candidate_retry_temperature_fn: Callable[..., Any]
    select_best_generation_candidate_fn: Callable[..., Any]


@dataclass(slots=True)
class ChapterCandidateGenerationResult(ChapterCandidateWorkingSet):
    pass


@dataclass(slots=True)
class ChapterCandidateOutputRequest:
    ai_service: AIService
    generate_kwargs: Dict[str, Any]
    candidate_index: int = 1
    max_output_chars: Optional[int] = None
    runtime_state: Optional[Dict[str, Any]] = None


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


def build_chapter_candidate_generation_dependencies(
    *,
    resolve_generation_attempt_labels_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    collect_generation_candidate_output_fn: Callable[..., Any],
    build_generation_candidate_record_fn: Callable[..., Any],
    should_generate_additional_candidate_fn: Callable[..., Any],
    build_candidate_retry_prompt_suffix_fn: Callable[..., Any],
    build_candidate_retry_strategy_suffix_fn: Callable[..., Any],
    resolve_candidate_retry_temperature_fn: Callable[..., Any],
    select_best_generation_candidate_fn: Callable[..., Any],
) -> ChapterCandidateGenerationDependencies:
    return ChapterCandidateGenerationDependencies(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
        should_generate_additional_candidate_fn=should_generate_additional_candidate_fn,
        build_candidate_retry_prompt_suffix_fn=build_candidate_retry_prompt_suffix_fn,
        build_candidate_retry_strategy_suffix_fn=build_candidate_retry_strategy_suffix_fn,
        resolve_candidate_retry_temperature_fn=resolve_candidate_retry_temperature_fn,
        select_best_generation_candidate_fn=select_best_generation_candidate_fn,
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


async def collect_generation_candidate_output(
    *,
    request: ChapterCandidateOutputRequest,
) -> tuple[str, List[str]]:
    full_content = ''
    chunks: List[str] = []
    candidate_index = max(int(request.candidate_index or 1), 1)
    candidate_total = candidate_index
    runtime_state = request.runtime_state
    max_output_chars = request.max_output_chars

    if runtime_state is not None:
        candidate_total = snapshot_chapter_candidate_runtime_state(
            runtime_state,
            default_candidate_total=candidate_index,
        ).candidate_total
        sync_chapter_candidate_runtime_state(
            runtime_state,
            candidate_index=candidate_index,
            candidate_total=candidate_total,
            current_chars=0,
            chunk_count=0,
        )

    async for chunk in request.ai_service.generate_text_stream(**request.generate_kwargs):
        full_content += chunk
        chunks.append(chunk)
        if runtime_state is not None:
            sync_chapter_candidate_runtime_state(
                runtime_state,
                candidate_index=candidate_index,
                candidate_total=candidate_total,
                current_chars=len(full_content),
                chunk_count=len(chunks),
            )
        if max_output_chars and len(full_content) >= max_output_chars:
            break
        await asyncio.sleep(0)

    if max_output_chars and len(full_content) > max_output_chars:
        full_content = trim_text_to_sentence_boundary(
            full_content,
            hard_limit=max_output_chars,
        )
        chunks = [full_content] if full_content else []

    return full_content, chunks


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


async def generate_candidate_pool_workflow(
    *,
    request: ChapterCandidateGenerationRequest,
    dependencies: ChapterCandidateGenerationDependencies,
) -> ChapterCandidateGenerationResult:
    resolved_max_candidates = max(int(request.max_candidates or 1), 1)
    candidates: List[Dict[str, Any]] = []
    retry_suffix = ""
    retry_temperature: Optional[float] = None

    initial_generation_path, initial_attempt_kind = (
        dependencies.resolve_generation_attempt_labels_fn(1)
    )
    dependencies.sync_generation_runtime_state_fn(
        request.runtime_state,
        candidate_index=1,
        candidate_total=resolved_max_candidates,
        current_chars=0,
        chunk_count=0,
        generation_path=initial_generation_path,
        attempt_kind=initial_attempt_kind,
        rerank_used=False,
        word_budget_repair_used=False,
    )

    for candidate_offset in range(resolved_max_candidates):
        candidate_index = candidate_offset + 1
        generation_path, attempt_kind = (
            dependencies.resolve_generation_attempt_labels_fn(candidate_index)
        )
        dependencies.sync_generation_runtime_state_fn(
            request.runtime_state,
            candidate_index=candidate_index,
            candidate_total=resolved_max_candidates,
            current_chars=0,
            chunk_count=0,
            generation_path=generation_path,
            attempt_kind=attempt_kind,
            rerank_used=candidate_index > 1,
            word_budget_repair_used=False,
        )

        current_generate_kwargs = dict(request.base_generate_kwargs)
        if retry_suffix:
            current_generate_kwargs["prompt"] = (
                f"{request.base_prompt}\n\n{retry_suffix}".strip()
            )
        if retry_temperature is not None:
            current_generate_kwargs["temperature"] = retry_temperature

        full_content, candidate_chunks = (
            await dependencies.collect_generation_candidate_output_fn(
                request.ai_service,
                current_generate_kwargs,
                candidate_index=candidate_index,
                runtime_state=request.runtime_state,
            )
        )
        candidate = dependencies.build_generation_candidate_record_fn(
            full_content=full_content,
            candidate_chunks=candidate_chunks,
            target_word_count=request.target_word_count,
            source=request.source,
            generation_label=request.generation_label,
            candidate_index=candidate_index,
            candidate_offset=candidate_offset,
            quality_evaluator=request.quality_evaluator,
            quality_gate_plan_builder=request.quality_gate_plan_builder,
            generation_path=generation_path,
            attempt_kind=attempt_kind,
        )
        candidates.append(candidate)

        if not dependencies.should_generate_additional_candidate_fn(
            candidate,
            produced_candidates=len(candidates),
            max_candidates=resolved_max_candidates,
        ):
            break

        retry_prompt_suffix = dependencies.build_candidate_retry_prompt_suffix_fn(
            candidate.get("quality_gate_plan"),
            attempt_index=candidate_index + 1,
        )
        retry_strategy_suffix = (
            dependencies.build_candidate_retry_strategy_suffix_fn(
                candidate.get("quality_gate_plan"),
                quality_metrics=candidate.get("quality_metrics"),
                attempt_index=candidate_index + 1,
                source=request.source,
            )
        )
        retry_suffix_parts = [
            part.strip()
            for part in (retry_prompt_suffix, retry_strategy_suffix)
            if isinstance(part, str) and part.strip()
        ]
        retry_suffix = "\n\n".join(retry_suffix_parts).strip()
        retry_temperature = dependencies.resolve_candidate_retry_temperature_fn(
            request.base_temperature,
            quality_metrics=candidate.get("quality_metrics"),
            quality_gate_plan=candidate.get("quality_gate_plan"),
            attempt_index=candidate_index + 1,
        )
        if not retry_suffix:
            break

    selected_candidate = dependencies.select_best_generation_candidate_fn(
        candidates
    ) or dict(candidates[-1])
    return ChapterCandidateGenerationResult(
        candidates=candidates,
        selected_candidate=selected_candidate,
    )


@dataclass(slots=True)
class ChapterCandidateExecutorDependencies:
    generation_dependencies: ChapterCandidateGenerationDependencies
    word_budget_repair_dependencies: ChapterCandidateWordBudgetRepairDependencies
    targeted_final_repair_dependencies: ChapterCandidateTargetedFinalRepairDependencies
    finalize_dependencies: ChapterCandidateFinalizeDependencies
    should_apply_targeted_final_repair_fn: Callable[..., Any]
    select_targeted_final_repair_seed_candidate_fn: Callable[..., Any]


def _resolve_candidate_finalize_state(
    *,
    request: ChapterCandidateFinalizeRequest,
    selected_candidate: Dict[str, Any],
    candidates: List[Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    dependencies: ChapterCandidateFinalizeDependencies,
    allow_word_budget_repair_promotion: bool,
) -> ChapterCandidateFinalizeState:
    final_state = resolve_final_candidate_state(
        request=request,
        selected_candidate=selected_candidate,
        candidates=candidates,
        quality_gate_plan_builder=quality_gate_plan_builder,
        dependencies=dependencies,
    )
    if not allow_word_budget_repair_promotion:
        return final_state
    return maybe_promote_best_word_budget_repair_candidate(
        request=request,
        state=final_state,
        quality_gate_plan_builder=quality_gate_plan_builder,
        dependencies=dependencies,
    )


def _select_post_finalize_targeted_repair_seed_candidate(
    *,
    selected_candidate: Dict[str, Any],
    candidates: List[Dict[str, Any]],
    deferred_followup_targeted_repair_seed_candidate: Optional[Dict[str, Any]],
    dependencies: ChapterCandidateExecutorDependencies,
) -> Optional[Dict[str, Any]]:
    targeted_final_repair_dependencies = dependencies.targeted_final_repair_dependencies
    if targeted_final_repair_dependencies.should_apply_followup_targeted_final_repair_fn(selected_candidate):
        return selected_candidate
    if deferred_followup_targeted_repair_seed_candidate is not None:
        return deferred_followup_targeted_repair_seed_candidate
    if is_targeted_quality_repair_candidate(selected_candidate):
        return None
    return dependencies.select_targeted_final_repair_seed_candidate_fn(
        selected_candidate,
        candidates,
    )


def _resolve_followup_targeted_repair_seed_candidate(
    *,
    final_state: ChapterCandidateFinalizeState,
    dependencies: ChapterCandidateExecutorDependencies,
) -> Optional[Dict[str, Any]]:
    if dependencies.targeted_final_repair_dependencies.should_apply_followup_targeted_final_repair_fn(
        final_state.selected_candidate
    ):
        return final_state.selected_candidate
    return None


def build_chapter_candidate_executor_dependencies(
    *,
    generation_dependencies: ChapterCandidateGenerationDependencies,
    word_budget_repair_dependencies: ChapterCandidateWordBudgetRepairDependencies,
    targeted_final_repair_dependencies: ChapterCandidateTargetedFinalRepairDependencies,
    finalize_dependencies: ChapterCandidateFinalizeDependencies,
    should_apply_targeted_final_repair_fn: Callable[..., Any],
    select_targeted_final_repair_seed_candidate_fn: Callable[..., Any],
) -> ChapterCandidateExecutorDependencies:
    return ChapterCandidateExecutorDependencies(
        generation_dependencies=generation_dependencies,
        word_budget_repair_dependencies=word_budget_repair_dependencies,
        targeted_final_repair_dependencies=targeted_final_repair_dependencies,
        finalize_dependencies=finalize_dependencies,
        should_apply_targeted_final_repair_fn=should_apply_targeted_final_repair_fn,
        select_targeted_final_repair_seed_candidate_fn=select_targeted_final_repair_seed_candidate_fn,
    )


@lru_cache(maxsize=8)
def get_chapter_candidate_executor_dependencies(
    *,
    resolve_generation_attempt_labels_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    collect_generation_candidate_output_fn: Callable[..., Any],
    build_generation_candidate_record_fn: Callable[..., Any],
):
    return build_default_chapter_candidate_executor_dependencies(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
    )


def build_default_chapter_candidate_executor_dependencies(
    *,
    resolve_generation_attempt_labels_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    collect_generation_candidate_output_fn: Callable[..., Any],
    build_generation_candidate_record_fn: Callable[..., Any],
):
    generation_dependencies = build_chapter_candidate_generation_dependencies(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
        should_generate_additional_candidate_fn=should_generate_additional_candidate,
        build_candidate_retry_prompt_suffix_fn=build_candidate_retry_prompt_suffix,
        build_candidate_retry_strategy_suffix_fn=build_candidate_retry_strategy_suffix,
        resolve_candidate_retry_temperature_fn=resolve_candidate_retry_temperature,
        select_best_generation_candidate_fn=select_best_generation_candidate,
    )
    word_budget_repair_dependencies = build_chapter_candidate_word_budget_repair_dependencies(
        should_apply_word_budget_repair_fn=should_apply_word_budget_repair,
        build_word_budget_repair_suffix_fn=build_word_budget_repair_suffix,
        should_relax_word_budget_repair_limits_fn=should_relax_word_budget_repair_limits,
        resolve_word_budget_repair_temperature_fn=resolve_word_budget_repair_temperature,
        resolve_word_budget_repair_max_tokens_fn=resolve_word_budget_repair_max_tokens,
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        resolve_word_budget_repair_char_limit_fn=resolve_word_budget_repair_char_limit,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
        should_keep_word_budget_repair_candidate_fn=should_keep_word_budget_repair_candidate,
        select_best_generation_candidate_fn=select_best_generation_candidate,
        should_prefer_word_budget_repair_candidate_fn=should_prefer_word_budget_repair_candidate,
    )
    targeted_final_repair_dependencies = build_chapter_candidate_targeted_final_repair_dependencies(
        build_targeted_final_repair_suffix_fn=build_targeted_final_repair_suffix,
        resolve_targeted_final_repair_temperature_fn=resolve_targeted_final_repair_temperature,
        resolve_targeted_final_repair_max_tokens_fn=resolve_targeted_final_repair_max_tokens,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        resolve_targeted_final_repair_char_limit_fn=resolve_targeted_final_repair_char_limit,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
        should_keep_targeted_final_repair_candidate_fn=should_keep_targeted_final_repair_candidate,
        should_adopt_targeted_final_repair_candidate_fn=should_adopt_targeted_final_repair_candidate,
        should_prefer_targeted_final_repair_candidate_fn=should_prefer_targeted_final_repair_candidate,
        should_apply_followup_targeted_final_repair_fn=should_apply_followup_targeted_final_repair,
    )
    finalize_dependencies = build_chapter_candidate_finalize_dependencies(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        build_candidate_selection_metadata_fn=build_candidate_selection_metadata,
        attach_candidate_selection_metadata_fn=attach_candidate_selection_metadata,
        normalize_candidate_quality_gate_plan_fn=normalize_candidate_quality_gate_plan,
        build_candidate_pool_summary_fn=build_candidate_pool_summary,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        select_best_generation_candidate_fn=select_best_generation_candidate,
        should_prefer_word_budget_repair_candidate_fn=should_prefer_word_budget_repair_candidate,
    )
    return build_chapter_candidate_executor_dependencies(
        generation_dependencies=generation_dependencies,
        word_budget_repair_dependencies=word_budget_repair_dependencies,
        targeted_final_repair_dependencies=targeted_final_repair_dependencies,
        finalize_dependencies=finalize_dependencies,
        should_apply_targeted_final_repair_fn=should_apply_targeted_final_repair,
        select_targeted_final_repair_seed_candidate_fn=select_targeted_final_repair_seed_candidate,
    )


async def collect_default_generation_candidate_output(
    ai_service: AIService,
    generate_kwargs: Dict[str, Any],
    *,
    candidate_index: int = 1,
    max_output_chars: Optional[int] = None,
    runtime_state: Optional[Dict[str, Any]] = None,
) -> tuple[str, list[str]]:
    return await collect_generation_candidate_output(
        request=ChapterCandidateOutputRequest(
            ai_service=ai_service,
            generate_kwargs=generate_kwargs,
            candidate_index=candidate_index,
            max_output_chars=max_output_chars,
            runtime_state=runtime_state,
        ),
    )


def resolve_default_generation_attempt_labels(
    candidate_index: int,
    *,
    is_word_budget_repair: bool = False,
) -> tuple[str, str]:
    return resolve_generation_attempt_labels(
        candidate_index,
        is_word_budget_repair=is_word_budget_repair,
    )


def sync_default_generation_runtime_state(
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
    from tests.test_support.chapter_candidate_runtime_state_test_support import (
        sync_chapter_candidate_runtime_state,
    )

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


def build_default_generation_candidate_record(
    *,
    full_content: str,
    candidate_chunks: list[str],
    target_word_count: int,
    source: str,
    generation_label: str,
    candidate_index: int,
    candidate_offset: int,
    quality_evaluator,
    quality_gate_plan_builder,
    generation_path: str,
    attempt_kind: str,
    log_warning_fn,
):
    return build_generation_candidate_record(
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


def build_default_generation_candidate_record_with_default_logging(**kwargs):
    return build_default_generation_candidate_record(
        **kwargs,
        log_warning_fn=logger.warning,
    )


async def generate_best_ranked_candidate_workflow(
    *,
    ai_service: AIService,
    base_generate_kwargs: Dict[str, Any],
    target_word_count: int,
    source: str,
    generation_label: str,
    quality_evaluator: Callable[[str], Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    max_candidates: int,
    runtime_state: Optional[Dict[str, Any]] = None,
    dependencies: ChapterCandidateExecutorDependencies,
) -> Dict[str, Any]:
    base_prompt = str(base_generate_kwargs.get("prompt") or "")
    try:
        base_temperature = float(base_generate_kwargs.get("temperature") or 0.8)
    except (TypeError, ValueError):
        base_temperature = 0.8

    candidate_generation_result = await generate_candidate_pool_workflow(
        request=ChapterCandidateGenerationRequest(
            ai_service=ai_service,
            base_generate_kwargs=base_generate_kwargs,
            base_prompt=base_prompt,
            base_temperature=base_temperature,
            target_word_count=target_word_count,
            source=source,
            generation_label=generation_label,
            quality_evaluator=quality_evaluator,
            quality_gate_plan_builder=quality_gate_plan_builder,
            max_candidates=max_candidates,
            runtime_state=runtime_state,
        ),
        dependencies=dependencies.generation_dependencies,
    )
    candidates = candidate_generation_result.candidates
    selected_candidate = candidate_generation_result.selected_candidate
    word_budget_repair_used = False

    word_budget_repair_result = await maybe_apply_word_budget_repair_workflow(
        request=ChapterCandidateWordBudgetRepairRequest(
            ai_service=ai_service,
            base_generate_kwargs=base_generate_kwargs,
            base_prompt=base_prompt,
            base_temperature=base_temperature,
            target_word_count=target_word_count,
            source=source,
            generation_label=generation_label,
            quality_evaluator=quality_evaluator,
            quality_gate_plan_builder=quality_gate_plan_builder,
            runtime_state=runtime_state,
        ),
        selected_candidate=selected_candidate,
        candidates=candidates,
        dependencies=dependencies.word_budget_repair_dependencies,
    )
    selected_candidate = word_budget_repair_result.selected_candidate
    candidates = word_budget_repair_result.candidates
    word_budget_repair_used = word_budget_repair_result.word_budget_repair_used

    deferred_followup_targeted_repair_seed_candidate = None
    if dependencies.should_apply_targeted_final_repair_fn(selected_candidate):
        targeted_final_repair_result = await execute_targeted_final_repair_pass_workflow(
            request=ChapterCandidateTargetedFinalRepairRequest(
                ai_service=ai_service,
                base_generate_kwargs=base_generate_kwargs,
                base_prompt=base_prompt,
                base_temperature=base_temperature,
                target_word_count=target_word_count,
                source=source,
                generation_label=generation_label,
                generation_label_suffix="targeted-repair",
                quality_evaluator=quality_evaluator,
                quality_gate_plan_builder=quality_gate_plan_builder,
                repair_seed_candidate=selected_candidate,
                current_winner_candidate=selected_candidate,
                runtime_state=runtime_state,
                allow_followup_seed_defer=True,
            ),
            selected_candidate=selected_candidate,
            candidates=candidates,
            dependencies=dependencies.targeted_final_repair_dependencies,
        )
        selected_candidate = targeted_final_repair_result.selected_candidate
        candidates = targeted_final_repair_result.candidates
        deferred_followup_targeted_repair_seed_candidate = (
            targeted_final_repair_result.deferred_followup_targeted_repair_seed_candidate
        )
    finalize_request = ChapterCandidateFinalizeRequest(
        target_word_count=target_word_count,
        source=source,
        runtime_state=runtime_state,
    )
    finalize_dependencies = dependencies.finalize_dependencies
    final_state = _resolve_candidate_finalize_state(
        request=finalize_request,
        selected_candidate=selected_candidate,
        candidates=candidates,
        quality_gate_plan_builder=quality_gate_plan_builder,
        dependencies=finalize_dependencies,
        allow_word_budget_repair_promotion=True,
    )
    selected_candidate = final_state.selected_candidate
    candidates = final_state.candidates

    targeted_final_repair_seed_candidate = _select_post_finalize_targeted_repair_seed_candidate(
        selected_candidate=selected_candidate,
        candidates=candidates,
        deferred_followup_targeted_repair_seed_candidate=deferred_followup_targeted_repair_seed_candidate,
        dependencies=dependencies,
    )
    if targeted_final_repair_seed_candidate:
        targeted_final_repair_result = await execute_targeted_final_repair_pass_workflow(
            request=ChapterCandidateTargetedFinalRepairRequest(
                ai_service=ai_service,
                base_generate_kwargs=base_generate_kwargs,
                base_prompt=base_prompt,
                base_temperature=base_temperature,
                target_word_count=target_word_count,
                source=source,
                generation_label=generation_label,
                generation_label_suffix="targeted-repair-post-finalize",
                quality_evaluator=quality_evaluator,
                quality_gate_plan_builder=quality_gate_plan_builder,
                repair_seed_candidate=targeted_final_repair_seed_candidate,
                current_winner_candidate=selected_candidate,
                runtime_state=runtime_state,
            ),
            selected_candidate=selected_candidate,
            candidates=candidates,
            dependencies=dependencies.targeted_final_repair_dependencies,
        )
        selected_candidate = targeted_final_repair_result.selected_candidate
        candidates = targeted_final_repair_result.candidates

        final_state = _resolve_candidate_finalize_state(
            request=finalize_request,
            selected_candidate=selected_candidate,
            candidates=candidates,
            quality_gate_plan_builder=quality_gate_plan_builder,
            dependencies=finalize_dependencies,
            allow_word_budget_repair_promotion=False,
        )
        followup_targeted_repair_seed_candidate = _resolve_followup_targeted_repair_seed_candidate(
            final_state=final_state,
            dependencies=dependencies,
        )
        if followup_targeted_repair_seed_candidate:
            targeted_final_repair_result = await execute_targeted_final_repair_pass_workflow(
                request=ChapterCandidateTargetedFinalRepairRequest(
                    ai_service=ai_service,
                    base_generate_kwargs=base_generate_kwargs,
                    base_prompt=base_prompt,
                    base_temperature=base_temperature,
                    target_word_count=target_word_count,
                    source=source,
                    generation_label=generation_label,
                    generation_label_suffix="targeted-repair-followup",
                    quality_evaluator=quality_evaluator,
                    quality_gate_plan_builder=quality_gate_plan_builder,
                    repair_seed_candidate=followup_targeted_repair_seed_candidate,
                    current_winner_candidate=final_state.selected_candidate,
                    runtime_state=runtime_state,
                ),
                selected_candidate=final_state.selected_candidate,
                candidates=candidates,
                dependencies=dependencies.targeted_final_repair_dependencies,
            )
            selected_candidate = targeted_final_repair_result.selected_candidate
            candidates = targeted_final_repair_result.candidates

    final_state = _resolve_candidate_finalize_state(
        request=finalize_request,
        selected_candidate=selected_candidate,
        candidates=candidates,
        quality_gate_plan_builder=quality_gate_plan_builder,
        dependencies=finalize_dependencies,
        allow_word_budget_repair_promotion=False,
    )
    return finalize_selected_candidate_result(
        request=finalize_request,
        state=final_state,
        dependencies=finalize_dependencies,
    )


async def generate_best_ranked_candidate(
    *,
    ai_service: AIService,
    base_generate_kwargs: Dict[str, Any],
    target_word_count: int,
    source: str,
    generation_label: str,
    quality_evaluator: Callable[[str], Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    max_candidates: int,
    runtime_state: Optional[Dict[str, Any]] = None,
    resolve_generation_attempt_labels_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    collect_generation_candidate_output_fn: Callable[..., Any],
    build_generation_candidate_record_fn: Callable[..., Any],
) -> Dict[str, Any]:
    dependencies = get_chapter_candidate_executor_dependencies(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
    )
    return await generate_best_ranked_candidate_workflow(
        ai_service=ai_service,
        base_generate_kwargs=base_generate_kwargs,
        target_word_count=target_word_count,
        source=source,
        generation_label=generation_label,
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
        max_candidates=max_candidates,
        runtime_state=runtime_state,
        dependencies=dependencies,
    )


async def generate_best_ranked_candidate_with_default_wiring(
    *,
    ai_service: AIService,
    base_generate_kwargs: Dict[str, Any],
    target_word_count: int,
    source: str,
    generation_label: str,
    quality_evaluator: Callable[[str], Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    max_candidates: int = 2,
    runtime_state: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    return await generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs=base_generate_kwargs,
        target_word_count=target_word_count,
        source=source,
        generation_label=generation_label,
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
        max_candidates=max_candidates,
        runtime_state=runtime_state,
        resolve_generation_attempt_labels_fn=resolve_default_generation_attempt_labels,
        sync_generation_runtime_state_fn=sync_default_generation_runtime_state,
        collect_generation_candidate_output_fn=collect_default_generation_candidate_output,
        build_generation_candidate_record_fn=build_default_generation_candidate_record_with_default_logging,
    )



