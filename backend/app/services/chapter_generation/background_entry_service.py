from __future__ import annotations

from typing import TYPE_CHECKING, Any, Callable

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active single-generation background access and launch "
    "workflow chain; this Python module is kept only as frozen "
    "rollback/source-map material after explicit support-shell freeze "
    "approval."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/services/chapter_access_service.rs"
SOURCE_MAP_ROLLBACK_FLAG = "legacy_single_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from fastapi import BackgroundTasks

from app.schemas.chapter import ChapterGenerateRequest
from app.services.ai_service import AIService
from app.services.batch_generation_orchestration_service import (
    orchestrate_single_chapter_background_generation,
)

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession


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
    from app.services.chapter_generation import route_wiring_service

    return await route_wiring_service.generate_chapter_content_background_with_explicit_wiring(
        db_session=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
        generate_request=generate_request,
        background_tasks=background_tasks,
        ai_service=ai_service,
        load_accessible_chapter_or_404_fn=load_accessible_chapter_or_404_fn,
        check_prerequisites_fn=check_prerequisites_fn,
        build_workflow_snapshot_fn=build_workflow_snapshot_fn,
        resolve_story_repair_state_fn=resolve_story_repair_state_fn,
        sync_task_story_repair_state_fn=sync_task_story_repair_state_fn,
        execution_callable=execution_callable,
        orchestrate_single_chapter_background_generation_fn=orchestrate_single_chapter_background_generation,
    )
