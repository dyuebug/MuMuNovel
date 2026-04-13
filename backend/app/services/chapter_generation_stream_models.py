from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, List, Optional

from app.models.chapter import Chapter
from app.models.project import Project


@dataclass(frozen=True)
class ChapterGenerationStreamExecutionDependencies:
    resolve_story_repair_state_fn: Any
    cancel_outline_postprocess_tasks_fn: Any
    memory_service: Any
    foreshadow_service: Any
    one_to_one_builder_cls: Any
    one_to_many_builder_cls: Any
    build_outline_structure_runtime_sources_fn: Any
    build_generation_runtime_bundle_fn: Any
    get_template_fn: Any
    format_prompt_fn: Any
    apply_style_to_prompt_fn: Any
    build_runtime_system_prompt_fn: Any
    calculate_max_tokens_fn: Any
    build_request_options_fn: Any
    detect_style_profile_fn: Any
    resolve_generation_temperature_fn: Any


@dataclass(frozen=True)
class ChapterGenerationStreamCandidateDependencies:
    build_quality_runtime_context_fn: Any
    compute_story_quality_metrics_fn: Any
    resolve_quality_gate_execution_plan_fn: Any
    candidate_rerank_limit: int
    candidate_generator_fn: Any
    build_draft_attempt_fn: Any
    attach_story_runtime_contract_fn: Any


@dataclass(frozen=True)
class ChapterGenerationStreamFinalizeDependencies:
    foreshadow_service: Any
    build_generation_history_payload_fn: Any
    create_analysis_task_fn: Any
    analyze_chapter_background_fn: Any
    build_candidate_draft_payload_fn: Any
    build_stream_result_payload_fn: Any


@dataclass(frozen=True)
class ChapterGenerationStreamDependencies:
    execution: ChapterGenerationStreamExecutionDependencies
    candidate: ChapterGenerationStreamCandidateDependencies
    finalize: ChapterGenerationStreamFinalizeDependencies


@dataclass(frozen=True)
class ChapterGenerationStreamExecutionSetup:
    stream_runtime_context: Any
    built_stream_context: Any
    current_chapter: Chapter
    project: Project
    quality_profile: Dict[str, Any]
    story_packet: Any
    story_runtime_contract: Optional[Dict[str, Any]]
    request_payload: Any


@dataclass(frozen=True)
class ChapterGenerationStreamCandidateStageResult:
    selected_candidate_outcome: Any
    full_content: str
    candidate_word_count: int
    quality_metrics: Optional[Dict[str, Any]]
    quality_gate_action: str
    quality_gate_requires_followup: bool
    quality_gate_message: Optional[str]
    quality_gate_snapshot: Optional[Dict[str, Any]]
    content_applied: bool
    draft_attempt: Any
    previous_status: Optional[str]
    chunk_payloads: List[Any]


@dataclass(frozen=True)
class ChapterGenerationAnalysisFollowupPlan:
    should_schedule_analysis: bool
    analysis_reason: Optional[str]
    chapter_content_override: Optional[str]
    chapter_word_count_override: Optional[int]
    completion_message: str
    analysis_started_message: Optional[str]


@dataclass(frozen=True)
class ChapterGenerationStreamResponseArtifacts:
    quality_metrics_event_payload: Dict[str, Any]
    quality_gate_event_payload: Optional[Dict[str, Any]]
    result_payload: Dict[str, Any]
    analysis_started_event_data: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class ChapterGenerationAnalysisScheduling:
    task_id: Optional[str]
    background_task_kwargs: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class ChapterGenerationPostPersistEffects:
    planted_count: int
    plant_error: Optional[str]


@dataclass(frozen=True)
class ChapterGenerationEmissionStep:
    kind: str
    payload: Optional[Dict[str, Any]] = None
    message: Optional[str] = None
    event: Optional[str] = None


@dataclass(frozen=True)
class ChapterGenerationStreamPreparation:
    chapter: Chapter
    previous_chapters_data: List[dict[str, str | int | None]]


@dataclass(frozen=True)
class ChapterGenerationStreamRuntimeContext:
    chapter: Chapter
    project: Project
    outline: Any
    outline_mode: str
    quality_profile: Dict[str, Any]
    story_packet: Any
    generation_guidance: Any
    story_repair_state: Dict[str, Any]
    story_repair_payload: Optional[Dict[str, Any]]
    resolved_style_id: Optional[int]
    style_content: str
    style_name: str
    style_preset_id: Any


@dataclass(frozen=True)
class ChapterGenerationStreamBuiltContext:
    chapter_context: Any
    generation_intent: Any
    prompt_quality_kwargs: Dict[str, Any]
    story_runtime_contract: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class ChapterGenerationStreamPrompt:
    chapter_perspective: str
    base_prompt: str
    prompt: str


@dataclass(frozen=True)
class ChapterGenerationStreamRequestPayload:
    system_prompt: str
    max_tokens: int
    generate_kwargs: Dict[str, Any]




def build_chapter_generation_stream_execution_dependencies(
    *,
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
) -> ChapterGenerationStreamExecutionDependencies:
    return ChapterGenerationStreamExecutionDependencies(
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
    build_quality_runtime_context_fn,
    compute_story_quality_metrics_fn,
    resolve_quality_gate_execution_plan_fn,
    candidate_rerank_limit: int,
    candidate_generator_fn,
    build_draft_attempt_fn,
    attach_story_runtime_contract_fn,
) -> ChapterGenerationStreamCandidateDependencies:
    return ChapterGenerationStreamCandidateDependencies(
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
    foreshadow_service: Any,
    build_generation_history_payload_fn,
    create_analysis_task_fn,
    analyze_chapter_background_fn,
    build_candidate_draft_payload_fn,
    build_stream_result_payload_fn,
) -> ChapterGenerationStreamFinalizeDependencies:
    return ChapterGenerationStreamFinalizeDependencies(
        foreshadow_service=foreshadow_service,
        build_generation_history_payload_fn=build_generation_history_payload_fn,
        create_analysis_task_fn=create_analysis_task_fn,
        analyze_chapter_background_fn=analyze_chapter_background_fn,
        build_candidate_draft_payload_fn=build_candidate_draft_payload_fn,
        build_stream_result_payload_fn=build_stream_result_payload_fn,
    )


def build_chapter_generation_stream_dependencies(
    *,
    execution: ChapterGenerationStreamExecutionDependencies,
    candidate: ChapterGenerationStreamCandidateDependencies,
    finalize: ChapterGenerationStreamFinalizeDependencies,
) -> ChapterGenerationStreamDependencies:
    return ChapterGenerationStreamDependencies(
        execution=execution,
        candidate=candidate,
        finalize=finalize,
    )
