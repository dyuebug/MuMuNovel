from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
import re
from typing import Any

from fastapi import BackgroundTasks, HTTPException, Request

from tests.test_support.prompt_template_facade_test_support import (
    format_prompt as _facade_format_prompt,
    get_template_for_owner,
)
from tests.test_support.chapter_schema_test_support import ChapterGenerateRequest
from tests.test_support.chapter_generation_stream_types import (
    ChapterGenerationStreamCandidateDependencies,
    ChapterGenerationStreamDependencies,
    ChapterGenerationStreamExecutionDependencies,
    ChapterGenerationStreamFinalizeDependencies,
)

CHAPTER_CANDIDATE_RERANK_LIMIT = 2
CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0
_PROMPT_SERVICE_SOURCE_PATH = (
    Path(__file__).resolve().parent / "prompt_service_test_support.py"
)


@dataclass(frozen=True)
class ChapterGenerationStreamPreparation:
    chapter: Any
    previous_chapters_data: list[dict[str, str | int | None]]


def build_chapter_generation_stream_execution_dependencies(
    *,
    dependency_types,
    resolve_story_repair_state_fn,
    cancel_outline_postprocess_tasks_fn,
    memory_service: Any,
    foreshadow_service: Any,
    one_to_one_builder_cls: Any,
    one_to_many_builder_cls: Any,
    build_outline_structure_runtime_sources_fn,
    build_generation_runtime_bundle_fn,
    get_template_fn,
    format_prompt_fn,
    apply_style_to_prompt_fn,
    build_runtime_system_prompt_fn,
    calculate_max_tokens_fn,
    build_request_options_fn,
    detect_style_profile_fn,
    resolve_generation_temperature_fn,
):
    return dependency_types.execution(
        resolve_story_repair_state_fn=resolve_story_repair_state_fn,
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks_fn,
        memory_service=memory_service,
        foreshadow_service=foreshadow_service,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources_fn,
        build_generation_runtime_bundle_fn=build_generation_runtime_bundle_fn,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=calculate_max_tokens_fn,
        build_request_options_fn=build_request_options_fn,
        detect_style_profile_fn=detect_style_profile_fn,
        resolve_generation_temperature_fn=resolve_generation_temperature_fn,
    )


def build_chapter_generation_stream_candidate_dependencies(
    *,
    dependency_types,
    build_quality_runtime_context_fn,
    compute_story_quality_metrics_fn,
    resolve_quality_gate_execution_plan_fn,
    candidate_rerank_limit: int,
    candidate_generator_fn,
    build_draft_attempt_fn,
    attach_story_runtime_contract_fn,
):
    return dependency_types.candidate(
        build_quality_runtime_context_fn=build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_rerank_limit=candidate_rerank_limit,
        candidate_generator_fn=candidate_generator_fn,
        build_draft_attempt_fn=build_draft_attempt_fn,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
    )


def build_chapter_generation_stream_finalize_dependencies(
    *,
    dependency_types,
    foreshadow_service: Any,
    build_generation_history_payload_fn,
    create_analysis_task_fn,
    analyze_chapter_background_fn,
    build_candidate_draft_payload_fn,
    build_stream_result_payload_fn,
):
    return dependency_types.finalize(
        foreshadow_service=foreshadow_service,
        build_generation_history_payload_fn=build_generation_history_payload_fn,
        create_analysis_task_fn=create_analysis_task_fn,
        analyze_chapter_background_fn=analyze_chapter_background_fn,
        build_candidate_draft_payload_fn=build_candidate_draft_payload_fn,
        build_stream_result_payload_fn=build_stream_result_payload_fn,
    )


def build_chapter_generation_stream_dependencies(
    *,
    dependency_types,
    execution,
    candidate,
    finalize,
):
    return dependency_types.root(
        execution=execution,
        candidate=candidate,
        finalize=finalize,
    )


def get_db(*args, **kwargs):
    from tests.test_support.database_test_support import get_db as impl

    return impl(*args, **kwargs)


async def check_chapter_generation_prerequisites(*args, **kwargs):
    from tests.test_support.chapter_query_test_support import (
        check_chapter_generation_prerequisites as impl,
    )

    return await impl(*args, **kwargs)


def serialize_previous_chapters(
    previous_chapters,
) -> list[dict[str, str | int | None]]:
    return [
        {
            "id": chapter.id,
            "chapter_number": chapter.chapter_number,
            "title": chapter.title,
            "content": chapter.content,
        }
        for chapter in previous_chapters
    ]


