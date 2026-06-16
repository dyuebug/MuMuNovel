from __future__ import annotations

from typing import Any, Callable

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active single-generation stream wiring chain; this Python "
    "module is kept only as frozen rollback/source-map material after explicit "
    "stream shell freeze approval."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs"
SOURCE_MAP_ROLLBACK_FLAG = "legacy_single_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.services.chapter_generation.stream.request_policy_service import (
    _build_chapter_generation_request_options,
    _calculate_chapter_generation_max_tokens,
)


class _LazyServiceProxy:
    def __init__(self, module_name: str, attr_name: str):
        self._module_name = module_name
        self._attr_name = attr_name

    def _resolve(self):
        from importlib import import_module

        return getattr(import_module(self._module_name), self._attr_name)

    def __getattr__(self, name: str):
        return getattr(self._resolve(), name)


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
memory_service = _LazyServiceProxy("app.services.memory_service", "memory_service")
foreshadow_service = _LazyServiceProxy("app.services.foreshadow_service", "foreshadow_service")


class PromptService:
    @staticmethod
    async def get_template(*args, **kwargs):
        from app.services.prompt_service import PromptService as PromptServiceImpl

        return await PromptServiceImpl.get_template(*args, **kwargs)

    @staticmethod
    def format_prompt(*args, **kwargs):
        from app.services.prompt_service import PromptService as PromptServiceImpl

        return PromptServiceImpl.format_prompt(*args, **kwargs)


class WritingStyleManager:
    @staticmethod
    def apply_style_to_prompt(*args, **kwargs):
        from app.services.prompt_service import WritingStyleManager as WritingStyleManagerImpl

        return WritingStyleManagerImpl.apply_style_to_prompt(*args, **kwargs)


def build_chapter_generation_stream_result_payload(*args, **kwargs):
    from app.schemas.generation_payload import (
        build_chapter_generation_stream_result_payload as build_chapter_generation_stream_result_payload_service,
    )

    return build_chapter_generation_stream_result_payload_service(*args, **kwargs)


def create_analysis_task_safely(*args, **kwargs):
    from app.services.analysis_task_service import (
        create_analysis_task_safely as create_analysis_task_safely_service,
    )

    return create_analysis_task_safely_service(*args, **kwargs)


def _build_candidate_draft_payload(*args, **kwargs):
    from app.services.chapter_generation.history_service import (
        _build_candidate_draft_payload as build_candidate_draft_payload_service,
    )

    return build_candidate_draft_payload_service(*args, **kwargs)


def build_generation_history_payload(*args, **kwargs):
    from app.services.chapter_generation.history_service import (
        build_generation_history_payload as build_generation_history_payload_service,
    )

    return build_generation_history_payload_service(*args, **kwargs)


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


def build_chapter_generation_runtime_bundle(*args, **kwargs):
    from app.services.chapter_generation.runtime.service import (
        build_chapter_generation_runtime_bundle as build_chapter_generation_runtime_bundle_service,
    )

    return build_chapter_generation_runtime_bundle_service(*args, **kwargs)


def build_chapter_quality_runtime_context(*args, **kwargs):
    from app.services.chapter_generation.runtime.service import (
        build_chapter_quality_runtime_context as build_chapter_quality_runtime_context_service,
    )

    return build_chapter_quality_runtime_context_service(*args, **kwargs)


def build_chapter_generation_stream_execution_dependencies(*args, **kwargs):
    from app.services.chapter_generation.stream.models import (
        build_chapter_generation_stream_execution_dependencies as build_execution_dependencies,
    )

    return build_execution_dependencies(*args, **kwargs)


def build_chapter_generation_stream_candidate_dependencies(*args, **kwargs):
    from app.services.chapter_generation.stream.models import (
        build_chapter_generation_stream_candidate_dependencies as build_candidate_dependencies,
    )

    return build_candidate_dependencies(*args, **kwargs)


def build_chapter_generation_stream_finalize_dependencies(*args, **kwargs):
    from app.services.chapter_generation.stream.models import (
        build_chapter_generation_stream_finalize_dependencies as build_finalize_dependencies,
    )

    return build_finalize_dependencies(*args, **kwargs)


def build_chapter_generation_stream_dependencies(*args, **kwargs):
    from app.services.chapter_generation.stream.models import (
        build_chapter_generation_stream_dependencies as build_stream_dependencies,
    )

    return build_stream_dependencies(*args, **kwargs)


async def execute_chapter_analysis_background(**kwargs):
    from app.services.manual_chapter_analysis_execution_service import (
        execute_chapter_analysis_background as execute_chapter_analysis_background_service,
    )

    return await execute_chapter_analysis_background_service(**kwargs)


def build_outline_structure_runtime_sources(*args, **kwargs):
    from app.services.outline_runtime_source_service import (
        build_outline_structure_runtime_sources as build_outline_structure_runtime_sources_service,
    )

    return build_outline_structure_runtime_sources_service(*args, **kwargs)


