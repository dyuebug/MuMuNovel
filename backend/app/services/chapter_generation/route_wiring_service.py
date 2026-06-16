"""Active route wiring owner for single-chapter generation routes."""
from __future__ import annotations

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active single-generation route wiring chain; this Python "
    "module is kept only as frozen rollback/source-map material after explicit "
    "stream shell freeze approval."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/api/chapter_generation_routes.rs"
SOURCE_MAP_ROLLBACK_FLAG = "legacy_single_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from fastapi import BackgroundTasks, HTTPException, Request

from app.logger import get_logger
from app.schemas.chapter import ChapterGenerateRequest
from typing import Any, Dict, Optional


async def execute_chapter_analysis_background(**kwargs):
    from app.services.manual_chapter_analysis_execution_service import (
        execute_chapter_analysis_background as execute_chapter_analysis_background_service,
    )

    return await execute_chapter_analysis_background_service(**kwargs)

CHAPTER_CANDIDATE_RERANK_LIMIT = 2
CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0


class _LazyOneToOneContextBuilder:
    def __new__(cls, *args, **kwargs):
        from app.services.chapter_context_service import OneToOneContextBuilder

        return OneToOneContextBuilder(*args, **kwargs)


class _LazyOneToManyContextBuilder:
    def __new__(cls, *args, **kwargs):
        from app.services.chapter_context_service import OneToManyContextBuilder

        return OneToManyContextBuilder(*args, **kwargs)


OneToOneContextBuilder = _LazyOneToOneContextBuilder
OneToManyContextBuilder = _LazyOneToManyContextBuilder


def build_chapter_runtime_system_prompt(*args, **kwargs):
    from app.services.chapter_generation.runtime.prompt_service import (
        build_chapter_runtime_system_prompt as build_chapter_runtime_system_prompt_service,
    )

    return build_chapter_runtime_system_prompt_service(*args, **kwargs)


def detect_style_profile(*args, **kwargs):
    from app.services.chapter_generation.runtime.prompt_service import (
        detect_style_profile as detect_style_profile_service,
    )

    return detect_style_profile_service(*args, **kwargs)


def resolve_generation_temperature(*args, **kwargs):
    from app.services.chapter_generation.runtime.prompt_service import (
        resolve_generation_temperature as resolve_generation_temperature_service,
    )

    return resolve_generation_temperature_service(*args, **kwargs)


async def resolve_generation_story_repair_state_for_chapter(*args, **kwargs):
    from app.services.story_repair_payload_service import (
        resolve_generation_story_repair_state_for_chapter as resolve_generation_story_repair_state_for_chapter_service,
    )

    return await resolve_generation_story_repair_state_for_chapter_service(*args, **kwargs)


def resolve_quality_gate_execution_plan(*args, **kwargs):
    from app.services.story_repair_payload_service import (
        resolve_quality_gate_execution_plan as resolve_quality_gate_execution_plan_service,
    )

    return resolve_quality_gate_execution_plan_service(*args, **kwargs)


async def sync_task_story_repair_state(*args, **kwargs):
    from app.services.task_workflow_runtime_service import (
        sync_task_story_repair_state as sync_task_story_repair_state_service,
    )

    return await sync_task_story_repair_state_service(*args, **kwargs)


async def get_template(*args, **kwargs):
    from app.services.prompt_service import PromptService

    return await PromptService.get_template(*args, **kwargs)


def format_prompt(*args, **kwargs):
    from app.services.prompt_service import PromptService

    return PromptService.format_prompt(*args, **kwargs)


def apply_style_to_prompt(*args, **kwargs):
    from app.services.prompt_service import WritingStyleManager

    return WritingStyleManager.apply_style_to_prompt(*args, **kwargs)


def compute_story_quality_metrics(*args, **kwargs):
    from app.services.story_quality_feedback_service import compute_story_quality_metrics

    return compute_story_quality_metrics(*args, **kwargs)

logger = get_logger(__name__)


def get_db(*args, **kwargs):
    from app.database import get_db as get_db_service

    return get_db_service(*args, **kwargs)


async def check_chapter_generation_prerequisites(*args, **kwargs):
    from app.services.chapter_generation.prerequisite_service import (
        check_chapter_generation_prerequisites as check_chapter_generation_prerequisites_service,
    )

    return await check_chapter_generation_prerequisites_service(*args, **kwargs)


async def _build_batch_task_workflow_snapshot_for_background_route(*args, **kwargs):
    from app.services.batch_generation.task_workflow_snapshot_service import (
        build_batch_task_workflow_snapshot,
    )

    return await build_batch_task_workflow_snapshot(*args, **kwargs)


