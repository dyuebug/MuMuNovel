"""章节管理API"""
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy import select, func
from sqlalchemy.orm import selectinload
import json
import asyncio
from typing import Optional, Dict, Any, List, Tuple, Callable
from datetime import datetime
from asyncio import Queue, Lock

from app.database import get_db, get_session_factory
from app.services.chapter_context_service import (
    OneToManyContextBuilder,
    OneToOneContextBuilder
)
from app.models.chapter import Chapter
from app.models.project import Project
from app.models.outline import Outline
from app.models.character import Character
from app.models.career import Career, CharacterCareer
from app.models.relationship import CharacterRelationship, Organization, OrganizationMember
from app.models.generation_history import GenerationHistory
from app.models.writing_style import WritingStyle
from app.models.analysis_task import AnalysisTask
from app.models.memory import PlotAnalysis, StoryMemory
from app.models.batch_generation_snapshot import BatchGenerationSnapshot
from app.models.batch_generation_task import BatchGenerationTask
from app.models.chapter_draft_attempt import ChapterDraftAttempt
from app.models.regeneration_task import RegenerationTask
from app.schemas.chapter import (
    ChapterCreate,
    ChapterUpdate,
    ChapterResponse,
    ChapterListResponse,
    ChapterGenerateRequest,
    BatchGenerateRequest,
    BatchGenerateResponse,
    BatchGenerateStatusResponse,
    PartialRegenerateRequest,
    ProjectChapterQualityTrendResponse,
)
from app.schemas.regeneration import (
    ChapterRegenerateRequest,
    RegenerationTaskResponse,
    RegenerationTaskStatus
)
from app.schemas.generation_payload import (
    build_chapter_generation_stream_result_payload,
    build_chapter_regeneration_stream_result_payload,
)
from app.services.ai_service import AIService
from app.services.manual_chapter_analysis_execution_service import (
    execute_chapter_analysis_background as analyze_chapter_background,
)
from app.services.prompt_service import (
    prompt_service,
    PromptService,
    WritingStyleManager,
)
from app.services.chapter_quality_context_service import (
    ChapterGenerationIntent,
    StoryPacket,
    StoryGenerationGuidance,
    build_analysis_quality_kwargs,
    build_prompt_quality_kwargs,
    build_story_generation_packet,
    build_story_generation_packet_with_project_continuity,
    clone_chapter_quality_profile,
    resolve_chapter_quality_profile,
)
from app.services.chapter_generation_runtime_service import (
    build_chapter_generation_runtime_bundle as _build_chapter_generation_runtime_bundle,
    build_chapter_prompt_quality_kwargs_from_runtime as _build_chapter_prompt_quality_kwargs,
    build_chapter_quality_runtime_context as _build_chapter_quality_runtime_context,
    create_chapter_generation_intent_from_runtime as _create_chapter_generation_intent,
)
from app.services.plot_analyzer import PlotAnalyzer
from app.services.memory_service import memory_service
from app.services.chapter_web_research_service import chapter_web_research_service
from app.services.foreshadow_service import foreshadow_service
from app.services.chapter_regenerator import ChapterRegenerator
from app.services.chapter_candidate_runtime_state_service import (
    snapshot_chapter_candidate_runtime_state,
)
from app.services.chapter_candidate_entry_compat_service import (
    generate_best_ranked_candidate as _generate_best_ranked_candidate_compat_service,
    get_chapter_candidate_executor_dependencies as _get_chapter_candidate_executor_dependencies_compat_service,
)
from app.services.chapter_candidate_executor_compat_service import (
    build_generation_candidate_record as _build_generation_candidate_record_compat_service,
    collect_generation_candidate_output as _collect_generation_candidate_output_compat_service,
    resolve_generation_attempt_labels as _resolve_generation_attempt_labels_compat_service,
    sync_generation_runtime_state as _sync_generation_runtime_state_compat_service,
)
from app.services.chapter_generated_text_compat_service import (
    contains_chapter_workflow_meta_text as _contains_chapter_workflow_meta_text_compat_service,
    is_likely_chapter_meta_line as _is_likely_chapter_meta_line_compat_service,
    lightly_polish_template_phrases as _lightly_polish_template_phrases_compat_service,
    sanitize_generated_narrative_text as _sanitize_generated_narrative_text_compat_service,
    trim_text_to_sentence_boundary as _trim_text_to_sentence_boundary_compat_service,
)
from app.services.chapter_prompt_quality_compat_service import (
    build_chapter_runtime_system_prompt as _build_chapter_runtime_system_prompt_compat_service,
    compute_story_quality_metrics as _compute_story_quality_metrics_compat_service,
    detect_style_profile as _detect_style_profile_compat_service,
    resolve_generation_temperature as _resolve_generation_temperature_compat_service,
)
from app.services.story_quality_feedback_service import (
    _calc_cliffhanger_rate,
    _calc_conflict_chain_rate,
    _calc_dialogue_naturalness_rate,
    _calc_opening_hook_rate,
    _calc_outline_alignment_rate,
    _calc_payoff_chain_rate,
    _calc_rule_grounding_rate,
    _expand_anchor_match_tokens,
    _extract_dialogue_segments,
    _extract_outline_anchor_tokens,
    _extract_outline_rule_hints,
    _extract_payoff_chain_hints,
    _extract_rule_keywords,
    _normalize_world_rules_text,
    _resolve_rule_grounding_source_text,
    advance_quality_metrics_summary_state,
    build_quality_gate_decision,
    build_quality_metrics_summary,
    build_quality_metrics_summary_from_state,
    build_quality_metrics_summary_state,
    build_story_continuity_preflight,
    build_story_repair_guidance,
    extract_quality_metrics_from_history_payload as _parse_quality_metrics_from_history,
)
from app.services.story_repair_payload_service import (
    StoryRepairPayload,
    attach_story_repair_quality_history as _attach_story_repair_quality_history,
    build_batch_quality_metrics_summary as _build_quality_metrics_summary,
    build_story_repair_payload_from_metrics,
    build_story_repair_runtime_state,
    load_latest_quality_metric_records_for_chapter_ids as _load_latest_quality_metric_records_for_chapter_ids,
    merge_story_repair_payload,
    normalize_story_repair_payload,
    resolve_generation_story_repair_state_for_batch as _resolve_generation_story_repair_state_for_batch,
    resolve_quality_gate_execution_plan as _resolve_quality_gate_execution_plan,
    resolve_generation_story_repair_state_for_chapter as _resolve_generation_story_repair_state_for_chapter,
    resolve_quality_gate_from_metrics as _resolve_quality_gate_from_metrics,
    resolve_story_repair_guidance_from_metrics as _resolve_story_repair_guidance_from_metrics,
    resolve_story_repair_prompt_kwargs,
    story_repair_payload_to_prompt_kwargs,
)
from app.services.story_runtime_serialization_service import (
    attach_story_runtime_contract as _attach_story_runtime_contract,
)
from app.services.chapter_candidate_rerank_service import (
    build_candidate_pool_summary,
    build_candidate_retry_prompt_suffix,
    build_candidate_retry_strategy_suffix,
    build_targeted_final_repair_suffix,
    build_word_budget_repair_suffix,
    is_candidate_word_count_in_target_window,
    resolve_candidate_retry_temperature,
    resolve_targeted_final_repair_char_limit,
    resolve_targeted_final_repair_max_tokens,
    resolve_targeted_final_repair_temperature,
    resolve_word_budget_repair_char_limit,
    resolve_word_budget_repair_max_tokens,
    resolve_word_budget_repair_temperature,
    select_best_generation_candidate,
    select_targeted_final_repair_seed_candidate,
    should_adopt_targeted_final_repair_candidate,
    should_apply_followup_targeted_final_repair,
    should_apply_targeted_final_repair,
    should_keep_targeted_final_repair_candidate,
    should_keep_word_budget_repair_candidate,
    should_apply_word_budget_repair,
    should_relax_word_budget_repair_limits,
    should_prefer_targeted_final_repair_candidate,
    should_prefer_word_budget_repair_candidate,
    should_generate_additional_candidate,
)
from app.services.project_quality_trend_snapshot_store import (
    load_project_quality_trend_snapshot,
    persist_project_quality_trend_snapshot,
)
from app.services.batch_generation_run_compat_service import (
    await_cancelable_batch_generation_result as _await_cancelable_batch_generation_result_compat_service,
    get_db_write_lock as _get_db_write_lock_compat_service,
)
from app.services.project_quality_trend_compat_service import (
    get_project_quality_trend_snapshot as _get_project_quality_trend_snapshot_compat_service,
)
from app.services.project_quality_trend_service import (
    build_project_quality_trend_response_payload,
    project_quality_trend_cache,
    project_quality_trend_lock,
)
from app.services.chapter_analysis_response_service import build_chapter_analysis_payload
from app.services.chapter_analysis_support_service import (
    _collect_reviser_priority_issues,
    build_checker_history_payload as _build_checker_history_payload,
    build_checker_report_text as _build_checker_report_text,
    build_reviser_history_payload as _build_reviser_history_payload,
    build_reviser_priority_issues_text as _build_reviser_priority_issues_text,
    merge_checker_suggestions as _merge_checker_suggestions,
    normalize_checker_result as _normalize_checker_result,
    run_chapter_text_checker as _run_chapter_text_checker,
    run_chapter_text_reviser as _run_chapter_text_reviser,
)
from app.services.project_quality_trend_query_service import load_project_quality_trend_query_context
from app.services.chapter_draft_apply_service import (
    apply_draft_content_with_history,
    ensure_draft_not_stale_or_raise,
    resolve_draft_apply_request_options,
    sanitize_draft_content_or_raise,
)
from app.services.batch_generation_status_service import (
    build_active_batch_generation_payload,
    build_batch_generation_status_response,
    build_batch_generation_task_list_item,
)
from app.services.batch_generation_query_service import (
    build_batch_task_workflow_snapshot as _build_batch_task_workflow_snapshot,
    load_active_project_batch_generation_task_view_context,
    load_active_user_batch_generation_task_view_contexts,
    load_batch_generation_task_view_context,
)
from app.services.batch_generation_orchestration_service import (
    orchestrate_batch_generation_create,
    orchestrate_batch_generation_resume,
)
from app.services.batch_generation_run_service import execute_batch_generation_in_order_workflow
from app.services.batch_generation_entry_compat_service import (
    execute_batch_generation_in_order as _execute_batch_generation_in_order_compat_service,
    generate_single_chapter_for_batch as _generate_single_chapter_for_batch_compat_service,
)
from app.services.batch_generation_stream_service import (
    build_batch_generation_event_stream,
    validate_batch_generation_stream_access,
)
from app.services.analysis_task_service import create_analysis_task_safely as _create_analysis_task_safely
from app.services.chapter_generation_prerequisite_service import (
    check_chapter_generation_prerequisites as check_prerequisites,
)
from app.services.regeneration_task_service import (
    create_regeneration_task,
    load_latest_regeneration_analysis,
    mark_latest_regeneration_task_failed,
)
from app.services.chapter_generation_stream_request_policy_service import (
    _build_chapter_generation_request_options,
    _calculate_chapter_generation_max_tokens,
)
from app.services.partial_regeneration_service import prepare_partial_regeneration
from app.services.character_context_service import build_characters_info_with_careers
from app.services.outline_runtime_source_service import (
    build_outline_structure_runtime_sources as _build_outline_structure_runtime_sources,
)
from app.services.outline_requirement_service import (
    extract_outline_anchor_lines as _extract_outline_anchor_lines,
)
from app.services.batch_generation_execution_service import (
    BatchGenerationChapterRuntimeState,
    BatchGenerationExecutionEnvironment,
    apply_generated_batch_chapter_candidate as _apply_generated_batch_chapter_candidate,
    build_batch_generation_candidate_quality_hooks,
    build_batch_generation_prompt,
    build_batch_generation_request_payload,
    build_batch_generation_selected_candidate_result,
    create_analysis_task_safely as _create_analysis_task_safely,
    create_batch_generation_candidate_execution,
    build_batch_chapter_draft_attempt as _build_chapter_draft_attempt,
    build_single_chapter_background_execution_context,
    calculate_estimated_time as calculate_batch_generation_estimated_time,
    complete_batch_generation_execution,
    create_batch_generation_task_record,
    emit_batch_generation_selected_candidate_events,
    enqueue_batch_generation_execution,
    execute_batch_generation_candidate_flow,
    execute_batch_generation_chapter_with_retries,
    execute_batch_generation_generation_stage,
    execute_batch_generation_prompt_stage,
    fail_batch_generation_on_unhandled_exception,
    handle_cancelled_batch_generation_execution,
    initialize_batch_generation_execution,
    resolve_batch_generation_chapter_runtime,
    run_batch_chapter_analysis as _run_batch_chapter_analysis_service,
    mark_batch_generation_current_chapter,
    wait_for_batch_generation_candidate,
)
from app.services.task_workflow_runtime_service import (
    publish_task_stream_event,
    _set_task_active_story_repair_payload,
    sync_task_story_repair_state as _sync_task_story_repair_state,
    task_workflow_lock,
    task_workflow_state_cache,
)
from app.services.task_workflow_runtime_compat_service import (
    SNAPSHOT_UNSET as _SNAPSHOT_UNSET_COMPAT_SERVICE,
    batch_task_exists as _batch_task_exists_compat_service,
    clear_task_runtime_caches as _clear_task_runtime_caches_compat_service,
    get_task_workflow_runtime_snapshot as _get_task_workflow_runtime_snapshot_compat_service,
    load_persisted_batch_generation_snapshot as _load_persisted_batch_generation_snapshot_compat_service,
    persist_task_workflow_runtime_snapshot as _persist_task_workflow_runtime_snapshot_compat_service,
    upsert_batch_generation_snapshot as _upsert_batch_generation_snapshot_compat_service,
)
from app.services.task_quality_snapshot_service import (
    record_task_quality_metrics as _record_task_quality_metrics,
    task_quality_metrics_cache,
)
from app.services.chapter_generation_history_service import (
    _build_candidate_draft_payload,
    _build_candidate_draft_quality_highlights,
    _extract_candidate_draft_full_content,
    _load_latest_candidate_draft_attempt,
    build_auto_revision_draft_payload as _build_auto_revision_draft_payload,
    build_generation_history_payload as _build_generation_history_payload,
    build_reviser_apply_history_payload as _build_reviser_apply_history_payload,
    is_reviser_draft_stale as _is_reviser_draft_stale,
    load_latest_reviser_history as _load_latest_reviser_history,
    parse_reviser_result_from_history as _parse_reviser_result_from_history,
)
from app.logger import get_logger
from app.api.settings import get_user_ai_service

