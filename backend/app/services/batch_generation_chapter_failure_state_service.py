"""批量生成章节失败状态 helper。"""
from __future__ import annotations

from datetime import datetime
from typing import TYPE_CHECKING, Any, Dict, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch terminal-state and failed-chapter persistence "
    "chain; this Python helper is kept only as frozen rollback/source-map "
    "material for legacy batch failure handling."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs"
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


def build_batch_chapter_draft_attempt(*args, **kwargs):
    from app.services.batch_generation_chapter_persistence_service import (
        build_batch_chapter_draft_attempt as build_batch_chapter_draft_attempt_service,
    )

    return build_batch_chapter_draft_attempt_service(*args, **kwargs)


async def fail_batch_generation_for_manual_review(
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
    quality_gate_plan: Dict[str, Any],
    quality_gate_snapshot: Optional[Dict[str, Any]],
    generated_content: str,
    generated_summary: str,
    generation_quality_metrics: Optional[Dict[str, Any]],
    active_story_repair_state: Dict[str, Any],
) -> None:
    quality_gate = quality_gate_snapshot or {}
    failed_metric_labels = [
        item.get("label")
        for item in (quality_gate.get("failed_metrics") or [])
        if isinstance(item, dict) and isinstance(item.get("label"), str) and item.get("label")
    ]
    error_message = quality_gate_plan.get("message") or (
        f"Chapter {chapter.chapter_number} is blocked by the quality gate and requires manual review."
    )
    failed_info = {
        'chapter_id': chapter_id,
        'chapter_number': chapter.chapter_number,
        'title': chapter.title,
        'error': error_message,
        'retry_count': retry_count,
        'phase': 'quality_blocked',
        'quality_gate_status': quality_gate.get('status'),
        'quality_gate_decision': quality_gate.get('decision'),
        'quality_gate_label': quality_gate.get('label'),
        'quality_gate_failed_metrics': failed_metric_labels,
    }

    manual_review_attempt = build_batch_chapter_draft_attempt(
        project_id=project.id,
        chapter_id=chapter_id,
        batch_task_id=batch_id,
        source="batch",
        attempt_state="manual_review",
        quality_gate_action="manual_review",
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
        task.failed_chapters = [
            *(task.failed_chapters or []),
            failed_info,
        ]
        task.status = 'failed'
        task.error_message = error_message[:500]
        task.completed_at = datetime.now()
        task.current_retry_count = 0
        db_session.add(manual_review_attempt)
        await db_session.commit()

    logger.error(f"Quality gate blocked chapter {chapter.chapter_number}: {error_message}")
    await emit_event(
        {
            "type": "error",
            "error": task.error_message or error_message,
            "code": 422,
            "phase": "quality_blocked",
        }
    )
    await emit_event({"type": "done"})


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
        'chapter_id': chapter_id,
        'chapter_number': chapter.chapter_number,
        'title': chapter.title,
        'error': f"章节分析失败，已重试3次: {analysis_error}",
        'retry_count': 3,
    }

    async with write_lock:
        task.failed_chapters = [
            *(task.failed_chapters or []),
            failed_info,
        ]
        task.status = 'failed'
        task.error_message = f"第{chapter.chapter_number}章分析失败，已重试3次: {analysis_error}"[:500]
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
        'chapter_id': chapter_id,
        'chapter_number': chapter_number,
        'title': chapter_title,
        'error': last_error,
        'retry_count': retry_count - 1,
    }

    async with write_lock:
        task.failed_chapters = [
            *(task.failed_chapters or []),
            failed_info,
        ]
        task.status = 'failed'
        task.error_message = f"第{chapter_number}章生成失败(重试{retry_count-1}次): {last_error}"[:500]
        task.completed_at = datetime.now()
        task.current_retry_count = 0
        await db_session.commit()

    if task.enable_analysis:
        logger.error("章节生成失败: 已达到最大重试次数/分析未通过")
    else:
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
