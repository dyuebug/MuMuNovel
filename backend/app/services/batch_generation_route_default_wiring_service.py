"""批量生成 route default wiring owner service。"""
from __future__ import annotations

from typing import Any

from fastapi import BackgroundTasks, HTTPException

from app.schemas.chapter import BatchGenerateRequest

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation route, runtime, and read-context "
    "chain; this Python default wiring owner module is kept only as frozen "
    "rollback/source-map material after the remaining legacy route wiring "
    "surface was split into narrower shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs; "
    "backend-rs/src/api/health.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"


async def verify_project_access(*args, **kwargs):
    from app.api.common import verify_project_access as verify_project_access_service

    return await verify_project_access_service(*args, **kwargs)


async def orchestrate_batch_generation_create_with_default_wiring(
    db_session: Any,
    *,
    project_id: str,
    project: Any,
    user_id: str,
    batch_request: BatchGenerateRequest,
    background_tasks: BackgroundTasks,
    ai_service: Any,
):
    from app.services.batch_generation_orchestration_service import (
        orchestrate_batch_generation_create,
    )
    from app.services.batch_generation_run_wiring_service import (
        execute_batch_generation_in_order_with_entry_service_seams,
    )
    from app.services.chapter_generation.prerequisite_service import (
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
        execution_callable=execute_batch_generation_in_order_with_entry_service_seams,
    )


async def orchestrate_batch_generation_resume_with_default_wiring(
    db_session: Any,
    *,
    batch_id: str,
    user_id: str,
    background_tasks: BackgroundTasks,
    ai_service: Any,
):
    from app.services.batch_generation_orchestration_service import (
        orchestrate_batch_generation_resume,
    )
    from app.services.batch_generation_run_wiring_service import (
        execute_batch_generation_in_order_with_entry_service_seams,
    )
    from app.services.chapter_generation.prerequisite_service import (
        check_chapter_generation_prerequisites,
    )
    from app.services.story_repair_payload_service import (
        resolve_generation_story_repair_state_for_batch,
    )

    return await orchestrate_batch_generation_resume(
        db_session,
        batch_id=batch_id,
        user_id=user_id,
        background_tasks=background_tasks,
        ai_service=ai_service,
        resolve_story_repair_state_for_batch=resolve_generation_story_repair_state_for_batch,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        execution_callable=execute_batch_generation_in_order_with_entry_service_seams,
    )


async def load_batch_generation_status_with_default_wiring(
    db_session: Any,
    *,
    batch_id: str,
):
    from app.services.batch_generation.route_wiring_service import (
        build_batch_generation_status_response,
        load_batch_generation_task_view_context,
    )

    task_view = await load_batch_generation_task_view_context(
        db_session,
        batch_id=batch_id,
    )
    if task_view is None:
        raise HTTPException(status_code=404, detail="Batch generation task not found")

    return build_batch_generation_status_response(
        task_view.task,
        quality_snapshot=task_view.quality_snapshot,
        workflow_snapshot=task_view.workflow_snapshot,
    )


async def load_active_project_batch_generation_with_default_wiring(
    db_session: Any,
    *,
    project_id: str,
    user_id: str | None,
):
    from app.services.batch_generation.route_wiring_service import (
        build_active_batch_generation_payload,
        load_active_project_batch_generation_task_view_context,
    )

    await verify_project_access(project_id, user_id, db_session)

    task_view = await load_active_project_batch_generation_task_view_context(
        db_session,
        project_id=project_id,
    )
    if task_view is None:
        return {
            "has_active_task": False,
            "task": None,
        }

    return build_active_batch_generation_payload(
        task_view.task,
        quality_snapshot=task_view.quality_snapshot,
        workflow_snapshot=task_view.workflow_snapshot,
    )


async def load_active_batch_generation_task_list_with_default_wiring(
    db_session: Any,
    *,
    user_id: str | None,
    limit: int,
):
    from app.services.batch_generation.route_wiring_service import (
        build_batch_generation_task_list_item,
        load_active_user_batch_generation_task_view_contexts,
    )

    if not user_id:
        raise HTTPException(status_code=401, detail="Not logged in")

    task_views = await load_active_user_batch_generation_task_view_contexts(
        db_session,
        user_id=user_id,
        limit=limit,
    )
    items = [
        build_batch_generation_task_list_item(
            task_view.task,
            quality_snapshot=task_view.quality_snapshot,
            workflow_snapshot=task_view.workflow_snapshot,
        )
        for task_view in task_views
    ]
    return {
        "total": len(items),
        "items": items,
    }
