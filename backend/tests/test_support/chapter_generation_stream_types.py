from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Callable, Dict, List, Optional

if TYPE_CHECKING:
    from migrator_app.models.chapter import Chapter
    from migrator_app.models.project import Project


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
    current_chapter: "Chapter"
    project: "Project"
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
class ChapterGenerationStreamRuntimeContext:
    chapter: "Chapter"
    project: "Project"
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


@dataclass(frozen=True)
class ChapterGenerationCandidateExecution:
    runtime_state: Dict[str, Any]
    selected_candidate_task: asyncio.Task


@dataclass(frozen=True)
class ChapterGenerationCandidateQualityHooks:
    quality_evaluator: Callable[[str], Dict[str, Any]]
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]]


@dataclass(frozen=True)
class ChapterGenerationSelectedCandidateOutcome:
    full_content: str
    candidate_word_count: int
    candidate_chunks: List[str]
    quality_metrics: Optional[Dict[str, Any]]
    quality_gate_plan: Dict[str, Any]
    quality_gate_action: str
    quality_gate_requires_followup: bool
    quality_gate_message: Optional[str]
    quality_gate_snapshot: Optional[Dict[str, Any]]
    content_applied: bool
    attempt_state: str
    draft_attempt: Any
    provisional_draft_allowed: bool


@dataclass(frozen=True)
class ChapterGenerationPersistencePreparation:
    previous_content: str
    previous_word_count: int
    previous_status: Optional[str]
    saved_word_count: int
    provisional_draft_saved: bool
    history: Any

