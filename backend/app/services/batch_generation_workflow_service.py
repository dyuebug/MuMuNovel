"""批量生成 workflow / persistence 协调 helper。"""
from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import TYPE_CHECKING, Any, Callable, Dict, Optional, Sequence

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch create/resume persistence and dispatch chain; "
    "this Python workflow helper is kept only as frozen rollback/source-map "
    "material for legacy batch task creation fallback."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.logger import get_logger

if TYPE_CHECKING:
    from fastapi import BackgroundTasks
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.batch_generation_task import BatchGenerationTask
    from app.models.project import Project
    from app.services.ai_service import AIService
    from app.services.chapter_quality_context_service import StoryPacket
    from app.services.story_repair_payload_service import StoryRepairPayload


logger = get_logger(__name__)


def _chapter_quality_context_service():
    from app.services import chapter_quality_context_service

    return chapter_quality_context_service


def _story_repair_payload_service():
    from app.services import story_repair_payload_service

    return story_repair_payload_service


def _story_repair_payload_type():
    return _story_repair_payload_service().StoryRepairPayload


def build_story_generation_packet_with_project_continuity(*args, **kwargs):
    return _chapter_quality_context_service().build_story_generation_packet_with_project_continuity(
        *args,
        **kwargs,
    )


async def resolve_chapter_quality_profile(*args, **kwargs):
    return await _chapter_quality_context_service().resolve_chapter_quality_profile(*args, **kwargs)


def resolve_story_repair_prompt_kwargs(*args, **kwargs):
    return _story_repair_payload_service().resolve_story_repair_prompt_kwargs(*args, **kwargs)


def _build_batch_generation_execution_kwargs(
    *,
    batch_id: str,
    user_id: str,
    ai_service: "AIService",
    custom_model: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    story_packet: Optional["StoryPacket"] = None,
    base_quality_profile: Optional[Dict[str, Any]] = None,
    enable_web_research: Optional[bool] = None,
    web_research_query: Optional[str] = None,
    story_repair_payload: Optional["StoryRepairPayload"] = None,
) -> Dict[str, Any]:
    kwargs: Dict[str, Any] = {
        "batch_id": batch_id,
        "user_id": user_id,
        "ai_service": ai_service,
        "custom_model": custom_model,
        "temp_narrative_perspective": temp_narrative_perspective,
        "story_packet": story_packet,
        "base_quality_profile": base_quality_profile,
        "enable_web_research": enable_web_research,
        "web_research_query": web_research_query,
        "story_repair_payload": story_repair_payload,
    }
    kwargs.update(resolve_story_repair_prompt_kwargs(story_repair_payload))
    return kwargs


async def create_batch_generation_task_record(
    db_session: "AsyncSession",
    *,
    project_id: str,
    user_id: str,
    start_chapter_number: int,
    chapter_ids: Sequence[str],
    style_id: Optional[int],
    target_word_count: int,
    enable_analysis: bool,
    max_retries: int,
) -> "BatchGenerationTask":
    from app.models.batch_generation_task import BatchGenerationTask

    normalized_chapter_ids = list(chapter_ids)
    chapter_count = len(normalized_chapter_ids)
    task = BatchGenerationTask(
        project_id=project_id,
        user_id=user_id,
        start_chapter_number=start_chapter_number,
        chapter_count=chapter_count,
        chapter_ids=normalized_chapter_ids,
        style_id=style_id,
        target_word_count=target_word_count,
        enable_analysis=enable_analysis,
        max_retries=max_retries,
        status="pending",
        total_chapters=chapter_count,
        completed_chapters=0,
        failed_chapters=[],
        current_retry_count=0,
    )
    db_session.add(task)
    await db_session.commit()
    await db_session.refresh(task)
    return task


def calculate_estimated_time(
    chapter_count: int,
    target_word_count: int,
    enable_analysis: bool,
) -> int:
    """计算预估耗时（分钟）。"""
    generation_time_per_chapter = (target_word_count / 3000) * 2
    analysis_time_per_chapter = 1 if enable_analysis else 0
    total_time = chapter_count * (generation_time_per_chapter + analysis_time_per_chapter)
    return max(1, int(total_time))


def enqueue_batch_generation_execution(
    background_tasks: "BackgroundTasks",
    execution_callable: Callable[..., Any],
    **kwargs: Any,
) -> None:
    """统一注册批量生成后台任务。"""
    background_tasks.add_task(execution_callable, **_build_batch_generation_execution_kwargs(**kwargs))


async def mark_batch_generation_current_chapter(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    chapter_id: str,
    write_lock,
) -> None:
    async with write_lock:
        task.current_chapter_id = chapter_id
        task.current_retry_count = 0
        await db_session.commit()


async def handle_cancelled_batch_generation_execution(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    batch_id: str,
    write_lock,
    emit_event,
    refresh_before_commit: bool = False,
) -> None:
    if refresh_before_commit:
        await db_session.refresh(task)

    if task.status != 'cancelled':
        async with write_lock:
            task.status = 'cancelled'
            task.completed_at = datetime.now()
            task.current_chapter_id = None
            task.current_chapter_number = None
            await db_session.commit()

    logger.info(f"Batch generation cancelled during execution: {batch_id}")
    await emit_event(
        {
            "type": "error",
            "error": "项目不存在",
            "code": 400,
            "phase": "cancelled",
        }
    )
    await emit_event({"type": "done"})


