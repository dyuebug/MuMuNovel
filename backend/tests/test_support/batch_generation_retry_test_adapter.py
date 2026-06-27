"""Test-only adapter for retired batch generation retry orchestration helpers."""
from __future__ import annotations

import asyncio
from dataclasses import dataclass
from datetime import datetime
from typing import TYPE_CHECKING, Any, Callable, Dict, Optional

from tests.test_support.retired_runtime_test_support import get_logger

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.batch_generation_task import BatchGenerationTask
    from migrator_app.models.chapter import Chapter
    from migrator_app.models.project import Project
    from tests.test_support.ai_gateway.ai_service import AIService
    from tests.test_support.story_packet_test_support import StoryPacket
    from tests.test_support.story_repair_payload_test_support import StoryRepairPayload


SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch retry/runtime orchestration chain; this "
    "test-only adapter keeps the retired Python retry seam available for tests."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "batch_generation_route_flag_retired_test_only_source_map"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "delete_completed_test_seam_migration"

logger = get_logger(__name__)


def _story_repair_payload_service():
    from tests.test_support import story_repair_payload_test_support as story_repair_payload_service

    return story_repair_payload_service


def _generation_payload_schema():
    from tests.test_support.schemas import generation_payload

    return generation_payload


def _foreshadow_service_instance():
    from tests.test_support.foreshadow_test_support import foreshadow_service

    return foreshadow_service


async def clear_task_quality_metrics_cache(*args, **kwargs):
    from tests.test_support.task_quality_snapshot_test_support import (
        clear_task_quality_metrics_cache as impl,
    )

    return await impl(*args, **kwargs)


async def clear_task_workflow_runtime_cache(*args, **kwargs):
    from tests.test_support.task_system import clear_task_workflow_runtime_cache as impl

    return await impl(*args, **kwargs)


async def check_chapter_generation_prerequisites(*args, **kwargs):
    from tests.test_support.chapter_query_test_support import (
        check_chapter_generation_prerequisites as impl,
    )

    return await impl(*args, **kwargs)


def clone_chapter_quality_profile(*args, **kwargs):
    from tests.test_support.chapter_prompt_quality_test_support import (
        clone_chapter_quality_profile as impl,
    )

    return impl(*args, **kwargs)


async def resolve_generation_story_repair_state_for_batch(*args, **kwargs):
    return await _story_repair_payload_service().resolve_generation_story_repair_state_for_batch(
        *args, **kwargs
    )


def resolve_quality_gate_execution_plan(*args, **kwargs):
    return _story_repair_payload_service().resolve_quality_gate_execution_plan(
        *args, **kwargs
    )


def attach_story_runtime_contract(*args, **kwargs):
    return _generation_payload_schema().attach_story_runtime_contract(
        *args, **kwargs
    )


async def record_task_quality_metrics(*args, **kwargs):
    from tests.test_support.task_quality_snapshot_test_support import (
        record_task_quality_metrics as impl,
    )

    return await impl(*args, **kwargs)


async def batch_task_exists(*args, **kwargs):
    from tests.test_support.task_system import batch_task_exists as impl

    return await impl(*args, **kwargs)


async def sync_task_story_repair_state(*args, **kwargs):
    from tests.test_support.task_system import sync_task_story_repair_state as impl

    return await impl(*args, **kwargs)


def build_chapter_generation_quality_history_payload(*args, **kwargs):
    return _generation_payload_schema().build_chapter_generation_quality_history_payload(
        *args, **kwargs
    )


