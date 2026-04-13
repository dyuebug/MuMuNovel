"""?????????? service?"""
from __future__ import annotations

from typing import Any, Callable, Dict

from fastapi import BackgroundTasks, HTTPException
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.models.project import Project
from app.services.ai_service import AIService
from app.services.batch_generation_create_service import (
    create_batch_generation_and_enqueue,
    prepare_batch_generation_create,
)
from app.services.batch_generation_resume_service import (
    create_resumed_batch_generation_and_enqueue,
    prepare_batch_generation_resume,
)
from app.services.single_chapter_background_generation_service import (
    create_single_chapter_background_generation_and_enqueue,
    load_existing_single_chapter_background_task_payload,
    prepare_single_chapter_background_generation,
)


async def orchestrate_single_chapter_background_generation(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    chapter: Chapter,
    project: Project,
    user_id: str,
    generate_request: Any,
    background_tasks: BackgroundTasks,
    ai_service: AIService,
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


async def orchestrate_batch_generation_create(
    db_session: AsyncSession,
    *,
    project_id: str,
    project: Project,
    user_id: str,
    batch_request: Any,
    background_tasks: BackgroundTasks,
    ai_service: AIService,
    check_prerequisites_fn,
    resolve_quality_profile_fn,
    build_story_packet_fn,
    resolve_story_repair_state_fn,
    sync_task_story_repair_state_fn,
    execution_callable: Callable[..., Any],
) -> Dict[str, Any]:
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
        execution_callable=execution_callable,
        sync_task_story_repair_state_fn=sync_task_story_repair_state_fn,
    )


async def orchestrate_batch_generation_resume(
    db_session: AsyncSession,
    *,
    batch_id: str,
    user_id: str,
    background_tasks: BackgroundTasks,
    ai_service: AIService,
    resolve_story_repair_state_for_batch,
    check_prerequisites_fn,
    execution_callable: Callable[..., Any],
) -> Dict[str, Any]:
    resume_preparation = await prepare_batch_generation_resume(
        db_session,
        batch_id=batch_id,
        user_id=user_id,
        resolve_story_repair_state_for_batch=resolve_story_repair_state_for_batch,
        check_prerequisites_fn=check_prerequisites_fn,
    )
    resume_result = await create_resumed_batch_generation_and_enqueue(
        db_session,
        preparation=resume_preparation,
        user_id=user_id,
        background_tasks=background_tasks,
        ai_service=ai_service,
        execution_callable=execution_callable,
    )
    return resume_result.response_payload
