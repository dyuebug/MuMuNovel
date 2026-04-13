from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any, Awaitable, Callable, Dict, List, Optional

from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.generation_history import GenerationHistory
from app.models.project import Project
from app.services.chapter_candidate_event_service import build_chapter_generation_progress_kwargs
from app.services.chapter_candidate_result_service import normalize_selected_candidate_result
from app.services.chapter_candidate_runtime_state_service import (
    build_chapter_candidate_runtime_state,
    snapshot_chapter_candidate_runtime_state,
)
from app.services.chapter_generation_stream_models import (
    ChapterGenerationStreamBuiltContext,
    ChapterGenerationStreamCandidateStageResult,
    ChapterGenerationStreamCandidateDependencies,
    ChapterGenerationStreamExecutionSetup,
    ChapterGenerationStreamRuntimeContext,
)

logger = get_logger(__name__)


@dataclass(frozen=True)
class ChapterGenerationCandidateExecution:
    runtime_state: Dict[str, Any]
    selected_candidate_task: asyncio.Task


@dataclass(frozen=True)
class ChapterGenerationCandidateQualityHooks:
    quality_evaluator: Callable[[str], Dict[str, Any]]
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]]


@dataclass(frozen=True)
class ChapterGenerationSelectedCandidateOutcome:
    full_content: str
    candidate_word_count: int
    candidate_chunks: List[str]
    quality_metrics: Optional[Dict[str, Any]]
    quality_gate_plan: Dict[str, Any]
    quality_gate_action: str
    quality_gate_requires_followup: bool
    quality_gate_message: Optional[str]
    quality_gate_snapshot: Optional[Dict[str, Any]]
    content_applied: bool
    attempt_state: str
    draft_attempt: Any
    provisional_draft_allowed: bool


@dataclass(frozen=True)
class ChapterGenerationPersistencePreparation:
    previous_content: str
    previous_word_count: int
    previous_status: Optional[str]
    saved_word_count: int
    provisional_draft_saved: bool
    history: GenerationHistory


