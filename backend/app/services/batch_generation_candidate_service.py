from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Awaitable, Callable, Dict, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch candidate runtime and event projection chain; "
    "this Python module is kept only as frozen rollback/source-map material "
    "for legacy batch fallback execution."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_candidate_event_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.logger import get_logger

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.chapter import Chapter
    from app.models.project import Project
    from app.services.chapter_quality_context_service import StoryPacket
    from app.services.story_repair_payload_service import StoryRepairPayload


logger = get_logger(__name__)


def _chapter_candidate_runtime_state_service():
    from app.services import chapter_candidate_runtime_state_service

    return chapter_candidate_runtime_state_service


def _chapter_candidate_event_service():
    from app.services import chapter_candidate_event_service

    return chapter_candidate_event_service


def _chapter_candidate_result_service():
    from app.services import chapter_candidate_result_service

    return chapter_candidate_result_service


def _chapter_candidate_view_service():
    from app.services import chapter_candidate_view_service

    return chapter_candidate_view_service


def _story_runtime_serialization_service():
    from app.services import story_runtime_serialization_service

    return story_runtime_serialization_service


def _task_workflow_runtime_service():
    from app.services import task_workflow_runtime_service

    return task_workflow_runtime_service


def build_chapter_candidate_runtime_state(*args, **kwargs):
    return _chapter_candidate_runtime_state_service().build_chapter_candidate_runtime_state(*args, **kwargs)


def snapshot_chapter_candidate_runtime_state(*args, **kwargs):
    return _chapter_candidate_runtime_state_service().snapshot_chapter_candidate_runtime_state(*args, **kwargs)


def build_batch_generation_candidate_progress_event(*args, **kwargs):
    return _chapter_candidate_event_service().build_batch_generation_candidate_progress_event(*args, **kwargs)


def build_batch_generation_chunk_event(*args, **kwargs):
    return _chapter_candidate_event_service().build_batch_generation_chunk_event(*args, **kwargs)


def build_batch_generation_selected_candidate_progress_event(*args, **kwargs):
    return _chapter_candidate_event_service().build_batch_generation_selected_candidate_progress_event(*args, **kwargs)


def build_batch_generation_start_progress_event(*args, **kwargs):
    return _chapter_candidate_event_service().build_batch_generation_start_progress_event(*args, **kwargs)


def normalize_selected_candidate_result(*args, **kwargs):
    return _chapter_candidate_result_service().normalize_selected_candidate_result(*args, **kwargs)


def snapshot_chapter_candidate(*args, **kwargs):
    return _chapter_candidate_view_service().snapshot_chapter_candidate(*args, **kwargs)


def attach_story_runtime_contract(*args, **kwargs):
    return _story_runtime_serialization_service().attach_story_runtime_contract(*args, **kwargs)


async def publish_task_stream_event(*args, **kwargs):
    return await _task_workflow_runtime_service().publish_task_stream_event(*args, **kwargs)


@dataclass(frozen=True)
class BatchGenerationCandidateQualityHooks:
    quality_evaluator: Callable[[str], Dict[str, Any]]
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]]


@dataclass(frozen=True)
class BatchGenerationCandidateExecution:
    runtime_state: Dict[str, Any]
    selected_candidate_task: asyncio.Task


@dataclass(frozen=True)
class BatchGenerationCandidateFlowResult:
    selected_candidate: Dict[str, Any]
    selected_candidate_result: Dict[str, Any]


