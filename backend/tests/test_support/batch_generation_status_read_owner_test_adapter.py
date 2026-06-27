"""Test-only adapter for the retired batch generation read/status owner shell."""
from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import TYPE_CHECKING, Any, Dict, List, Optional

from tests.test_support.chapter_schema_test_support import BatchGenerateStatusResponse

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession
    from migrator_app.models.batch_generation_task import BatchGenerationTask


@dataclass(frozen=True)
class BatchGenerationTaskViewContext:
    task: Any
    quality_snapshot: Dict[str, Any]
    workflow_snapshot: Dict[str, Any]


def recover_stale_batch_generation_task_if_needed(task: "BatchGenerationTask") -> bool:
    current_time = datetime.now(timezone.utc).replace(tzinfo=None)
    auto_recovered = False

    if task.status == "running":
        if task.started_at and (current_time - task.started_at) > timedelta(minutes=15):
            task.status = "failed"
            task.error_message = "任务超时（超过15分钟未完成，已自动恢复）"
            task.completed_at = current_time
            auto_recovered = True
    elif task.status == "pending":
        if task.created_at and (current_time - task.created_at) > timedelta(minutes=3):
            task.status = "failed"
            task.error_message = "任务启动超时（超过3分钟未启动，已自动恢复）"
            task.completed_at = current_time
            auto_recovered = True

    return auto_recovered


async def recover_stale_batch_generation_tasks(
    db_session: "AsyncSession",
    tasks: List["BatchGenerationTask"],
) -> bool:
    changed = False
    for task in tasks:
        if recover_stale_batch_generation_task_if_needed(task):
            changed = True
    if changed:
        await db_session.commit()
    return changed