def create_chapter_generation_candidate_execution(
    *,
    ai_service: Any,
    generate_kwargs: Dict[str, Any],
    target_word_count: int,
    chapter_id: str,
    quality_evaluator: Callable[[str], Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    max_candidates: int,
    candidate_generator_fn: Callable[..., Any],
) -> ChapterGenerationCandidateExecution:
    runtime_state = build_chapter_candidate_runtime_state(max_candidates=max_candidates)
    selected_candidate_task = asyncio.create_task(
        candidate_generator_fn(
            ai_service=ai_service,
            base_generate_kwargs=generate_kwargs,
            target_word_count=target_word_count,
            source="chapter",
            generation_label=f"chapter_id={chapter_id}",
            quality_evaluator=quality_evaluator,
            quality_gate_plan_builder=quality_gate_plan_builder,
            max_candidates=max_candidates,
            runtime_state=runtime_state,
        )
    )
    return ChapterGenerationCandidateExecution(
        runtime_state=runtime_state,
        selected_candidate_task=selected_candidate_task,
    )


async def wait_for_chapter_generation_candidate(
    *,
    selected_candidate_task: asyncio.Task,
    runtime_state: Dict[str, Any],
    target_word_count: int,
    heartbeat_interval_seconds: float,
    default_candidate_total: int,
    emit_generating_fn: Callable[..., Any],
    emit_heartbeat_fn: Callable[[], Any],
) -> Dict[str, Any]:
    try:
        while True:
            try:
                return await asyncio.wait_for(
                    asyncio.shield(selected_candidate_task),
                    timeout=heartbeat_interval_seconds,
                )
            except asyncio.TimeoutError:
                runtime_snapshot = snapshot_chapter_candidate_runtime_state(
                    runtime_state,
                    default_candidate_total=default_candidate_total,
                )
                await emit_generating_fn(
                    **build_chapter_generation_progress_kwargs(
                        runtime_snapshot=runtime_snapshot,
                        target_word_count=target_word_count,
                    )
                )
                await emit_heartbeat_fn()
    finally:
        if not selected_candidate_task.done():
            selected_candidate_task.cancel()


def build_chapter_generation_candidate_quality_hooks(
    *,
    runtime_context: ChapterGenerationStreamRuntimeContext,
    built_context: ChapterGenerationStreamBuiltContext,
    target_word_count: int,
    build_quality_runtime_context_fn: Callable[..., Dict[str, Any]],
    compute_story_quality_metrics_fn: Callable[..., Dict[str, Any]],
    resolve_quality_gate_execution_plan_fn: Callable[..., Dict[str, Any]],
    retry_count: int = 0,
    max_retries: int = 1,
    scope: str = "chapter",
    log_prefix: str = "Chapter",
) -> ChapterGenerationCandidateQualityHooks:
    chapter = runtime_context.chapter
    project = runtime_context.project
    chapter_context = built_context.chapter_context
    generation_intent = built_context.generation_intent
    current_story_repair_payload = runtime_context.story_repair_payload

    def quality_evaluator(generated_content: str) -> Dict[str, Any]:
        quality_runtime_context = build_quality_runtime_context_fn(
            story_packet=runtime_context.story_packet,
            project=project,
            chapter=chapter,
            chapter_context=chapter_context,
            target_word_count=target_word_count,
            generation_intent=generation_intent,
        )
        metrics = compute_story_quality_metrics_fn(
            content=generated_content,
            chapter_outline=chapter_context.chapter_outline,
            world_rules=project.world_rules,
            quality_runtime_context=quality_runtime_context,
        )
        logger.info(
            f'{log_prefix} candidate metrics - overall={metrics["overall_score"]}, '
            f'conflict={metrics.get("conflict_chain_hit_rate")}, '
            f'rule={metrics.get("rule_grounding_hit_rate")}'
        )
        return metrics

    def quality_gate_plan_builder(candidate_metrics: Dict[str, Any], attempt_offset: int) -> Dict[str, Any]:
        return resolve_quality_gate_execution_plan_fn(
            candidate_metrics if isinstance(candidate_metrics, dict) else None,
            retry_count=retry_count,
            max_retries=max_retries,
            current_story_repair_payload=current_story_repair_payload,
            scope=scope,
        )

    return ChapterGenerationCandidateQualityHooks(
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
    )


def build_chapter_generation_selected_candidate_outcome(
    *,
    selected_candidate: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    previous_content: str,
    previous_word_count: int,
    project_id: str,
    chapter_id: str,
    build_draft_attempt_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any],
) -> ChapterGenerationSelectedCandidateOutcome:
    selected_candidate_result = normalize_selected_candidate_result(
        selected_candidate=selected_candidate,
        story_runtime_contract=story_runtime_contract,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
        include_quality_gate_snapshot_in_metrics=True,
    )
    full_content = str(selected_candidate_result.full_content or "")
    candidate_word_count = int(selected_candidate_result.candidate_word_count or len(full_content))
    candidate_chunks = list(selected_candidate_result.candidate_chunks or [])
    quality_metrics = selected_candidate_result.quality_metrics if isinstance(selected_candidate_result.quality_metrics, dict) else None

    quality_gate_plan = selected_candidate_result.quality_gate_plan or {}
    if not isinstance(quality_gate_plan, dict):
        quality_gate_plan = {}
    quality_gate_action = str(selected_candidate_result.quality_gate_action or "continue")
    quality_gate_requires_followup = quality_gate_action != "continue"
    quality_gate_message = quality_gate_plan.get("message")
    quality_gate_snapshot = selected_candidate_result.quality_gate_snapshot if isinstance(selected_candidate_result.quality_gate_snapshot, dict) else None
    provisional_draft_allowed = quality_gate_requires_followup and quality_gate_action == "retry"
    should_create_draft_attempt = quality_gate_requires_followup
    content_applied = not quality_gate_requires_followup
    attempt_state = "applied" if content_applied else quality_gate_action

    draft_attempt = None
    if should_create_draft_attempt:
        draft_attempt = build_draft_attempt_fn(
            project_id=project_id,
            chapter_id=chapter_id,
            source="chapter",
            attempt_state=attempt_state,
            quality_gate_action=quality_gate_action,
            quality_gate_decision=(quality_gate_snapshot or {}).get("decision") if isinstance(quality_gate_snapshot, dict) else None,
            full_content=full_content,
            summary_preview=full_content[:220] if full_content else None,
            quality_metrics=quality_metrics if isinstance(quality_metrics, dict) else None,
            repair_payload=quality_gate_plan.get("active_story_repair_payload"),
            previous_content=previous_content,
            previous_word_count=previous_word_count,
        )

    return ChapterGenerationSelectedCandidateOutcome(
        full_content=full_content,
        candidate_word_count=candidate_word_count,
        candidate_chunks=candidate_chunks,
        quality_metrics=quality_metrics if isinstance(quality_metrics, dict) else None,
        quality_gate_plan=quality_gate_plan,
        quality_gate_action=quality_gate_action,
        quality_gate_requires_followup=quality_gate_requires_followup,
        quality_gate_message=quality_gate_message,
        quality_gate_snapshot=quality_gate_snapshot,
        content_applied=content_applied,
        attempt_state=attempt_state,
        draft_attempt=draft_attempt,
        provisional_draft_allowed=provisional_draft_allowed,
    )


