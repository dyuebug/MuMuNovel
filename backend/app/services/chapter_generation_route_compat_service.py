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

from app.services.chapter_candidate_entry_compat_service import (
    generate_best_ranked_candidate as _generate_best_ranked_candidate_entry,
)
from app.services.chapter_candidate_executor_compat_service import (
    build_generation_candidate_record,
    collect_generation_candidate_output,
    resolve_generation_attempt_labels,
    sync_generation_runtime_state,
)
from app.services.chapter_context_service import (
    OneToManyContextBuilder,
    OneToOneContextBuilder,
)
from app.services import batch_generation_entry_compat_service
from app.services.batch_generation_query_service import (
    build_batch_task_workflow_snapshot,
)
from app.services.chapter_generation_background_entry_service import (
    generate_chapter_content_background_with_default_wiring as generate_chapter_content_background_entry_with_default_wiring,
)
from app.services.chapter_generation_prerequisite_service import (
    check_chapter_generation_prerequisites,
)
from app.services.chapter_generation_stream_entry_service import (
    generate_chapter_content_stream_with_default_wiring,
)
from app.services.chapter_prompt_quality_compat_service import (
    build_chapter_runtime_system_prompt,
    compute_story_quality_metrics,
    detect_style_profile,
    resolve_generation_temperature,
)
from app.services.manual_chapter_analysis_execution_service import (
    execute_chapter_analysis_background,
)
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

logger = get_logger(__name__)


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
    return await _generate_best_ranked_candidate_entry(
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
        execution_callable=batch_generation_entry_compat_service.execute_batch_generation_in_order,
    )
