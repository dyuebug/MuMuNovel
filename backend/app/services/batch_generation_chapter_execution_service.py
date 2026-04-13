"""?????????? helper?"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Optional

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.batch_generation_task import BatchGenerationTask
from app.models.chapter import Chapter
from app.models.project import Project
from app.services.task_quality_snapshot_service import clear_task_quality_metrics_cache
from app.services.task_workflow_runtime_service import clear_task_workflow_runtime_cache


logger = get_logger(__name__)


@dataclass(frozen=True)
class BatchGenerationChapterAttemptPreparation:
    chapter: Chapter
    analysis_quality_profile: Dict[str, Any]


async def prepare_batch_generation_chapter_attempt(
    db_session: AsyncSession,
    *,
    task: BatchGenerationTask,
    project: Project,
    chapter_id: str,
    retry_count: int,
    write_lock,
    emit_event,
    cached_analysis_quality_profile: Dict[str, Any],
    clone_quality_profile_fn,
) -> BatchGenerationChapterAttemptPreparation:
    chapter_result = await db_session.execute(
        select(Chapter).where(Chapter.id == chapter_id)
    )
    chapter = chapter_result.scalar_one_or_none()
    if chapter is None:
        raise Exception(f"?? {chapter_id} ???")
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
    chapter: Chapter,
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
        generation_quality_metrics = {
            "quality_gate": quality_gate_snapshot,
        }

    generation_quality_metrics = attach_story_runtime_contract_fn(
        generation_quality_metrics,
        generation_story_runtime_contract if isinstance(generation_story_runtime_contract, dict) else None,
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
        generation_quality_metrics=generation_quality_metrics if isinstance(generation_quality_metrics, dict) else None,
        generation_story_runtime_contract=(
            generation_story_runtime_contract if isinstance(generation_story_runtime_contract, dict) else None
        ),
        quality_gate_plan=quality_gate_plan,
        quality_gate_snapshot=quality_gate_snapshot if isinstance(quality_gate_snapshot, dict) else None,
        quality_gate_action=quality_gate_action,
        quality_gate_requires_followup=quality_gate_requires_followup,
        should_run_analysis=should_run_analysis,
        metrics_event=metrics_event,
    )


async def clear_batch_generation_execution_caches(task_id: str) -> None:
    await clear_task_quality_metrics_cache(task_id)
    await clear_task_workflow_runtime_cache(task_id)