def apply_chapter_generation_outcome_and_build_history(
    *,
    chapter: Chapter,
    project: Project,
    outcome: ChapterGenerationSelectedCandidateOutcome,
    story_runtime_contract: Optional[Dict[str, Any]],
    build_generation_history_payload_fn: Callable[..., str],
    history_model: str = "default",
) -> ChapterGenerationPersistencePreparation:
    previous_content = chapter.content or ""
    previous_word_count = int(chapter.word_count or len(previous_content))
    previous_status = chapter.status

    provisional_draft_saved = bool(outcome.provisional_draft_allowed and not outcome.content_applied)
    if outcome.content_applied or provisional_draft_saved:
        chapter.content = outcome.full_content
        chapter.word_count = outcome.candidate_word_count
        chapter.status = "completed" if outcome.content_applied else "draft"
        project.current_words = int(project.current_words or 0) - previous_word_count + outcome.candidate_word_count

    saved_word_count = int(chapter.word_count or 0)
    history_payload = build_generation_history_payload_fn(
        outcome.full_content,
        outcome.quality_metrics,
        content_applied=outcome.content_applied,
        attempt_state=outcome.attempt_state,
        story_runtime_contract=story_runtime_contract,
    )
    history = GenerationHistory(
        project_id=chapter.project_id,
        chapter_id=chapter.id,
        prompt=f"chapter:{chapter.chapter_number}:{chapter.title}",
        generated_content=history_payload,
        model=history_model,
    )
    return ChapterGenerationPersistencePreparation(
        previous_content=previous_content,
        previous_word_count=previous_word_count,
        previous_status=previous_status,
        saved_word_count=saved_word_count,
        provisional_draft_saved=provisional_draft_saved,
        history=history,
    )


