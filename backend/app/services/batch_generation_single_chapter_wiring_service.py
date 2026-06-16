from __future__ import annotations

from asyncio import Lock
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Callable, Dict, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation route/read/runtime chain and this "
    "default-import wiring file is retained only as frozen "
    "rollback/source-map material after the batch retired-wiring closeout "
    "review."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/health.rs; "
    "backend-rs/src/services/chapter_single_generation_prepare_service/research_payload_owner.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.chapter import Chapter
    from app.services.ai_service import AIService
    from app.services.chapter_quality_context_service import StoryPacket
    from app.services.story_repair_payload_service import StoryRepairPayload

CHAPTER_CANDIDATE_RERANK_LIMIT = 2
CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0


def _batch_generation_runtime_service():
    from app.services import batch_generation_runtime_service

    return batch_generation_runtime_service


def _batch_generation_candidate_service():
    from app.services import batch_generation_candidate_service

    return batch_generation_candidate_service


def _batch_generation_prompt_service():
    from app.services import batch_generation_prompt_service

    return batch_generation_prompt_service


def _chapter_generation_runtime_prompt_service():
    from app.services.chapter_generation.runtime import prompt_service

    return prompt_service


def _chapter_generation_runtime_service():
    from app.services.chapter_generation.runtime import service

    return service


def _chapter_generation_stream_request_policy_service():
    from app.services.chapter_generation.stream import request_policy_service

    return request_policy_service


def _chapter_quality_context_service():
    from app.services import chapter_quality_context_service

    return chapter_quality_context_service


def _foreshadow_service_instance():
    from app.services.foreshadow_service import foreshadow_service

    return foreshadow_service


def _memory_service_instance():
    from app.services.memory_service import memory_service

    return memory_service


def _outline_runtime_source_service():
    from app.services import outline_runtime_source_service

    return outline_runtime_source_service


def _story_runtime_serialization_service():
    from app.services import story_runtime_serialization_service

    return story_runtime_serialization_service


def _task_workflow_runtime_service():
    from app.services import task_workflow_runtime_service

    return task_workflow_runtime_service


async def prepare_batch_generation_project_outline_context(*args, **kwargs):
    return await _batch_generation_runtime_service().prepare_batch_generation_project_outline_context(
        *args,
        **kwargs,
    )


async def resolve_batch_generation_chapter_runtime(*args, **kwargs):
    return await _batch_generation_runtime_service().resolve_batch_generation_chapter_runtime(
        *args,
        **kwargs,
    )


async def execute_batch_generation_generation_stage(*args, **kwargs):
    return await _batch_generation_candidate_service().execute_batch_generation_generation_stage(
        *args,
        **kwargs,
    )


async def execute_batch_generation_prompt_stage(*args, **kwargs):
    return await _batch_generation_prompt_service().execute_batch_generation_prompt_stage(
        *args,
        **kwargs,
    )


def detect_style_profile(*args, **kwargs):
    return _chapter_generation_runtime_prompt_service().detect_style_profile(*args, **kwargs)


def resolve_generation_temperature(*args, **kwargs):
    return _chapter_generation_runtime_prompt_service().resolve_generation_temperature(*args, **kwargs)


def build_chapter_generation_runtime_bundle(*args, **kwargs):
    return _chapter_generation_runtime_service().build_chapter_generation_runtime_bundle(*args, **kwargs)


def build_chapter_quality_runtime_context(*args, **kwargs):
    return _chapter_generation_runtime_service().build_chapter_quality_runtime_context(*args, **kwargs)


def _build_chapter_generation_request_options(*args, **kwargs):
    return _chapter_generation_stream_request_policy_service()._build_chapter_generation_request_options(
        *args,
        **kwargs,
    )


def _calculate_chapter_generation_max_tokens(*args, **kwargs):
    return _chapter_generation_stream_request_policy_service()._calculate_chapter_generation_max_tokens(
        *args,
        **kwargs,
    )


def build_story_generation_packet_with_project_continuity(*args, **kwargs):
    return _chapter_quality_context_service().build_story_generation_packet_with_project_continuity(
        *args,
        **kwargs,
    )


def clone_chapter_quality_profile(*args, **kwargs):
    return _chapter_quality_context_service().clone_chapter_quality_profile(*args, **kwargs)


class _LazyForeshadowService:
    def __getattr__(self, name: str):
        return getattr(_foreshadow_service_instance(), name)


