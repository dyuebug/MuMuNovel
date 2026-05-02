from __future__ import annotations

from typing import Any, AsyncIterator, Callable, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.services.chapter_quality_context_service import (
    build_story_generation_packet_with_project_continuity,
    resolve_chapter_quality_profile,
)
from app.services.chapter_generation.stream.request_policy_service import (
    _build_chapter_generation_request_options,
    _calculate_chapter_generation_max_tokens,
)
from app.utils.sse_response import SSEResponse, WizardProgressTracker
from app.services.chapter_generation.stream.models import (
    ChapterGenerationAnalysisFollowupPlan,
    ChapterGenerationAnalysisScheduling,
    ChapterGenerationEmissionStep,
    ChapterGenerationPostPersistEffects,
    ChapterGenerationStreamBuiltContext,
    ChapterGenerationStreamCandidateStageResult,
    ChapterGenerationStreamDependencies,
    ChapterGenerationStreamExecutionSetup,
    ChapterGenerationStreamPreparation,
    ChapterGenerationStreamPrompt,
    ChapterGenerationStreamRequestPayload,
    ChapterGenerationStreamResponseArtifacts,
    ChapterGenerationStreamRuntimeContext,
)
from app.services.chapter_generation.stream.candidate_service import (
    ChapterGenerationCandidateExecution,
    ChapterGenerationCandidateQualityHooks,
    ChapterGenerationPersistencePreparation,
    ChapterGenerationSelectedCandidateOutcome,
    apply_chapter_generation_outcome_and_build_history,
    build_chapter_generation_candidate_quality_hooks,
    build_chapter_generation_selected_candidate_outcome,
    create_chapter_generation_candidate_execution,
    execute_chapter_generation_candidate_stage,
    wait_for_chapter_generation_candidate,
)
from app.services.chapter_generation.stream.execution_service import (
    build_chapter_generation_stream_context as _build_chapter_generation_stream_context,
    build_chapter_generation_stream_prompt as _build_chapter_generation_stream_prompt,
    build_chapter_generation_stream_request_payload as _build_chapter_generation_stream_request_payload,
    load_chapter_generation_stream_runtime_context as _load_chapter_generation_stream_runtime_context,
    prepare_chapter_generation_stream_execution as _prepare_chapter_generation_stream_execution,
    prepare_chapter_generation_stream_request as _prepare_chapter_generation_stream_request,
)
from app.services.chapter_generation.stream.finalize_service import (
    build_chapter_generation_analysis_followup_plan,
    build_chapter_generation_stream_emission_plan,
    build_chapter_generation_stream_response_artifacts,
    emit_chapter_generation_stream_plan,
    finalize_chapter_generation_stream_result as _finalize_chapter_generation_stream_result,
    prepare_chapter_generation_analysis_scheduling,
    run_chapter_generation_post_persist_effects,
)

logger = get_logger(__name__)

__all__ = (
    "ChapterGenerationAnalysisFollowupPlan",
    "ChapterGenerationAnalysisScheduling",
    "ChapterGenerationCandidateExecution",
    "ChapterGenerationCandidateQualityHooks",
    "ChapterGenerationEmissionStep",
    "ChapterGenerationPersistencePreparation",
    "ChapterGenerationPostPersistEffects",
    "ChapterGenerationSelectedCandidateOutcome",
    "ChapterGenerationStreamBuiltContext",
    "ChapterGenerationStreamCandidateStageResult",
    "ChapterGenerationStreamDependencies",
    "ChapterGenerationStreamExecutionSetup",
    "ChapterGenerationStreamPreparation",
    "ChapterGenerationStreamPrompt",
    "ChapterGenerationStreamRequestPayload",
    "ChapterGenerationStreamResponseArtifacts",
    "ChapterGenerationStreamRuntimeContext",
    "apply_chapter_generation_outcome_and_build_history",
    "build_chapter_generation_analysis_followup_plan",
    "build_chapter_generation_candidate_quality_hooks",
    "build_chapter_generation_event_stream",
    "build_chapter_generation_selected_candidate_outcome",
    "build_chapter_generation_stream_context",
    "build_chapter_generation_stream_emission_plan",
    "build_chapter_generation_stream_prompt",
    "build_chapter_generation_stream_request_payload",
    "build_chapter_generation_stream_response_artifacts",
    "create_chapter_generation_candidate_execution",
    "finalize_chapter_generation_stream_result",
    "load_chapter_generation_stream_runtime_context",
    "prepare_chapter_generation_analysis_scheduling",
    "prepare_chapter_generation_stream_execution",
    "prepare_chapter_generation_stream_request",
    "run_chapter_generation_post_persist_effects",
    "wait_for_chapter_generation_candidate",
)


prepare_chapter_generation_stream_request = _prepare_chapter_generation_stream_request
build_chapter_generation_stream_context = _build_chapter_generation_stream_context
build_chapter_generation_stream_prompt = _build_chapter_generation_stream_prompt
build_chapter_generation_stream_request_payload = _build_chapter_generation_stream_request_payload


