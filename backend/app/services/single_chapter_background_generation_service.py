"""单章后台生成编排 service。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Callable, Dict, Optional

from fastapi import HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active single-generation background access and launch "
    "workflow chain; this Python single-chapter background orchestration "
    "module is kept only as frozen rollback/source-map material after the "
    "remaining batch orchestration owner was split into narrower shells."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/services/chapter_access_service.rs"
SOURCE_MAP_ROLLBACK_FLAG = "legacy_single_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.services.single_chapter_background_task_helper_service import (
    recover_stale_single_chapter_background_task_if_needed,
    single_chapter_background_task_contains_chapter,
)

if TYPE_CHECKING:
    from app.models.batch_generation_task import BatchGenerationTask
    from app.models.chapter import Chapter
    from app.models.project import Project
    from app.services.ai_service import AIService
    from app.services.single_chapter_background_context_service import (
        SingleChapterBackgroundExecutionContext,
    )


@dataclass(frozen=True)
class SingleChapterBackgroundGenerationPreparation:
    execution_context: "SingleChapterBackgroundExecutionContext"
    story_repair_state: Dict[str, Any]
    enable_web_research: Optional[bool]
    web_research_query: Optional[str]


async def load_existing_single_chapter_background_task_payload(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    user_id: str,
    project_id: str,
    build_workflow_snapshot_fn,
) -> Optional[Dict[str, Any]]:
    from app.models.batch_generation_task import BatchGenerationTask
    from app.services.batch_generation_workflow_service import calculate_estimated_time

    active_result = await db_session.execute(
        select(BatchGenerationTask)
        .where(BatchGenerationTask.user_id == user_id)
        .where(BatchGenerationTask.project_id == project_id)
        .where(BatchGenerationTask.status.in_(["pending", "running"]))
        .order_by(BatchGenerationTask.created_at.desc())
    )
    active_tasks = active_result.scalars().all()
    changed = False

    for task in active_tasks:
        if recover_stale_single_chapter_background_task_if_needed(task):
            changed = True

    if changed:
        await db_session.commit()

    for task in active_tasks:
        if task.status not in {"pending", "running"}:
            continue
        if not single_chapter_background_task_contains_chapter(task, chapter_id):
            continue

        workflow_snapshot = await build_workflow_snapshot_fn(task, db_session=db_session)
        return {
            "task_id": task.id,
            "chapter_id": chapter_id,
            "status": task.status,
            "message": "已有后台生成任务正在执行",
            "estimated_time_minutes": calculate_estimated_time(
                chapter_count=1,
                target_word_count=task.target_word_count or 3000,
                enable_analysis=bool(task.enable_analysis),
            ),
            "active_story_repair_payload": workflow_snapshot.get(
                "active_story_repair_payload"
            ),
        }

    return None


async def prepare_single_chapter_background_generation(
    db_session: AsyncSession,
    *,
    chapter: "Chapter",
    project: "Project",
    user_id: str,
    generate_request: Any,
    resolve_story_repair_state_fn,
) -> SingleChapterBackgroundGenerationPreparation:
    from app.services.single_chapter_background_context_service import (
        build_single_chapter_background_execution_context,
    )

    execution_context = await build_single_chapter_background_execution_context(
        db_session,
        user_id=user_id,
        project=project,
        generate_request=generate_request,
    )
    story_repair_state = await resolve_story_repair_state_fn(
        db_session,
        chapter=chapter,
        story_repair_summary=getattr(generate_request, "story_repair_summary", None),
        story_repair_targets=getattr(generate_request, "story_repair_targets", None),
        story_preserve_strengths=getattr(
            generate_request, "story_preserve_strengths", None
        ),
    )

    return SingleChapterBackgroundGenerationPreparation(
        execution_context=execution_context,
        story_repair_state=(
            dict(story_repair_state) if isinstance(story_repair_state, dict) else {}
        ),
        enable_web_research=getattr(generate_request, "enable_web_research", None),
        web_research_query=getattr(generate_request, "web_research_query", None),
    )


async def create_single_chapter_background_generation_and_enqueue(
    db_session: AsyncSession,
    *,
    chapter: "Chapter",
    user_id: str,
    preparation: SingleChapterBackgroundGenerationPreparation,
    background_tasks,
    ai_service: "AIService",
    execution_callable: Callable[..., Any],
    sync_task_story_repair_state_fn,
) -> Dict[str, Any]:
    from app.services.batch_generation_workflow_service import (
        calculate_estimated_time,
        create_batch_generation_task_record,
        enqueue_batch_generation_execution,
    )

    execution_context = preparation.execution_context
    task = await create_batch_generation_task_record(
        db_session,
        project_id=chapter.project_id,
        user_id=user_id,
        start_chapter_number=chapter.chapter_number,
        chapter_ids=[chapter.id],
        style_id=execution_context.resolved_style_id,
        target_word_count=execution_context.target_word_count,
        enable_analysis=execution_context.enable_analysis,
        max_retries=3,
    )
    story_repair_state = await sync_task_story_repair_state_fn(
        task.id,
        story_repair_state=preparation.story_repair_state,
        db_session=db_session,
    )
    enqueue_batch_generation_execution(
        background_tasks,
        execution_callable,
        batch_id=task.id,
        user_id=user_id,
        ai_service=ai_service,
        custom_model=execution_context.custom_model,
        temp_narrative_perspective=execution_context.temp_narrative_perspective,
        story_packet=execution_context.story_packet,
        base_quality_profile=execution_context.quality_profile,
        enable_web_research=preparation.enable_web_research,
        web_research_query=preparation.web_research_query,
        story_repair_payload=preparation.story_repair_state.get("payload"),
    )
    estimated_time = calculate_estimated_time(
        chapter_count=1,
        target_word_count=execution_context.target_word_count,
        enable_analysis=execution_context.enable_analysis,
    )
    return {
        "task_id": task.id,
        "chapter_id": chapter.id,
        "status": "pending",
        "message": "后台生成任务已创建，正在排队执行",
        "estimated_time_minutes": estimated_time,
        "active_story_repair_payload": story_repair_state.get(
            "active_story_repair_payload"
        ),
    }


async def orchestrate_single_chapter_background_generation(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    chapter: "Chapter",
    project: "Project",
    user_id: str,
    generate_request: Any,
    background_tasks,
    ai_service: "AIService",
    check_prerequisites_fn,
    build_workflow_snapshot_fn,
    resolve_story_repair_state_fn,
    sync_task_story_repair_state_fn,
    execution_callable: Callable[..., Any],
) -> Dict[str, Any]:
    can_generate, error_msg, _ = await check_prerequisites_fn(db_session, chapter)
    if not can_generate:
        raise HTTPException(status_code=400, detail=error_msg)

    existing_task_payload = await load_existing_single_chapter_background_task_payload(
        db_session,
        chapter_id=chapter_id,
        user_id=user_id,
        project_id=chapter.project_id,
        build_workflow_snapshot_fn=build_workflow_snapshot_fn,
    )
    if existing_task_payload is not None:
        return existing_task_payload

    generation_preparation = await prepare_single_chapter_background_generation(
        db_session,
        chapter=chapter,
        project=project,
        user_id=user_id,
        generate_request=generate_request,
        resolve_story_repair_state_fn=resolve_story_repair_state_fn,
    )
    return await create_single_chapter_background_generation_and_enqueue(
        db_session,
        chapter=chapter,
        user_id=user_id,
        preparation=generation_preparation,
        background_tasks=background_tasks,
        ai_service=ai_service,
        execution_callable=execution_callable,
        sync_task_story_repair_state_fn=sync_task_story_repair_state_fn,
    )
