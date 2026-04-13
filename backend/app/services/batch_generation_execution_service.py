"""???????????? fa?ade?"""
from __future__ import annotations

import asyncio
from typing import Any, Awaitable, Callable, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.models.project import Project
from app.services.analysis_task_service import create_analysis_task_safely as _create_analysis_task_safely_impl
from app.services.batch_generation_analysis_service import (
    run_batch_chapter_analysis as _run_batch_chapter_analysis,
)
from app.services.batch_generation_candidate_service import (
    BatchGenerationCandidateExecution,
    BatchGenerationCandidateFlowResult,
    BatchGenerationCandidateQualityHooks,
    build_batch_generation_candidate_quality_hooks as _build_batch_generation_candidate_quality_hooks,
    build_batch_generation_candidate_runtime_state as _build_batch_generation_candidate_runtime_state,
    build_batch_generation_selected_candidate_result as _build_batch_generation_selected_candidate_result,
    create_batch_generation_candidate_execution as _create_batch_generation_candidate_execution,
    emit_batch_generation_selected_candidate_events as _emit_batch_generation_selected_candidate_events,
    execute_batch_generation_candidate_flow as _execute_batch_generation_candidate_flow,
    execute_batch_generation_generation_stage as _execute_batch_generation_generation_stage,
    wait_for_batch_generation_candidate as _wait_for_batch_generation_candidate,
)
from app.services.batch_generation_chapter_execution_service import (
    BatchGenerationChapterAttemptPreparation as _BatchGenerationChapterAttemptPreparation,
    BatchGenerationPreparedChapterResult as _BatchGenerationPreparedChapterResult,
    clear_batch_generation_execution_caches as _clear_batch_generation_execution_caches,
    prepare_batch_generation_chapter_attempt as _prepare_batch_generation_chapter_attempt,
    prepare_batch_generation_chapter_result as _prepare_batch_generation_chapter_result,
)
from app.services.batch_generation_chapter_failure_state_service import (
    fail_batch_generation_after_analysis as _fail_batch_generation_after_analysis,
    fail_batch_generation_after_max_retries as _fail_batch_generation_after_max_retries,
    fail_batch_generation_for_manual_review as _fail_batch_generation_for_manual_review,
)
from app.services.batch_generation_chapter_persistence_service import (
    apply_generated_batch_chapter_candidate as _apply_generated_batch_chapter_candidate_impl,
    build_batch_chapter_draft_attempt as _build_batch_chapter_draft_attempt_impl,
)
from app.services.batch_generation_chapter_success_state_service import (
    BatchGenerationAppliedChapterState as _BatchGenerationAppliedChapterState,
    BatchGenerationQualityGateRetryPreparation as _BatchGenerationQualityGateRetryPreparation,
    apply_successful_batch_generation_chapter as _apply_successful_batch_generation_chapter,
    finalize_successful_batch_generation_chapter as _finalize_successful_batch_generation_chapter,
    handle_batch_generation_quality_gate_retry as _handle_batch_generation_quality_gate_retry,
)
from app.services.batch_generation_prompt_service import (
    BatchGenerationPrompt as _BatchGenerationPrompt,
    BatchGenerationPromptStageResult as _BatchGenerationPromptStageResult,
    BatchGenerationRequestPayload as _BatchGenerationRequestPayload,
    build_batch_generation_prompt as _build_batch_generation_prompt,
    build_batch_generation_request_payload as _build_batch_generation_request_payload,
    execute_batch_generation_prompt_stage as _execute_batch_generation_prompt_stage,
)
from app.services.batch_generation_retry_service import (
    BatchGenerationChapterExecutionOutcome as _BatchGenerationChapterExecutionOutcome,
    BatchGenerationChapterRuntimeState as _BatchGenerationChapterRuntimeState,
    BatchGenerationExecutionEnvironment as _BatchGenerationExecutionEnvironment,
    execute_batch_generation_chapter_with_retries as _execute_batch_generation_chapter_with_retries,
)
from app.services.batch_generation_runtime_service import (
    BatchGenerationBuiltContext as _BatchGenerationBuiltContext,
    BatchGenerationChapterRuntimeArtifacts as _BatchGenerationChapterRuntimeArtifacts,
    BatchGenerationResolvedRuntime as _BatchGenerationResolvedRuntime,
    BatchGenerationRuntimePreparation as _BatchGenerationRuntimePreparation,
    build_batch_generation_context as _build_batch_generation_context,
    finalize_batch_generation_runtime as _finalize_batch_generation_runtime,
    prepare_batch_generation_runtime as _prepare_batch_generation_runtime,
    resolve_batch_generation_chapter_runtime as _resolve_batch_generation_chapter_runtime,
)
from app.services.batch_generation_workflow_service import (
    BatchGenerationExecutionInitialization as _BatchGenerationExecutionInitialization,
    calculate_estimated_time as _calculate_estimated_time,
    complete_batch_generation_execution as _complete_batch_generation_execution,
    create_batch_generation_task_record as _create_batch_generation_task_record,
    enqueue_batch_generation_execution as _enqueue_batch_generation_execution,
    fail_batch_generation_on_unhandled_exception as _fail_batch_generation_on_unhandled_exception,
    handle_cancelled_batch_generation_execution as _handle_cancelled_batch_generation_execution,
    initialize_batch_generation_execution as _initialize_batch_generation_execution,
    mark_batch_generation_current_chapter as _mark_batch_generation_current_chapter,
)
from app.services.chapter_quality_context_service import StoryPacket
from app.services.single_chapter_background_context_service import (
    SingleChapterBackgroundExecutionContext as _SingleChapterBackgroundExecutionContext,
    build_single_chapter_background_execution_context as _build_single_chapter_background_execution_context,
)
from app.services.story_repair_payload_service import StoryRepairPayload
from app.services.task_workflow_runtime_service import publish_task_stream_event