def build_batch_generation_candidate_quality_hooks(
    *,
    story_packet: "StoryPacket",
    project: "Project",
    chapter: "Chapter",
    chapter_context: Any,
    target_word_count: int,
    generation_intent: Any,
    retry_count: int,
    max_retries: int,
    current_story_repair_payload: Optional["StoryRepairPayload"],
    build_quality_runtime_context_fn: Callable[..., Dict[str, Any]],
    compute_story_quality_metrics_fn: Callable[..., Dict[str, Any]],
    resolve_quality_gate_execution_plan_fn: Callable[..., Dict[str, Any]],
    log_prefix: str = 'Batch',
) -> BatchGenerationCandidateQualityHooks:
    def quality_evaluator(generated_content: str) -> Dict[str, Any]:
        quality_runtime_context = build_quality_runtime_context_fn(
            story_packet=story_packet,
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
            f'conflict={metrics["conflict_chain_hit_rate"]}, '
            f'rule={metrics["rule_grounding_hit_rate"]}'
        )
        return metrics

    def quality_gate_plan_builder(
        candidate_metrics: Dict[str, Any],
        attempt_offset: int,
    ) -> Dict[str, Any]:
        return resolve_quality_gate_execution_plan_fn(
            candidate_metrics if isinstance(candidate_metrics, dict) else None,
            retry_count=retry_count,
            max_retries=max_retries,
            current_story_repair_payload=current_story_repair_payload,
            scope='batch',
        )

    return BatchGenerationCandidateQualityHooks(
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
    )


def build_batch_generation_candidate_runtime_state(*, max_candidates: int) -> Dict[str, Any]:
    return build_chapter_candidate_runtime_state(max_candidates=max_candidates)


def create_batch_generation_candidate_execution(
    *,
    ai_service: Any,
    generate_kwargs: Dict[str, Any],
    target_word_count: int,
    chapter_number: int,
    quality_evaluator: Callable[[str], Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    max_candidates: int,
    candidate_generator_fn: Callable[..., Any],
) -> BatchGenerationCandidateExecution:
    runtime_state = build_batch_generation_candidate_runtime_state(max_candidates=max_candidates)
    selected_candidate_task = asyncio.create_task(
        candidate_generator_fn(
            ai_service=ai_service,
            base_generate_kwargs=generate_kwargs,
            target_word_count=target_word_count,
            source='batch',
            generation_label=f'chapter={chapter_number}',
            quality_evaluator=quality_evaluator,
            quality_gate_plan_builder=quality_gate_plan_builder,
            max_candidates=max_candidates,
            runtime_state=runtime_state,
        )
    )
    return BatchGenerationCandidateExecution(
        runtime_state=runtime_state,
        selected_candidate_task=selected_candidate_task,
    )


async def wait_for_batch_generation_candidate(
    *,
    selected_candidate_task: asyncio.Task,
    runtime_state: Dict[str, Any],
    stream_task_id: Optional[str],
    chapter: "Chapter",
    target_word_count: int,
    heartbeat_interval_seconds: float,
    db_session: "AsyncSession",
    publish_stream_event_fn: Callable[..., Awaitable[None]] = publish_task_stream_event,
) -> Dict[str, Any]:
    try:
        while True:
            try:
                return await asyncio.wait_for(
                    asyncio.shield(selected_candidate_task),
                    timeout=heartbeat_interval_seconds,
                )
            except asyncio.TimeoutError:
                runtime_snapshot = snapshot_chapter_candidate_runtime_state(runtime_state)
                if stream_task_id:
                    await publish_stream_event_fn(
                        stream_task_id,
                        build_batch_generation_candidate_progress_event(
                            chapter=chapter,
                            runtime_snapshot=runtime_snapshot,
                            target_word_count=target_word_count,
                        ),
                        db_session=db_session,
                    )
    finally:
        if not selected_candidate_task.done():
            selected_candidate_task.cancel()


async def emit_batch_generation_selected_candidate_events(
    *,
    stream_task_id: Optional[str],
    stream_chunks: bool,
    chapter: "Chapter",
    selected_candidate: Dict[str, Any],
    candidate_word_count: int,
    quality_gate_plan: Dict[str, Any],
    chapter_context_stats: Dict[str, Any],
    db_session: "AsyncSession",
    publish_stream_event_fn: Callable[..., Awaitable[None]] = publish_task_stream_event,
) -> None:
    selected_candidate_view = snapshot_chapter_candidate(selected_candidate)
    if stream_task_id:
        await publish_stream_event_fn(
            stream_task_id,
            build_batch_generation_selected_candidate_progress_event(
                chapter=chapter,
                selected_candidate_view=selected_candidate_view,
                candidate_word_count=candidate_word_count,
                chapter_context_stats=chapter_context_stats,
            ),
            db_session=db_session,
        )

    if stream_task_id and stream_chunks and str(quality_gate_plan.get('action') or 'continue') == 'continue':
        for chunk in selected_candidate_view.candidate_chunks:
            await publish_stream_event_fn(
                stream_task_id,
                build_batch_generation_chunk_event(chapter=chapter, chunk=chunk),
            )


def build_batch_generation_selected_candidate_result(
    *,
    chapter: "Chapter",
    selected_candidate: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any] = attach_story_runtime_contract,
) -> Dict[str, Any]:
    normalized_result = normalize_selected_candidate_result(
        selected_candidate=selected_candidate,
        story_runtime_contract=story_runtime_contract,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
    )
    full_content = normalized_result.full_content
    candidate_word_count = normalized_result.candidate_word_count
    quality_metrics = dict(normalized_result.quality_metrics)
    quality_gate_plan = dict(normalized_result.quality_gate_plan)
    candidate_count = normalized_result.candidate_count

    logger.info(f'Batch candidate ready: chapter={chapter.chapter_number}, word_count={candidate_word_count}')
    if candidate_count > 1:
        logger.info(
            f'Batch candidate rerank winner: chapter={chapter.chapter_number}, '
            f'candidate_count={candidate_count}, '
            f"winner={normalized_result.candidate_index}"
        )

    summary_preview = full_content[:300].replace('\n', ' ') if full_content else ''
    return {
        'full_content': full_content,
        'word_count': candidate_word_count,
        'summary_preview': summary_preview,
        'quality_metrics': quality_metrics,
        'quality_gate_plan': quality_gate_plan,
        'candidate_count': candidate_count,
        'story_runtime_contract': story_runtime_contract,
    }


