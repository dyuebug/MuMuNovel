"""Compatibility helpers for chapter generation route defaults."""
from __future__ import annotations

from fastapi import BackgroundTasks, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.api.outlines import cancel_outline_postprocess_tasks
from app.logger import get_logger
from app.database import get_db
from app.schemas.chapter import ChapterGenerateRequest
from app.services.ai_service import AIService
from typing import Any, Dict, Optional

from app.services.prompt_service import PromptService, WritingStyleManager

from app.services.chapter_candidate_executor_service import (
    generate_best_ranked_candidate as _generate_best_ranked_candidate_service,
)
from app.services.chapter_candidate_generation_service import (
    resolve_generation_attempt_labels as _resolve_generation_attempt_labels_service,
)
from app.services.chapter_candidate_output_service import (
    ChapterCandidateOutputRequest,
    collect_generation_candidate_output as _collect_generation_candidate_output_service,
)
from app.services.chapter_candidate_record_service import (
    ChapterCandidateRecordRequest,
    build_generation_candidate_record as _build_generation_candidate_record_service,
)
from app.services.chapter_candidate_runtime_state_service import (
    sync_chapter_candidate_runtime_state as _sync_chapter_candidate_runtime_state_service,
)
from app.services.chapter_context_service import (
    OneToManyContextBuilder,
    OneToOneContextBuilder,
)
from app.api import chapters as chapters_api
from app.services.batch_generation.query_service import (
    build_batch_task_workflow_snapshot,
)
from app.services.chapter_generation.background_entry_service import (
    generate_chapter_content_background_with_default_wiring as generate_chapter_content_background_entry_with_default_wiring,
)
from app.services.chapter_generation.prerequisite_service import (
    check_chapter_generation_prerequisites,
)
from app.services.chapter_generation.stream.entry_service import (
    generate_chapter_content_stream_with_default_wiring,
)
from app.services.chapter_generation.runtime.prompt_service import (
    build_chapter_runtime_system_prompt,
    detect_style_profile,
    resolve_generation_temperature,
)
from app.services.story_quality_feedback_service import compute_story_quality_metrics
from app.services.manual_chapter_analysis_execution_service import (
    execute_chapter_analysis_background as _default_execute_chapter_analysis_background,
)

execute_chapter_analysis_background = _default_execute_chapter_analysis_background
from app.services.story_repair_payload_service import (
    resolve_generation_story_repair_state_for_chapter,
    resolve_quality_gate_execution_plan,
)
from app.services.task_workflow_runtime_service import (
    sync_task_story_repair_state,
)

CHAPTER_CANDIDATE_RERANK_LIMIT = 2
CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0

get_template = PromptService.get_template
format_prompt = PromptService.format_prompt
apply_style_to_prompt = WritingStyleManager.apply_style_to_prompt

_DEFAULT_ONE_TO_ONE_CONTEXT_BUILDER = OneToOneContextBuilder
_DEFAULT_ONE_TO_MANY_CONTEXT_BUILDER = OneToManyContextBuilder
_DEFAULT_GET_TEMPLATE = get_template
_DEFAULT_FORMAT_PROMPT = format_prompt
_DEFAULT_APPLY_STYLE_TO_PROMPT = apply_style_to_prompt
_DEFAULT_BUILD_RUNTIME_SYSTEM_PROMPT = build_chapter_runtime_system_prompt
_DEFAULT_COMPUTE_STORY_QUALITY_METRICS = compute_story_quality_metrics
_DEFAULT_RESOLVE_QUALITY_GATE_EXECUTION_PLAN = resolve_quality_gate_execution_plan

logger = get_logger(__name__)


def _prefer_local_override(local_value, default_value, shared_value):
    if local_value is not default_value:
        return local_value
    return shared_value


async def collect_generation_candidate_output(
    ai_service: AIService,
    generate_kwargs: Dict[str, Any],
    *,
    candidate_index: int = 1,
    max_output_chars: Optional[int] = None,
    runtime_state: Optional[Dict[str, Any]] = None,
) -> tuple[str, list[str]]:
    return await _collect_generation_candidate_output_service(
        request=ChapterCandidateOutputRequest(
            ai_service=ai_service,
            generate_kwargs=generate_kwargs,
            candidate_index=candidate_index,
            max_output_chars=max_output_chars,
            runtime_state=runtime_state,
        ),
    )


def resolve_generation_attempt_labels(
    candidate_index: int,
    *,
    is_word_budget_repair: bool = False,
) -> tuple[str, str]:
    return _resolve_generation_attempt_labels_service(
        candidate_index,
        is_word_budget_repair=is_word_budget_repair,
    )