async def prepare_chapter_generation_stream_request(
    db_session,
    *,
    chapter_id: str,
    check_prerequisites_fn,
    load_chapter_fn=None,
) -> ChapterGenerationStreamPreparation:
    if load_chapter_fn is None:
        load_chapter_fn = _load_chapter_generation_target

    chapter = await load_chapter_fn(db_session, chapter_id)
    if chapter is None:
        raise ValueError("章节不存在")

    can_generate, error_msg, previous_chapters = await check_prerequisites_fn(
        db_session,
        chapter,
    )
    if not can_generate:
        raise RuntimeError(error_msg)

    return ChapterGenerationStreamPreparation(
        chapter=chapter,
        previous_chapters_data=serialize_previous_chapters(previous_chapters),
    )


async def _load_chapter_generation_target(db_session, chapter_id: str):
    from sqlalchemy import select

    from migrator_app.models.chapter import Chapter

    result = await db_session.execute(
        select(Chapter).where(Chapter.id == chapter_id)
    )
    return result.scalar_one_or_none()


class _LazyOneToOneContextBuilder:
    def __new__(cls, *args, **kwargs):
        from tests.test_support.chapter_context_test_support import OneToOneContextBuilder

        return OneToOneContextBuilder(*args, **kwargs)


class _LazyOneToManyContextBuilder:
    def __new__(cls, *args, **kwargs):
        from tests.test_support.chapter_context_test_support import OneToManyContextBuilder

        return OneToManyContextBuilder(*args, **kwargs)


OneToOneContextBuilder = _LazyOneToOneContextBuilder
OneToManyContextBuilder = _LazyOneToManyContextBuilder


@lru_cache(maxsize=1)
def _load_single_generation_prompt_template_map() -> dict[str, str]:
    source = _PROMPT_SERVICE_SOURCE_PATH.read_text(encoding="utf-8")
    template_keys = (
        "CHAPTER_GENERATION_ONE_TO_ONE",
        "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
        "CHAPTER_GENERATION_ONE_TO_MANY",
        "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
    )
    templates: dict[str, str] = {}
    for template_key in template_keys:
        match = re.search(
            rf'^\s*{template_key}\s*=\s*"""(.*?)"""',
            source,
            flags=re.MULTILINE | re.DOTALL,
        )
        if not match:
            raise RuntimeError(
                f"single generation stream test adapter 未找到模板常量: {template_key}"
            )
        templates[template_key] = match.group(1)
    return templates


def _single_generation_template_lookup(template_key: str) -> str | None:
    return _load_single_generation_prompt_template_map().get(template_key)


class PromptService:
    CHAPTER_GENERATION_ONE_TO_ONE = "CHAPTER_GENERATION_ONE_TO_ONE"
    CHAPTER_GENERATION_ONE_TO_ONE_NEXT = "CHAPTER_GENERATION_ONE_TO_ONE_NEXT"
    CHAPTER_GENERATION_ONE_TO_MANY = "CHAPTER_GENERATION_ONE_TO_MANY"
    CHAPTER_GENERATION_ONE_TO_MANY_NEXT = "CHAPTER_GENERATION_ONE_TO_MANY_NEXT"

    @staticmethod
    async def get_template(*args, **kwargs):
        return await get_template_for_owner(
            *args,
            template_lookup=_single_generation_template_lookup,
            **kwargs,
        )

    @staticmethod
    def format_prompt(template: str, **kwargs) -> str:
        return _facade_format_prompt(template, **kwargs)


def get_template(*args, **kwargs):
    return get_template_for_owner(
        *args,
        template_lookup=_single_generation_template_lookup,
        **kwargs,
    )


def format_prompt(*args, **kwargs):
    return _facade_format_prompt(*args, **kwargs)


def apply_style_to_prompt(*args, **kwargs):
    from tests.test_support.story_writing_style_test_support import WritingStyleManager

    return WritingStyleManager.apply_style_to_prompt(*args, **kwargs)


def build_chapter_runtime_system_prompt(*args, **kwargs):
    from tests.test_support.chapter_generation_runtime_prompt_test_support import (
        build_chapter_runtime_system_prompt as impl,
    )

    return impl(*args, **kwargs)


def detect_style_profile(*args, **kwargs):
    from tests.test_support.schemas.novel_quality_rules import (
        detect_style_profile as impl,
    )

    return impl(*args, **kwargs)


def resolve_generation_temperature(*args, **kwargs):
    from tests.test_support.chapter_generation_runtime_prompt_test_support import (
        resolve_generation_temperature as impl,
    )

    return impl(*args, **kwargs)


