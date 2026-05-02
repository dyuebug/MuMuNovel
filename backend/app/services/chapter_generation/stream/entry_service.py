from __future__ import annotations

from typing import Any, Callable

from fastapi import BackgroundTasks, HTTPException, Request

from app.schemas.chapter import ChapterGenerateRequest
from app.services.ai_service import AIService
from app.services.prompt_service import PromptService, WritingStyleManager
from app.services.chapter_generation.runtime.prompt_service import (
    build_chapter_runtime_system_prompt,
    detect_style_profile,
    resolve_generation_temperature,
)
from app.services.manual_chapter_analysis_execution_service import (
    execute_chapter_analysis_background,
)
from app.services.story_quality_feedback_service import compute_story_quality_metrics
from app.services.story_repair_payload_service import resolve_quality_gate_execution_plan
from app.services.chapter_generation.stream.service import (
    build_chapter_generation_event_stream,
    prepare_chapter_generation_stream_request,
)
from app.services.chapter_generation.stream.wiring_service import (
    build_default_chapter_generation_stream_dependencies,
)
from app.utils.sse_response import create_sse_response


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
            await prepare_chapter_generation_stream_request(
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

    stream_dependencies = build_default_chapter_generation_stream_dependencies(
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
        # Keep compat monkeypatch reachable from tests by resolving at call time.
        analyze_chapter_background_fn=__import__(
            "app.services.chapter_generation_route_compat_service",
            fromlist=["execute_chapter_analysis_background"],
        ).execute_chapter_analysis_background,
    )

    current_user_id = getattr(request.state, 'user_id', 'system')
    return create_sse_response(
        build_chapter_generation_event_stream(
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
