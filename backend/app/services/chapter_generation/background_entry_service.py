from __future__ import annotations

from typing import Any, Callable

from fastapi import BackgroundTasks, HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.project import Project
from app.schemas.chapter import ChapterGenerateRequest
from app.services.ai_service import AIService
from app.services.batch_generation_orchestration_service import (
    orchestrate_single_chapter_background_generation,
)


async def generate_chapter_content_background_with_default_wiring(
    *,
    db_session: AsyncSession,
    chapter_id: str,
    user_id: str,
    generate_request: ChapterGenerateRequest,
    background_tasks: BackgroundTasks,
    ai_service: AIService,
    load_accessible_chapter_or_404_fn: Callable[..., Any],
    check_prerequisites_fn: Callable[..., Any],
    build_workflow_snapshot_fn: Callable[..., Any],
    resolve_story_repair_state_fn: Callable[..., Any],
    sync_task_story_repair_state_fn: Callable[..., Any],
    execution_callable: Callable[..., Any],
):
    chapter = await load_accessible_chapter_or_404_fn(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    project_result = await db_session.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        raise HTTPException(status_code=404, detail='Project not found')

    return await orchestrate_single_chapter_background_generation(
        db_session,
        chapter_id=chapter_id,
        chapter=chapter,
        project=project,
        user_id=user_id,
        generate_request=generate_request,
        background_tasks=background_tasks,
        ai_service=ai_service,
        check_prerequisites_fn=check_prerequisites_fn,
        build_workflow_snapshot_fn=build_workflow_snapshot_fn,
        resolve_story_repair_state_fn=resolve_story_repair_state_fn,
        sync_task_story_repair_state_fn=sync_task_story_repair_state_fn,
        execution_callable=execution_callable,
    )
