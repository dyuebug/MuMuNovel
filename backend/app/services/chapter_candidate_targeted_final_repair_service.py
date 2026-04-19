"""章节候选定向最终修复服务。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional

from app.logger import get_logger
from app.services.ai_service import AIService
from app.services.chapter_candidate_models import ChapterCandidateWorkingSet
from app.services.chapter_candidate_selection_metadata_service import attach_repair_seed_candidate_metadata

logger = get_logger(__name__)


@dataclass(slots=True)
class ChapterCandidateTargetedFinalRepairRequest:
    ai_service: AIService
    base_generate_kwargs: Dict[str, Any]
    base_prompt: str
    base_temperature: float
    target_word_count: int
    source: str
    generation_label: str
    generation_label_suffix: str
    quality_evaluator: Callable[[str], Dict[str, Any]]
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]]
    repair_seed_candidate: Dict[str, Any]
    current_winner_candidate: Dict[str, Any]
    runtime_state: Optional[Dict[str, Any]] = None
    allow_followup_seed_defer: bool = False


@dataclass(slots=True)
class ChapterCandidateTargetedFinalRepairDependencies:
    build_targeted_final_repair_suffix_fn: Callable[..., Any]
    resolve_targeted_final_repair_temperature_fn: Callable[..., Any]
    resolve_targeted_final_repair_max_tokens_fn: Callable[..., Any]
    sync_generation_runtime_state_fn: Callable[..., Any]
    collect_generation_candidate_output_fn: Callable[..., Any]
    resolve_targeted_final_repair_char_limit_fn: Callable[..., Any]
    build_generation_candidate_record_fn: Callable[..., Any]
    should_keep_targeted_final_repair_candidate_fn: Callable[..., Any]
    should_adopt_targeted_final_repair_candidate_fn: Callable[..., Any]
    should_prefer_targeted_final_repair_candidate_fn: Callable[..., Any]
    should_apply_followup_targeted_final_repair_fn: Callable[..., Any]


@dataclass(slots=True)
class ChapterCandidateTargetedFinalRepairResult(ChapterCandidateWorkingSet):
    deferred_followup_targeted_repair_seed_candidate: Optional[Dict[str, Any]] = None


def build_chapter_candidate_targeted_final_repair_dependencies(
    *,
    build_targeted_final_repair_suffix_fn: Callable[..., Any],
    resolve_targeted_final_repair_temperature_fn: Callable[..., Any],
    resolve_targeted_final_repair_max_tokens_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    collect_generation_candidate_output_fn: Callable[..., Any],
    resolve_targeted_final_repair_char_limit_fn: Callable[..., Any],
    build_generation_candidate_record_fn: Callable[..., Any],
    should_keep_targeted_final_repair_candidate_fn: Callable[..., Any],
    should_adopt_targeted_final_repair_candidate_fn: Callable[..., Any],
    should_prefer_targeted_final_repair_candidate_fn: Callable[..., Any],
    should_apply_followup_targeted_final_repair_fn: Callable[..., Any],
) -> ChapterCandidateTargetedFinalRepairDependencies:
    return ChapterCandidateTargetedFinalRepairDependencies(
        build_targeted_final_repair_suffix_fn=build_targeted_final_repair_suffix_fn,
        resolve_targeted_final_repair_temperature_fn=resolve_targeted_final_repair_temperature_fn,
        resolve_targeted_final_repair_max_tokens_fn=resolve_targeted_final_repair_max_tokens_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        resolve_targeted_final_repair_char_limit_fn=resolve_targeted_final_repair_char_limit_fn,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
        should_keep_targeted_final_repair_candidate_fn=should_keep_targeted_final_repair_candidate_fn,
        should_adopt_targeted_final_repair_candidate_fn=should_adopt_targeted_final_repair_candidate_fn,
        should_prefer_targeted_final_repair_candidate_fn=should_prefer_targeted_final_repair_candidate_fn,
        should_apply_followup_targeted_final_repair_fn=should_apply_followup_targeted_final_repair_fn,
    )


async def execute_targeted_final_repair_pass_workflow(
    *,
    request: ChapterCandidateTargetedFinalRepairRequest,
    selected_candidate: Dict[str, Any],
    candidates: List[Dict[str, Any]],
    dependencies: ChapterCandidateTargetedFinalRepairDependencies,
) -> ChapterCandidateTargetedFinalRepairResult:
    deferred_followup_targeted_repair_seed_candidate: Optional[Dict[str, Any]] = None

    try:
        final_repair_attempt_index = len(candidates) + 1
        final_repair_suffix = dependencies.build_targeted_final_repair_suffix_fn(
            quality_metrics=request.repair_seed_candidate.get("quality_metrics"),
            quality_gate_plan=request.repair_seed_candidate.get("quality_gate_plan"),
            target_word_count=request.target_word_count,
            attempt_index=final_repair_attempt_index,
            source=request.source,
        )
        if final_repair_suffix:
            final_repair_prompt_sections = [
                request.base_prompt,
                final_repair_suffix,
                "Previous draft to rewrite:\n<<<CHAPTER_DRAFT",
                str(request.repair_seed_candidate.get("full_content") or ""),
                "CHAPTER_DRAFT>>>",
            ]
            final_repair_generate_kwargs = dict(request.base_generate_kwargs)
            final_repair_generate_kwargs["prompt"] = "\n\n".join(
                section.strip()
                for section in final_repair_prompt_sections
                if isinstance(section, str) and section.strip()
            )
            final_repair_generate_kwargs["temperature"] = dependencies.resolve_targeted_final_repair_temperature_fn(
                request.base_temperature,
                quality_gate_plan=request.repair_seed_candidate.get("quality_gate_plan"),
            )
            final_repair_generate_kwargs["max_tokens"] = dependencies.resolve_targeted_final_repair_max_tokens_fn(
                request.target_word_count,
                current_word_count=int(request.repair_seed_candidate.get("word_count") or 0),
            )
            final_repair_generation_path = "targeted_quality_repair"
            final_repair_attempt_kind = "targeted_quality_repair"
            dependencies.sync_generation_runtime_state_fn(
                request.runtime_state,
                candidate_index=final_repair_attempt_index,
                candidate_total=final_repair_attempt_index,
                current_chars=0,
                chunk_count=0,
                generation_path=final_repair_generation_path,
                attempt_kind=final_repair_attempt_kind,
                rerank_used=False,
                word_budget_repair_used=False,
            )
            final_repaired_content, final_repaired_chunks = await dependencies.collect_generation_candidate_output_fn(
                request.ai_service,
                final_repair_generate_kwargs,
                candidate_index=final_repair_attempt_index,
                max_output_chars=dependencies.resolve_targeted_final_repair_char_limit_fn(request.target_word_count),
                runtime_state=request.runtime_state,
            )
            final_repair_candidate = dependencies.build_generation_candidate_record_fn(
                full_content=final_repaired_content,
                candidate_chunks=final_repaired_chunks,
                target_word_count=request.target_word_count,
                source=request.source,
                generation_label=f"{request.generation_label}-{request.generation_label_suffix}",
                candidate_index=final_repair_attempt_index,
                candidate_offset=final_repair_attempt_index - 1,
                quality_evaluator=request.quality_evaluator,
                quality_gate_plan_builder=request.quality_gate_plan_builder,
                generation_path=final_repair_generation_path,
                attempt_kind=final_repair_attempt_kind,
            )
            final_repair_candidate = attach_repair_seed_candidate_metadata(
                repair_candidate=final_repair_candidate,
                repair_seed_candidate=request.repair_seed_candidate,
            )
            if dependencies.should_keep_targeted_final_repair_candidate_fn(
                request.repair_seed_candidate,
                final_repair_candidate,
            ):
                candidates.append(final_repair_candidate)
                if (
                    dependencies.should_adopt_targeted_final_repair_candidate_fn(
                        request.repair_seed_candidate,
                        final_repair_candidate,
                    )
                    and dependencies.should_prefer_targeted_final_repair_candidate_fn(
                        request.current_winner_candidate,
                        final_repair_candidate,
                    )
                ):
                    selected_candidate = final_repair_candidate
                elif (
                    request.allow_followup_seed_defer
                    and dependencies.should_apply_followup_targeted_final_repair_fn(final_repair_candidate)
                ):
                    deferred_followup_targeted_repair_seed_candidate = final_repair_candidate
    except Exception as exc:
        logger.warning(
            f"Targeted quality repair pass failed for {request.generation_label}: {type(exc).__name__}: {exc}"
        )

    return ChapterCandidateTargetedFinalRepairResult(
        selected_candidate=selected_candidate,
        candidates=candidates,
        deferred_followup_targeted_repair_seed_candidate=deferred_followup_targeted_repair_seed_candidate,
    )
