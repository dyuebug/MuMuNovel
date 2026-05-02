"""Compatibility helpers for batch generation route defaults."""
from __future__ import annotations

from typing import Any, Dict

from fastapi import BackgroundTasks, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.project import Project
from app.services import batch_generation_entry_compat_service
from app.api import chapters as chapters_api
from app.services.batch_generation_orchestration_service import (
    orchestrate_batch_generation_create,
    orchestrate_batch_generation_resume,
)
from app.services.batch_generation_stream_service import (
    build_batch_generation_event_stream,
    validate_batch_generation_stream_access,
)
from app.services.chapter_generation_prerequisite_service import (
    check_chapter_generation_prerequisites,
)
from app.services.chapter_quality_context_service import (
    build_story_generation_packet_with_project_continuity,
    resolve_chapter_quality_profile,
)
from app.services.story_repair_payload_service import (
    resolve_generation_story_repair_state_for_batch,
)
from app.services.task_workflow_runtime_service import sync_task_story_repair_state
from app.services.ai_service import AIService


async def orchestrate_batch_generation_create_with_default_wiring(
    db_session: AsyncSession,
    *,
    project_id: str,
    project: Project,
    user_id: str,
    batch_request: Any,
    background_tasks: BackgroundTasks,
    ai_service: AIService,
) -> Dict[str, Any]:
    return await orchestrate_batch_generation_create(
        db_session,
        project_id=project_id,
        project=project,
        user_id=user_id,
        batch_request=batch_request,
        background_tasks=background_tasks,
        ai_service=ai_service,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        resolve_quality_profile_fn=resolve_chapter_quality_profile,
        build_story_packet_fn=build_story_generation_packet_with_project_continuity,
        resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_batch,
        sync_task_story_repair_state_fn=sync_task_story_repair_state,
        execution_callable=chapters_api.execute_batch_generation_in_order,
    )


async def orchestrate_batch_generation_resume_with_default_wiring(
    db_session: AsyncSession,
    *,
    batch_id: str,
    user_id: str,
    background_tasks: BackgroundTasks,
    ai_service: AIService,
) -> Dict[str, Any]:
    return await orchestrate_batch_generation_resume(
        db_session,
        batch_id=batch_id,
        user_id=user_id,
        background_tasks=background_tasks,
        ai_service=ai_service,
        resolve_story_repair_state_for_batch=resolve_generation_story_repair_state_for_batch,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        execution_callable=batch_generation_entry_compat_service.execute_batch_generation_in_order,
    )

from app.utils.sse_response import create_sse_response


async def stream_batch_generation_events_with_default_route_wiring(
    db_session: AsyncSession,
    *,
    batch_id: str,
    request: Request,
):
    user_id = getattr(request.state, "user_id", None)
    await validate_batch_generation_stream_access(
        db_session,
        batch_id=batch_id,
        user_id=user_id,
    )
    return create_sse_response(
        build_batch_generation_event_stream(
            db_session,
            batch_id=batch_id,
        )
    )