async def execute_batch_generation_candidate_flow(
    *,
    stream_task_id: Optional[str],
    stream_chunks: bool,
    chapter: "Chapter",
    effective_story_packet: "StoryPacket",
    project: "Project",
    chapter_context: Any,
    target_word_count: int,
    generation_intent: Any,
    current_story_repair_payload: Optional["StoryRepairPayload"],
    retry_count: int,
    max_retries: int,
    default_candidate_limit: int,
    ai_service: Any,
    generate_kwargs: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    db_session: "AsyncSession",
    heartbeat_interval_seconds: float,
    build_quality_runtime_context_fn: Callable[..., Dict[str, Any]],
    compute_story_quality_metrics_fn: Callable[..., Dict[str, Any]],
    resolve_quality_gate_execution_plan_fn: Callable[..., Dict[str, Any]],
    candidate_generator_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any],
    build_candidate_quality_hooks_fn: Callable[..., BatchGenerationCandidateQualityHooks] = build_batch_generation_candidate_quality_hooks,
    create_candidate_execution_fn: Callable[..., BatchGenerationCandidateExecution] = create_batch_generation_candidate_execution,
    wait_for_candidate_fn: Callable[..., Awaitable[Dict[str, Any]]] = wait_for_batch_generation_candidate,
    emit_selected_candidate_events_fn: Callable[..., Awaitable[None]] = emit_batch_generation_selected_candidate_events,
    build_selected_candidate_result_fn: Callable[..., Dict[str, Any]] = build_batch_generation_selected_candidate_result,
) -> BatchGenerationCandidateFlowResult:
    max_candidates = max(1, int(default_candidate_limit or 1)) if retry_count <= 0 else 1
    candidate_quality_hooks = build_candidate_quality_hooks_fn(
        story_packet=effective_story_packet,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        generation_intent=generation_intent,
        retry_count=retry_count,
        max_retries=max_retries,
        current_story_repair_payload=current_story_repair_payload,
        build_quality_runtime_context_fn=build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        log_prefix='Batch',
    )
    evaluate_candidate_quality = candidate_quality_hooks.quality_evaluator
    build_candidate_quality_gate_plan = candidate_quality_hooks.quality_gate_plan_builder

    if stream_task_id and stream_chunks:
        candidate_execution = create_candidate_execution_fn(
            ai_service=ai_service,
            generate_kwargs=generate_kwargs,
            target_word_count=target_word_count,
            chapter_number=chapter.chapter_number,
            quality_evaluator=evaluate_candidate_quality,
            quality_gate_plan_builder=build_candidate_quality_gate_plan,
            max_candidates=max_candidates,
            candidate_generator_fn=candidate_generator_fn,
        )
        selected_candidate = await wait_for_candidate_fn(
            selected_candidate_task=candidate_execution.selected_candidate_task,
            runtime_state=candidate_execution.runtime_state,
            stream_task_id=stream_task_id,
            chapter=chapter,
            target_word_count=target_word_count,
            heartbeat_interval_seconds=heartbeat_interval_seconds,
            db_session=db_session,
        )
    else:
        selected_candidate = await candidate_generator_fn(
            ai_service=ai_service,
            base_generate_kwargs=generate_kwargs,
            target_word_count=target_word_count,
            source='batch',
            generation_label=f'chapter={chapter.chapter_number}',
            quality_evaluator=evaluate_candidate_quality,
            quality_gate_plan_builder=build_candidate_quality_gate_plan,
            max_candidates=max_candidates,
        )

    chapter_context_stats = (
        dict(chapter_context.context_stats)
        if isinstance(getattr(chapter_context, 'context_stats', None), dict)
        else {}
    )
    selected_candidate_result = build_selected_candidate_result_fn(
        chapter=chapter,
        selected_candidate=selected_candidate,
        story_runtime_contract=story_runtime_contract,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
    )
    await emit_selected_candidate_events_fn(
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        chapter=chapter,
        selected_candidate=selected_candidate,
        candidate_word_count=int(selected_candidate_result.get('word_count') or 0),
        quality_gate_plan=selected_candidate_result.get('quality_gate_plan') or {},
        chapter_context_stats=chapter_context_stats,
        db_session=db_session,
    )
    return BatchGenerationCandidateFlowResult(
        selected_candidate=selected_candidate,
        selected_candidate_result=selected_candidate_result,
    )