def _normalize_json_payload(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, dict):
        return {str(key): _normalize_json_payload(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_normalize_json_payload(item) for item in value]
    if hasattr(value, "model_dump"):
        return _normalize_json_payload(value.model_dump())
    if hasattr(value, "dict"):
        return _normalize_json_payload(value.dict())
    return str(value)


def build_batch_chapter_draft_attempt(
    *,
    project_id: str,
    chapter_id: Optional[str],
    batch_task_id: Optional[str] = None,
    source: str,
    attempt_state: str,
    quality_gate_action: Optional[str],
    quality_gate_decision: Optional[str],
    full_content: str,
    summary_preview: Optional[str] = None,
    quality_metrics: Optional[Dict[str, Any]] = None,
    repair_payload: Optional[Dict[str, Any]] = None,
) -> "ChapterDraftAttempt":
    from migrator_app.models import ChapterDraftAttempt

    normalized_content = str(full_content or "")
    normalized_summary = str(summary_preview or "").strip()
    if not normalized_summary and normalized_content:
        normalized_summary = normalized_content[:220]

    if isinstance(repair_payload, dict):
        normalized_repair_payload: Optional[Dict[str, Any]] = dict(repair_payload)
    else:
        normalized_repair_payload = {}
    if normalized_content:
        normalized_repair_payload.setdefault("candidate_full_content", normalized_content)
        normalized_repair_payload["content_complete"] = True

    return ChapterDraftAttempt(
        project_id=project_id,
        chapter_id=chapter_id,
        batch_task_id=batch_task_id,
        source=source,
        attempt_state=str(attempt_state or "candidate"),
        quality_gate_action=quality_gate_action,
        quality_gate_decision=quality_gate_decision,
        word_count=len(normalized_content),
        summary_preview=normalized_summary[:500] or None,
        content_preview=normalized_content[:4000] or None,
        quality_metrics=_normalize_json_payload(quality_metrics)
        if isinstance(quality_metrics, dict)
        else None,
        repair_payload=_normalize_json_payload(normalized_repair_payload)
        if normalized_repair_payload
        else None,
    )


def _build_generation_history_payload(
    content: str,
    metrics: Optional[Dict[str, Any]],
    *,
    content_applied: bool = True,
    attempt_state: Optional[str] = None,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
) -> str:
    payload = build_chapter_generation_quality_history_payload(
        content,
        metrics,
        content_applied=content_applied,
        attempt_state=attempt_state,
        story_runtime_contract=story_runtime_contract,
    )
    return payload.model_dump_json(exclude_none=True)


async def apply_generated_batch_chapter_candidate(
    db_session: "AsyncSession",
    *,
    chapter: "Chapter",
    project: "Project",
    write_lock,
    full_content: str,
    word_count: int,
    quality_metrics: Optional[Dict[str, Any]] = None,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
) -> None:
    from migrator_app.models import GenerationHistory

    async with write_lock:
        old_word_count = chapter.word_count or 0
        chapter.content = full_content
        chapter.word_count = word_count
        chapter.status = "completed"
        project.current_words = (project.current_words or 0) - old_word_count + word_count

        history = GenerationHistory(
            project_id=chapter.project_id,
            chapter_id=chapter.id,
            prompt=f"批量生成: 第{chapter.chapter_number}章《{chapter.title}》",
            generated_content=_build_generation_history_payload(
                full_content,
                quality_metrics if isinstance(quality_metrics, dict) else None,
                story_runtime_contract=story_runtime_contract,
            ),
            model="default",
        )
        db_session.add(history)

        await db_session.commit()
        await db_session.refresh(chapter)

    logger.info(f"章节已持久化: 第{chapter.chapter_number}章，共 {word_count} 字")

    try:
        async with write_lock:
            plant_result = await _foreshadow_service_instance().auto_plant_pending_foreshadows(
                db=db_session,
                project_id=chapter.project_id,
                chapter_id=chapter.id,
                chapter_number=chapter.chapter_number,
                chapter_content=full_content,
            )
        if plant_result.get("planted_count", 0) > 0:
            logger.info(f"伏笔埋入 - 已记录 {plant_result['planted_count']} 条")
    except Exception as plant_error:
        logger.warning(f"伏笔回收 - 自动埋入失败: {str(plant_error)}")


@dataclass(frozen=True)
class BatchGenerationExecutionEnvironment:
    batch_id: str
    user_id: str
    ai_service: "AIService"
    write_lock: Any
    emit_event: Callable[..., Any]
    batch_story_packet: "StoryPacket"
    task_base_quality_profile: Dict[str, Any]
    cached_analysis_quality_profile: Dict[str, Any]
    custom_model: Optional[str]
    temp_narrative_perspective: Optional[str]
    creative_mode: Optional[str]
    story_focus: Optional[str]
    plot_stage: Optional[str]
    story_creation_brief: Optional[str]
    quality_preset: Optional[str]
    quality_notes: Optional[str]
    enable_web_research: Optional[bool]
    web_research_query: Optional[str]
    story_repair_summary: Optional[str]
    story_repair_targets: Optional[list[str]]
    story_preserve_strengths: Optional[list[str]]
    stream_chunks: bool
    run_generation_fn: Callable[..., Any]
    await_generation_result_fn: Callable[..., Any]
    run_batch_analysis_fn: Callable[..., Any]


@dataclass(frozen=True)
class BatchGenerationChapterRuntimeState:
    chapter_id: str
    chapter_index: int
    last_generated_summary: Optional[str]
    active_story_repair_state: Dict[str, Any]
    active_story_repair_payload: Optional["StoryRepairPayload"]


@dataclass(frozen=True)
class BatchGenerationChapterExecutionOutcome:
    active_story_repair_state: Dict[str, Any]
    active_story_repair_payload: Optional["StoryRepairPayload"]
    last_generated_summary: Optional[str]


@dataclass(frozen=True)
class BatchGenerationQualityGateRetryPreparation:
    active_story_repair_state: Dict[str, Any]
    active_story_repair_payload: Any
    next_retry_count: int


@dataclass(frozen=True)
class BatchGenerationChapterAttemptPreparation:
    chapter: "Chapter"
    analysis_quality_profile: Dict[str, Any]


async def prepare_batch_generation_chapter_attempt(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    project: "Project",
    chapter_id: str,
    retry_count: int,
    write_lock,
    emit_event,
    cached_analysis_quality_profile: Dict[str, Any],
    clone_quality_profile_fn,
) -> BatchGenerationChapterAttemptPreparation:
    from sqlalchemy import select

    from migrator_app.models.chapter import Chapter

    chapter_result = await db_session.execute(select(Chapter).where(Chapter.id == chapter_id))
    chapter = chapter_result.scalar_one_or_none()
    if chapter is None:
        raise Exception(f"章节 {chapter_id} 不存在")
    if chapter.project_id != project.id:
        raise Exception(f"Chapter {chapter_id} project mismatch")

    async with write_lock:
        task.current_chapter_number = chapter.chapter_number
        task.current_retry_count = retry_count
        await db_session.commit()

    if retry_count == 0:
        await emit_event(
            {
                "type": "chapter_start",
                "chapter_id": chapter_id,
                "chapter_number": chapter.chapter_number,
                "title": chapter.title,
                "progress": 15,
                "phase": "preparing",
                "current_retry_count": retry_count,
                "max_retries": task.max_retries,
            }
        )

    analysis_quality_profile = clone_quality_profile_fn(cached_analysis_quality_profile)
    if not isinstance(analysis_quality_profile, dict):
        analysis_quality_profile = {}

    return BatchGenerationChapterAttemptPreparation(
        chapter=chapter,
        analysis_quality_profile=analysis_quality_profile,
    )


@dataclass(frozen=True)
class BatchGenerationPreparedChapterResult:
    generated_summary: str
    generated_content: str
    generated_word_count: int
    generation_quality_metrics: Optional[Dict[str, Any]]
    generation_story_runtime_contract: Optional[Dict[str, Any]]
    quality_gate_plan: Dict[str, Any]
    quality_gate_snapshot: Optional[Dict[str, Any]]
    quality_gate_action: str
    quality_gate_requires_followup: bool
    should_run_analysis: bool
    metrics_event: Optional[Dict[str, Any]]


def prepare_batch_generation_chapter_result(
    generation_result: Dict[str, Any],
    *,
    chapter: "Chapter",
    retry_count: int,
    max_retries: int,
    active_story_repair_payload,
    enable_analysis: bool,
    resolve_quality_gate_plan_fn,
    attach_story_runtime_contract_fn,
) -> BatchGenerationPreparedChapterResult:
    generated_summary = str(generation_result.get("summary_preview") or "").strip()
    generated_content = str(generation_result.get("full_content") or "")
    generated_word_count = int(generation_result.get("word_count") or len(generated_content))
    generation_quality_metrics = generation_result.get("quality_metrics")
    generation_story_runtime_contract = generation_result.get("story_runtime_contract")

    quality_gate_plan = generation_result.get("quality_gate_plan") or resolve_quality_gate_plan_fn(
        generation_quality_metrics if isinstance(generation_quality_metrics, dict) else None,
        retry_count=retry_count,
        max_retries=max_retries,
        current_story_repair_payload=active_story_repair_payload,
        scope="batch",
    )
    if not isinstance(quality_gate_plan, dict):
        quality_gate_plan = {}

    quality_gate_snapshot = quality_gate_plan.get("quality_gate")
    if isinstance(generation_quality_metrics, dict) and isinstance(quality_gate_snapshot, dict):
        generation_quality_metrics = {
            **generation_quality_metrics,
            "quality_gate": quality_gate_snapshot,
        }
    elif isinstance(quality_gate_snapshot, dict):
        generation_quality_metrics = {"quality_gate": quality_gate_snapshot}

    generation_quality_metrics = attach_story_runtime_contract_fn(
        generation_quality_metrics,
        generation_story_runtime_contract
        if isinstance(generation_story_runtime_contract, dict)
        else None,
    )

    metrics_event = None
    if isinstance(generation_quality_metrics, dict):
        metrics_event = {
            "type": "quality_metrics",
            "chapter_id": chapter.id,
            "chapter_number": chapter.chapter_number,
            **generation_quality_metrics,
        }

    quality_gate_action = str(quality_gate_plan.get("action") or "apply")
    quality_gate_requires_followup = quality_gate_action in {"retry", "manual_review"}
    should_run_analysis = enable_analysis or quality_gate_requires_followup

    return BatchGenerationPreparedChapterResult(
        generated_summary=generated_summary,
        generated_content=generated_content,
        generated_word_count=generated_word_count,
        generation_quality_metrics=generation_quality_metrics
        if isinstance(generation_quality_metrics, dict)
        else None,
        generation_story_runtime_contract=generation_story_runtime_contract
        if isinstance(generation_story_runtime_contract, dict)
        else None,
        quality_gate_plan=quality_gate_plan,
        quality_gate_snapshot=quality_gate_snapshot
        if isinstance(quality_gate_snapshot, dict)
        else None,
        quality_gate_action=quality_gate_action,
        quality_gate_requires_followup=quality_gate_requires_followup,
        should_run_analysis=should_run_analysis,
        metrics_event=metrics_event,
    )


async def fail_batch_generation_after_analysis(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    chapter: "Chapter",
    chapter_id: str,
    analysis_error: Optional[str],
    write_lock,
    emit_event,
) -> None:
    failed_info = {
        "chapter_id": chapter_id,
        "chapter_number": chapter.chapter_number,
        "title": chapter.title,
        "error": f"章节分析失败，已重试3次: {analysis_error}",
        "retry_count": 3,
    }

    async with write_lock:
        task.failed_chapters = [*(task.failed_chapters or []), failed_info]
        task.status = "failed"
        task.error_message = (
            f"第{chapter.chapter_number}章分析失败，已重试3次: {analysis_error}"[:500]
        )
        task.completed_at = datetime.now()
        task.current_retry_count = 0
        await db_session.commit()

    logger.error(f"章节分析失败: 第{chapter.chapter_number}章已终止")
    await emit_event(
        {
            "type": "error",
            "error": task.error_message or "章节分析失败",
            "code": 500,
            "phase": "failed",
        }
    )
    await emit_event({"type": "done"})


async def fail_batch_generation_after_max_retries(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    chapter: Optional["Chapter"],
    chapter_id: str,
    last_error: str,
    retry_count: int,
    write_lock,
    emit_event,
) -> None:
    chapter_number = chapter.chapter_number if chapter else -1
    chapter_title = chapter.title if chapter else "未命名章节"
    failed_info = {
        "chapter_id": chapter_id,
        "chapter_number": chapter_number,
        "title": chapter_title,
        "error": last_error,
        "retry_count": retry_count - 1,
    }

    async with write_lock:
        task.failed_chapters = [*(task.failed_chapters or []), failed_info]
        task.status = "failed"
        task.error_message = (
            f"第{chapter_number}章生成失败(重试{retry_count-1}次): {last_error}"[:500]
        )
        task.completed_at = datetime.now()
        task.current_retry_count = 0
        await db_session.commit()

    logger.error(f"章节生成失败: 第{chapter_number}章已终止")
    await emit_event(
        {
            "type": "error",
            "error": task.error_message or last_error,
            "code": 500,
            "phase": "failed",
        }
    )
    await emit_event({"type": "done"})


async def handle_batch_generation_quality_gate_retry(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    chapter: "Chapter",
    project: "Project",
    batch_id: str,
    chapter_id: str,
    retry_count: int,
    write_lock,
    emit_event,
    sync_task_story_repair_state_fn,
    quality_gate_plan: Dict[str, Any],
    quality_gate_snapshot: Optional[Dict[str, Any]],
    generated_content: str,
    generated_summary: str,
    generation_quality_metrics: Optional[Dict[str, Any]],
    active_story_repair_payload,
    active_story_repair_state: Dict[str, Any],
) -> BatchGenerationQualityGateRetryPreparation:
    active_story_repair_payload = (
        quality_gate_plan.get("repair_payload") or active_story_repair_payload
    )
    active_story_repair_state = await sync_task_story_repair_state_fn(
        batch_id,
        payload=active_story_repair_payload,
        active_story_repair_payload=quality_gate_plan.get("active_story_repair_payload"),
        db_session=db_session,
    )

    next_retry_count = retry_count + 1
    retry_attempt = build_batch_chapter_draft_attempt(
        project_id=project.id,
        chapter_id=chapter_id,
        batch_task_id=batch_id,
        source="batch",
        attempt_state="retry",
        quality_gate_action="retry",
        quality_gate_decision=(quality_gate_snapshot or {}).get("decision"),
        full_content=generated_content,
        summary_preview=generated_summary,
        quality_metrics=generation_quality_metrics
        if isinstance(generation_quality_metrics, dict)
        else None,
        repair_payload=(
            quality_gate_plan.get("active_story_repair_payload")
            or active_story_repair_state.get("active_story_repair_payload")
        ),
    )
    async with write_lock:
        task.current_retry_count = next_retry_count
        db_session.add(retry_attempt)
        await db_session.commit()

    retry_message = quality_gate_plan.get("message") or (
        f"Chapter {chapter.chapter_number} triggered a quality-gate retry."
    )
    await emit_event(
        {
            "type": "quality_gate_retry",
            "chapter_id": chapter_id,
            "chapter_number": chapter.chapter_number,
            "message": retry_message,
            "progress": 74,
            "status": "running",
            "phase": "generating",
            "current_retry_count": next_retry_count,
            "max_retries": task.max_retries,
            "quality_gate": quality_gate_snapshot,
            "active_story_repair_payload": quality_gate_plan.get(
                "active_story_repair_payload"
            ),
        }
    )

    return BatchGenerationQualityGateRetryPreparation(
        active_story_repair_state=dict(active_story_repair_state)
        if isinstance(active_story_repair_state, dict)
        else {},
        active_story_repair_payload=active_story_repair_payload,
        next_retry_count=next_retry_count,
    )


@dataclass(frozen=True)
class BatchGenerationAppliedChapterState:
    active_story_repair_state: Dict[str, Any]
    active_story_repair_payload: Any
    last_generated_summary: Optional[str]


async def apply_successful_batch_generation_chapter(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    chapter: "Chapter",
    batch_id: str,
    retry_count: int,
    emit_event,
    project: "Project",
    write_lock,
    resolve_story_repair_state_fn,
    sync_task_story_repair_state_fn,
    generated_content: str,
    generated_word_count: int,
    generation_quality_metrics: Optional[Dict[str, Any]],
    generation_story_runtime_contract: Optional[Dict[str, Any]],
    generated_summary: str,
    should_run_analysis: bool,
    story_repair_summary: Optional[str],
    story_repair_targets: Optional[list[str]],
    story_preserve_strengths: Optional[list[str]],
) -> BatchGenerationAppliedChapterState:
    await apply_generated_batch_chapter_candidate(
        db_session=db_session,
        chapter=chapter,
        project=project,
        write_lock=write_lock,
        full_content=generated_content,
        word_count=generated_word_count,
        quality_metrics=generation_quality_metrics,
        story_runtime_contract=generation_story_runtime_contract,
    )

    last_generated_summary = None
    if generated_summary:
        last_generated_summary = (
            f"Chapter {chapter.chapter_number} ({chapter.title}): {generated_summary}"
        )
        logger.info(
            f"Updated previous chapter summary context: {last_generated_summary[:50]}..."
        )

    active_story_repair_state = await resolve_story_repair_state_fn(
        db_session,
        project_id=task.project_id,
        before_chapter_number=chapter.chapter_number + 1,
        story_repair_summary=story_repair_summary,
        story_repair_targets=story_repair_targets,
        story_preserve_strengths=story_preserve_strengths,
    )
    active_story_repair_payload = active_story_repair_state.get("payload")
    active_story_repair_state = await sync_task_story_repair_state_fn(
        batch_id,
        story_repair_state=active_story_repair_state,
        db_session=db_session,
    )

    logger.info(f"Chapter generation completed: #{chapter.chapter_number}")
    await emit_event(
        {
            "type": "progress",
            "message": f"第 {chapter.chapter_number} 章已生成",
            "progress": 80 if not should_run_analysis else 70,
            "status": "running",
            "phase": "saving" if not should_run_analysis else "generating",
            "current_retry_count": retry_count,
            "max_retries": task.max_retries,
        }
    )

    return BatchGenerationAppliedChapterState(
        active_story_repair_state=dict(active_story_repair_state)
        if isinstance(active_story_repair_state, dict)
        else {},
        active_story_repair_payload=active_story_repair_payload,
        last_generated_summary=last_generated_summary,
    )


async def finalize_successful_batch_generation_chapter(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    emit_event,
    write_lock,
) -> None:
    async with write_lock:
        task.completed_chapters += 1
        task.current_retry_count = 0
        await db_session.commit()

    logger.info(f"批量进度: {task.completed_chapters}/{task.total_chapters}")
    completed_ratio = task.completed_chapters / max(task.total_chapters, 1)
    await emit_event(
        {
            "type": "progress",
            "message": f"已完成 {task.completed_chapters}/{task.total_chapters}",
            "progress": 15 + int(completed_ratio * 80),
            "status": "running",
            "phase": "loading"
            if task.completed_chapters < task.total_chapters
            else "saving",
            "current_retry_count": 0,
            "max_retries": task.max_retries,
        }
    )


async def clear_batch_generation_execution_caches(task_id: str) -> None:
    await clear_task_quality_metrics_cache(task_id)
    await clear_task_workflow_runtime_cache(task_id)


async def execute_batch_generation_chapter_with_retries(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    project: "Project",
    execution_context: BatchGenerationExecutionEnvironment,
    runtime_state: BatchGenerationChapterRuntimeState,
) -> Optional[BatchGenerationChapterExecutionOutcome]:
    from sqlalchemy import select

    from migrator_app.models.chapter import Chapter

    retry_count = 0
    chapter_success = False
    chapter = None
    current_last_generated_summary = runtime_state.last_generated_summary
    current_active_story_repair_state = runtime_state.active_story_repair_state
    current_active_story_repair_payload = runtime_state.active_story_repair_payload

    while retry_count <= task.max_retries and not chapter_success:
        try:
            attempt_preparation = await prepare_batch_generation_chapter_attempt(
                db_session,
                task=task,
                project=project,
                chapter_id=runtime_state.chapter_id,
                retry_count=retry_count,
                write_lock=execution_context.write_lock,
                emit_event=execution_context.emit_event,
                cached_analysis_quality_profile=execution_context.cached_analysis_quality_profile,
                clone_quality_profile_fn=clone_chapter_quality_profile,
            )
            chapter = attempt_preparation.chapter
            analysis_quality_profile = attempt_preparation.analysis_quality_profile

            if retry_count > 0:
                logger.info(
                    f"重试 [{runtime_state.chapter_index}/{task.total_chapters}] 继续生成 (第{retry_count}次): "
                    f"第{chapter.chapter_number}章《{chapter.title}》"
                )
            else:
                logger.info(
                    f"开始 [{runtime_state.chapter_index}/{task.total_chapters}] 生成章节: "
                    f"第{chapter.chapter_number}章《{chapter.title}》"
                )

            can_generate, error_msg, _ = await check_chapter_generation_prerequisites(
                db_session, chapter
            )
            if not can_generate:
                raise Exception(f"章节生成失败: {error_msg}")

            generation_result = await execution_context.await_generation_result_fn(
                generation_coro=execution_context.run_generation_fn(
                    db_session=db_session,
                    chapter=chapter,
                    user_id=execution_context.user_id,
                    style_id=task.style_id,
                    target_word_count=task.target_word_count,
                    ai_service=execution_context.ai_service,
                    write_lock=execution_context.write_lock,
                    story_packet=execution_context.batch_story_packet,
                    base_quality_profile=execution_context.task_base_quality_profile,
                    custom_model=execution_context.custom_model,
                    previous_summary_context=current_last_generated_summary,
                    temp_narrative_perspective=execution_context.temp_narrative_perspective,
                    creative_mode=execution_context.creative_mode,
                    story_focus=execution_context.story_focus,
                    plot_stage=execution_context.plot_stage,
                    story_creation_brief=execution_context.story_creation_brief,
                    quality_preset=execution_context.quality_preset,
                    quality_notes=execution_context.quality_notes,
                    enable_web_research=execution_context.enable_web_research,
                    web_research_query=execution_context.web_research_query,
                    story_repair_summary=execution_context.story_repair_summary,
                    story_repair_targets=execution_context.story_repair_targets,
                    story_preserve_strengths=execution_context.story_preserve_strengths,
                    story_repair_payload=current_active_story_repair_payload,
                    active_story_repair_snapshot=current_active_story_repair_state.get(
                        "active_story_repair_payload"
                    ),
                    story_repair_state=current_active_story_repair_state,
                    stream_task_id=execution_context.batch_id,
                    stream_chunks=execution_context.stream_chunks,
                    retry_count=retry_count,
                    max_retries=task.max_retries,
                ),
                task=task,
                db_session=db_session,
            )

            prepared_result = prepare_batch_generation_chapter_result(
                generation_result,
                chapter=chapter,
                retry_count=retry_count,
                max_retries=task.max_retries,
                active_story_repair_payload=current_active_story_repair_payload,
                enable_analysis=task.enable_analysis,
                resolve_quality_gate_plan_fn=resolve_quality_gate_execution_plan,
                attach_story_runtime_contract_fn=attach_story_runtime_contract,
            )
            generated_summary = prepared_result.generated_summary
            generated_content = prepared_result.generated_content
            generated_word_count = prepared_result.generated_word_count
            generation_quality_metrics = prepared_result.generation_quality_metrics
            generation_story_runtime_contract = (
                prepared_result.generation_story_runtime_contract
            )
            quality_gate_plan = prepared_result.quality_gate_plan
            quality_gate_snapshot = prepared_result.quality_gate_snapshot
            quality_gate_action = prepared_result.quality_gate_action
            should_run_analysis = prepared_result.should_run_analysis

            if prepared_result.metrics_event is not None:
                await execution_context.emit_event(prepared_result.metrics_event)
                await record_task_quality_metrics(
                    execution_context.batch_id,
                    prepared_result.metrics_event,
                    db_session=db_session,
                )

            retry_preparation = None
            if quality_gate_action == "retry":
                retry_preparation = await handle_batch_generation_quality_gate_retry(
                    db_session,
                    task=task,
                    chapter=chapter,
                    project=project,
                    batch_id=execution_context.batch_id,
                    chapter_id=runtime_state.chapter_id,
                    retry_count=retry_count,
                    write_lock=execution_context.write_lock,
                    emit_event=execution_context.emit_event,
                    sync_task_story_repair_state_fn=sync_task_story_repair_state,
                    quality_gate_plan=quality_gate_plan,
                    quality_gate_snapshot=quality_gate_snapshot,
                    generated_content=generated_content,
                    generated_summary=generated_summary,
                    generation_quality_metrics=generation_quality_metrics,
                    active_story_repair_payload=current_active_story_repair_payload,
                    active_story_repair_state=current_active_story_repair_state,
                )
                current_active_story_repair_state = (
                    retry_preparation.active_story_repair_state
                )
                current_active_story_repair_payload = (
                    retry_preparation.active_story_repair_payload
                )
            else:
                if quality_gate_action == "manual_review":
                    current_active_story_repair_payload = (
                        quality_gate_plan.get("repair_payload")
                        or current_active_story_repair_payload
                    )
                    current_active_story_repair_state = await sync_task_story_repair_state(
                        execution_context.batch_id,
                        payload=current_active_story_repair_payload,
                        active_story_repair_payload=quality_gate_plan.get(
                            "active_story_repair_payload"
                        ),
                        db_session=db_session,
                    )
                    notice_message = quality_gate_plan.get("message") or (
                        f"Chapter {chapter.chapter_number} hit the quality gate; content is kept and optimization is recommended."
                    )
                    await execution_context.emit_event(
                        {
                            "type": "progress",
                            "chapter_id": runtime_state.chapter_id,
                            "chapter_number": chapter.chapter_number,
                            "message": notice_message,
                            "progress": 76,
                            "status": "running",
                            "phase": "saving",
                            "current_retry_count": retry_count,
                            "max_retries": task.max_retries,
                            "quality_gate": quality_gate_snapshot,
                            "active_story_repair_payload": quality_gate_plan.get(
                                "active_story_repair_payload"
                            ),
                        }
                    )
                applied_state = await apply_successful_batch_generation_chapter(
                    db_session,
                    task=task,
                    chapter=chapter,
                    batch_id=execution_context.batch_id,
                    retry_count=retry_count,
                    emit_event=execution_context.emit_event,
                    project=project,
                    write_lock=execution_context.write_lock,
                    resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_batch,
                    sync_task_story_repair_state_fn=sync_task_story_repair_state,
                    generated_content=generated_content,
                    generated_word_count=generated_word_count,
                    generation_quality_metrics=generation_quality_metrics,
                    generation_story_runtime_contract=generation_story_runtime_contract,
                    generated_summary=generated_summary,
                    should_run_analysis=should_run_analysis,
                    story_repair_summary=execution_context.story_repair_summary,
                    story_repair_targets=execution_context.story_repair_targets,
                    story_preserve_strengths=execution_context.story_preserve_strengths,
                )
                current_active_story_repair_state = (
                    applied_state.active_story_repair_state
                )
                current_active_story_repair_payload = (
                    applied_state.active_story_repair_payload
                )
                if applied_state.last_generated_summary:
                    current_last_generated_summary = applied_state.last_generated_summary

            if not await batch_task_exists(db_session, execution_context.batch_id):
                await clear_batch_generation_execution_caches(execution_context.batch_id)
                logger.info(
                    f"Stop batch generation because task no longer exists: {execution_context.batch_id}"
                )
                return None

            chapter_exists_result = await db_session.execute(
                select(Chapter.id).where(Chapter.id == runtime_state.chapter_id)
            )
            if chapter_exists_result.scalar_one_or_none() is None:
                await clear_batch_generation_execution_caches(execution_context.batch_id)
                logger.info(
                    f"Stop batch generation because chapter no longer exists: {runtime_state.chapter_id}"
                )
                return None

            if should_run_analysis:
                analysis_success, analysis_error = (
                    await execution_context.run_batch_analysis_fn(
                        db_session=db_session,
                        write_lock=execution_context.write_lock,
                        batch_id=execution_context.batch_id,
                        chapter=chapter,
                        user_id=execution_context.user_id,
                        project_id=task.project_id,
                        retry_count=retry_count,
                        max_retries=task.max_retries,
                        ai_service=execution_context.ai_service,
                        quality_profile=analysis_quality_profile,
                        story_packet=execution_context.batch_story_packet,
                        chapter_content_override=generated_content,
                        chapter_word_count_override=generated_word_count,
                        story_repair_summary=execution_context.story_repair_summary,
                        story_repair_targets=execution_context.story_repair_targets,
                        story_preserve_strengths=execution_context.story_preserve_strengths,
                        story_repair_payload=current_active_story_repair_payload,
                    )
                )
                if not analysis_success:
                    await fail_batch_generation_after_analysis(
                        db_session,
                        task=task,
                        chapter=chapter,
                        chapter_id=runtime_state.chapter_id,
                        analysis_error=analysis_error,
                        write_lock=execution_context.write_lock,
                        emit_event=execution_context.emit_event,
                    )
                    return None

            if quality_gate_action == "retry":
                next_retry_count = retry_preparation.next_retry_count
                wait_time = min(2**next_retry_count, 10)
                logger.info(
                    f"Quality-gate retry for chapter {chapter.chapter_number}; "
                    f"waiting {wait_time}s before retry #{next_retry_count}"
                )
                await asyncio.sleep(wait_time)
                retry_count = next_retry_count
                continue

            chapter_success = True
            await finalize_successful_batch_generation_chapter(
                db_session,
                task=task,
                emit_event=execution_context.emit_event,
                write_lock=execution_context.write_lock,
            )
        except Exception as e:
            last_error = str(e)
            logger.error(
                f"批量生成错误: Chapter {chapter.chapter_number if chapter else '?'} generation failed: {last_error}"
            )

            retry_count += 1
            if retry_count <= task.max_retries:
                wait_time = min(2**retry_count, 10)
                logger.info(f"将在 {wait_time} 秒后重试...")
                await asyncio.sleep(wait_time)
            else:
                logger.error(
                    f"❌ 已超过最大重试次数({task.max_retries}): "
                    f"第{chapter.chapter_number if chapter else '?'}章"
                )
                await fail_batch_generation_after_max_retries(
                    db_session,
                    task=task,
                    chapter=chapter,
                    chapter_id=runtime_state.chapter_id,
                    last_error=last_error,
                    retry_count=retry_count,
                    write_lock=execution_context.write_lock,
                    emit_event=execution_context.emit_event,
                )
                return None

    if not chapter_success:
        return None

    return BatchGenerationChapterExecutionOutcome(
        active_story_repair_state=current_active_story_repair_state,
        active_story_repair_payload=current_active_story_repair_payload,
        last_generated_summary=current_last_generated_summary,
    )




