"""Compatibility helpers for chapter candidate executor entry orchestration."""
from __future__ import annotations

from functools import lru_cache
from typing import Any, Callable, Dict, Optional

from app.services.ai_service import AIService
from app.services.chapter_candidate_executor_compat_service import (
    build_default_chapter_candidate_executor_dependencies as _build_default_chapter_candidate_executor_dependencies_compat_service,
)
from app.services.chapter_candidate_executor_service import (
    generate_best_ranked_candidate_workflow,
)


@lru_cache(maxsize=8)
def get_chapter_candidate_executor_dependencies(
    *,
    resolve_generation_attempt_labels_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    collect_generation_candidate_output_fn: Callable[..., Any],
    build_generation_candidate_record_fn: Callable[..., Any],
):
    return _build_default_chapter_candidate_executor_dependencies_compat_service(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
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
    runtime_state: Optional[Dict[str, Any]],
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
