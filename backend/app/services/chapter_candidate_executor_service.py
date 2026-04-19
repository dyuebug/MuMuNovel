"""章节候选 rerank / repair 执行 service。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional

from app.logger import get_logger
from app.services.ai_service import AIService
from app.services.chapter_candidate_classification_service import is_targeted_quality_repair_candidate
from app.services.chapter_candidate_generation_service import (
    ChapterCandidateGenerationDependencies,
    ChapterCandidateGenerationRequest,
    generate_candidate_pool_workflow,
)
from app.services.chapter_candidate_word_budget_repair_service import (
    ChapterCandidateWordBudgetRepairDependencies,
    ChapterCandidateWordBudgetRepairRequest,
    maybe_apply_word_budget_repair_workflow,
)
from app.services.chapter_candidate_targeted_final_repair_service import (
    ChapterCandidateTargetedFinalRepairDependencies,
    ChapterCandidateTargetedFinalRepairRequest,
    execute_targeted_final_repair_pass_workflow,
)
from app.services.chapter_candidate_finalize_service import (
    ChapterCandidateFinalizeDependencies,
    ChapterCandidateFinalizeRequest,
    ChapterCandidateFinalizeState,
    finalize_selected_candidate_result,
    maybe_promote_best_word_budget_repair_candidate,
    resolve_final_candidate_state,
)


logger = get_logger(__name__)


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