class _LazyMemoryService:
    def __getattr__(self, name: str):
        return getattr(_memory_service_instance(), name)


_foreshadow_service = _LazyForeshadowService()
_memory_service = _LazyMemoryService()


def build_outline_structure_runtime_sources(*args, **kwargs):
    return _outline_runtime_source_service().build_outline_structure_runtime_sources(*args, **kwargs)


class WritingStyleManager:
    @staticmethod
    def apply_style_to_prompt(*args, **kwargs):
        from app.services.prompt_service import WritingStyleManager as WritingStyleManagerImpl

        return WritingStyleManagerImpl.apply_style_to_prompt(*args, **kwargs)


def attach_story_runtime_contract(*args, **kwargs):
    return _story_runtime_serialization_service().attach_story_runtime_contract(*args, **kwargs)


async def _publish_task_stream_event(*args, **kwargs):
    return await _task_workflow_runtime_service().publish_task_stream_event(*args, **kwargs)


class PromptService:
    @staticmethod
    async def get_template(*args, **kwargs):
        from app.services.prompt_service import PromptService as PromptServiceImpl

        return await PromptServiceImpl.get_template(*args, **kwargs)

    @staticmethod
    def format_prompt(*args, **kwargs):
        from app.services.prompt_service import PromptService as PromptServiceImpl

        return PromptServiceImpl.format_prompt(*args, **kwargs)


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
_chapter_web_research_service = chapter_web_research_service


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