def build_batch_task_terminal_status(
    task: "BatchGenerationTask",
    *,
    workflow_snapshot: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    failed_chapters = task.failed_chapters if isinstance(task.failed_chapters, list) else []
    latest_failed = next(
        (item for item in reversed(failed_chapters) if isinstance(item, dict)),
        None,
    )
    active_story_repair_payload = None
    if isinstance(workflow_snapshot, dict):
        candidate_payload = workflow_snapshot.get("active_story_repair_payload")
        if isinstance(candidate_payload, dict):
            active_story_repair_payload = candidate_payload

    terminal_reason: Optional[str] = None
    terminal_label: Optional[str] = None
    review_required = False

    if task.status == "completed":
        terminal_reason = "completed"
        terminal_label = "已完成"
    elif task.status == "cancelled":
        terminal_reason = "cancelled"
        terminal_label = "已取消"
    elif task.status == "failed":
        quality_gate_decision = None
        quality_gate_label = None
        failure_phase = None
        if isinstance(latest_failed, dict):
            raw_decision = latest_failed.get("quality_gate_decision")
            raw_label = latest_failed.get("quality_gate_label")
            raw_phase = latest_failed.get("phase")
            quality_gate_decision = (
                raw_decision if isinstance(raw_decision, str) and raw_decision else None
            )
            quality_gate_label = (
                raw_label if isinstance(raw_label, str) and raw_label else None
            )
            failure_phase = raw_phase if isinstance(raw_phase, str) and raw_phase else None

        if quality_gate_decision is None and isinstance(active_story_repair_payload, dict):
            raw_decision = active_story_repair_payload.get("quality_gate_decision")
            quality_gate_decision = (
                raw_decision if isinstance(raw_decision, str) and raw_decision else None
            )
        if quality_gate_label is None and isinstance(active_story_repair_payload, dict):
            raw_label = active_story_repair_payload.get("quality_gate_label")
            quality_gate_label = raw_label if isinstance(raw_label, str) and raw_label else None

        if quality_gate_decision == "manual_review" or failure_phase == "quality_blocked":
            terminal_reason = "manual_review"
            terminal_label = quality_gate_label or "需人工复核"
            review_required = True
        else:
            terminal_reason = "error"
            terminal_label = "执行失败"

    return {
        "terminal_reason": terminal_reason,
        "terminal_label": terminal_label,
        "review_required": review_required,
        "can_resume": task.status in {"failed", "cancelled"} and not review_required,
    }


def _default_batch_progress_phase(task: "BatchGenerationTask") -> str:
    if task.status == "pending":
        return "init"
    if task.status == "completed":
        return "complete"
    if task.status == "failed":
        return "failed"
    if task.status == "cancelled":
        return "cancelled"
    if task.current_retry_count and task.current_retry_count > 0:
        return "generating"
    if task.current_chapter_number is not None:
        return "generating"
    return "loading"


def _compose_batch_stage_code(base: str, phase: Optional[str]) -> str:
    if not phase or phase == "init":
        return base
    return f"{base}.{phase}"


async def build_batch_task_workflow_snapshot(
    task: "BatchGenerationTask",
    db_session: Optional["AsyncSession"] = None,
) -> Dict[str, Any]:
    from tests.test_support.task_system import get_task_workflow_runtime_snapshot

    runtime = await get_task_workflow_runtime_snapshot(task.id, db_session=db_session)

    phase = str(runtime.get("phase") or "").strip().lower() or _default_batch_progress_phase(
        task
    )
    stage_code = _compose_batch_stage_code("6.writing", phase)
    progress_value = runtime.get("progress")
    if not isinstance(progress_value, int):
        completed = max(int(task.completed_chapters or 0), 0)
        total = max(int(task.total_chapters or 0), 1)
        progress_value = 100 if task.status == "completed" else int((completed / total) * 100)

    checkpoint = {
        "current_chapter_id": task.current_chapter_id,
        "current_chapter_number": task.current_chapter_number,
        "current_retry_count": task.current_retry_count,
        "max_retries": task.max_retries,
        "progress_phase": phase,
        "progress": max(0, min(progress_value, 100)),
        "last_event": runtime.get("last_event"),
        "last_message": runtime.get("last_message"),
        "candidate_index": runtime.get("candidate_index"),
        "candidate_count": runtime.get("candidate_count"),
        "word_count": runtime.get("word_count"),
        "generation_path": runtime.get("generation_path"),
        "attempt_kind": runtime.get("attempt_kind"),
        "rerank_used": runtime.get("rerank_used")
        if isinstance(runtime.get("rerank_used"), bool)
        else None,
        "word_budget_repair_used": runtime.get("word_budget_repair_used")
        if isinstance(runtime.get("word_budget_repair_used"), bool)
        else None,
        "winner_candidate_index": runtime.get("winner_candidate_index"),
        "pre_compaction_total_length": runtime.get("pre_compaction_total_length"),
        "context_budget_limit": runtime.get("context_budget_limit"),
        "compaction_applied": runtime.get("compaction_applied")
        if isinstance(runtime.get("compaction_applied"), bool)
        else None,
        "compaction_details": runtime.get("compaction_details")
        if isinstance(runtime.get("compaction_details"), dict)
        else None,
    }
    active_story_repair_payload = runtime.get("active_story_repair_payload")
    return {
        "stage_code": stage_code,
        "execution_mode": "interactive",
        "checkpoint": checkpoint,
        "active_story_repair_payload": (
            dict(active_story_repair_payload)
            if isinstance(active_story_repair_payload, dict)
            else None
        ),
    }


async def build_batch_generation_task_view_context(
    task: "BatchGenerationTask",
    *,
    db_session: "AsyncSession",
) -> BatchGenerationTaskViewContext:
    from tests.test_support.task_quality_snapshot_test_support import (
        get_task_quality_metrics_snapshot,
    )

    quality_snapshot = await get_task_quality_metrics_snapshot(task.id, db_session=db_session)
    workflow_snapshot = await build_batch_task_workflow_snapshot(task, db_session=db_session)
    return BatchGenerationTaskViewContext(
        task=task,
        quality_snapshot=quality_snapshot,
        workflow_snapshot=workflow_snapshot,
    )


async def load_batch_generation_task_view_context(
    db_session: "AsyncSession",
    *,
    batch_id: str,
) -> Optional[BatchGenerationTaskViewContext]:
    from sqlalchemy import select
    from migrator_app.models.batch_generation_task import BatchGenerationTask

    result = await db_session.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    task = result.scalar_one_or_none()
    if task is None:
        return None
    await recover_stale_batch_generation_tasks(db_session, [task])
    return await build_batch_generation_task_view_context(task, db_session=db_session)


async def load_active_project_batch_generation_task_view_context(
    db_session: "AsyncSession",
    *,
    project_id: str,
) -> Optional[BatchGenerationTaskViewContext]:
    from sqlalchemy import select
    from migrator_app.models.batch_generation_task import BatchGenerationTask

    result = await db_session.execute(
        select(BatchGenerationTask)
        .where(BatchGenerationTask.project_id == project_id)
        .where(BatchGenerationTask.status.in_(["pending", "running"]))
        .order_by(BatchGenerationTask.created_at.desc())
    )
    tasks = result.scalars().all()
    if not tasks:
        return None
    await recover_stale_batch_generation_tasks(db_session, tasks)
    task = next((item for item in tasks if item.status in {"pending", "running"}), None)
    if task is None:
        return None
    return await build_batch_generation_task_view_context(task, db_session=db_session)


async def load_active_user_batch_generation_task_view_contexts(
    db_session: "AsyncSession",
    *,
    user_id: str,
    limit: int,
) -> List[BatchGenerationTaskViewContext]:
    from sqlalchemy import select
    from migrator_app.models.batch_generation_task import BatchGenerationTask

    result = await db_session.execute(
        select(BatchGenerationTask)
        .where(BatchGenerationTask.user_id == user_id)
        .where(BatchGenerationTask.status.in_(["pending", "running"]))
        .order_by(BatchGenerationTask.created_at.desc())
        .limit(limit)
    )
    tasks = result.scalars().all()
    await recover_stale_batch_generation_tasks(db_session, tasks)
    contexts: List[BatchGenerationTaskViewContext] = []
    for task in tasks:
        if task.status not in {"pending", "running"}:
            continue
        contexts.append(await build_batch_generation_task_view_context(task, db_session=db_session))
    return contexts


def build_batch_generation_status_response(
    task: "BatchGenerationTask",
    *,
    quality_snapshot: Dict[str, Any],
    workflow_snapshot: Dict[str, Any],
) -> BatchGenerateStatusResponse:
    terminal_status = build_batch_task_terminal_status(
        task,
        workflow_snapshot=workflow_snapshot,
    )
    return BatchGenerateStatusResponse(
        batch_id=task.id,
        status=task.status,
        stage_code=workflow_snapshot["stage_code"],
        execution_mode=workflow_snapshot["execution_mode"],
        total=task.total_chapters,
        completed=task.completed_chapters,
        current_chapter_id=task.current_chapter_id,
        current_chapter_number=task.current_chapter_number,
        current_retry_count=task.current_retry_count,
        max_retries=task.max_retries,
        checkpoint=workflow_snapshot["checkpoint"],
        failed_chapters=task.failed_chapters or [],
        created_at=task.created_at.isoformat() if task.created_at else None,
        started_at=task.started_at.isoformat() if task.started_at else None,
        completed_at=task.completed_at.isoformat() if task.completed_at else None,
        error_message=task.error_message,
        latest_quality_metrics=quality_snapshot.get("latest"),
        quality_metrics_summary=quality_snapshot.get("summary"),
        active_story_repair_payload=workflow_snapshot.get("active_story_repair_payload"),
        terminal_reason=terminal_status["terminal_reason"],
        terminal_label=terminal_status["terminal_label"],
        review_required=terminal_status["review_required"],
        can_resume=terminal_status["can_resume"],
    )


def build_active_batch_generation_payload(
    task: "BatchGenerationTask",
    *,
    quality_snapshot: Dict[str, Any],
    workflow_snapshot: Dict[str, Any],
) -> Dict[str, Any]:
    return {
        "has_active_task": True,
        "task": {
            "batch_id": task.id,
            "status": task.status,
            "stage_code": workflow_snapshot["stage_code"],
            "execution_mode": workflow_snapshot["execution_mode"],
            "total": task.total_chapters,
            "completed": task.completed_chapters,
            "current_chapter_id": task.current_chapter_id,
            "current_chapter_number": task.current_chapter_number,
            "checkpoint": workflow_snapshot["checkpoint"],
            "latest_quality_metrics": quality_snapshot.get("latest"),
            "quality_metrics_summary": quality_snapshot.get("summary"),
            "active_story_repair_payload": workflow_snapshot.get(
                "active_story_repair_payload"
            ),
            "created_at": task.created_at.isoformat() if task.created_at else None,
            "started_at": task.started_at.isoformat() if task.started_at else None,
        },
    }


def _resolve_batch_generation_task_type(task: "BatchGenerationTask") -> str:
    if task.chapter_count == 1 and len(task.chapter_ids or []) == 1:
        return "chapter_single_generate"
    return "chapters_batch_generate"


def build_batch_generation_task_list_item(
    task: "BatchGenerationTask",
    *,
    quality_snapshot: Dict[str, Any],
    workflow_snapshot: Dict[str, Any],
) -> Dict[str, Any]:
    return {
        "task_type": _resolve_batch_generation_task_type(task),
        "stage_code": workflow_snapshot["stage_code"],
        "execution_mode": workflow_snapshot["execution_mode"],
        "project_id": task.project_id,
        "batch_id": task.id,
        "status": task.status,
        "total": task.total_chapters,
        "completed": task.completed_chapters,
        "current_chapter_id": task.current_chapter_id,
        "current_chapter_number": task.current_chapter_number,
        "checkpoint": workflow_snapshot["checkpoint"],
        "latest_quality_metrics": quality_snapshot.get("latest"),
        "quality_metrics_summary": quality_snapshot.get("summary"),
        "active_story_repair_payload": workflow_snapshot.get("active_story_repair_payload"),
        "created_at": task.created_at.isoformat() if task.created_at else None,
        "started_at": task.started_at.isoformat() if task.started_at else None,
        "completed_at": task.completed_at.isoformat() if task.completed_at else None,
        "error_message": task.error_message,
    }