async def _collect_generation_candidate_output(
    ai_service: AIService,
    generate_kwargs: Dict[str, Any],
    *,
    candidate_index: int = 1,
    max_output_chars: Optional[int] = None,
    runtime_state: Optional[Dict[str, Any]] = None,
) -> tuple[str, list[str]]:
    from app.services.chapter_candidate_output_service import (
        ChapterCandidateOutputRequest,
        collect_generation_candidate_output,
    )

    return await collect_generation_candidate_output(
        request=ChapterCandidateOutputRequest(
            ai_service=ai_service,
            generate_kwargs=generate_kwargs,
            candidate_index=candidate_index,
            max_output_chars=max_output_chars,
            runtime_state=runtime_state,
        ),
    )


def _resolve_generation_attempt_labels(
    candidate_index: int,
    *,
    is_word_budget_repair: bool = False,
) -> tuple[str, str]:
    from app.services.chapter_candidate_generation_service import (
        resolve_generation_attempt_labels,
    )

    return resolve_generation_attempt_labels(
        candidate_index,
        is_word_budget_repair=is_word_budget_repair,
    )


def _sync_generation_runtime_state(
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
    from app.services.chapter_candidate_runtime_state_service import (
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


def _build_generation_candidate_record(
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
    from app.services.chapter_candidate_record_service import (
        ChapterCandidateRecordRequest,
        build_generation_candidate_record,
    )

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


def _build_generation_candidate_record_with_default_logging(
    **kwargs,
):
    return _build_generation_candidate_record(
        **kwargs,
        log_warning_fn=logger.warning,
    )


async def _generate_best_ranked_candidate(
    *,
    ai_service,
    base_generate_kwargs: Dict[str, Any],
    target_word_count: int,
    source: str,
    generation_label: str,
    quality_evaluator,
    quality_gate_plan_builder,
    max_candidates: int = CHAPTER_CANDIDATE_RERANK_LIMIT,
    runtime_state: Optional[Dict[str, Any]] = None,
):
    from app.services.chapter_candidate_executor_service import generate_best_ranked_candidate

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
        resolve_generation_attempt_labels_fn=_resolve_generation_attempt_labels,
        sync_generation_runtime_state_fn=_sync_generation_runtime_state,
        collect_generation_candidate_output_fn=_collect_generation_candidate_output,
        build_generation_candidate_record_fn=_build_generation_candidate_record_with_default_logging,
    )


async def generate_chapter_content_stream_with_explicit_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest,
    user_ai_service,
    get_db_fn,
    check_prerequisites_fn,
    build_default_stream_dependencies_fn,
    prepare_stream_request_fn,
    build_event_stream_fn,
    create_sse_response_fn,
    cancel_outline_postprocess_tasks_fn,
    candidate_generator_fn,
    candidate_rerank_limit: int,
    one_to_one_builder_cls,
    one_to_many_builder_cls,
    get_template_fn,
    format_prompt_fn,
    apply_style_to_prompt_fn,
    build_runtime_system_prompt_fn,
    detect_style_profile_fn,
    resolve_generation_temperature_fn,
    compute_story_quality_metrics_fn,
    resolve_quality_gate_execution_plan_fn,
    analyze_chapter_background_fn,
    heartbeat_interval_seconds: float,
):
    style_id = generate_request.style_id
    target_word_count = generate_request.target_word_count or 3000
    enable_analysis = bool(getattr(generate_request, 'enable_analysis', False))
    custom_model = generate_request.model if hasattr(generate_request, 'model') else None
    temp_narrative_perspective = (
        generate_request.narrative_perspective
        if hasattr(generate_request, 'narrative_perspective')
        else None
    )

    async for temp_db in get_db_fn(request):
        try:
            await prepare_stream_request_fn(
                temp_db,
                chapter_id=chapter_id,
                check_prerequisites_fn=check_prerequisites_fn,
            )
        except ValueError as exc:
            raise HTTPException(status_code=404, detail=str(exc)) from exc
        except RuntimeError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc
        finally:
            await temp_db.close()
        break

    stream_dependencies = build_default_stream_dependencies_fn(
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks_fn,
        candidate_generator_fn=candidate_generator_fn,
        candidate_rerank_limit=candidate_rerank_limit,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        detect_style_profile_fn=detect_style_profile_fn,
        resolve_generation_temperature_fn=resolve_generation_temperature_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        analyze_chapter_background_fn=analyze_chapter_background_fn,
    )

    current_user_id = getattr(request.state, 'user_id', 'system')
    return create_sse_response_fn(
        build_event_stream_fn(
            db_session_source=lambda: get_db_fn(request),
            chapter_id=chapter_id,
            current_user_id=current_user_id,
            generate_request=generate_request,
            background_tasks=background_tasks,
            user_ai_service=user_ai_service,
            target_word_count=target_word_count,
            enable_analysis=enable_analysis,
            heartbeat_interval_seconds=heartbeat_interval_seconds,
            custom_model=custom_model,
            temp_narrative_perspective=temp_narrative_perspective,
            style_id=style_id,
            dependencies=stream_dependencies,
        )
    )


async def generate_chapter_content_stream_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest,
    user_ai_service: AIService,
):
    from app.api.outlines import cancel_outline_postprocess_tasks
    from app.services.chapter_generation.stream.service import (
        build_chapter_generation_event_stream,
        prepare_chapter_generation_stream_request,
    )
    from app.services.chapter_generation.stream.wiring_service import (
        build_default_chapter_generation_stream_dependencies,
    )
    from app.utils.sse_response import create_sse_response

    return await generate_chapter_content_stream_with_explicit_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        generate_request=generate_request,
        user_ai_service=user_ai_service,
        get_db_fn=get_db,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        build_default_stream_dependencies_fn=build_default_chapter_generation_stream_dependencies,
        prepare_stream_request_fn=prepare_chapter_generation_stream_request,
        build_event_stream_fn=build_chapter_generation_event_stream,
        create_sse_response_fn=create_sse_response,
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks,
        candidate_generator_fn=_generate_best_ranked_candidate,
        candidate_rerank_limit=CHAPTER_CANDIDATE_RERANK_LIMIT,
        one_to_one_builder_cls=OneToOneContextBuilder,
        one_to_many_builder_cls=OneToManyContextBuilder,
        get_template_fn=get_template,
        format_prompt_fn=format_prompt,
        apply_style_to_prompt_fn=apply_style_to_prompt,
        build_runtime_system_prompt_fn=build_chapter_runtime_system_prompt,
        detect_style_profile_fn=detect_style_profile,
        resolve_generation_temperature_fn=resolve_generation_temperature,
        compute_story_quality_metrics_fn=compute_story_quality_metrics,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan,
        analyze_chapter_background_fn=execute_chapter_analysis_background,
        heartbeat_interval_seconds=CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS,
    )