def compute_story_quality_metrics(*args, **kwargs):
    from tests.test_support.story_quality_metrics_aggregation_test_support import (
        compute_story_quality_metrics as impl,
    )

    return impl(*args, **kwargs)


def resolve_quality_gate_execution_plan(*args, **kwargs):
    from tests.test_support.story_repair_payload_test_support import (
        resolve_quality_gate_execution_plan as impl,
    )

    return impl(*args, **kwargs)


async def execute_chapter_analysis_background(*args, **kwargs):
    from tests.test_support.manual_chapter_analysis_execution_test_support import (
        execute_chapter_analysis_background as impl,
    )

    return await impl(*args, **kwargs)


async def _generate_best_ranked_candidate(
    *,
    ai_service,
    base_generate_kwargs,
    target_word_count: int,
    source: str,
    generation_label: str,
    quality_evaluator,
    quality_gate_plan_builder,
    max_candidates: int = CHAPTER_CANDIDATE_RERANK_LIMIT,
    runtime_state=None,
):
    from tests.test_support.chapter_candidate_executor_test_support import (
        generate_best_ranked_candidate_with_default_wiring,
    )

    return await generate_best_ranked_candidate_with_default_wiring(
        ai_service=ai_service,
        base_generate_kwargs=base_generate_kwargs,
        target_word_count=target_word_count,
        source=source,
        generation_label=generation_label,
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
        max_candidates=max_candidates,
        runtime_state=runtime_state,
    )