logger = get_logger(__name__)

# 兼容层：通过 gateway / seam facade 复用批量生成 FastAPI 路由能力

# 全局数据库写入锁（每个用户一个锁，用于保护SQLite写入操作）


# ==================== Batch / Runtime seam ====================

async def get_db_write_lock(user_id: str) -> Lock:
    return await _get_db_write_lock_compat_service(user_id)


_SNAPSHOT_UNSET = _SNAPSHOT_UNSET_COMPAT_SERVICE


async def _await_cancelable_batch_generation_result(
    *,
    generation_coro,
    task: BatchGenerationTask,
    db_session: AsyncSession,
    poll_interval_seconds: Optional[float] = None,
):
    if poll_interval_seconds is None:
        poll_interval_seconds = CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS
    return await _await_cancelable_batch_generation_result_compat_service(
        generation_coro=generation_coro,
        task=task,
        db_session=db_session,
        poll_interval_seconds=poll_interval_seconds,
    )

CHAPTER_CANDIDATE_RERANK_LIMIT = 2
CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0

# ==================== Project Quality seam ====================

async def _get_project_quality_trend_snapshot(
    *,
    project_id: str,
    limit: int,
    items: List[Dict[str, Any]],
    metrics_history: List[Dict[str, Any]],
    total_chapters: int,
    analyzed_chapters: int,
    last_generated_at: Optional[datetime],
) -> Dict[str, Any]:
    return await _get_project_quality_trend_snapshot_compat_service(
        project_id=project_id,
        limit=limit,
        items=items,
        metrics_history=metrics_history,
        total_chapters=total_chapters,
        analyzed_chapters=analyzed_chapters,
        last_generated_at=last_generated_at,
        build_summary_state_fn=build_quality_metrics_summary_state,
        advance_summary_state_fn=advance_quality_metrics_summary_state,
        summary_from_state_fn=build_quality_metrics_summary_from_state,
        load_snapshot_fn=load_project_quality_trend_snapshot,
        persist_snapshot_fn=persist_project_quality_trend_snapshot,
    )