async def generate_chapter_content_background_with_explicit_wiring(
    *,
    db_session,
    chapter_id: str,
    user_id: str,
    generate_request: ChapterGenerateRequest,
    background_tasks: BackgroundTasks,
    ai_service,
    load_accessible_chapter_or_404_fn,
    check_prerequisites_fn,
    build_workflow_snapshot_fn,
    resolve_story_repair_state_fn,
    sync_task_story_repair_state_fn,
    execution_callable,
    orchestrate_single_chapter_background_generation_fn,
):
    from app.models.project import Project
    from sqlalchemy import select

    chapter = await load_accessible_chapter_or_404_fn(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    project_result = await db_session.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        raise HTTPException(status_code=404, detail='Project not found')

    return await orchestrate_single_chapter_background_generation_fn(
        db_session,
        chapter_id=chapter_id,
        chapter=chapter,
        project=project,
        user_id=user_id,
        generate_request=generate_request,
        background_tasks=background_tasks,
        ai_service=ai_service,
        check_prerequisites_fn=check_prerequisites_fn,
        build_workflow_snapshot_fn=build_workflow_snapshot_fn,
        resolve_story_repair_state_fn=resolve_story_repair_state_fn,
        sync_task_story_repair_state_fn=sync_task_story_repair_state_fn,
        execution_callable=execution_callable,
    )


async def generate_chapter_content_background_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest,
    db_session: AsyncSession,
    user_ai_service: AIService,
):
    from app.api.chapter_route_helpers import (
        load_accessible_chapter_or_404,
        require_authenticated_user_id,
    )
    from app.models.project import Project
    from app.services.batch_generation_run_wiring_service import (
        execute_batch_generation_in_order_with_entry_service_seams,
    )
    from app.services.batch_generation_orchestration_service import (
        orchestrate_single_chapter_background_generation,
    )

    user_id = require_authenticated_user_id(request)
    return await generate_chapter_content_background_with_explicit_wiring(
        db_session=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
        generate_request=generate_request,
        background_tasks=background_tasks,
        ai_service=user_ai_service,
        load_accessible_chapter_or_404_fn=load_accessible_chapter_or_404,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        build_workflow_snapshot_fn=_build_batch_task_workflow_snapshot_for_background_route,
        resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_chapter,
        sync_task_story_repair_state_fn=sync_task_story_repair_state,
        execution_callable=execute_batch_generation_in_order_with_entry_service_seams,
        orchestrate_single_chapter_background_generation_fn=orchestrate_single_chapter_background_generation,
    )