def build_default_chapter_generation_stream_dependencies(
    *,
    dependency_types=None,
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
    resolve_story_repair_state_fn=None,
    memory_service=None,
    foreshadow_service=None,
    build_outline_structure_runtime_sources_fn=None,
    build_generation_runtime_bundle_fn=None,
    calculate_max_tokens_fn=None,
    build_request_options_fn=None,
    build_quality_runtime_context_fn=None,
    build_draft_attempt_fn=None,
    attach_story_runtime_contract_fn=None,
    build_generation_history_payload_fn=None,
    create_analysis_task_fn=None,
    build_candidate_draft_payload_fn=None,
    build_stream_result_payload_fn=None,
):
    from tests.test_support.schemas.generation_payload import (
        build_chapter_generation_stream_result_payload,
    )
    from tests.test_support.chapter_generation_history_test_support import (
        _build_candidate_draft_payload,
        build_generation_history_payload,
    )
    from tests.test_support.story_packet_test_support import (
        build_chapter_generation_runtime_bundle,
        build_chapter_quality_runtime_context,
    )
    from tests.test_support.batch_generation_single_chapter_wiring_test_adapter import (
        _build_chapter_generation_request_options,
        _calculate_chapter_generation_max_tokens,
    )
    from tests.test_support.analysis_task_test_support import (
        create_analysis_task_safely,
    )
    from tests.test_support.single_generation_stream_candidate_test_adapter import (
        build_chapter_stream_draft_attempt,
    )
    from tests.test_support.foreshadow_test_support import foreshadow_service as default_foreshadow_service
    from tests.test_support.memory_service_test_support import memory_service as default_memory_service
    from tests.test_support.outline_runtime_source_test_support import (
        build_outline_structure_runtime_sources,
    )
    from tests.test_support.story_repair_payload_test_support import (
        resolve_generation_story_repair_state_for_chapter,
    )
    from tests.test_support.schemas.generation_payload import (
        attach_story_runtime_contract,
    )

    if dependency_types is None:
        dependency_types = type(
            "_StreamDependencyTypes",
            (),
            {
                "execution": ChapterGenerationStreamExecutionDependencies,
                "candidate": ChapterGenerationStreamCandidateDependencies,
                "finalize": ChapterGenerationStreamFinalizeDependencies,
                "root": ChapterGenerationStreamDependencies,
            },
        )
    if resolve_story_repair_state_fn is None:
        resolve_story_repair_state_fn = resolve_generation_story_repair_state_for_chapter
    resolved_memory_service = (
        default_memory_service if memory_service is None else memory_service
    )
    resolved_foreshadow_service = (
        default_foreshadow_service
        if foreshadow_service is None
        else foreshadow_service
    )
    resolved_outline_runtime_sources_fn = (
        build_outline_structure_runtime_sources
        if build_outline_structure_runtime_sources_fn is None
        else build_outline_structure_runtime_sources_fn
    )
    resolved_generation_runtime_bundle_fn = (
        build_chapter_generation_runtime_bundle
        if build_generation_runtime_bundle_fn is None
        else build_generation_runtime_bundle_fn
    )
    resolved_calculate_max_tokens_fn = (
        _calculate_chapter_generation_max_tokens
        if calculate_max_tokens_fn is None
        else calculate_max_tokens_fn
    )
    resolved_build_request_options_fn = (
        _build_chapter_generation_request_options
        if build_request_options_fn is None
        else build_request_options_fn
    )
    resolved_build_quality_runtime_context_fn = (
        build_chapter_quality_runtime_context
        if build_quality_runtime_context_fn is None
        else build_quality_runtime_context_fn
    )
    resolved_build_draft_attempt_fn = (
        build_chapter_stream_draft_attempt
        if build_draft_attempt_fn is None
        else build_draft_attempt_fn
    )
    resolved_attach_story_runtime_contract_fn = (
        attach_story_runtime_contract
        if attach_story_runtime_contract_fn is None
        else attach_story_runtime_contract_fn
    )
    resolved_build_generation_history_payload_fn = (
        build_generation_history_payload
        if build_generation_history_payload_fn is None
        else build_generation_history_payload_fn
    )
    resolved_create_analysis_task_fn = (
        create_analysis_task_safely
        if create_analysis_task_fn is None
        else create_analysis_task_fn
    )
    resolved_build_candidate_draft_payload_fn = (
        _build_candidate_draft_payload
        if build_candidate_draft_payload_fn is None
        else build_candidate_draft_payload_fn
    )
    resolved_build_stream_result_payload_fn = (
        build_chapter_generation_stream_result_payload
        if build_stream_result_payload_fn is None
        else build_stream_result_payload_fn
    )

    execution_dependencies = build_chapter_generation_stream_execution_dependencies(
        dependency_types=dependency_types,
        resolve_story_repair_state_fn=resolve_story_repair_state_fn,
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks_fn,
        memory_service=resolved_memory_service,
        foreshadow_service=resolved_foreshadow_service,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=resolved_outline_runtime_sources_fn,
        build_generation_runtime_bundle_fn=resolved_generation_runtime_bundle_fn,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=resolved_calculate_max_tokens_fn,
        build_request_options_fn=resolved_build_request_options_fn,
        detect_style_profile_fn=detect_style_profile_fn,
        resolve_generation_temperature_fn=resolve_generation_temperature_fn,
    )
    candidate_dependencies = build_chapter_generation_stream_candidate_dependencies(
        dependency_types=dependency_types,
        build_quality_runtime_context_fn=resolved_build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_rerank_limit=candidate_rerank_limit,
        candidate_generator_fn=candidate_generator_fn,
        build_draft_attempt_fn=resolved_build_draft_attempt_fn,
        attach_story_runtime_contract_fn=resolved_attach_story_runtime_contract_fn,
    )
    finalize_dependencies = build_chapter_generation_stream_finalize_dependencies(
        dependency_types=dependency_types,
        foreshadow_service=resolved_foreshadow_service,
        build_generation_history_payload_fn=resolved_build_generation_history_payload_fn,
        create_analysis_task_fn=resolved_create_analysis_task_fn,
        analyze_chapter_background_fn=analyze_chapter_background_fn,
        build_candidate_draft_payload_fn=resolved_build_candidate_draft_payload_fn,
        build_stream_result_payload_fn=resolved_build_stream_result_payload_fn,
    )
    return build_chapter_generation_stream_dependencies(
        dependency_types=dependency_types,
        execution=execution_dependencies,
        candidate=candidate_dependencies,
        finalize=finalize_dependencies,
    )


async def build_chapter_generation_event_stream(*args, **kwargs):
    from tests.test_support.single_generation_stream_orchestration_test_adapter import (
        build_chapter_generation_event_stream_with_default_wiring,
    )

    async for payload in build_chapter_generation_event_stream_with_default_wiring(
        *args,
        **kwargs,
    ):
        yield payload


async def generate_chapter_content_stream_with_explicit_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest,
    user_ai_service: Any,
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
    enable_analysis = bool(getattr(generate_request, "enable_analysis", False))
    custom_model = generate_request.model if hasattr(generate_request, "model") else None
    temp_narrative_perspective = (
        generate_request.narrative_perspective
        if hasattr(generate_request, "narrative_perspective")
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

    current_user_id = getattr(request.state, "user_id", "system")
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


async def generate_chapter_content_stream_with_default_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest,
    user_ai_service: Any,
):
    from tests.test_support.outlines_route_test_adapter import (
        cancel_outline_postprocess_tasks,
    )
    from tests.test_support.utils.sse_response import create_sse_response

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





