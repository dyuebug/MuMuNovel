from __future__ import annotations

from typing import Any

from fastapi import HTTPException, Request
from sqlalchemy import select

from tests.test_support.batch_generation_status_read_owner_test_adapter import (
    build_batch_task_workflow_snapshot,
)


def _project_model():
    from migrator_app.models.project import Project

    return Project


def require_authenticated_user_id(*args, **kwargs):
    from tests.test_support.chapter_route_helpers_test_support import (
        require_authenticated_user_id as impl,
    )

    return impl(*args, **kwargs)


async def load_accessible_chapter_or_404(*args, **kwargs):
    from tests.test_support.chapter_route_helpers_test_support import (
        load_accessible_chapter_or_404 as impl,
    )

    return await impl(*args, **kwargs)


async def check_chapter_generation_prerequisites(*args, **kwargs):
    from tests.test_support.chapter_query_test_support import (
        check_chapter_generation_prerequisites as impl,
    )

    return await impl(*args, **kwargs)


async def execute_batch_generation_in_order_with_default_wiring(*args, **kwargs):
    from tests.test_support.batch_generation_run_wiring_test_adapter import (
        execute_batch_generation_in_order_with_default_wiring as impl,
    )

    return await impl(*args, **kwargs)


async def orchestrate_single_chapter_background_generation(*args, **kwargs):
    from tests.test_support.single_generation_background_orchestration_test_adapter import (
        orchestrate_single_chapter_background_generation as impl,
    )

    return await impl(*args, **kwargs)


async def resolve_generation_story_repair_state_for_chapter(*args, **kwargs):
    from tests.test_support.story_repair_payload_test_support import (
        resolve_generation_story_repair_state_for_chapter as impl,
    )

    return await impl(*args, **kwargs)


async def sync_task_story_repair_state(*args, **kwargs):
    from tests.test_support.task_system import (
        sync_task_story_repair_state as impl,
    )

    return await impl(*args, **kwargs)


async def generate_chapter_content_background_with_explicit_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: Any,
    generate_request: Any,
    db_session: Any,
    user_ai_service: Any,
    require_authenticated_user_id_fn,
    load_accessible_chapter_or_404_fn,
    project_model,
    check_prerequisites_fn,
    build_workflow_snapshot_fn,
    resolve_story_repair_state_fn,
    sync_task_story_repair_state_fn,
    orchestrate_background_generation_fn,
    execution_callable=None,
):
    user_id = require_authenticated_user_id_fn(request)
    chapter = await load_accessible_chapter_or_404_fn(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    project_result = await db_session.execute(
        select(project_model).where(project_model.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        raise HTTPException(status_code=404, detail="Project not found")

    orchestrate_kwargs = {
        "chapter_id": chapter_id,
        "chapter": chapter,
        "project": project,
        "user_id": user_id,
        "generate_request": generate_request,
        "background_tasks": background_tasks,
        "ai_service": user_ai_service,
        "check_prerequisites_fn": check_prerequisites_fn,
        "build_workflow_snapshot_fn": build_workflow_snapshot_fn,
        "resolve_story_repair_state_fn": resolve_story_repair_state_fn,
        "sync_task_story_repair_state_fn": sync_task_story_repair_state_fn,
    }
    if execution_callable is not None:
        orchestrate_kwargs["execution_callable"] = execution_callable

    return await orchestrate_background_generation_fn(
        db_session,
        **orchestrate_kwargs,
    )


async def generate_chapter_content_background_with_default_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: Any,
    generate_request: Any,
    db_session: Any,
    user_ai_service: Any,
):
    return await generate_chapter_content_background_with_explicit_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        generate_request=generate_request,
        db_session=db_session,
        user_ai_service=user_ai_service,
        require_authenticated_user_id_fn=require_authenticated_user_id,
        load_accessible_chapter_or_404_fn=load_accessible_chapter_or_404,
        project_model=_project_model(),
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        build_workflow_snapshot_fn=build_batch_task_workflow_snapshot,
        resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_chapter,
        sync_task_story_repair_state_fn=sync_task_story_repair_state,
        orchestrate_background_generation_fn=orchestrate_single_chapter_background_generation,
    )