BatchGenerationPrompt = _BatchGenerationPrompt
BatchGenerationPromptStageResult = _BatchGenerationPromptStageResult
BatchGenerationRequestPayload = _BatchGenerationRequestPayload
BatchGenerationRuntimePreparation = _BatchGenerationRuntimePreparation
BatchGenerationResolvedRuntime = _BatchGenerationResolvedRuntime
BatchGenerationBuiltContext = _BatchGenerationBuiltContext
BatchGenerationChapterRuntimeArtifacts = _BatchGenerationChapterRuntimeArtifacts
BatchGenerationExecutionInitialization = _BatchGenerationExecutionInitialization
SingleChapterBackgroundExecutionContext = _SingleChapterBackgroundExecutionContext
BatchGenerationExecutionEnvironment = _BatchGenerationExecutionEnvironment
BatchGenerationChapterRuntimeState = _BatchGenerationChapterRuntimeState
BatchGenerationChapterExecutionOutcome = _BatchGenerationChapterExecutionOutcome
BatchGenerationChapterAttemptPreparation = _BatchGenerationChapterAttemptPreparation
BatchGenerationPreparedChapterResult = _BatchGenerationPreparedChapterResult
BatchGenerationQualityGateRetryPreparation = _BatchGenerationQualityGateRetryPreparation
BatchGenerationAppliedChapterState = _BatchGenerationAppliedChapterState
prepare_batch_generation_runtime = _prepare_batch_generation_runtime
build_batch_generation_context = _build_batch_generation_context
finalize_batch_generation_runtime = _finalize_batch_generation_runtime
resolve_batch_generation_chapter_runtime = _resolve_batch_generation_chapter_runtime
build_batch_generation_prompt = _build_batch_generation_prompt
build_batch_generation_request_payload = _build_batch_generation_request_payload
execute_batch_generation_prompt_stage = _execute_batch_generation_prompt_stage
create_batch_generation_task_record = _create_batch_generation_task_record
calculate_estimated_time = _calculate_estimated_time
enqueue_batch_generation_execution = _enqueue_batch_generation_execution
mark_batch_generation_current_chapter = _mark_batch_generation_current_chapter
handle_cancelled_batch_generation_execution = _handle_cancelled_batch_generation_execution
complete_batch_generation_execution = _complete_batch_generation_execution
fail_batch_generation_on_unhandled_exception = _fail_batch_generation_on_unhandled_exception
initialize_batch_generation_execution = _initialize_batch_generation_execution
build_single_chapter_background_execution_context = _build_single_chapter_background_execution_context
build_batch_chapter_draft_attempt = _build_batch_chapter_draft_attempt_impl
apply_generated_batch_chapter_candidate = _apply_generated_batch_chapter_candidate_impl
prepare_batch_generation_chapter_attempt = _prepare_batch_generation_chapter_attempt
prepare_batch_generation_chapter_result = _prepare_batch_generation_chapter_result
handle_batch_generation_quality_gate_retry = _handle_batch_generation_quality_gate_retry
fail_batch_generation_for_manual_review = _fail_batch_generation_for_manual_review
apply_successful_batch_generation_chapter = _apply_successful_batch_generation_chapter
fail_batch_generation_after_analysis = _fail_batch_generation_after_analysis
finalize_successful_batch_generation_chapter = _finalize_successful_batch_generation_chapter
fail_batch_generation_after_max_retries = _fail_batch_generation_after_max_retries
clear_batch_generation_execution_caches = _clear_batch_generation_execution_caches
create_analysis_task_safely = _create_analysis_task_safely_impl
run_batch_chapter_analysis = _run_batch_chapter_analysis
execute_batch_generation_chapter_with_retries = _execute_batch_generation_chapter_with_retries
BatchGenerationCandidateQualityHooks = BatchGenerationCandidateQualityHooks
BatchGenerationCandidateExecution = BatchGenerationCandidateExecution
BatchGenerationCandidateFlowResult = BatchGenerationCandidateFlowResult
build_batch_generation_candidate_quality_hooks = _build_batch_generation_candidate_quality_hooks
build_batch_generation_candidate_runtime_state = _build_batch_generation_candidate_runtime_state
create_batch_generation_candidate_execution = _create_batch_generation_candidate_execution
build_batch_generation_selected_candidate_result = _build_batch_generation_selected_candidate_result


