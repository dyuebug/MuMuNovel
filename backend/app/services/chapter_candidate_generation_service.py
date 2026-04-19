"""章节候选生成服务。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional

from app.services.ai_service import AIService
from app.services.chapter_candidate_models import ChapterCandidateWorkingSet


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


async def generate_candidate_pool_workflow(
    *,
    request: ChapterCandidateGenerationRequest,
    dependencies: ChapterCandidateGenerationDependencies,
) -> ChapterCandidateGenerationResult:
    resolved_max_candidates = max(int(request.max_candidates or 1), 1)
    candidates: List[Dict[str, Any]] = []
    retry_suffix = ""
    retry_temperature: Optional[float] = None

    initial_generation_path, initial_attempt_kind = dependencies.resolve_generation_attempt_labels_fn(1)
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
        generation_path, attempt_kind = dependencies.resolve_generation_attempt_labels_fn(candidate_index)
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
            current_generate_kwargs["prompt"] = f"{request.base_prompt}\n\n{retry_suffix}".strip()
        if retry_temperature is not None:
            current_generate_kwargs["temperature"] = retry_temperature

        full_content, candidate_chunks = await dependencies.collect_generation_candidate_output_fn(
            request.ai_service,
            current_generate_kwargs,
            candidate_index=candidate_index,
            runtime_state=request.runtime_state,
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
        retry_strategy_suffix = dependencies.build_candidate_retry_strategy_suffix_fn(
            candidate.get("quality_gate_plan"),
            quality_metrics=candidate.get("quality_metrics"),
            attempt_index=candidate_index + 1,
            source=request.source,
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

    selected_candidate = dependencies.select_best_generation_candidate_fn(candidates) or dict(candidates[-1])
    return ChapterCandidateGenerationResult(
        candidates=candidates,
        selected_candidate=selected_candidate,
    )
