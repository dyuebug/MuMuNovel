from __future__ import annotations

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active single-generation stream entry chain; this Python "
    "module is kept only as frozen rollback/source-map material after explicit "
    "stream shell freeze approval."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_generation_routes.rs; "
    "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_single_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from typing import Any, Callable

from fastapi import BackgroundTasks, Request

from app.schemas.chapter import ChapterGenerateRequest
from app.services.ai_service import AIService
from app.services.prompt_service import PromptService, WritingStyleManager
from app.services.story_quality_feedback_service import compute_story_quality_metrics


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


async def execute_chapter_analysis_background(**kwargs):
    from app.services.manual_chapter_analysis_execution_service import (
        execute_chapter_analysis_background as execute_chapter_analysis_background_service,
    )

    return await execute_chapter_analysis_background_service(**kwargs)


def resolve_quality_gate_execution_plan(*args, **kwargs):
    from app.services.story_repair_payload_service import (
        resolve_quality_gate_execution_plan as resolve_quality_gate_execution_plan_service,
    )

    return resolve_quality_gate_execution_plan_service(*args, **kwargs)


def build_default_chapter_generation_stream_dependencies(*args, **kwargs):
    from app.services.chapter_generation.stream.wiring_service import (
        build_default_chapter_generation_stream_dependencies as build_default_chapter_generation_stream_dependencies_service,
    )

    return build_default_chapter_generation_stream_dependencies_service(*args, **kwargs)


async def prepare_chapter_generation_stream_request(*args, **kwargs):
    from app.services.chapter_generation.stream.service import (
        prepare_chapter_generation_stream_request as prepare_chapter_generation_stream_request_service,
    )

    return await prepare_chapter_generation_stream_request_service(*args, **kwargs)


def build_chapter_generation_event_stream(*args, **kwargs):
    from app.services.chapter_generation.stream.service import (
        build_chapter_generation_event_stream as build_chapter_generation_event_stream_service,
    )

    return build_chapter_generation_event_stream_service(*args, **kwargs)


def create_sse_response(*args, **kwargs):
    from app.utils.sse_response import create_sse_response as create_sse_response_service

    return create_sse_response_service(*args, **kwargs)


async def generate_chapter_content_stream_with_default_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest,
    user_ai_service: AIService,
    get_db_fn: Callable[..., Any],
    check_prerequisites_fn: Callable[..., Any],
    cancel_outline_postprocess_tasks_fn: Callable[..., Any],
    candidate_generator_fn: Callable[..., Any],
    candidate_rerank_limit: int,
    one_to_one_builder_cls: Any,
    one_to_many_builder_cls: Any,
    get_template_fn: Callable[..., Any] = PromptService.get_template,
    format_prompt_fn: Callable[..., str] = PromptService.format_prompt,
    apply_style_to_prompt_fn: Callable[..., str] = WritingStyleManager.apply_style_to_prompt,
    build_runtime_system_prompt_fn: Callable[..., Any] = build_chapter_runtime_system_prompt,
    detect_style_profile_fn: Callable[..., Any] = detect_style_profile,
    resolve_generation_temperature_fn: Callable[..., Any] = resolve_generation_temperature,
    compute_story_quality_metrics_fn: Callable[..., Any] = compute_story_quality_metrics,
    resolve_quality_gate_execution_plan_fn: Callable[..., Any] = resolve_quality_gate_execution_plan,
    analyze_chapter_background_fn: Callable[..., Any] = execute_chapter_analysis_background,
    heartbeat_interval_seconds: float = 10.0,
):
    from app.services.chapter_generation import route_wiring_service

    return await route_wiring_service.generate_chapter_content_stream_with_explicit_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        generate_request=generate_request,
        user_ai_service=user_ai_service,
        get_db_fn=get_db_fn,
        check_prerequisites_fn=check_prerequisites_fn,
        build_default_stream_dependencies_fn=build_default_chapter_generation_stream_dependencies,
        prepare_stream_request_fn=prepare_chapter_generation_stream_request,
        build_event_stream_fn=build_chapter_generation_event_stream,
        create_sse_response_fn=create_sse_response,
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
        heartbeat_interval_seconds=heartbeat_interval_seconds,
    )