async def collect_batch_single_chapter_generation_candidate_output(
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


def resolve_batch_single_chapter_generation_attempt_labels(
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


def sync_batch_single_chapter_generation_runtime_state(
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


def build_batch_single_chapter_generation_candidate_record(
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


def build_batch_single_chapter_generation_candidate_record_with_default_logging(**kwargs):
    from app.logger import get_logger

    logger = get_logger(__name__)
    return build_batch_single_chapter_generation_candidate_record(
        **kwargs,
        log_warning_fn=logger.warning,
    )


async def generate_best_ranked_batch_single_chapter_candidate(
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
        resolve_generation_attempt_labels_fn=resolve_batch_single_chapter_generation_attempt_labels,
        sync_generation_runtime_state_fn=sync_batch_single_chapter_generation_runtime_state,
        collect_generation_candidate_output_fn=collect_batch_single_chapter_generation_candidate_output,
        build_generation_candidate_record_fn=build_batch_single_chapter_generation_candidate_record_with_default_logging,
    )


@dataclass(slots=True)
class BatchGenerationSingleChapterRequest:
    db_session: AsyncSession
    chapter: Chapter
    user_id: str
    style_id: Optional[int]
    target_word_count: int
    ai_service: AIService
    write_lock: Lock
    story_packet: Optional[StoryPacket] = None
    base_quality_profile: Optional[Dict[str, Any]] = None
    custom_model: Optional[str] = None
    previous_summary_context: Optional[str] = None
    temp_narrative_perspective: Optional[str] = None
    creative_mode: Optional[str] = None
    story_focus: Optional[str] = None
    plot_stage: Optional[str] = None
    story_creation_brief: Optional[str] = None
    quality_preset: Optional[str] = None
    quality_notes: Optional[str] = None
    enable_web_research: Optional[bool] = None
    web_research_query: Optional[str] = None
    story_repair_summary: Optional[str] = None
    story_repair_targets: Optional[list[str]] = None
    story_preserve_strengths: Optional[list[str]] = None
    story_repair_payload: Optional[StoryRepairPayload] = None
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None
    story_repair_state: Optional[Dict[str, Any]] = None
    stream_task_id: Optional[str] = None
    stream_chunks: bool = False
    retry_count: int = 0
    max_retries: int = 1


@dataclass(slots=True)
class BatchGenerationSingleChapterDependencies:
    chapter_web_research_service: Any
    publish_task_stream_event_fn: Callable[..., Any]
    prepare_project_outline_context_fn: Callable[..., Any]
    resolve_batch_generation_chapter_runtime_fn: Callable[..., Any]
    build_generation_runtime_bundle_fn: Callable[..., Any]
    build_story_packet_fn: Callable[..., Any]
    clone_quality_profile_fn: Callable[..., Any]
    resolve_quality_profile_fn: Callable[..., Any]
    one_to_one_builder_cls: Any
    one_to_many_builder_cls: Any
    build_outline_structure_runtime_sources_fn: Callable[..., Any]
    execute_prompt_stage_fn: Callable[..., Any]
    get_template_fn: Callable[..., Any]
    format_prompt_fn: Callable[..., Any]
    apply_style_to_prompt_fn: Callable[..., Any]
    build_runtime_system_prompt_fn: Callable[..., Any]
    calculate_max_tokens_fn: Callable[..., Any]
    build_request_options_fn: Callable[..., Any]
    detect_style_profile_fn: Callable[..., Any]
    resolve_generation_temperature_fn: Callable[..., Any]
    execute_generation_stage_fn: Callable[..., Any]
    build_quality_runtime_context_fn: Callable[..., Any]
    compute_story_quality_metrics_fn: Callable[..., Any]
    resolve_quality_gate_execution_plan_fn: Callable[..., Any]
    candidate_generator_fn: Callable[..., Any]
    attach_story_runtime_contract_fn: Callable[..., Any]
    memory_service: Any
    foreshadow_service: Any
    default_candidate_limit: int
    heartbeat_interval_seconds: float


def build_batch_generation_single_chapter_request(
    *,
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
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    stream_task_id: Optional[str] = None,
    stream_chunks: bool = False,
    retry_count: int = 0,
    max_retries: int = 1,
) -> BatchGenerationSingleChapterRequest:
    return BatchGenerationSingleChapterRequest(
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


def build_batch_generation_single_chapter_dependencies(
    *,
    chapter_web_research_service: Any,
    publish_task_stream_event_fn: Callable[..., Any],
    prepare_project_outline_context_fn: Callable[..., Any],
    resolve_batch_generation_chapter_runtime_fn: Callable[..., Any],
    build_generation_runtime_bundle_fn: Callable[..., Any],
    build_story_packet_fn: Callable[..., Any],
    clone_quality_profile_fn: Callable[..., Any],
    resolve_quality_profile_fn: Callable[..., Any],
    one_to_one_builder_cls: Any,
    one_to_many_builder_cls: Any,
    build_outline_structure_runtime_sources_fn: Callable[..., Any],
    execute_prompt_stage_fn: Callable[..., Any],
    get_template_fn: Callable[..., Any],
    format_prompt_fn: Callable[..., Any],
    apply_style_to_prompt_fn: Callable[..., Any],
    build_runtime_system_prompt_fn: Callable[..., Any],
    calculate_max_tokens_fn: Callable[..., Any],
    build_request_options_fn: Callable[..., Any],
    detect_style_profile_fn: Callable[..., Any],
    resolve_generation_temperature_fn: Callable[..., Any],
    execute_generation_stage_fn: Callable[..., Any],
    build_quality_runtime_context_fn: Callable[..., Any],
    compute_story_quality_metrics_fn: Callable[..., Any],
    resolve_quality_gate_execution_plan_fn: Callable[..., Any],
    candidate_generator_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[..., Any],
    memory_service: Any,
    foreshadow_service: Any,
    default_candidate_limit: int,
    heartbeat_interval_seconds: float,
) -> BatchGenerationSingleChapterDependencies:
    return BatchGenerationSingleChapterDependencies(
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
        prepare_project_outline_context_fn=prepare_project_outline_context_fn,
        resolve_batch_generation_chapter_runtime_fn=resolve_batch_generation_chapter_runtime_fn,
        build_generation_runtime_bundle_fn=build_generation_runtime_bundle_fn,
        build_story_packet_fn=build_story_packet_fn,
        clone_quality_profile_fn=clone_quality_profile_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources_fn,
        execute_prompt_stage_fn=execute_prompt_stage_fn,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=calculate_max_tokens_fn,
        build_request_options_fn=build_request_options_fn,
        detect_style_profile_fn=detect_style_profile_fn,
        resolve_generation_temperature_fn=resolve_generation_temperature_fn,
        execute_generation_stage_fn=execute_generation_stage_fn,
        build_quality_runtime_context_fn=build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=candidate_generator_fn,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
        memory_service=memory_service,
        foreshadow_service=foreshadow_service,
        default_candidate_limit=default_candidate_limit,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
    )


async def generate_single_chapter_for_batch_workflow(
    *,
    request: BatchGenerationSingleChapterRequest,
    dependencies: BatchGenerationSingleChapterDependencies,
) -> Dict[str, Any]:
    project_outline_context = await dependencies.prepare_project_outline_context_fn(
        db_session=request.db_session,
        chapter=request.chapter,
        user_id=request.user_id,
        story_creation_brief=request.story_creation_brief,
        enable_web_research=request.enable_web_research,
        web_research_query=request.web_research_query,
        stream_task_id=request.stream_task_id,
        write_lock=request.write_lock,
        chapter_web_research_service=dependencies.chapter_web_research_service,
        publish_task_stream_event_fn=dependencies.publish_task_stream_event_fn,
    )

    resolved_chapter_runtime = await dependencies.resolve_batch_generation_chapter_runtime_fn(
        db_session=request.db_session,
        user_id=request.user_id,
        project=project_outline_context.project,
        chapter=request.chapter,
        outline=project_outline_context.outline,
        outline_mode=project_outline_context.outline_mode,
        target_word_count=request.target_word_count,
        style_id=request.style_id,
        story_packet=request.story_packet,
        base_quality_profile=request.base_quality_profile,
        research_assets=project_outline_context.research_assets,
        creative_mode=request.creative_mode,
        story_focus=request.story_focus,
        plot_stage=request.plot_stage,
        story_creation_brief=request.story_creation_brief,
        quality_preset=request.quality_preset,
        quality_notes=request.quality_notes,
        memory_service=dependencies.memory_service,
        foreshadow_service=dependencies.foreshadow_service,
        story_repair_state=request.story_repair_state,
        story_repair_payload=request.story_repair_payload,
        active_story_repair_snapshot=request.active_story_repair_snapshot,
        build_generation_runtime_bundle_fn=dependencies.build_generation_runtime_bundle_fn,
        build_story_packet_fn=dependencies.build_story_packet_fn,
        clone_quality_profile_fn=dependencies.clone_quality_profile_fn,
        resolve_quality_profile_fn=dependencies.resolve_quality_profile_fn,
        one_to_one_builder_cls=dependencies.one_to_one_builder_cls,
        one_to_many_builder_cls=dependencies.one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=dependencies.build_outline_structure_runtime_sources_fn,
    )
    effective_story_packet = resolved_chapter_runtime.effective_story_packet
    chapter_context = resolved_chapter_runtime.chapter_context
    generation_intent = resolved_chapter_runtime.generation_intent
    prompt_quality_kwargs = resolved_chapter_runtime.prompt_quality_kwargs
    story_runtime_contract = resolved_chapter_runtime.story_runtime_contract

    prompt_stage_result = await dependencies.execute_prompt_stage_fn(
        db_session=request.db_session,
        chapter=request.chapter,
        project=project_outline_context.project,
        chapter_context=chapter_context,
        outline_mode=project_outline_context.outline_mode,
        current_user_id=request.user_id,
        target_word_count=request.target_word_count,
        temp_narrative_perspective=request.temp_narrative_perspective,
        previous_summary_context=request.previous_summary_context,
        prompt_quality_kwargs=prompt_quality_kwargs,
        style_content=resolved_chapter_runtime.style_content,
        style_name=resolved_chapter_runtime.style_name,
        style_preset_id=resolved_chapter_runtime.style_preset_id,
        ai_service=request.ai_service,
        custom_model=request.custom_model,
        story_runtime_contract=story_runtime_contract,
        research_assets=project_outline_context.research_assets,
        get_template_fn=dependencies.get_template_fn,
        format_prompt_fn=dependencies.format_prompt_fn,
        apply_style_to_prompt_fn=dependencies.apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=dependencies.build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=dependencies.calculate_max_tokens_fn,
        build_request_options_fn=dependencies.build_request_options_fn,
        detect_style_profile_fn=dependencies.detect_style_profile_fn,
        resolve_generation_temperature_fn=dependencies.resolve_generation_temperature_fn,
    )

    candidate_flow_result = await dependencies.execute_generation_stage_fn(
        stream_task_id=request.stream_task_id,
        stream_chunks=request.stream_chunks,
        chapter=request.chapter,
        effective_story_packet=effective_story_packet,
        project=project_outline_context.project,
        chapter_context=chapter_context,
        target_word_count=request.target_word_count,
        generation_intent=generation_intent,
        current_story_repair_payload=request.story_repair_payload,
        retry_count=request.retry_count,
        max_retries=request.max_retries,
        default_candidate_limit=dependencies.default_candidate_limit,
        ai_service=request.ai_service,
        generate_kwargs=prompt_stage_result.generate_kwargs,
        story_runtime_contract=story_runtime_contract,
        db_session=request.db_session,
        heartbeat_interval_seconds=dependencies.heartbeat_interval_seconds,
        build_quality_runtime_context_fn=dependencies.build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=dependencies.compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=dependencies.resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=dependencies.candidate_generator_fn,
        attach_story_runtime_contract_fn=dependencies.attach_story_runtime_contract_fn,
    )

    return candidate_flow_result.selected_candidate_result


def build_default_batch_generation_single_chapter_dependencies(
    *,
    candidate_generator_fn: Callable[..., Any],
    default_candidate_limit: int,
    heartbeat_interval_seconds: float,
    chapter_web_research_service: Any = chapter_web_research_service,
    publish_task_stream_event_fn: Callable[..., Any] = _publish_task_stream_event,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    one_to_one_builder_cls: Any = OneToOneContextBuilder,
    one_to_many_builder_cls: Any = OneToManyContextBuilder,
    get_template_fn: Callable[..., Any] = PromptService.get_template,
    format_prompt_fn: Callable[..., Any] = PromptService.format_prompt,
    build_runtime_system_prompt_fn: Callable[..., Any] = build_chapter_runtime_system_prompt,
    compute_story_quality_metrics_fn: Callable[..., Any] = compute_story_quality_metrics,
    resolve_quality_gate_execution_plan_fn: Callable[..., Any] = resolve_quality_gate_execution_plan,
):
    return build_batch_generation_single_chapter_dependencies(
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
        prepare_project_outline_context_fn=prepare_batch_generation_project_outline_context,
        resolve_batch_generation_chapter_runtime_fn=resolve_batch_generation_chapter_runtime,
        build_generation_runtime_bundle_fn=build_chapter_generation_runtime_bundle,
        build_story_packet_fn=build_story_generation_packet_with_project_continuity,
        clone_quality_profile_fn=clone_chapter_quality_profile,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources,
        execute_prompt_stage_fn=execute_batch_generation_prompt_stage,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=WritingStyleManager.apply_style_to_prompt,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=_calculate_chapter_generation_max_tokens,
        build_request_options_fn=_build_chapter_generation_request_options,
        detect_style_profile_fn=detect_style_profile,
        resolve_generation_temperature_fn=resolve_generation_temperature,
        execute_generation_stage_fn=execute_batch_generation_generation_stage,
        build_quality_runtime_context_fn=build_chapter_quality_runtime_context,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=candidate_generator_fn,
        attach_story_runtime_contract_fn=attach_story_runtime_contract,
        memory_service=_memory_service,
        foreshadow_service=_foreshadow_service,
        default_candidate_limit=default_candidate_limit,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
    )


async def generate_single_chapter_for_batch_with_default_wiring(
    *,
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
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    stream_task_id: Optional[str] = None,
    stream_chunks: bool = False,
    retry_count: int = 0,
    max_retries: int = 1,
    candidate_generator_fn: Callable[..., Any],
    default_candidate_limit: int,
    heartbeat_interval_seconds: float,
    chapter_web_research_service: Any = chapter_web_research_service,
    publish_task_stream_event_fn: Callable[..., Any] = _publish_task_stream_event,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    one_to_one_builder_cls: Any = OneToOneContextBuilder,
    one_to_many_builder_cls: Any = OneToManyContextBuilder,
    get_template_fn: Callable[..., Any] = PromptService.get_template,
    format_prompt_fn: Callable[..., Any] = PromptService.format_prompt,
    build_runtime_system_prompt_fn: Callable[..., Any] = build_chapter_runtime_system_prompt,
    compute_story_quality_metrics_fn: Callable[..., Any] = compute_story_quality_metrics,
    resolve_quality_gate_execution_plan_fn: Callable[..., Any] = resolve_quality_gate_execution_plan,
) -> Dict[str, Any]:
    workflow_request = build_batch_generation_single_chapter_request(
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
    workflow_dependencies = build_default_batch_generation_single_chapter_dependencies(
        candidate_generator_fn=candidate_generator_fn,
        default_candidate_limit=default_candidate_limit,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
    )
    return await generate_single_chapter_for_batch_workflow(
        request=workflow_request,
        dependencies=workflow_dependencies,
    )