async def execute_batch_generation_generation_stage(
    *,
    stream_task_id: Optional[str],
    stream_chunks: bool,
    chapter: "Chapter",
    effective_story_packet: "StoryPacket",
    project: "Project",
    chapter_context: Any,
    target_word_count: int,
    generation_intent: Any,
    current_story_repair_payload: Optional["StoryRepairPayload"],
    retry_count: int,
    max_retries: int,
    default_candidate_limit: int,
    ai_service: Any,
    generate_kwargs: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    db_session: "AsyncSession",
    heartbeat_interval_seconds: float,
    build_quality_runtime_context_fn: Callable[..., Dict[str, Any]],
    compute_story_quality_metrics_fn: Callable[..., Dict[str, Any]],
    resolve_quality_gate_execution_plan_fn: Callable[..., Dict[str, Any]],
    candidate_generator_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any],
    publish_stream_event_fn: Callable[..., Awaitable[None]] = publish_task_stream_event,
    execute_candidate_flow_fn: Callable[..., Awaitable[BatchGenerationCandidateFlowResult]] = execute_batch_generation_candidate_flow,
) -> BatchGenerationCandidateFlowResult:
    if stream_task_id and stream_chunks:
        await publish_stream_event_fn(
            stream_task_id,
            build_batch_generation_start_progress_event(chapter=chapter),
            db_session=db_session,
        )

    return await execute_candidate_flow_fn(
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        chapter=chapter,
        effective_story_packet=effective_story_packet,
        project=project,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        generation_intent=generation_intent,
        current_story_repair_payload=current_story_repair_payload,
        retry_count=retry_count,
        max_retries=max_retries,
        default_candidate_limit=default_candidate_limit,
        ai_service=ai_service,
        generate_kwargs=generate_kwargs,
        story_runtime_contract=story_runtime_contract,
        db_session=db_session,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
        build_quality_runtime_context_fn=build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=candidate_generator_fn,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
    )
