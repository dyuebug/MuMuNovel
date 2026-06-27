"""Test-only adapter for retired batch generation orchestration helpers."""
from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import TYPE_CHECKING, Any, Callable, Dict, List, Optional, Sequence

from fastapi import BackgroundTasks, HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

if TYPE_CHECKING:
    from migrator_app.models.batch_generation_task import BatchGenerationTask
    from migrator_app.models.chapter import Chapter
    from migrator_app.models.project import Project
    from tests.test_support.ai_gateway.ai_service import AIService
    from tests.test_support.story_repair_payload_test_support import StoryRepairPayload


@dataclass(frozen=True)
class BatchGenerationCreatePreparation:
    chapters_to_generate: List["Chapter"]
    batch_quality_profile: Dict[str, Any]
    batch_story_packet: Any
    batch_story_repair_state: Dict[str, Any]


@dataclass(frozen=True)
class BatchGenerationResumePreparation:
    source_task: "BatchGenerationTask"
    remaining_chapter_ids: List[str]
    remaining_chapters: List["Chapter"]
    first_chapter: "Chapter"
    resumed_story_repair_payload: Optional["StoryRepairPayload"]
    active_story_repair_payload_snapshot: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class BatchGenerationResumeExecutionResult:
    resumed_task: "BatchGenerationTask"
    response_payload: Dict[str, Any]


def _build_batch_generation_execution_kwargs(
    *,
    batch_id: str,
    user_id: str,
    ai_service: "AIService",
    custom_model: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    story_packet: Optional[Any] = None,
    base_quality_profile: Optional[Dict[str, Any]] = None,
    enable_web_research: Optional[bool] = None,
    web_research_query: Optional[str] = None,
    story_repair_payload: Optional["StoryRepairPayload"] = None,
) -> Dict[str, Any]:
    from tests.test_support.story_repair_payload_test_support import resolve_story_repair_prompt_kwargs

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
    db_session: AsyncSession,
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
    from migrator_app.models.batch_generation_task import BatchGenerationTask

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
    background_tasks: BackgroundTasks,
    execution_callable: Callable[..., Any],
    **kwargs: Any,
) -> None:
    """统一注册批量生成后台任务。"""
    background_tasks.add_task(
        execution_callable,
        **_build_batch_generation_execution_kwargs(**kwargs),
    )


async def prepare_batch_generation_create(*args, **kwargs):
    db_session = args[0] if args else kwargs["db_session"]
    project_id = kwargs["project_id"]
    project = kwargs["project"]
    user_id = kwargs["user_id"]
    batch_request = kwargs["batch_request"]
    check_prerequisites_fn = kwargs["check_prerequisites_fn"]
    resolve_quality_profile_fn = kwargs["resolve_quality_profile_fn"]
    build_story_packet_fn = kwargs["build_story_packet_fn"]
    resolve_story_repair_state_fn = kwargs["resolve_story_repair_state_fn"]
    from migrator_app.models.chapter import Chapter

    result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == project_id)
        .order_by(Chapter.chapter_number)
    )
    all_chapters = result.scalars().all()
    if not all_chapters:
        raise HTTPException(status_code=404, detail="项目下暂无章节")

    start_number = batch_request.start_chapter_number
    end_number = start_number + batch_request.count - 1
    chapters_to_generate = [
        chapter
        for chapter in all_chapters
        if start_number <= chapter.chapter_number <= end_number
    ]
    if not chapters_to_generate:
        raise HTTPException(status_code=404, detail="未找到指定范围内的章节")

    first_chapter = chapters_to_generate[0]
    can_generate, error_msg, _ = await check_prerequisites_fn(db_session, first_chapter)
    if not can_generate:
        raise HTTPException(status_code=400, detail=f"批量生成前置检查未通过：{error_msg}")

    batch_quality_profile = await resolve_quality_profile_fn(
        db_session=db_session,
        user_id=user_id,
        project=project,
        style_id=batch_request.style_id,
        enable_mcp=True,
        prefer_project_default_style=not bool(batch_request.style_id),
        log_prefix="批量生成",
    )
    batch_story_repair_state = await resolve_story_repair_state_fn(
        db_session,
        project_id=project_id,
        before_chapter_number=start_number,
        story_repair_summary=batch_request.story_repair_summary,
        story_repair_targets=batch_request.story_repair_targets,
        story_preserve_strengths=batch_request.story_preserve_strengths,
    )
    batch_story_packet = await build_story_packet_fn(
        db_session,
        project,
        source=batch_request,
        source_label="batch-generate-request",
    )
    return BatchGenerationCreatePreparation(
        chapters_to_generate=chapters_to_generate,
        batch_quality_profile=(
            dict(batch_quality_profile) if isinstance(batch_quality_profile, dict) else {}
        ),
        batch_story_packet=batch_story_packet,
        batch_story_repair_state=(
            dict(batch_story_repair_state)
            if isinstance(batch_story_repair_state, dict)
            else {}
        ),
    )