async def complete_batch_generation_execution(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    batch_id: str,
    write_lock,
    emit_event,
) -> None:
    async with write_lock:
        task.status = 'completed'
        task.completed_at = datetime.now()
        task.current_chapter_id = None
        task.current_chapter_number = None
        await db_session.commit()

    logger.info(f"批量生成任务已完成: {batch_id}, 共完成 {task.completed_chapters} 章")
    await emit_event(
        {
            "type": "progress",
            "message": "批量生成完成",
            "progress": 100,
            "status": "success",
            "phase": "complete",
        }
    )
    await emit_event({"type": "done"})


async def fail_batch_generation_on_unhandled_exception(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    error: Exception,
    write_lock,
    emit_event,
) -> None:
    async with write_lock:
        task.status = 'failed'
        task.error_message = str(error)[:500]
        task.completed_at = datetime.now()
        await db_session.commit()

    await emit_event(
        {
            "type": "error",
            "error": task.error_message or str(error),
            "code": 500,
            "phase": "failed",
        }
    )
    await emit_event({"type": "done"})


@dataclass(frozen=True)
class BatchGenerationExecutionInitialization:
    task: "BatchGenerationTask"
    project: "Project"
    batch_story_packet: "StoryPacket"
    task_base_quality_profile: Dict[str, Any]
    cached_analysis_quality_profile: Dict[str, Any]
    active_story_repair_state: Dict[str, Any]
    active_story_repair_payload: Optional["StoryRepairPayload"]
    stream_chunks: bool


async def initialize_batch_generation_execution(
    db_session: "AsyncSession",
    *,
    batch_id: str,
    user_id: str,
    write_lock,
    emit_event,
    story_packet: Optional["StoryPacket"],
    creative_mode: Optional[str],
    story_focus: Optional[str],
    plot_stage: Optional[str],
    story_creation_brief: Optional[str],
    quality_preset: Optional[str],
    quality_notes: Optional[str],
    base_quality_profile: Optional[Dict[str, Any]],
    story_repair_summary: Optional[str],
    story_repair_targets: Optional[list[str]],
    story_preserve_strengths: Optional[list[str]],
    clone_quality_profile_fn,
    resolve_story_repair_state_fn,
    sync_task_story_repair_state_fn,
) -> Optional[BatchGenerationExecutionInitialization]:
    from sqlalchemy import select
    from app.models.batch_generation_task import BatchGenerationTask
    from app.models.project import Project

    task_result = await db_session.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    task = task_result.scalar_one_or_none()
    if task is None:
        return None

    project_result = await db_session.execute(
        select(Project).where(Project.id == task.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        async with write_lock:
            task.status = 'failed'
            task.error_message = '项目不存在'
            task.completed_at = datetime.now()
            await db_session.commit()
        await emit_event(
            {
                'type': 'error',
                'error': '项目不存在',
                'code': 404,
                'phase': 'loading',
            }
        )
        await emit_event({'type': 'done'})
        return None

    batch_story_packet = (
        story_packet
        if story_packet is not None
        else await build_story_generation_packet_with_project_continuity(
            db_session,
            project,
            creative_mode=creative_mode,
            story_focus=story_focus,
            plot_stage=plot_stage,
            story_creation_brief=story_creation_brief,
            quality_preset=quality_preset,
            quality_notes=quality_notes,
            source_label='batch-execution-request',
        )
    )

    task_base_quality_profile = (
        clone_quality_profile_fn(base_quality_profile)
        if isinstance(base_quality_profile, dict) and base_quality_profile
        else await resolve_chapter_quality_profile(
            db_session=db_session,
            user_id=user_id,
            project=project,
            style_id=task.style_id,
            enable_mcp=True,
            prefer_project_default_style=not bool(task.style_id),
            log_prefix='批量生成',
        )
    )
    cached_analysis_quality_profile = clone_quality_profile_fn(task_base_quality_profile)

    async with write_lock:
        task.status = 'running'
        task.started_at = datetime.now()
        await db_session.commit()
    await emit_event(
        {
            'type': 'progress',
            'message': '正在准备批量生成',
            'progress': 5,
            'status': 'running',
            'phase': 'loading',
        }
    )

    active_story_repair_state = await resolve_story_repair_state_fn(
        db_session,
        project_id=task.project_id,
        before_chapter_number=task.start_chapter_number,
        story_repair_summary=story_repair_summary,
        story_repair_targets=story_repair_targets,
        story_preserve_strengths=story_preserve_strengths,
    )
    active_story_repair_payload = active_story_repair_state.get('payload')
    active_story_repair_state = await sync_task_story_repair_state_fn(
        batch_id,
        story_repair_state=active_story_repair_state,
        db_session=db_session,
    )

    return BatchGenerationExecutionInitialization(
        task=task,
        project=project,
        batch_story_packet=batch_story_packet,
        task_base_quality_profile=(
            dict(task_base_quality_profile) if isinstance(task_base_quality_profile, dict) else {}
        ),
        cached_analysis_quality_profile=(
            dict(cached_analysis_quality_profile) if isinstance(cached_analysis_quality_profile, dict) else {}
        ),
        active_story_repair_state=(
            dict(active_story_repair_state) if isinstance(active_story_repair_state, dict) else {}
        ),
        active_story_repair_payload=(
            active_story_repair_payload
            if isinstance(active_story_repair_payload, _story_repair_payload_type())
            else None
        ),
        stream_chunks=bool(task.total_chapters == 1),
    )
