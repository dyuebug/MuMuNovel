"""章节管理API"""
SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active aggregate chapter route groups; this Python module "
    "is kept only as repointed rollback/source-map material after explicit "
    "aggregate repoint approval."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/api/chapter_generation_routes.rs; backend-rs/src/api/health.rs"
SOURCE_MAP_ROLLBACK_FLAG = "aggregate_chapters_python_source_map"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "repoint"

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
from app.services.chapter_generation.runtime.service import (
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
    sync_chapter_candidate_runtime_state as _sync_chapter_candidate_runtime_state_service,
)
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text as _contains_chapter_workflow_meta_text_service,
    is_likely_chapter_meta_line as _is_likely_chapter_meta_line_service,
    lightly_polish_template_phrases as _lightly_polish_template_phrases_service,
    sanitize_generated_narrative_text as _sanitize_generated_narrative_text_service,
    trim_text_to_sentence_boundary as _trim_text_to_sentence_boundary_service,
)
from app.services.chapter_generation.runtime.prompt_service import (
    build_chapter_runtime_system_prompt as _build_chapter_runtime_system_prompt_service,
    detect_style_profile as _detect_style_profile_service,
    resolve_generation_temperature as _resolve_generation_temperature_service,
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
    build_story_repair_payload_from_metrics,
    build_story_repair_runtime_state,
    merge_story_repair_payload,
    normalize_story_repair_payload,
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
from app.services.project_quality_trend_service import (
    build_project_quality_trend_response_payload,
    get_project_quality_trend_snapshot_with_default_wiring as _get_project_quality_trend_snapshot_service,
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
from app.services.analysis_task_service import create_analysis_task_safely as _create_analysis_task_safely
from app.services.chapter_generation.prerequisite_service import (
    check_chapter_generation_prerequisites as check_prerequisites,
)
from app.services.regeneration_task_service import (
    create_regeneration_task,
    load_latest_regeneration_analysis,
    mark_latest_regeneration_task_failed,
)
from app.services.chapter_generation.stream.request_policy_service import (
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
from app.services.task_workflow_runtime_service import (
    publish_task_stream_event,
    _set_task_active_story_repair_payload,
    task_workflow_lock,
    task_workflow_state_cache,
)
from app.services.task_workflow_runtime_service import SNAPSHOT_UNSET as _SNAPSHOT_UNSET_SERVICE
from app.services.task_quality_snapshot_service import (
    record_task_quality_metrics as _record_task_quality_metrics,
    task_quality_metrics_cache,
)
from app.services.chapter_generation.history_service import (
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
    from app.services.batch_generation_run_service import (
        get_db_write_lock as get_db_write_lock_service,
    )

    return await get_db_write_lock_service(user_id)


_SNAPSHOT_UNSET = _SNAPSHOT_UNSET_SERVICE

CHAPTER_CANDIDATE_RERANK_LIMIT = 2

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
    return await _get_project_quality_trend_snapshot_service(
        project_id=project_id,
        limit=limit,
        items=items,
        metrics_history=metrics_history,
        total_chapters=total_chapters,
        analyzed_chapters=analyzed_chapters,
        last_generated_at=last_generated_at,
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


def _resolve_generation_attempt_labels(
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
    _sync_chapter_candidate_runtime_state_service(
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
        log_warning_fn=logger.warning,
    )


def _get_chapter_candidate_executor_dependencies():
    from app.services.chapter_candidate_executor_service import (
        get_chapter_candidate_executor_dependencies,
    )

    return get_chapter_candidate_executor_dependencies(
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
    from app.services.chapter_candidate_executor_service import (
        generate_best_ranked_candidate,
    )

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
        resolve_generation_attempt_labels_fn=_resolve_generation_attempt_labels,
        sync_generation_runtime_state_fn=_sync_generation_runtime_state,
        collect_generation_candidate_output_fn=_collect_generation_candidate_output,
        build_generation_candidate_record_fn=_build_generation_candidate_record,
    )

# ==================== Runtime Snapshot / Cache seam ====================

async def _clear_task_runtime_caches(task_id: str) -> None:
    from app.services.task_workflow_runtime_service import clear_task_runtime_caches

    await clear_task_runtime_caches(task_id)


async def _batch_task_exists(db_session: AsyncSession, task_id: str) -> bool:
    from app.services.task_workflow_runtime_service import batch_task_exists

    return await batch_task_exists(db_session, task_id)


async def _upsert_batch_generation_snapshot(
    db_session: AsyncSession,
    task_id: str,
    *,
    latest_quality_metrics: Any = _SNAPSHOT_UNSET,
    quality_metrics_history: Any = _SNAPSHOT_UNSET,
    quality_metrics_summary: Any = _SNAPSHOT_UNSET,
    workflow_runtime_state: Any = _SNAPSHOT_UNSET,
) -> Optional[BatchGenerationSnapshot]:
    from app.services.task_workflow_runtime_service import (
        upsert_batch_generation_snapshot,
    )

    return await upsert_batch_generation_snapshot(
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
    from app.services.task_workflow_runtime_service import (
        load_persisted_batch_generation_snapshot,
    )

    return await load_persisted_batch_generation_snapshot(db_session, task_id)


async def _persist_task_workflow_runtime_snapshot(
    db_session: AsyncSession,
    task_id: str,
    runtime_snapshot: Dict[str, Any],
) -> None:
    from app.services.task_workflow_runtime_service import (
        persist_task_workflow_runtime_snapshot,
    )

    await persist_task_workflow_runtime_snapshot(
        db_session,
        task_id,
        runtime_snapshot,
    )


async def _get_task_workflow_runtime_snapshot(
    task_id: str,
    db_session: Optional[AsyncSession] = None,
) -> Dict[str, Any]:
    from app.services.task_workflow_runtime_service import (
        get_task_workflow_runtime_snapshot,
    )

    return await get_task_workflow_runtime_snapshot(
        task_id,
        db_session,
    )


# ==================== Text / Prompt seam ====================

def _trim_text_to_sentence_boundary(text: str, *, hard_limit: int, lookback_chars: int = 220) -> str:
    from app.services.chapter_generated_text_service import trim_text_to_sentence_boundary

    return trim_text_to_sentence_boundary(
        text,
        hard_limit=hard_limit,
        lookback_chars=lookback_chars,
    )


def _is_likely_chapter_meta_line(line: str) -> bool:
    from app.services.chapter_generated_text_service import is_likely_chapter_meta_line

    return is_likely_chapter_meta_line(line)


def _contains_chapter_workflow_meta_text(text: str) -> bool:
    from app.services.chapter_generated_text_service import (
        contains_chapter_workflow_meta_text,
    )

    return contains_chapter_workflow_meta_text(text)


def _lightly_polish_template_phrases(text: str) -> str:
    from app.services.chapter_generated_text_service import lightly_polish_template_phrases

    return lightly_polish_template_phrases(text)



def _sanitize_generated_narrative_text(text: str) -> tuple[str, int]:
    from app.services.chapter_generated_text_service import (
        sanitize_generated_narrative_text,
    )

    return sanitize_generated_narrative_text(text)
def compute_story_quality_metrics(
    content: str,
    chapter_outline: Optional[str],
    world_rules: Optional[str],
    quality_runtime_context: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    return _compute_story_quality_metrics(
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
    from app.services.chapter_generation.runtime.prompt_service import (
        detect_style_profile,
    )

    return detect_style_profile(
        style_name=style_name,
        style_preset_id=style_preset_id,
        style_content=style_content,
    )


def _resolve_generation_temperature(style_profile: str) -> float:
    from app.services.chapter_generation.runtime.prompt_service import (
        resolve_generation_temperature,
    )

    return resolve_generation_temperature(style_profile)


def _build_chapter_runtime_system_prompt(
    project: Project,
    style_content: str,
    chapter_outline: Optional[str],
    previous_summary: Optional[str] = None,
    style_name: Optional[str] = None,
    style_preset_id: Optional[str] = None,
    target_word_count: Optional[int] = None,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
    web_research_grounding_block: Optional[str] = None,
) -> str:
    from app.services.chapter_generation.runtime.prompt_service import (
        build_chapter_runtime_system_prompt,
    )

    return build_chapter_runtime_system_prompt(
        project=project,
        style_content=style_content,
        chapter_outline=chapter_outline,
        previous_summary=previous_summary,
        style_name=style_name,
        style_preset_id=style_preset_id,
        target_word_count=target_word_count,
        story_runtime_contract=story_runtime_contract,
        web_research_grounding_block=web_research_grounding_block,
    )

# ==================== 章节重新生成相关API ====================



# ==================== 局部重写相关API ====================