async def load_chapter_generation_stream_runtime_context(*args, **kwargs):
    kwargs.setdefault("resolve_quality_profile_fn", resolve_chapter_quality_profile)
    kwargs.setdefault("build_story_packet_fn", build_story_generation_packet_with_project_continuity)
    return await _load_chapter_generation_stream_runtime_context(*args, **kwargs)


async def prepare_chapter_generation_stream_execution(*args, **kwargs):
    kwargs.setdefault("resolve_quality_profile_fn", resolve_chapter_quality_profile)
    kwargs.setdefault("build_story_packet_fn", build_story_generation_packet_with_project_continuity)
    return await _prepare_chapter_generation_stream_execution(*args, **kwargs)


async def finalize_chapter_generation_stream_result(*args, **kwargs):
    kwargs.setdefault(
        "apply_outcome_and_build_history_fn",
        apply_chapter_generation_outcome_and_build_history,
    )
    return await _finalize_chapter_generation_stream_result(*args, **kwargs)


async def build_chapter_generation_event_stream(
    *,
    db_session_source: Callable[[], AsyncIterator[AsyncSession]],
    chapter_id: str,
    current_user_id: str,
    generate_request: Any,
    background_tasks: Any,
    user_ai_service: Any,
    target_word_count: int,
    enable_analysis: bool,
    heartbeat_interval_seconds: float,
    custom_model: Optional[str],
    temp_narrative_perspective: Optional[str],
    style_id: Optional[int],
    dependencies: ChapterGenerationStreamDependencies,
) -> AsyncIterator[Any]:
    db_session = None
    db_committed = False
    tracker = WizardProgressTracker("章节生成")

    try:
        yield await tracker.start()

        async for db_session in db_session_source():
            yield await tracker.loading("Loading generation context...", 0.2)

            try:
                execution_setup = await prepare_chapter_generation_stream_execution(
                    db_session=db_session,
                    chapter_id=chapter_id,
                    current_user_id=current_user_id,
                    generate_request=generate_request,
                    user_ai_service=user_ai_service,
                    target_word_count=target_word_count,
                    custom_model=custom_model,
                    temp_narrative_perspective=temp_narrative_perspective,
                    style_id=style_id,
                    dependencies=dependencies.execution,
                )
            except ValueError as exc:
                detail = str(exc)
                error_code = 404 if ("未找到" in detail or "不存在" in detail) else 400
                yield await tracker.error(detail, error_code)
                return

            yield await tracker.loading("Chapter context built", 0.8)
            yield await tracker.preparing("Preparing AI prompts...")
            logger.info(f"Starting chapter stream generation: {chapter_id}")
            yield await tracker.generating(current_chars=0, estimated_total=target_word_count)

            candidate_stage_result = await execute_chapter_generation_candidate_stage(
                chapter_id=chapter_id,
                user_ai_service=user_ai_service,
                target_word_count=target_word_count,
                heartbeat_interval_seconds=heartbeat_interval_seconds,
                execution_setup=execution_setup,
                dependencies=dependencies.candidate,
                emit_generating_fn=lambda **kwargs: tracker.generating(**kwargs),
                emit_heartbeat_fn=tracker.heartbeat,
                emit_chunk_fn=tracker.generating_chunk,
            )
            for chunk_payload in candidate_stage_result.chunk_payloads:
                yield chunk_payload

            saving_payload, emission_plan = await finalize_chapter_generation_stream_result(
                db_session=db_session,
                chapter_id=chapter_id,
                current_user_id=current_user_id,
                background_tasks=background_tasks,
                user_ai_service=user_ai_service,
                enable_analysis=enable_analysis,
                execution_setup=execution_setup,
                candidate_stage_result=candidate_stage_result,
                dependencies=dependencies.finalize,
                emit_saving_fn=tracker.saving,
            )
            yield saving_payload
            db_committed = True

            async for emitted_payload in emit_chapter_generation_stream_plan(
                emission_plan=emission_plan,
                tracker_complete_fn=tracker.complete,
                tracker_result_fn=tracker.result,
                tracker_done_fn=tracker.done,
                format_sse_fn=SSEResponse.format_sse,
                send_event_fn=SSEResponse.send_event,
            ):
                yield emitted_payload

            break

    except GeneratorExit:
        logger.warning("Chapter stream generator closed early (SSE disconnect)")
        db_session = None
    except Exception as exc:
        logger.error(f"Chapter stream generation failed: {exc}")
        if db_session and not db_committed:
            try:
                if db_session.in_transaction():
                    await db_session.rollback()
                    logger.info("Rolled back uncommitted chapter stream transaction")
            except Exception as rollback_error:
                logger.error(f"Chapter stream rollback failed: {rollback_error}")
        db_session = None
        yield await tracker.error(str(exc))
    finally:
        if db_session:
            # db_session is owned by db_session_source() context; do not manage lifecycle here.
            # Rely on the upstream context manager to handle rollback/close.
            db_session = None