# ==================== Candidate Generation seam ====================

async def _collect_generation_candidate_output(
    ai_service: AIService,
    generate_kwargs: Dict[str, Any],
    *,
    candidate_index: int = 1,
    max_output_chars: Optional[int] = None,
    runtime_state: Optional[Dict[str, Any]] = None,
) -> tuple[str, List[str]]:
    return await _collect_generation_candidate_output_compat_service(
        ai_service=ai_service,
        generate_kwargs=generate_kwargs,
        candidate_index=candidate_index,
        max_output_chars=max_output_chars,
        runtime_state=runtime_state,
    )


def _resolve_generation_attempt_labels(
    candidate_index: int,
    *,
    is_word_budget_repair: bool = False,
) -> tuple[str, str]:
    return _resolve_generation_attempt_labels_compat_service(
        candidate_index,
        is_word_budget_repair=is_word_budget_repair,
    )


def _sync_generation_runtime_state(
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
    _sync_generation_runtime_state_compat_service(
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

def _build_generation_candidate_record(
    *,
    full_content: str,
    candidate_chunks: List[str],
    target_word_count: int,
    source: str,
    generation_label: str,
    candidate_index: int,
    candidate_offset: int,
    quality_evaluator: Callable[[str], Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    generation_path: str,
    attempt_kind: str,
) -> Dict[str, Any]:
    return _build_generation_candidate_record_compat_service(
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
        log_warning_fn=logger.warning,
    )
def _get_chapter_candidate_executor_dependencies():
    return _get_chapter_candidate_executor_dependencies_compat_service(
        resolve_generation_attempt_labels_fn=_resolve_generation_attempt_labels,
        sync_generation_runtime_state_fn=_sync_generation_runtime_state,
        collect_generation_candidate_output_fn=_collect_generation_candidate_output,
        build_generation_candidate_record_fn=_build_generation_candidate_record,
    )


async def _generate_best_ranked_candidate(
    *,
    ai_service: AIService,
    base_generate_kwargs: Dict[str, Any],
    target_word_count: int,
    source: str,
    generation_label: str,
    quality_evaluator: Callable[[str], Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    max_candidates: int = CHAPTER_CANDIDATE_RERANK_LIMIT,
    runtime_state: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    return await _generate_best_ranked_candidate_compat_service(
        ai_service=ai_service,
        base_generate_kwargs=base_generate_kwargs,
        target_word_count=target_word_count,
        source=source,
        generation_label=generation_label,
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
        max_candidates=max_candidates,
        runtime_state=runtime_state,
        resolve_generation_attempt_labels_fn=_resolve_generation_attempt_labels,
        sync_generation_runtime_state_fn=_sync_generation_runtime_state,
        collect_generation_candidate_output_fn=_collect_generation_candidate_output,
        build_generation_candidate_record_fn=_build_generation_candidate_record,
    )

# ==================== Runtime Snapshot / Cache seam ====================

async def _clear_task_runtime_caches(task_id: str) -> None:
    await _clear_task_runtime_caches_compat_service(task_id)


async def _batch_task_exists(db_session: AsyncSession, task_id: str) -> bool:
    return await _batch_task_exists_compat_service(db_session, task_id)


async def _upsert_batch_generation_snapshot(
    db_session: AsyncSession,
    task_id: str,
    *,
    latest_quality_metrics: Any = _SNAPSHOT_UNSET,
    quality_metrics_history: Any = _SNAPSHOT_UNSET,
    quality_metrics_summary: Any = _SNAPSHOT_UNSET,
    workflow_runtime_state: Any = _SNAPSHOT_UNSET,
) -> Optional[BatchGenerationSnapshot]:
    return await _upsert_batch_generation_snapshot_compat_service(
        db_session,
        task_id,
        latest_quality_metrics=latest_quality_metrics,
        quality_metrics_history=quality_metrics_history,
        quality_metrics_summary=quality_metrics_summary,
        workflow_runtime_state=workflow_runtime_state,
    )


async def _load_persisted_batch_generation_snapshot(
    db_session: AsyncSession,
    task_id: str,
) -> Optional[BatchGenerationSnapshot]:
    return await _load_persisted_batch_generation_snapshot_compat_service(db_session, task_id)


async def _persist_task_workflow_runtime_snapshot(
    db_session: AsyncSession,
    task_id: str,
    runtime_snapshot: Dict[str, Any],
) -> None:
    await _persist_task_workflow_runtime_snapshot_compat_service(
        db_session,
        task_id,
        runtime_snapshot,
    )


async def _get_task_workflow_runtime_snapshot(
    task_id: str,
    db_session: Optional[AsyncSession] = None,
) -> Dict[str, Any]:
    return await _get_task_workflow_runtime_snapshot_compat_service(
        task_id,
        db_session,
    )


# ==================== Text / Prompt seam ====================

def _trim_text_to_sentence_boundary(text: str, *, hard_limit: int, lookback_chars: int = 220) -> str:
    return _trim_text_to_sentence_boundary_compat_service(
        text,
        hard_limit=hard_limit,
        lookback_chars=lookback_chars,
    )


def _is_likely_chapter_meta_line(line: str) -> bool:
    return _is_likely_chapter_meta_line_compat_service(line)


def _contains_chapter_workflow_meta_text(text: str) -> bool:
    return _contains_chapter_workflow_meta_text_compat_service(text)


def _lightly_polish_template_phrases(text: str) -> str:
    return _lightly_polish_template_phrases_compat_service(text)



def _sanitize_generated_narrative_text(text: str) -> tuple[str, int]:
    return _sanitize_generated_narrative_text_compat_service(text)
def compute_story_quality_metrics(
    content: str,
    chapter_outline: Optional[str],
    world_rules: Optional[str],
    quality_runtime_context: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    return _compute_story_quality_metrics_compat_service(
        content=content,
        chapter_outline=chapter_outline,
        world_rules=world_rules,
        quality_runtime_context=quality_runtime_context,
    )
def _detect_style_profile(
    style_name: Optional[str],
    style_preset_id: Optional[str],
    style_content: Optional[str] = None,
) -> str:
    return _detect_style_profile_compat_service(
        style_name=style_name,
        style_preset_id=style_preset_id,
        style_content=style_content,
    )


def _resolve_generation_temperature(style_profile: str) -> float:
    return _resolve_generation_temperature_compat_service(style_profile)


def _build_chapter_runtime_system_prompt(
    project: Project,
    style_content: str,
    chapter_outline: Optional[str],
    previous_summary: Optional[str] = None,
    style_name: Optional[str] = None,
    style_preset_id: Optional[str] = None,
    target_word_count: Optional[int] = None,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
) -> str:
    return _build_chapter_runtime_system_prompt_compat_service(
        project=project,
        style_content=style_content,
        chapter_outline=chapter_outline,
        previous_summary=previous_summary,
        style_name=style_name,
        style_preset_id=style_preset_id,
        target_word_count=target_word_count,
        story_runtime_contract=story_runtime_contract,
    )

# ==================== Batch Entry seam ====================

async def _run_batch_chapter_analysis(
    db_session: AsyncSession,
    write_lock: Lock,
    batch_id: str,
    chapter: Chapter,
    user_id: str,
    project_id: str,
    retry_count: int,
    max_retries: int,
    ai_service: AIService,
    quality_profile: Optional[Dict[str, Any]] = None,
    story_packet: Optional[StoryPacket] = None,
    generation_guidance: Optional[StoryGenerationGuidance] = None,
    chapter_content_override: Optional[str] = None,
    chapter_word_count_override: Optional[int] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
) -> Tuple[bool, Optional[str]]:
    return await _run_batch_chapter_analysis_service(
        db_session,
        write_lock=write_lock,
        batch_id=batch_id,
        chapter=chapter,
        user_id=user_id,
        project_id=project_id,
        retry_count=retry_count,
        max_retries=max_retries,
        ai_service=ai_service,
        quality_profile=quality_profile,
        story_packet=story_packet,
        generation_guidance=generation_guidance,
        chapter_content_override=chapter_content_override,
        chapter_word_count_override=chapter_word_count_override,
        story_repair_summary=story_repair_summary,
        story_repair_targets=story_repair_targets,
        story_preserve_strengths=story_preserve_strengths,
        story_repair_payload=story_repair_payload,
        create_analysis_task_fn=_create_analysis_task_safely,
        analyze_chapter_background_fn=analyze_chapter_background,
    )


async def execute_batch_generation_in_order(
    batch_id: str,
    user_id: str,
    ai_service: AIService,
    custom_model: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    story_packet: Optional[StoryPacket] = None,
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
    base_quality_profile: Optional[Dict[str, Any]] = None,
):
    """执行兼容层批量生成流程。"""
    return await _execute_batch_generation_in_order_compat_service(
        batch_id=batch_id,
        user_id=user_id,
        ai_service=ai_service,
        custom_model=custom_model,
        temp_narrative_perspective=temp_narrative_perspective,
        story_packet=story_packet,
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
        base_quality_profile=base_quality_profile,
        get_db_write_lock_fn=get_db_write_lock,
        run_generation_fn=generate_single_chapter_for_batch,
        await_generation_result_fn=_await_cancelable_batch_generation_result,
        run_batch_analysis_fn=_run_batch_chapter_analysis,
        resolve_story_repair_state_fn=_resolve_generation_story_repair_state_for_batch,
        sync_task_story_repair_state_fn=_sync_task_story_repair_state,
        publish_task_stream_event_fn=publish_task_stream_event,
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
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    stream_task_id: Optional[str] = None,
    stream_chunks: bool = False,
    retry_count: int = 0,
    max_retries: int = 1,
) -> Dict[str, Any]:
    return await _generate_single_chapter_for_batch_compat_service(
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
        candidate_generator_fn=_generate_best_ranked_candidate,
        default_candidate_limit=CHAPTER_CANDIDATE_RERANK_LIMIT,
        heartbeat_interval_seconds=CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS,
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event,
        resolve_quality_profile_fn=resolve_chapter_quality_profile,
        one_to_one_builder_cls=OneToOneContextBuilder,
        one_to_many_builder_cls=OneToManyContextBuilder,
        get_template_fn=PromptService.get_template,
        format_prompt_fn=PromptService.format_prompt,
        build_runtime_system_prompt_fn=_build_chapter_runtime_system_prompt,
        compute_story_quality_metrics_fn=compute_story_quality_metrics,
        resolve_quality_gate_execution_plan_fn=_resolve_quality_gate_execution_plan,
    )
# ==================== 章节重新生成相关API ====================



# ==================== 局部重写相关API ====================