async def execute_chapter_generation_candidate_stage(
    *,
    chapter_id: str,
    user_ai_service: Any,
    target_word_count: int,
    heartbeat_interval_seconds: float,
    execution_setup: ChapterGenerationStreamExecutionSetup,
    dependencies: ChapterGenerationStreamCandidateDependencies,
    emit_generating_fn: Callable[..., Awaitable[Any]],
    emit_heartbeat_fn: Callable[..., Awaitable[Any]],
    emit_chunk_fn: Callable[[str], Awaitable[Any]],
) -> ChapterGenerationStreamCandidateStageResult:
    candidate_quality_hooks = build_chapter_generation_candidate_quality_hooks(
        runtime_context=execution_setup.stream_runtime_context,
        built_context=execution_setup.built_stream_context,
        target_word_count=target_word_count,
        build_quality_runtime_context_fn=dependencies.build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=dependencies.compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=dependencies.resolve_quality_gate_execution_plan_fn,
        retry_count=0,
        max_retries=1,
        scope="chapter",
        log_prefix="Chapter",
    )
    candidate_execution = create_chapter_generation_candidate_execution(
        ai_service=user_ai_service,
        generate_kwargs=execution_setup.request_payload.generate_kwargs,
        target_word_count=target_word_count,
        chapter_id=chapter_id,
        quality_evaluator=candidate_quality_hooks.quality_evaluator,
        quality_gate_plan_builder=candidate_quality_hooks.quality_gate_plan_builder,
        max_candidates=dependencies.candidate_rerank_limit,
        candidate_generator_fn=dependencies.candidate_generator_fn,
    )
    selected_candidate = await wait_for_chapter_generation_candidate(
        selected_candidate_task=candidate_execution.selected_candidate_task,
        runtime_state=candidate_execution.runtime_state,
        target_word_count=target_word_count,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
        default_candidate_total=dependencies.candidate_rerank_limit,
        emit_generating_fn=emit_generating_fn,
        emit_heartbeat_fn=emit_heartbeat_fn,
    )

    previous_content = execution_setup.current_chapter.content or ""
    previous_word_count = int(execution_setup.current_chapter.word_count or len(previous_content))
    previous_status = execution_setup.current_chapter.status
    selected_candidate_outcome = build_chapter_generation_selected_candidate_outcome(
        selected_candidate=selected_candidate,
        story_runtime_contract=execution_setup.story_runtime_contract,
        previous_content=previous_content,
        previous_word_count=previous_word_count,
        project_id=execution_setup.project.id,
        chapter_id=execution_setup.current_chapter.id,
        build_draft_attempt_fn=dependencies.build_draft_attempt_fn,
        attach_story_runtime_contract_fn=dependencies.attach_story_runtime_contract_fn,
    )

    chunk_payloads: List[Any] = []
    quality_gate_requires_followup = selected_candidate_outcome.quality_gate_requires_followup
    if quality_gate_requires_followup:
        logger.warning(
            f"Quality gate requires follow-up: chapter_id={chapter_id}, "
            f"action={selected_candidate_outcome.quality_gate_action}, "
            f"decision={selected_candidate_outcome.quality_gate_snapshot.get('decision') if isinstance(selected_candidate_outcome.quality_gate_snapshot, dict) else None}"
        )
        if selected_candidate_outcome.provisional_draft_allowed:
            for chunk in selected_candidate_outcome.candidate_chunks:
                chunk_payloads.append(await emit_chunk_fn(chunk))
    else:
        for chunk in selected_candidate_outcome.candidate_chunks:
            chunk_payloads.append(await emit_chunk_fn(chunk))

    return ChapterGenerationStreamCandidateStageResult(
        selected_candidate_outcome=selected_candidate_outcome,
        full_content=selected_candidate_outcome.full_content,
        candidate_word_count=selected_candidate_outcome.candidate_word_count,
        quality_metrics=selected_candidate_outcome.quality_metrics,
        quality_gate_action=selected_candidate_outcome.quality_gate_action,
        quality_gate_requires_followup=quality_gate_requires_followup,
        quality_gate_message=selected_candidate_outcome.quality_gate_message,
        quality_gate_snapshot=selected_candidate_outcome.quality_gate_snapshot,
        content_applied=selected_candidate_outcome.content_applied,
        draft_attempt=selected_candidate_outcome.draft_attempt,
        previous_status=previous_status,
        chunk_payloads=chunk_payloads,
    )


