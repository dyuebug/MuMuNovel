"""批量生成章节成功状态 helper。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Dict, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch success-state, retry, and applied-chapter "
    "projection chain; this Python helper is kept only as frozen "
    "rollback/source-map material for legacy batch success handling."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.logger import get_logger

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.batch_generation_task import BatchGenerationTask
    from app.models.chapter import Chapter
    from app.models.project import Project


logger = get_logger(__name__)


def apply_generated_batch_chapter_candidate(*args, **kwargs):
    from app.services.batch_generation_chapter_persistence_service import (
        apply_generated_batch_chapter_candidate as apply_generated_batch_chapter_candidate_service,
    )

    return apply_generated_batch_chapter_candidate_service(*args, **kwargs)


def build_batch_chapter_draft_attempt(*args, **kwargs):
    from app.services.batch_generation_chapter_persistence_service import (
        build_batch_chapter_draft_attempt as build_batch_chapter_draft_attempt_service,
    )

    return build_batch_chapter_draft_attempt_service(*args, **kwargs)


@dataclass(frozen=True)
class BatchGenerationQualityGateRetryPreparation:
    active_story_repair_state: Dict[str, Any]
    active_story_repair_payload: Any
    next_retry_count: int


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
    active_story_repair_payload = quality_gate_plan.get("repair_payload") or active_story_repair_payload
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
        quality_metrics=generation_quality_metrics if isinstance(generation_quality_metrics, dict) else None,
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
            "active_story_repair_payload": quality_gate_plan.get("active_story_repair_payload"),
        }
    )

    return BatchGenerationQualityGateRetryPreparation(
        active_story_repair_state=(
            dict(active_story_repair_state) if isinstance(active_story_repair_state, dict) else {}
        ),
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
        last_generated_summary = f"Chapter {chapter.chapter_number} ({chapter.title}): {generated_summary}"
        logger.info(f"Updated previous chapter summary context: {last_generated_summary[:50]}...")

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
        active_story_repair_state=(
            dict(active_story_repair_state) if isinstance(active_story_repair_state, dict) else {}
        ),
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
            "phase": "loading" if task.completed_chapters < task.total_chapters else "saving",
            "current_retry_count": 0,
            "max_retries": task.max_retries,
        }
    )