def sync_generation_runtime_state(
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
    _sync_chapter_candidate_runtime_state_service(
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


def build_generation_candidate_record(
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
    return _build_generation_candidate_record_service(
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


async def generate_best_ranked_candidate(
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
    return await _generate_best_ranked_candidate_service(
        ai_service=ai_service,
        base_generate_kwargs=base_generate_kwargs,
        target_word_count=target_word_count,
        source=source,
        generation_label=generation_label,
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
        max_candidates=max_candidates,
        runtime_state=runtime_state,
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels,
        sync_generation_runtime_state_fn=sync_generation_runtime_state,
        collect_generation_candidate_output_fn=collect_generation_candidate_output,
        build_generation_candidate_record_fn=build_generation_candidate_record_with_default_logging,
    )


def build_generation_candidate_record_with_default_logging(
    **kwargs,
):
    return build_generation_candidate_record(
        **kwargs,
        log_warning_fn=logger.warning,
    )


async def generate_chapter_content_stream_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest,
    user_ai_service: AIService,
):
    resolved_one_to_one_builder_cls = _prefer_local_override(
        OneToOneContextBuilder,
        _DEFAULT_ONE_TO_ONE_CONTEXT_BUILDER,
        chapters_api.OneToOneContextBuilder,
    )
    resolved_one_to_many_builder_cls = _prefer_local_override(
        OneToManyContextBuilder,
        _DEFAULT_ONE_TO_MANY_CONTEXT_BUILDER,
        chapters_api.OneToManyContextBuilder,
    )
    resolved_get_template_fn = _prefer_local_override(
        get_template,
        _DEFAULT_GET_TEMPLATE,
        chapters_api.PromptService.get_template,
    )
    resolved_format_prompt_fn = _prefer_local_override(
        format_prompt,
        _DEFAULT_FORMAT_PROMPT,
        chapters_api.PromptService.format_prompt,
    )
    resolved_apply_style_to_prompt_fn = _prefer_local_override(
        apply_style_to_prompt,
        _DEFAULT_APPLY_STYLE_TO_PROMPT,
        chapters_api.WritingStyleManager.apply_style_to_prompt,
    )
    resolved_build_runtime_system_prompt_fn = _prefer_local_override(
        build_chapter_runtime_system_prompt,
        _DEFAULT_BUILD_RUNTIME_SYSTEM_PROMPT,
        chapters_api._build_chapter_runtime_system_prompt,
    )
    resolved_compute_story_quality_metrics_fn = _prefer_local_override(
        compute_story_quality_metrics,
        _DEFAULT_COMPUTE_STORY_QUALITY_METRICS,
        chapters_api.compute_story_quality_metrics,
    )
    resolved_quality_gate_execution_plan_fn = _prefer_local_override(
        resolve_quality_gate_execution_plan,
        _DEFAULT_RESOLVE_QUALITY_GATE_EXECUTION_PLAN,
        chapters_api._resolve_quality_gate_execution_plan,
    )

    return await generate_chapter_content_stream_with_default_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        generate_request=generate_request,
        user_ai_service=user_ai_service,
        get_db_fn=get_db,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks,
        candidate_generator_fn=generate_best_ranked_candidate,
        candidate_rerank_limit=CHAPTER_CANDIDATE_RERANK_LIMIT,
        one_to_one_builder_cls=resolved_one_to_one_builder_cls,
        one_to_many_builder_cls=resolved_one_to_many_builder_cls,
        get_template_fn=resolved_get_template_fn,
        format_prompt_fn=resolved_format_prompt_fn,
        apply_style_to_prompt_fn=resolved_apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=resolved_build_runtime_system_prompt_fn,
        detect_style_profile_fn=detect_style_profile,
        resolve_generation_temperature_fn=resolve_generation_temperature,
        compute_story_quality_metrics_fn=resolved_compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolved_quality_gate_execution_plan_fn,
        analyze_chapter_background_fn=execute_chapter_analysis_background,
        heartbeat_interval_seconds=CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS,
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
    user_id = require_authenticated_user_id(request)
    return await generate_chapter_content_background_entry_with_default_wiring(
        db_session=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
        generate_request=generate_request,
        background_tasks=background_tasks,
        ai_service=user_ai_service,
        load_accessible_chapter_or_404_fn=load_accessible_chapter_or_404,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        build_workflow_snapshot_fn=build_batch_task_workflow_snapshot,
        resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_chapter,
        sync_task_story_repair_state_fn=sync_task_story_repair_state,
        execution_callable=chapters_api.execute_batch_generation_in_order,
    )
