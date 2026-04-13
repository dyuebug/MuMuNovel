from __future__ import annotations

from typing import Any, Dict, Optional

from app.models.batch_generation_task import BatchGenerationTask
from app.schemas.chapter import BatchGenerateStatusResponse


def build_batch_task_terminal_status(
    task: BatchGenerationTask,
    *,
    workflow_snapshot: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    failed_chapters = task.failed_chapters if isinstance(task.failed_chapters, list) else []
    latest_failed = next(
        (
            item
            for item in reversed(failed_chapters)
            if isinstance(item, dict)
        ),
        None,
    )
    active_story_repair_payload = None
    if isinstance(workflow_snapshot, dict):
        candidate_payload = workflow_snapshot.get('active_story_repair_payload')
        if isinstance(candidate_payload, dict):
            active_story_repair_payload = candidate_payload

    terminal_reason: Optional[str] = None
    terminal_label: Optional[str] = None
    review_required = False

    if task.status == 'completed':
        terminal_reason = 'completed'
        terminal_label = '已完成'
    elif task.status == 'cancelled':
        terminal_reason = 'cancelled'
        terminal_label = '已取消'
    elif task.status == 'failed':
        quality_gate_decision = None
        quality_gate_label = None
        failure_phase = None
        if isinstance(latest_failed, dict):
            raw_decision = latest_failed.get('quality_gate_decision')
            raw_label = latest_failed.get('quality_gate_label')
            raw_phase = latest_failed.get('phase')
            quality_gate_decision = raw_decision if isinstance(raw_decision, str) and raw_decision else None
            quality_gate_label = raw_label if isinstance(raw_label, str) and raw_label else None
            failure_phase = raw_phase if isinstance(raw_phase, str) and raw_phase else None

        if quality_gate_decision is None and isinstance(active_story_repair_payload, dict):
            raw_decision = active_story_repair_payload.get('quality_gate_decision')
            quality_gate_decision = raw_decision if isinstance(raw_decision, str) and raw_decision else None
        if quality_gate_label is None and isinstance(active_story_repair_payload, dict):
            raw_label = active_story_repair_payload.get('quality_gate_label')
            quality_gate_label = raw_label if isinstance(raw_label, str) and raw_label else None

        if quality_gate_decision == 'manual_review' or failure_phase == 'quality_blocked':
            terminal_reason = 'manual_review'
            terminal_label = quality_gate_label or '需人工复核'
            review_required = True
        else:
            terminal_reason = 'error'
            terminal_label = '执行失败'

    return {
        'terminal_reason': terminal_reason,
        'terminal_label': terminal_label,
        'review_required': review_required,
        'can_resume': task.status in {'failed', 'cancelled'},
    }


def build_batch_generation_status_response(
    task: BatchGenerationTask,
    *,
    quality_snapshot: Dict[str, Any],
    workflow_snapshot: Dict[str, Any],
) -> BatchGenerateStatusResponse:
    terminal_status = build_batch_task_terminal_status(task, workflow_snapshot=workflow_snapshot)
    return BatchGenerateStatusResponse(
        batch_id=task.id,
        status=task.status,
        stage_code=workflow_snapshot['stage_code'],
        execution_mode=workflow_snapshot['execution_mode'],
        total=task.total_chapters,
        completed=task.completed_chapters,
        current_chapter_id=task.current_chapter_id,
        current_chapter_number=task.current_chapter_number,
        current_retry_count=task.current_retry_count,
        max_retries=task.max_retries,
        checkpoint=workflow_snapshot['checkpoint'],
        failed_chapters=task.failed_chapters or [],
        created_at=task.created_at.isoformat() if task.created_at else None,
        started_at=task.started_at.isoformat() if task.started_at else None,
        completed_at=task.completed_at.isoformat() if task.completed_at else None,
        error_message=task.error_message,
        latest_quality_metrics=quality_snapshot.get('latest'),
        quality_metrics_summary=quality_snapshot.get('summary'),
        active_story_repair_payload=workflow_snapshot.get('active_story_repair_payload'),
        terminal_reason=terminal_status['terminal_reason'],
        terminal_label=terminal_status['terminal_label'],
        review_required=terminal_status['review_required'],
        can_resume=terminal_status['can_resume'],
    )


def build_active_batch_generation_payload(
    task: BatchGenerationTask,
    *,
    quality_snapshot: Dict[str, Any],
    workflow_snapshot: Dict[str, Any],
) -> Dict[str, Any]:
    return {
        'has_active_task': True,
        'task': {
            'batch_id': task.id,
            'status': task.status,
            'stage_code': workflow_snapshot['stage_code'],
            'execution_mode': workflow_snapshot['execution_mode'],
            'total': task.total_chapters,
            'completed': task.completed_chapters,
            'current_chapter_id': task.current_chapter_id,
            'current_chapter_number': task.current_chapter_number,
            'checkpoint': workflow_snapshot['checkpoint'],
            'latest_quality_metrics': quality_snapshot.get('latest'),
            'quality_metrics_summary': quality_snapshot.get('summary'),
            'active_story_repair_payload': workflow_snapshot.get('active_story_repair_payload'),
            'created_at': task.created_at.isoformat() if task.created_at else None,
            'started_at': task.started_at.isoformat() if task.started_at else None,
        },
    }


def _resolve_batch_generation_task_type(task: BatchGenerationTask) -> str:
    if task.chapter_count == 1 and len(task.chapter_ids or []) == 1:
        return 'chapter_single_generate'
    return 'chapters_batch_generate'


def build_batch_generation_task_list_item(
    task: BatchGenerationTask,
    *,
    quality_snapshot: Dict[str, Any],
    workflow_snapshot: Dict[str, Any],
) -> Dict[str, Any]:
    return {
        'task_type': _resolve_batch_generation_task_type(task),
        'stage_code': workflow_snapshot['stage_code'],
        'execution_mode': workflow_snapshot['execution_mode'],
        'project_id': task.project_id,
        'batch_id': task.id,
        'status': task.status,
        'total': task.total_chapters,
        'completed': task.completed_chapters,
        'current_chapter_id': task.current_chapter_id,
        'current_chapter_number': task.current_chapter_number,
        'checkpoint': workflow_snapshot['checkpoint'],
        'latest_quality_metrics': quality_snapshot.get('latest'),
        'quality_metrics_summary': quality_snapshot.get('summary'),
        'active_story_repair_payload': workflow_snapshot.get('active_story_repair_payload'),
        'created_at': task.created_at.isoformat() if task.created_at else None,
        'started_at': task.started_at.isoformat() if task.started_at else None,
        'completed_at': task.completed_at.isoformat() if task.completed_at else None,
        'error_message': task.error_message,
    }