async def wait_for_batch_generation_candidate(
    *,
    selected_candidate_task: asyncio.Task,
    runtime_state: Dict[str, Any],
    stream_task_id: Optional[str],
    chapter: Chapter,
    target_word_count: int,
    heartbeat_interval_seconds: float,
    db_session: AsyncSession,
    publish_stream_event_fn: Optional[Callable[..., Awaitable[None]]] = None,
) -> Dict[str, Any]:
    resolved_publish_stream_event_fn = publish_stream_event_fn or publish_task_stream_event
    return await _wait_for_batch_generation_candidate(
        selected_candidate_task=selected_candidate_task,
        runtime_state=runtime_state,
        stream_task_id=stream_task_id,
        chapter=chapter,
        target_word_count=target_word_count,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
        db_session=db_session,
        publish_stream_event_fn=resolved_publish_stream_event_fn,
    )


async def emit_batch_generation_selected_candidate_events(
    *,
    stream_task_id: Optional[str],
    stream_chunks: bool,
    chapter: Chapter,
    selected_candidate: Dict[str, Any],
    candidate_word_count: int,
    quality_gate_plan: Dict[str, Any],
    chapter_context_stats: Dict[str, Any],
    db_session: AsyncSession,
    publish_stream_event_fn: Optional[Callable[..., Awaitable[None]]] = None,
) -> None:
    resolved_publish_stream_event_fn = publish_stream_event_fn or publish_task_stream_event
    await _emit_batch_generation_selected_candidate_events(
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        chapter=chapter,
        selected_candidate=selected_candidate,
        candidate_word_count=candidate_word_count,
        quality_gate_plan=quality_gate_plan,
        chapter_context_stats=chapter_context_stats,
        db_session=db_session,
        publish_stream_event_fn=resolved_publish_stream_event_fn,
    )


