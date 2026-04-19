"""章节候选字数预算修复服务。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional

from app.logger import get_logger
from app.services.ai_service import AIService
from app.services.chapter_candidate_models import ChapterCandidateWorkingSet
from app.services.chapter_candidate_selection_metadata_service import attach_repair_seed_candidate_metadata

logger = get_logger(__name__)


@dataclass(slots=True)
class ChapterCandidateWordBudgetRepairRequest:
    ai_service: AIService
    base_generate_kwargs: Dict[str, Any]
    base_prompt: str
    base_temperature: float
    target_word_count: int
    source: str
    generation_label: str
    quality_evaluator: Callable[[str], Dict[str, Any]]
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]]
    runtime_state: Optional[Dict[str, Any]] = None


@dataclass(slots=True)
class ChapterCandidateWordBudgetRepairDependencies:
    should_apply_word_budget_repair_fn: Callable[..., Any]
    build_word_budget_repair_suffix_fn: Callable[..., Any]
    should_relax_word_budget_repair_limits_fn: Callable[..., Any]
    resolve_word_budget_repair_temperature_fn: Callable[..., Any]
    resolve_word_budget_repair_max_tokens_fn: Callable[..., Any]
    resolve_generation_attempt_labels_fn: Callable[..., Any]
    sync_generation_runtime_state_fn: Callable[..., Any]
    collect_generation_candidate_output_fn: Callable[..., Any]
    resolve_word_budget_repair_char_limit_fn: Callable[..., Any]
    build_generation_candidate_record_fn: Callable[..., Any]
    should_keep_word_budget_repair_candidate_fn: Callable[..., Any]
    select_best_generation_candidate_fn: Callable[..., Any]
    should_prefer_word_budget_repair_candidate_fn: Callable[..., Any]


@dataclass(slots=True)
class ChapterCandidateWordBudgetRepairResult(ChapterCandidateWorkingSet):
    word_budget_repair_used: bool


def build_chapter_candidate_word_budget_repair_dependencies(
    *,
    should_apply_word_budget_repair_fn: Callable[..., Any],
    build_word_budget_repair_suffix_fn: Callable[..., Any],
    should_relax_word_budget_repair_limits_fn: Callable[..., Any],
    resolve_word_budget_repair_temperature_fn: Callable[..., Any],
    resolve_word_budget_repair_max_tokens_fn: Callable[..., Any],
    resolve_generation_attempt_labels_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    collect_generation_candidate_output_fn: Callable[..., Any],
    resolve_word_budget_repair_char_limit_fn: Callable[..., Any],
    build_generation_candidate_record_fn: Callable[..., Any],
    should_keep_word_budget_repair_candidate_fn: Callable[..., Any],
    select_best_generation_candidate_fn: Callable[..., Any],
    should_prefer_word_budget_repair_candidate_fn: Callable[..., Any],
) -> ChapterCandidateWordBudgetRepairDependencies:
    return ChapterCandidateWordBudgetRepairDependencies(
        should_apply_word_budget_repair_fn=should_apply_word_budget_repair_fn,
        build_word_budget_repair_suffix_fn=build_word_budget_repair_suffix_fn,
        should_relax_word_budget_repair_limits_fn=should_relax_word_budget_repair_limits_fn,
        resolve_word_budget_repair_temperature_fn=resolve_word_budget_repair_temperature_fn,
        resolve_word_budget_repair_max_tokens_fn=resolve_word_budget_repair_max_tokens_fn,
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        resolve_word_budget_repair_char_limit_fn=resolve_word_budget_repair_char_limit_fn,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
        should_keep_word_budget_repair_candidate_fn=should_keep_word_budget_repair_candidate_fn,
        select_best_generation_candidate_fn=select_best_generation_candidate_fn,
        should_prefer_word_budget_repair_candidate_fn=should_prefer_word_budget_repair_candidate_fn,
    )



async def maybe_apply_word_budget_repair_workflow(
    *,
    request: ChapterCandidateWordBudgetRepairRequest,
    selected_candidate: Dict[str, Any],
    candidates: List[Dict[str, Any]],
    dependencies: ChapterCandidateWordBudgetRepairDependencies,
) -> ChapterCandidateWordBudgetRepairResult:
    if not dependencies.should_apply_word_budget_repair_fn(selected_candidate):
        return ChapterCandidateWordBudgetRepairResult(
            selected_candidate=selected_candidate,
            candidates=candidates,
            word_budget_repair_used=False,
        )

    word_budget_repair_used = False
    try:
        repair_attempt_index = len(candidates) + 1
        repair_suffix = dependencies.build_word_budget_repair_suffix_fn(
            quality_metrics=selected_candidate.get("quality_metrics"),
            quality_gate_plan=selected_candidate.get("quality_gate_plan"),
            current_content=selected_candidate.get("full_content"),
            target_word_count=request.target_word_count,
            attempt_index=repair_attempt_index,
            source=request.source,
        )
        if repair_suffix:
            repair_source_word_count = int(selected_candidate.get("word_count") or 0)
            relax_content_budget = dependencies.should_relax_word_budget_repair_limits_fn(
                selected_candidate.get("quality_gate_plan")
            )
            repair_prompt_sections = [
                request.base_prompt,
                repair_suffix,
                "Previous draft to rewrite:\n<<<CHAPTER_DRAFT",
                str(selected_candidate.get("full_content") or ""),
                "CHAPTER_DRAFT>>>",
            ]
            repair_generate_kwargs = dict(request.base_generate_kwargs)
            repair_generate_kwargs["prompt"] = "\n\n".join(
                section.strip()
                for section in repair_prompt_sections
                if isinstance(section, str) and section.strip()
            )
            repair_generate_kwargs["temperature"] = dependencies.resolve_word_budget_repair_temperature_fn(
                request.base_temperature,
                quality_metrics=selected_candidate.get("quality_metrics"),
            )
            repair_generate_kwargs["max_tokens"] = dependencies.resolve_word_budget_repair_max_tokens_fn(
                request.target_word_count,
                current_word_count=repair_source_word_count,
                relax_content_budget=relax_content_budget,
            )
            repair_generation_path, repair_attempt_kind = dependencies.resolve_generation_attempt_labels_fn(
                repair_attempt_index,
                is_word_budget_repair=True,
            )
            dependencies.sync_generation_runtime_state_fn(
                request.runtime_state,
                candidate_index=repair_attempt_index,
                candidate_total=repair_attempt_index,
                current_chars=0,
                chunk_count=0,
                generation_path=repair_generation_path,
                attempt_kind=repair_attempt_kind,
                rerank_used=False,
                word_budget_repair_used=True,
            )
            repaired_content, repaired_chunks = await dependencies.collect_generation_candidate_output_fn(
                request.ai_service,
                repair_generate_kwargs,
                candidate_index=repair_attempt_index,
                max_output_chars=dependencies.resolve_word_budget_repair_char_limit_fn(
                    request.target_word_count,
                    relax_content_budget=relax_content_budget,
                ),
                runtime_state=request.runtime_state,
            )
            repair_candidate = dependencies.build_generation_candidate_record_fn(
                full_content=repaired_content,
                candidate_chunks=repaired_chunks,
                target_word_count=request.target_word_count,
                source=request.source,
                generation_label=f"{request.generation_label}-budget-repair",
                candidate_index=repair_attempt_index,
                candidate_offset=repair_attempt_index - 1,
                quality_evaluator=request.quality_evaluator,
                quality_gate_plan_builder=request.quality_gate_plan_builder,
                generation_path=repair_generation_path,
                attempt_kind=repair_attempt_kind,
            )
            repair_candidate = attach_repair_seed_candidate_metadata(
                repair_candidate=repair_candidate,
                repair_seed_candidate=selected_candidate,
            )
            if dependencies.should_keep_word_budget_repair_candidate_fn(selected_candidate, repair_candidate):
                candidates.append(repair_candidate)
                word_budget_repair_used = True
                reranked_candidate = dependencies.select_best_generation_candidate_fn(candidates) or repair_candidate
                if dependencies.should_prefer_word_budget_repair_candidate_fn(reranked_candidate, repair_candidate):
                    selected_candidate = repair_candidate
                else:
                    selected_candidate = reranked_candidate
    except Exception as exc:
        logger.warning(
            f"Word-budget repair pass failed for {request.generation_label}: {type(exc).__name__}: {exc}"
        )

    return ChapterCandidateWordBudgetRepairResult(
        selected_candidate=selected_candidate,
        candidates=candidates,
        word_budget_repair_used=word_budget_repair_used,
    )
