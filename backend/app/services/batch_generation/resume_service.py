from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any, Callable, Dict, List, Optional

from fastapi import BackgroundTasks, HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.batch_generation_task import BatchGenerationTask
from app.models.chapter import Chapter
from app.services.ai_service import AIService
from app.services.batch_generation.status_response_builder import build_batch_task_terminal_status
from app.services.batch_generation_workflow_service import (
    create_batch_generation_task_record,
    enqueue_batch_generation_execution,
)
from app.services.story_repair_payload_service import StoryRepairPayload
from app.services.task_workflow_runtime_service import (
    get_task_workflow_runtime_snapshot,
    persist_task_workflow_runtime_snapshot,
    set_task_workflow_runtime_snapshot,
)


@dataclass(frozen=True)
class BatchGenerationResumePreparation:
    source_task: BatchGenerationTask
    remaining_chapter_ids: List[str]
    remaining_chapters: List[Chapter]
    first_chapter: Chapter
    resumed_story_repair_payload: Optional[StoryRepairPayload]
    active_story_repair_payload_snapshot: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class BatchGenerationResumeExecutionResult:
    resumed_task: BatchGenerationTask
    response_payload: Dict[str, Any]


async def prepare_batch_generation_resume(
    db_session: AsyncSession,
    *,
    batch_id: str,
    user_id: str,
    resolve_story_repair_state_for_batch,
    check_prerequisites_fn,
) -> BatchGenerationResumePreparation:
    result = await db_session.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    source_task = result.scalar_one_or_none()
    if not source_task or source_task.user_id != user_id:
        raise HTTPException(status_code=404, detail='Batch generation task not found')
    if source_task.status not in {'failed', 'cancelled'}:
        raise HTTPException(
            status_code=400,
            detail='Only failed or cancelled tasks can be resumed',
        )

    source_workflow_snapshot = await get_task_workflow_runtime_snapshot(source_task.id, db_session=db_session)
    terminal_status = build_batch_task_terminal_status(
        source_task,
        workflow_snapshot=source_workflow_snapshot if isinstance(source_workflow_snapshot, dict) else None,
    )
    if terminal_status.get('review_required'):
        raise HTTPException(status_code=400, detail='Manual review blocked tasks cannot be resumed')

    chapter_ids = list(source_task.chapter_ids or [])
    if not chapter_ids:
        raise HTTPException(status_code=400, detail='No resumable chapters found')

    if source_task.current_chapter_id and source_task.current_chapter_id in chapter_ids:
        resume_start_index = chapter_ids.index(source_task.current_chapter_id)
    else:
        resume_start_index = max(int(source_task.completed_chapters or 0), 0)

    if resume_start_index >= len(chapter_ids):
        raise HTTPException(status_code=400, detail='No chapters left to resume')

    remaining_chapter_ids = chapter_ids[resume_start_index:]
    chapter_result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == source_task.project_id)
        .where(Chapter.id.in_(remaining_chapter_ids))
    )
    chapters = chapter_result.scalars().all()
    chapter_map = {chapter.id: chapter for chapter in chapters}
    missing_ids = [chapter_id for chapter_id in remaining_chapter_ids if chapter_id not in chapter_map]
    if missing_ids:
        raise HTTPException(status_code=400, detail='Some chapters no longer exist')

    remaining_chapters = [chapter_map[chapter_id] for chapter_id in remaining_chapter_ids]
    first_chapter = remaining_chapters[0]
    can_generate, error_msg, _ = await check_prerequisites_fn(db_session, first_chapter)
    if not can_generate:
        raise HTTPException(status_code=400, detail=f'Resume blocked by prerequisites: {error_msg}')

    source_active_story_repair_payload = (
        source_workflow_snapshot.get('active_story_repair_payload')
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
    resumed_story_repair_payload = resumed_story_repair_state.get('payload')
    active_story_repair_payload_snapshot = resumed_story_repair_state.get('active_story_repair_payload')
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


def build_resumed_batch_generation_runtime_snapshot(
    resumed_task: BatchGenerationTask,
    *,
    preparation: BatchGenerationResumePreparation,
) -> Dict[str, Any]:
    first_chapter = preparation.first_chapter
    return {
        'phase': 'loading',
        'last_event': 'resume',
        'last_message': 'Task resumed and queued',
        'progress': 0,
        'status': 'pending',
        'current_chapter_id': preparation.remaining_chapter_ids[0],
        'current_chapter_number': first_chapter.chapter_number,
        'current_retry_count': 0,
        'max_retries': resumed_task.max_retries,
        'resume_from_batch_id': preparation.source_task.id,
        'active_story_repair_payload': preparation.active_story_repair_payload_snapshot,
        'updated_at': datetime.now().isoformat(),
    }


def build_resumed_batch_generation_response(
    resumed_task: BatchGenerationTask,
    *,
    preparation: BatchGenerationResumePreparation,
    runtime_snapshot: Dict[str, Any],
) -> Dict[str, Any]:
    task_type = (
        'chapter_single_generate'
        if resumed_task.chapter_count == 1 and len(resumed_task.chapter_ids or []) == 1
        else 'chapters_batch_generate'
    )
    checkpoint = {
        'current_chapter_id': preparation.remaining_chapter_ids[0],
        'current_chapter_number': preparation.first_chapter.chapter_number,
        'current_retry_count': 0,
        'max_retries': resumed_task.max_retries,
        'progress_phase': runtime_snapshot.get('phase') or 'loading',
        'progress': runtime_snapshot.get('progress', 0),
        'resume_from_batch_id': preparation.source_task.id,
    }
    return {
        'message': 'Task resumed and queued',
        'batch_id': resumed_task.id,
        'project_id': resumed_task.project_id,
        'task_type': task_type,
        'status': resumed_task.status,
        'stage_code': '6.writing.loading',
        'execution_mode': 'interactive',
        'checkpoint': checkpoint,
        'resumed_from_batch_id': preparation.source_task.id,
        'total_chapters': resumed_task.total_chapters,
        'completed_chapters': resumed_task.completed_chapters,
        'created_at': resumed_task.created_at.isoformat() if resumed_task.created_at else None,
    }


async def create_resumed_batch_generation_and_enqueue(
    db_session: AsyncSession,
    *,
    preparation: BatchGenerationResumePreparation,
    user_id: str,
    background_tasks: BackgroundTasks,
    ai_service: AIService,
    execution_callable: Callable[..., Any],
) -> BatchGenerationResumeExecutionResult:
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
        execution_callable,
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