async def create_batch_generation_and_enqueue(*args, **kwargs):
    from tests.test_support.batch_generation_run_wiring_test_adapter import (
        execute_batch_generation_in_order_with_default_wiring,
    )

    db_session = args[0] if args else kwargs["db_session"]
    project_id = kwargs["project_id"]
    user_id = kwargs["user_id"]
    batch_request = kwargs["batch_request"]
    preparation = kwargs["preparation"]
    background_tasks = kwargs["background_tasks"]
    ai_service = kwargs["ai_service"]
    sync_task_story_repair_state_fn = kwargs["sync_task_story_repair_state_fn"]

    batch_task = await create_batch_generation_task_record(
        db_session,
        project_id=project_id,
        user_id=user_id,
        start_chapter_number=batch_request.start_chapter_number,
        chapter_ids=[chapter.id for chapter in preparation.chapters_to_generate],
        style_id=preparation.batch_quality_profile.get("resolved_style_id"),
        target_word_count=batch_request.target_word_count,
        enable_analysis=batch_request.enable_analysis,
        max_retries=batch_request.max_retries,
    )
    batch_id = batch_task.id

    enqueue_batch_generation_execution(
        background_tasks,
        execute_batch_generation_in_order_with_default_wiring,
        batch_id=batch_id,
        user_id=user_id,
        ai_service=ai_service,
        custom_model=batch_request.model,
        story_packet=preparation.batch_story_packet,
        base_quality_profile=preparation.batch_quality_profile,
        enable_web_research=batch_request.enable_web_research,
        web_research_query=batch_request.web_research_query,
        story_repair_payload=preparation.batch_story_repair_state.get("payload"),
    )
    await sync_task_story_repair_state_fn(
        batch_id,
        story_repair_state=preparation.batch_story_repair_state,
        db_session=db_session,
    )
    estimated_time = calculate_estimated_time(
        chapter_count=len(preparation.chapters_to_generate),
        target_word_count=batch_request.target_word_count,
        enable_analysis=batch_request.enable_analysis,
    )
    return {
        "batch_id": batch_id,
        "message": f"已创建批量生成任务，共 {len(preparation.chapters_to_generate)} 章",
        "chapters_to_generate": [
            {
                "id": chapter.id,
                "chapter_number": chapter.chapter_number,
                "title": chapter.title,
            }
            for chapter in preparation.chapters_to_generate
        ],
        "estimated_time_minutes": estimated_time,
    }


async def orchestrate_batch_generation_create(*args, **kwargs) -> Dict[str, Any]:
    db_session = args[0] if args else kwargs["db_session"]
    project_id = kwargs["project_id"]
    project = kwargs["project"]
    user_id = kwargs["user_id"]
    batch_request = kwargs["batch_request"]
    background_tasks = kwargs["background_tasks"]
    ai_service = kwargs["ai_service"]
    check_prerequisites_fn = kwargs["check_prerequisites_fn"]
    resolve_quality_profile_fn = kwargs["resolve_quality_profile_fn"]
    build_story_packet_fn = kwargs["build_story_packet_fn"]
    resolve_story_repair_state_fn = kwargs["resolve_story_repair_state_fn"]
    sync_task_story_repair_state_fn = kwargs["sync_task_story_repair_state_fn"]

    batch_preparation = await prepare_batch_generation_create(
        db_session,
        project_id=project_id,
        project=project,
        user_id=user_id,
        batch_request=batch_request,
        check_prerequisites_fn=check_prerequisites_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        build_story_packet_fn=build_story_packet_fn,
        resolve_story_repair_state_fn=resolve_story_repair_state_fn,
    )
    return await create_batch_generation_and_enqueue(
        db_session,
        project_id=project_id,
        user_id=user_id,
        batch_request=batch_request,
        preparation=batch_preparation,
        background_tasks=background_tasks,
        ai_service=ai_service,
        sync_task_story_repair_state_fn=sync_task_story_repair_state_fn,
    )


