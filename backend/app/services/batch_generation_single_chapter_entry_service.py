"""批量生成单章执行入口冻结 shim。

该文件保留给 rollback/source-map 和测试 patch surface 使用，
实际 owner 已下沉到 execution / wiring 层。
"""
from __future__ import annotations

import sys
from typing import TYPE_CHECKING, Any, Dict, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch single-chapter execution path; this Python "
    "entry module is kept only as frozen rollback/source-map material for "
    "legacy batch fallback execution and tests."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs; "
    "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from asyncio import Lock
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.chapter import Chapter
    from app.services.ai_service import AIService
    from app.services.chapter_quality_context_service import StoryPacket
    from app.services.story_repair_payload_service import StoryRepairPayload


CHAPTER_CANDIDATE_RERANK_LIMIT = 2
CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0


def _wiring_service():
    from app.services import batch_generation_single_chapter_wiring_service

    return batch_generation_single_chapter_wiring_service


def _chapters_api_module():
    return sys.modules.get("app.api.chapters")


class PromptService:
    @staticmethod
    async def get_template(*args, **kwargs):
        from app.services.prompt_service import PromptService as PromptServiceImpl

        return await PromptServiceImpl.get_template(*args, **kwargs)

    @staticmethod
    def format_prompt(*args, **kwargs):
        from app.services.prompt_service import PromptService as PromptServiceImpl

        return PromptServiceImpl.format_prompt(*args, **kwargs)


_ORIGINAL_PROMPT_GET_TEMPLATE = PromptService.get_template
_ORIGINAL_PROMPT_FORMAT = PromptService.format_prompt


class _LazyChapterWebResearchService:
    def is_enabled(self, *args, **kwargs):
        from app.services.chapter_web_research_service import chapter_web_research_service

        return chapter_web_research_service.is_enabled(*args, **kwargs)

    async def collect_for_chapter(self, *args, **kwargs):
        from app.services.chapter_web_research_service import chapter_web_research_service

        return await chapter_web_research_service.collect_for_chapter(*args, **kwargs)

    async def replace_chapter_memories(self, *args, **kwargs):
        from app.services.chapter_web_research_service import chapter_web_research_service

        return await chapter_web_research_service.replace_chapter_memories(*args, **kwargs)


chapter_web_research_service = _LazyChapterWebResearchService()


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


def compute_story_quality_metrics(*args, **kwargs):
    from app.services.story_quality_feedback_service import compute_story_quality_metrics

    return compute_story_quality_metrics(*args, **kwargs)


async def resolve_chapter_quality_profile(*args, **kwargs):
    from app.services.chapter_quality_context_service import resolve_chapter_quality_profile

    return await resolve_chapter_quality_profile(*args, **kwargs)


def resolve_quality_gate_execution_plan(*args, **kwargs):
    from app.services.story_repair_payload_service import (
        resolve_quality_gate_execution_plan as resolve_quality_gate_execution_plan_impl,
    )

    return resolve_quality_gate_execution_plan_impl(*args, **kwargs)


async def publish_task_stream_event_service(*args, **kwargs):
    from app.services.task_workflow_runtime_service import publish_task_stream_event

    return await publish_task_stream_event(*args, **kwargs)


_ORIGINAL_RESOLVE_CHAPTER_QUALITY_PROFILE = resolve_chapter_quality_profile
_ORIGINAL_BUILD_RUNTIME_SYSTEM_PROMPT = build_chapter_runtime_system_prompt
_ORIGINAL_COMPUTE_STORY_QUALITY_METRICS = compute_story_quality_metrics
_ORIGINAL_RESOLVE_QUALITY_GATE_EXECUTION_PLAN = resolve_quality_gate_execution_plan


def _prefer_entry_override(
    current_value,
    original_value,
    route_value=None,
):
    if current_value is not original_value:
        return current_value
    if route_value is not None:
        return route_value
    return current_value


def _is_original_chapters_api_compute_story_quality_metrics(candidate: Any) -> bool:
    return (
        getattr(candidate, "__module__", "") == "app.api.chapters"
        and getattr(candidate, "__name__", "") == "compute_story_quality_metrics"
    )