def compute_story_quality_metrics(*args, **kwargs):
    from app.services.story_quality_feedback_service import (
        compute_story_quality_metrics as compute_story_quality_metrics_service,
    )

    return compute_story_quality_metrics_service(*args, **kwargs)


async def resolve_generation_story_repair_state_for_chapter(*args, **kwargs):
    from app.services.story_repair_payload_service import (
        resolve_generation_story_repair_state_for_chapter as resolve_story_repair_state_service,
    )

    return await resolve_story_repair_state_service(*args, **kwargs)


def resolve_quality_gate_execution_plan(*args, **kwargs):
    from app.services.story_repair_payload_service import (
        resolve_quality_gate_execution_plan as resolve_quality_gate_execution_plan_service,
    )

    return resolve_quality_gate_execution_plan_service(*args, **kwargs)


def attach_story_runtime_contract(*args, **kwargs):
    from app.services.story_runtime_serialization_service import (
        attach_story_runtime_contract as attach_story_runtime_contract_service,
    )

    return attach_story_runtime_contract_service(*args, **kwargs)


def _build_chapter_stream_draft_attempt(
    *,
    project_id: str,
    chapter_id: str,
    source: str,
    attempt_state: str,
    quality_gate_action: str | None,
    quality_gate_decision: str | None,
    full_content: str,
    summary_preview: str | None = None,
    quality_metrics: dict[str, Any] | None = None,
    repair_payload: dict[str, Any] | None = None,
    previous_content: str = '',
    previous_word_count: int = 0,
):
    from app.services.batch_generation_chapter_persistence_service import (
        build_batch_chapter_draft_attempt,
    )

    normalized_repair_payload = dict(repair_payload or {}) if isinstance(repair_payload, dict) else {}
    normalized_repair_payload.setdefault('previous_content', previous_content)
    normalized_repair_payload.setdefault('previous_word_count', previous_word_count)
    return build_batch_chapter_draft_attempt(
        project_id=project_id,
        chapter_id=chapter_id,
        source=source,
        attempt_state=attempt_state,
        quality_gate_action=quality_gate_action,
        quality_gate_decision=quality_gate_decision,
        full_content=full_content,
        summary_preview=summary_preview,
        quality_metrics=quality_metrics,
        repair_payload=normalized_repair_payload,
    )


def build_default_chapter_generation_stream_dependencies(
    *,
    cancel_outline_postprocess_tasks_fn: Callable[..., Any],
    candidate_generator_fn: Callable[..., Any],
    candidate_rerank_limit: int,
    one_to_one_builder_cls: Any = OneToOneContextBuilder,
    one_to_many_builder_cls: Any = OneToManyContextBuilder,
    get_template_fn: Callable[..., Any] = PromptService.get_template,
    format_prompt_fn: Callable[..., str] = PromptService.format_prompt,
    apply_style_to_prompt_fn: Callable[..., str] = WritingStyleManager.apply_style_to_prompt,
    build_runtime_system_prompt_fn: Callable[..., Any] = build_chapter_runtime_system_prompt,
    detect_style_profile_fn: Callable[..., Any] = detect_style_profile,
    resolve_generation_temperature_fn: Callable[..., Any] = resolve_generation_temperature,
    compute_story_quality_metrics_fn: Callable[..., Any] = compute_story_quality_metrics,
    resolve_quality_gate_execution_plan_fn: Callable[..., Any] = resolve_quality_gate_execution_plan,
    analyze_chapter_background_fn: Callable[..., Any] = execute_chapter_analysis_background,
):
    execution_dependencies = build_chapter_generation_stream_execution_dependencies(
        resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_chapter,
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks_fn,
        memory_service=memory_service,
        foreshadow_service=foreshadow_service,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources,
        build_generation_runtime_bundle_fn=build_chapter_generation_runtime_bundle,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=_calculate_chapter_generation_max_tokens,
        build_request_options_fn=_build_chapter_generation_request_options,
        detect_style_profile_fn=detect_style_profile_fn,
        resolve_generation_temperature_fn=resolve_generation_temperature_fn,
    )
    candidate_dependencies = build_chapter_generation_stream_candidate_dependencies(
        build_quality_runtime_context_fn=build_chapter_quality_runtime_context,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_rerank_limit=candidate_rerank_limit,
        candidate_generator_fn=candidate_generator_fn,
        build_draft_attempt_fn=_build_chapter_stream_draft_attempt,
        attach_story_runtime_contract_fn=attach_story_runtime_contract,
    )
    finalize_dependencies = build_chapter_generation_stream_finalize_dependencies(
        foreshadow_service=foreshadow_service,
        build_generation_history_payload_fn=build_generation_history_payload,
        create_analysis_task_fn=create_analysis_task_safely,
        analyze_chapter_background_fn=analyze_chapter_background_fn,
        build_candidate_draft_payload_fn=_build_candidate_draft_payload,
        build_stream_result_payload_fn=build_chapter_generation_stream_result_payload,
    )
    return build_chapter_generation_stream_dependencies(
        execution=execution_dependencies,
        candidate=candidate_dependencies,
        finalize=finalize_dependencies,
    )