async def execute_batch_generation_candidate_flow(
    *,
    stream_task_id: Optional[str],
    stream_chunks: bool,
    chapter: Chapter,
    effective_story_packet: StoryPacket,
    project: Project,
    chapter_context: Any,
    target_word_count: int,
    generation_intent: Any,
    current_story_repair_payload: Optional[StoryRepairPayload],
    retry_count: int,
    max_retries: int,
    default_candidate_limit: int,
    ai_service: Any,
    generate_kwargs: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    db_session: AsyncSession,
    heartbeat_interval_seconds: float,
    build_quality_runtime_context_fn: Callable[..., Dict[str, Any]],
    compute_story_quality_metrics_fn: Callable[..., Dict[str, Any]],
    resolve_quality_gate_execution_plan_fn: Callable[..., Dict[str, Any]],
    candidate_generator_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any],
    build_candidate_quality_hooks_fn: Optional[Callable[..., BatchGenerationCandidateQualityHooks]] = None,
    create_candidate_execution_fn: Optional[Callable[..., BatchGenerationCandidateExecution]] = None,
    wait_for_candidate_fn: Optional[Callable[..., Awaitable[Dict[str, Any]]]] = None,
    emit_selected_candidate_events_fn: Optional[Callable[..., Awaitable[None]]] = None,
    build_selected_candidate_result_fn: Optional[Callable[..., Dict[str, Any]]] = None,
) -> BatchGenerationCandidateFlowResult:
    return await _execute_batch_generation_candidate_flow(
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
        build_candidate_quality_hooks_fn=build_candidate_quality_hooks_fn or build_batch_generation_candidate_quality_hooks,
        create_candidate_execution_fn=create_candidate_execution_fn or create_batch_generation_candidate_execution,
        wait_for_candidate_fn=wait_for_candidate_fn or wait_for_batch_generation_candidate,
        emit_selected_candidate_events_fn=emit_selected_candidate_events_fn or emit_batch_generation_selected_candidate_events,
        build_selected_candidate_result_fn=build_selected_candidate_result_fn or build_batch_generation_selected_candidate_result,
    )


async def execute_batch_generation_generation_stage(
    *,
    stream_task_id: Optional[str],
    stream_chunks: bool,
    chapter: Chapter,
    effective_story_packet: StoryPacket,
    project: Project,
    chapter_context: Any,
    target_word_count: int,
    generation_intent: Any,
    current_story_repair_payload: Optional[StoryRepairPayload],
    retry_count: int,
    max_retries: int,
    default_candidate_limit: int,
    ai_service: Any,
    generate_kwargs: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    db_session: AsyncSession,
    heartbeat_interval_seconds: float,
    build_quality_runtime_context_fn: Callable[..., Dict[str, Any]],
    compute_story_quality_metrics_fn: Callable[..., Dict[str, Any]],
    resolve_quality_gate_execution_plan_fn: Callable[..., Dict[str, Any]],
    candidate_generator_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any],
    publish_stream_event_fn: Optional[Callable[..., Awaitable[None]]] = None,
    execute_candidate_flow_fn: Optional[Callable[..., Awaitable[BatchGenerationCandidateFlowResult]]] = None,
) -> BatchGenerationCandidateFlowResult:
    return await _execute_batch_generation_generation_stage(
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
        publish_stream_event_fn=publish_stream_event_fn or publish_task_stream_event,
        execute_candidate_flow_fn=execute_candidate_flow_fn or execute_batch_generation_candidate_flow,
    )
