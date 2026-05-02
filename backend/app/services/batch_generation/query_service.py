from __future__ import annotations

from typing import Any, Dict, List, Optional

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.batch_generation_task import BatchGenerationTask
from app.services.batch_generation.status_models import BatchGenerationTaskViewContext
from app.services.task_quality_snapshot_service import get_task_quality_metrics_snapshot
from app.services.task_workflow_runtime_service import get_task_workflow_runtime_snapshot


def _default_batch_progress_phase(task: BatchGenerationTask) -> str:
    if task.status == 'pending':
        return 'init'
    if task.status == 'completed':
        return 'complete'
    if task.status == 'failed':
        return 'failed'
    if task.status == 'cancelled':
        return 'cancelled'
    if task.current_retry_count and task.current_retry_count > 0:
        return 'generating'
    if task.current_chapter_number is not None:
        return 'generating'
    return 'loading'


def _compose_batch_stage_code(base: str, phase: Optional[str]) -> str:
    if not phase or phase == 'init':
        return base
    return f'{base}.{phase}'


async def build_batch_task_workflow_snapshot(
    task: BatchGenerationTask,
    db_session: Optional[AsyncSession] = None,
) -> Dict[str, Any]:
    runtime = await get_task_workflow_runtime_snapshot(task.id, db_session=db_session)

    phase = str(runtime.get('phase') or '').strip().lower() or _default_batch_progress_phase(task)
    stage_code = _compose_batch_stage_code('6.writing', phase)
    progress_value = runtime.get('progress')
    if not isinstance(progress_value, int):
        completed = max(int(task.completed_chapters or 0), 0)
        total = max(int(task.total_chapters or 0), 1)
        progress_value = 100 if task.status == 'completed' else int((completed / total) * 100)

    checkpoint = {
        'current_chapter_id': task.current_chapter_id,
        'current_chapter_number': task.current_chapter_number,
        'current_retry_count': task.current_retry_count,
        'max_retries': task.max_retries,
        'progress_phase': phase,
        'progress': max(0, min(progress_value, 100)),
        'last_event': runtime.get('last_event'),
        'last_message': runtime.get('last_message'),
        'candidate_index': runtime.get('candidate_index'),
        'candidate_count': runtime.get('candidate_count'),
        'word_count': runtime.get('word_count'),
        'generation_path': runtime.get('generation_path'),
        'attempt_kind': runtime.get('attempt_kind'),
        'rerank_used': runtime.get('rerank_used') if isinstance(runtime.get('rerank_used'), bool) else None,
        'word_budget_repair_used': runtime.get('word_budget_repair_used') if isinstance(runtime.get('word_budget_repair_used'), bool) else None,
        'winner_candidate_index': runtime.get('winner_candidate_index'),
        'pre_compaction_total_length': runtime.get('pre_compaction_total_length'),
        'context_budget_limit': runtime.get('context_budget_limit'),
        'compaction_applied': runtime.get('compaction_applied') if isinstance(runtime.get('compaction_applied'), bool) else None,
        'compaction_details': runtime.get('compaction_details') if isinstance(runtime.get('compaction_details'), dict) else None,
    }
    active_story_repair_payload = runtime.get('active_story_repair_payload')
    return {
        'stage_code': stage_code,
        'execution_mode': 'interactive',
        'checkpoint': checkpoint,
        'active_story_repair_payload': dict(active_story_repair_payload) if isinstance(active_story_repair_payload, dict) else None,
    }


async def build_batch_generation_task_view_context(
    task: BatchGenerationTask,
    *,
    db_session: AsyncSession,
) -> BatchGenerationTaskViewContext:
    quality_snapshot = await get_task_quality_metrics_snapshot(task.id, db_session=db_session)
    workflow_snapshot = await build_batch_task_workflow_snapshot(task, db_session=db_session)
    return BatchGenerationTaskViewContext(
        task=task,
        quality_snapshot=quality_snapshot,
        workflow_snapshot=workflow_snapshot,
    )


async def load_batch_generation_task_view_context(
    db_session: AsyncSession,
    *,
    batch_id: str,
) -> Optional[BatchGenerationTaskViewContext]:
    result = await db_session.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    task = result.scalar_one_or_none()
    if task is None:
        return None
    return await build_batch_generation_task_view_context(task, db_session=db_session)


async def load_active_project_batch_generation_task_view_context(
    db_session: AsyncSession,
    *,
    project_id: str,
) -> Optional[BatchGenerationTaskViewContext]:
    result = await db_session.execute(
        select(BatchGenerationTask)
        .where(BatchGenerationTask.project_id == project_id)
        .where(BatchGenerationTask.status.in_(['pending', 'running']))
        .order_by(BatchGenerationTask.created_at.desc())
        .limit(1)
    )
    task = result.scalar_one_or_none()
    if task is None:
        return None
    return await build_batch_generation_task_view_context(task, db_session=db_session)


async def load_active_user_batch_generation_task_view_contexts(
    db_session: AsyncSession,
    *,
    user_id: str,
    limit: int,
) -> List[BatchGenerationTaskViewContext]:
    result = await db_session.execute(
        select(BatchGenerationTask)
        .where(BatchGenerationTask.user_id == user_id)
        .where(BatchGenerationTask.status.in_(['pending', 'running']))
        .order_by(BatchGenerationTask.created_at.desc())
        .limit(limit)
    )
    tasks = result.scalars().all()
    contexts: List[BatchGenerationTaskViewContext] = []
    for task in tasks:
        contexts.append(await build_batch_generation_task_view_context(task, db_session=db_session))
    return contexts