async def orchestrate_batch_generation_resume(
    db_session: AsyncSession,
    *,
    batch_id: str,
    user_id: str,
    background_tasks: BackgroundTasks,
    ai_service: "AIService",
    resolve_story_repair_state_for_batch,
    check_prerequisites_fn,
    build_batch_task_terminal_status_fn,
) -> Dict[str, Any]:
    resume_preparation = await prepare_batch_generation_resume(
        db_session,
        batch_id=batch_id,
        user_id=user_id,
        resolve_story_repair_state_for_batch=resolve_story_repair_state_for_batch,
        check_prerequisites_fn=check_prerequisites_fn,
        build_batch_task_terminal_status_fn=build_batch_task_terminal_status_fn,
    )
    resume_result = await create_resumed_batch_generation_and_enqueue(
        db_session,
        preparation=resume_preparation,
        user_id=user_id,
        background_tasks=background_tasks,
        ai_service=ai_service,
    )
    return resume_result.response_payload


async def prepare_batch_generation_resume(*args, **kwargs):
    from migrator_app.models.batch_generation_task import BatchGenerationTask
    from migrator_app.models.chapter import Chapter
    from tests.test_support.story_repair_payload_test_support import StoryRepairPayload
    from tests.test_support.task_system import (
        get_task_workflow_runtime_snapshot,
    )

    db_session = args[0] if args else kwargs["db_session"]
    batch_id = kwargs["batch_id"]
    user_id = kwargs["user_id"]
    resolve_story_repair_state_for_batch = kwargs["resolve_story_repair_state_for_batch"]
    check_prerequisites_fn = kwargs["check_prerequisites_fn"]
    build_batch_task_terminal_status_fn = kwargs["build_batch_task_terminal_status_fn"]

    result = await db_session.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    source_task = result.scalar_one_or_none()
    if not source_task or source_task.user_id != user_id:
        raise HTTPException(status_code=404, detail="Batch generation task not found")
    if source_task.status not in {"failed", "cancelled"}:
        raise HTTPException(
            status_code=400,
            detail="Only failed or cancelled tasks can be resumed",
        )

    source_workflow_snapshot = await get_task_workflow_runtime_snapshot(
        source_task.id,
        db_session=db_session,
    )
    terminal_status = build_batch_task_terminal_status_fn(
        source_task,
        workflow_snapshot=source_workflow_snapshot
        if isinstance(source_workflow_snapshot, dict)
        else None,
    )
    if terminal_status.get("review_required"):
        raise HTTPException(
            status_code=400,
            detail="Manual review blocked tasks cannot be resumed",
        )

    chapter_ids = list(source_task.chapter_ids or [])
    if not chapter_ids:
        raise HTTPException(status_code=400, detail="No resumable chapters found")

    if source_task.current_chapter_id and source_task.current_chapter_id in chapter_ids:
        resume_start_index = chapter_ids.index(source_task.current_chapter_id)
    else:
        resume_start_index = max(int(source_task.completed_chapters or 0), 0)

    if resume_start_index >= len(chapter_ids):
        raise HTTPException(status_code=400, detail="No chapters left to resume")

    remaining_chapter_ids = chapter_ids[resume_start_index:]
    chapter_result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == source_task.project_id)
        .where(Chapter.id.in_(remaining_chapter_ids))
    )
    chapters = chapter_result.scalars().all()
    chapter_map = {chapter.id: chapter for chapter in chapters}
    missing_ids = [
        chapter_id for chapter_id in remaining_chapter_ids if chapter_id not in chapter_map
    ]
    if missing_ids:
        raise HTTPException(status_code=400, detail="Some chapters no longer exist")

    remaining_chapters = [chapter_map[chapter_id] for chapter_id in remaining_chapter_ids]
    first_chapter = remaining_chapters[0]
    can_generate, error_msg, _ = await check_prerequisites_fn(db_session, first_chapter)
    if not can_generate:
        raise HTTPException(
            status_code=400,
            detail=f"Resume blocked by prerequisites: {error_msg}",
        )

    source_active_story_repair_payload = (
        source_workflow_snapshot.get("active_story_repair_payload")
        if isinstance(source_workflow_snapshot, dict)
        else None
    )
    resumed_story_repair_state = await resolve_story_repair_state_for_batch(
        db_session,
        project_id=source_task.project_id,
        before_chapter_number=first_chapter.chapter_number,
        active_story_repair_payload=(
            source_active_story_repair_payload
            if isinstance(source_active_story_repair_payload, dict)
            else None
        ),
    )
    resumed_story_repair_payload = resumed_story_repair_state.get("payload")
    active_story_repair_payload_snapshot = resumed_story_repair_state.get(
        "active_story_repair_payload"
    )
    if not isinstance(active_story_repair_payload_snapshot, dict):
        active_story_repair_payload_snapshot = None

    return BatchGenerationResumePreparation(
        source_task=source_task,
        remaining_chapter_ids=remaining_chapter_ids,
        remaining_chapters=remaining_chapters,
        first_chapter=first_chapter,
        resumed_story_repair_payload=(
            resumed_story_repair_payload
            if isinstance(resumed_story_repair_payload, StoryRepairPayload)
            else None
        ),
        active_story_repair_payload_snapshot=active_story_repair_payload_snapshot,
    )