def _build_default_dependencies(**overrides):
    chapters_api = _chapters_api_module()
    route_quality_profile_fn = (
        getattr(chapters_api, "resolve_chapter_quality_profile", None)
        if chapters_api is not None
        else None
    )
    route_prompt_service = (
        getattr(chapters_api, "PromptService", None)
        if chapters_api is not None
        else None
    )
    route_runtime_prompt_fn = (
        getattr(chapters_api, "_build_chapter_runtime_system_prompt", None)
        if chapters_api is not None
        else None
    )
    route_quality_gate_plan_fn = (
        getattr(chapters_api, "_resolve_quality_gate_execution_plan", None)
        if chapters_api is not None
        else None
    )
    route_quality_metrics_fn = (
        getattr(chapters_api, "compute_story_quality_metrics", None)
        if chapters_api is not None
        else None
    )
    prompt_get_template_fn = _prefer_entry_override(
        PromptService.get_template,
        _ORIGINAL_PROMPT_GET_TEMPLATE,
        (
            route_prompt_service.get_template
            if route_prompt_service is not None
            else None
        ),
    )
    prompt_format_fn = _prefer_entry_override(
        PromptService.format_prompt,
        _ORIGINAL_PROMPT_FORMAT,
        (
            route_prompt_service.format_prompt
            if route_prompt_service is not None
            else None
        ),
    )
    resolve_quality_profile_fn = _prefer_entry_override(
        resolve_chapter_quality_profile,
        _ORIGINAL_RESOLVE_CHAPTER_QUALITY_PROFILE,
        route_quality_profile_fn,
    )
    build_runtime_system_prompt_fn = _prefer_entry_override(
        build_chapter_runtime_system_prompt,
        _ORIGINAL_BUILD_RUNTIME_SYSTEM_PROMPT,
        route_runtime_prompt_fn,
    )
    compute_story_quality_metrics_fn = _prefer_entry_override(
        compute_story_quality_metrics,
        _ORIGINAL_COMPUTE_STORY_QUALITY_METRICS,
        (
            route_quality_metrics_fn
            if route_quality_metrics_fn is not None
            and not _is_original_chapters_api_compute_story_quality_metrics(
                route_quality_metrics_fn
            )
            else None
        ),
    )
    resolve_quality_gate_execution_plan_fn = _prefer_entry_override(
        resolve_quality_gate_execution_plan,
        _ORIGINAL_RESOLVE_QUALITY_GATE_EXECUTION_PLAN,
        route_quality_gate_plan_fn,
    )
    return _wiring_service().build_default_batch_generation_single_chapter_dependencies(
        candidate_generator_fn=overrides.pop("candidate_generator_fn", _generate_best_ranked_candidate),
        default_candidate_limit=overrides.pop("default_candidate_limit", CHAPTER_CANDIDATE_RERANK_LIMIT),
        heartbeat_interval_seconds=overrides.pop(
            "heartbeat_interval_seconds",
            CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS,
        ),
        chapter_web_research_service=overrides.pop(
            "chapter_web_research_service",
            chapter_web_research_service,
        ),
        publish_task_stream_event_fn=overrides.pop(
            "publish_task_stream_event_fn",
            publish_task_stream_event_service,
        ),
        resolve_quality_profile_fn=overrides.pop(
            "resolve_quality_profile_fn",
            resolve_quality_profile_fn,
        ),
        one_to_one_builder_cls=overrides.pop(
            "one_to_one_builder_cls",
            OneToOneContextBuilder,
        ),
        one_to_many_builder_cls=overrides.pop(
            "one_to_many_builder_cls",
            OneToManyContextBuilder,
        ),
        get_template_fn=overrides.pop("get_template_fn", prompt_get_template_fn),
        format_prompt_fn=overrides.pop("format_prompt_fn", prompt_format_fn),
        build_runtime_system_prompt_fn=overrides.pop(
            "build_runtime_system_prompt_fn",
            build_runtime_system_prompt_fn,
        ),
        compute_story_quality_metrics_fn=overrides.pop(
            "compute_story_quality_metrics_fn",
            compute_story_quality_metrics_fn,
        ),
        resolve_quality_gate_execution_plan_fn=overrides.pop(
            "resolve_quality_gate_execution_plan_fn",
            resolve_quality_gate_execution_plan_fn,
        ),
        **overrides,
    )


def _collect_generation_candidate_output(*args, **kwargs):
    return _wiring_service().collect_batch_single_chapter_generation_candidate_output(
        *args,
        **kwargs,
    )


def _resolve_generation_attempt_labels(*args, **kwargs):
    return _wiring_service().resolve_batch_single_chapter_generation_attempt_labels(
        *args,
        **kwargs,
    )


def _sync_generation_runtime_state(*args, **kwargs):
    return _wiring_service().sync_batch_single_chapter_generation_runtime_state(
        *args,
        **kwargs,
    )


def _build_generation_candidate_record(*args, **kwargs):
    return _wiring_service().build_batch_single_chapter_generation_candidate_record(
        *args,
        **kwargs,
    )


def _build_generation_candidate_record_with_default_logging(**kwargs):
    return _wiring_service().build_batch_single_chapter_generation_candidate_record_with_default_logging(
        **kwargs,
    )


async def _generate_best_ranked_candidate(*args, **kwargs):
    return await _wiring_service().generate_best_ranked_batch_single_chapter_candidate(
        *args,
        **kwargs,
    )


async def generate_single_chapter_for_batch(
    db_session: AsyncSession,
    chapter: Chapter,
    user_id: str,
    style_id: Optional[int],
    target_word_count: int,
    ai_service: AIService,
    write_lock: Lock,
    story_packet: Optional[StoryPacket] = None,
    base_quality_profile: Optional[Dict[str, Any]] = None,
    custom_model: Optional[str] = None,
    previous_summary_context: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    enable_web_research: Optional[bool] = None,
    web_research_query: Optional[str] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional[Dict[str, Any]] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    stream_task_id: Optional[str] = None,
    stream_chunks: bool = False,
    retry_count: int = 0,
    max_retries: int = 1,
) -> Dict[str, Any]:
    workflow_request = _wiring_service().build_batch_generation_single_chapter_request(
        db_session=db_session,
        chapter=chapter,
        user_id=user_id,
        style_id=style_id,
        target_word_count=target_word_count,
        ai_service=ai_service,
        write_lock=write_lock,
        story_packet=story_packet,
        base_quality_profile=base_quality_profile,
        custom_model=custom_model,
        previous_summary_context=previous_summary_context,
        temp_narrative_perspective=temp_narrative_perspective,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        enable_web_research=enable_web_research,
        web_research_query=web_research_query,
        story_repair_summary=story_repair_summary,
        story_repair_targets=story_repair_targets,
        story_preserve_strengths=story_preserve_strengths,
        story_repair_payload=story_repair_payload,
        active_story_repair_snapshot=active_story_repair_snapshot,
        story_repair_state=story_repair_state,
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        retry_count=retry_count,
        max_retries=max_retries,
    )
    workflow_dependencies = _build_default_dependencies()
    return await _wiring_service().generate_single_chapter_for_batch_workflow(
        request=workflow_request,
        dependencies=workflow_dependencies,
    )