def build_resumed_batch_generation_runtime_snapshot(*args, **kwargs):
    resumed_task = args[0] if args else kwargs["resumed_task"]
    preparation = kwargs["preparation"]
    first_chapter = preparation.first_chapter
    return {
        "phase": "loading",
        "last_event": "resume",
        "last_message": "Task resumed and queued",
        "progress": 0,
        "status": "pending",
        "current_chapter_id": preparation.remaining_chapter_ids[0],
        "current_chapter_number": first_chapter.chapter_number,
        "current_retry_count": 0,
        "max_retries": resumed_task.max_retries,
        "resume_from_batch_id": preparation.source_task.id,
        "active_story_repair_payload": preparation.active_story_repair_payload_snapshot,
        "updated_at": datetime.now().isoformat(),
    }


def build_resumed_batch_generation_response(*args, **kwargs):
    resumed_task = args[0] if args else kwargs["resumed_task"]
    preparation = kwargs["preparation"]
    runtime_snapshot = kwargs["runtime_snapshot"]
    task_type = (
        "chapter_single_generate"
        if resumed_task.chapter_count == 1 and len(resumed_task.chapter_ids or []) == 1
        else "chapters_batch_generate"
    )
    checkpoint = {
        "current_chapter_id": preparation.remaining_chapter_ids[0],
        "current_chapter_number": preparation.first_chapter.chapter_number,
        "current_retry_count": 0,
        "max_retries": resumed_task.max_retries,
        "progress_phase": runtime_snapshot.get("phase") or "loading",
        "progress": runtime_snapshot.get("progress", 0),
        "resume_from_batch_id": preparation.source_task.id,
    }
    return {
        "message": "Task resumed and queued",
        "batch_id": resumed_task.id,
        "project_id": resumed_task.project_id,
        "task_type": task_type,
        "status": resumed_task.status,
        "stage_code": "6.writing.loading",
        "execution_mode": "interactive",
        "checkpoint": checkpoint,
        "resumed_from_batch_id": preparation.source_task.id,
        "total_chapters": resumed_task.total_chapters,
        "completed_chapters": resumed_task.completed_chapters,
        "created_at": resumed_task.created_at.isoformat() if resumed_task.created_at else None,
    }


async def create_resumed_batch_generation_and_enqueue(*args, **kwargs):
    from tests.test_support.batch_generation_run_wiring_test_adapter import (
        execute_batch_generation_in_order_with_default_wiring,
    )
    from tests.test_support.task_system import (
        persist_task_workflow_runtime_snapshot,
        set_task_workflow_runtime_snapshot,
    )

    db_session = args[0] if args else kwargs["db_session"]
    preparation = kwargs["preparation"]
    user_id = kwargs["user_id"]
    background_tasks = kwargs["background_tasks"]
    ai_service = kwargs["ai_service"]

    source_task = preparation.source_task
    resumed_task = await create_batch_generation_task_record(
        db_session,
        project_id=source_task.project_id,
        user_id=user_id,
        start_chapter_number=preparation.first_chapter.chapter_number,
        chapter_ids=preparation.remaining_chapter_ids,
        style_id=source_task.style_id,
        target_word_count=source_task.target_word_count,
        enable_analysis=bool(source_task.enable_analysis),
        max_retries=source_task.max_retries or 3,
    )

    runtime_snapshot = build_resumed_batch_generation_runtime_snapshot(
        resumed_task,
        preparation=preparation,
    )
    await set_task_workflow_runtime_snapshot(resumed_task.id, runtime_snapshot)
    await persist_task_workflow_runtime_snapshot(
        db_session,
        resumed_task.id,
        runtime_snapshot,
    )

    enqueue_batch_generation_execution(
        background_tasks,
        execute_batch_generation_in_order_with_default_wiring,
        batch_id=resumed_task.id,
        user_id=user_id,
        ai_service=ai_service,
        story_repair_payload=preparation.resumed_story_repair_payload,
    )

    return BatchGenerationResumeExecutionResult(
        resumed_task=resumed_task,
        response_payload=build_resumed_batch_generation_response(
            resumed_task,
            preparation=preparation,
            runtime_snapshot=runtime_snapshot,
        ),
    )

