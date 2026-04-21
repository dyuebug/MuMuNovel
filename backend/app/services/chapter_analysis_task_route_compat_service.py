from __future__ import annotations

import asyncio

from fastapi import BackgroundTasks, HTTPException, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.api.common import verify_project_access
from app.logger import get_logger
from app.services.ai_service import AIService
from app.services.analysis_task_query_service import (
    build_empty_batch_analysis_status_response,
    load_batch_analysis_status_query_context,
    load_latest_analysis_task_for_chapter,
    normalize_batch_analysis_chapter_ids,
)
from app.services.analysis_task_status_service import (
    build_analysis_task_status_payload,
    build_batch_analysis_status_items,
)
from app.services.chapter_generation_prerequisite_service import (
    check_chapter_generation_prerequisites,
)
from app.services.manual_chapter_analysis_execution_service import (
    execute_chapter_analysis_background,
)
from app.services.manual_chapter_analysis_service import prepare_manual_chapter_analysis

logger = get_logger(__name__)


async def get_analysis_task_status_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    db_session: AsyncSession,
):
    user_id = require_authenticated_user_id(request)
    await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    task = await load_latest_analysis_task_for_chapter(
        db_session,
        chapter_id=chapter_id,
    )
    status_result = build_analysis_task_status_payload(chapter_id, task)
    if status_result.changed:
        await db_session.commit()

    return status_result.payload


async def get_batch_analysis_task_status_with_default_route_wiring(
    *,
    chapter_ids_input,
    request: Request | None,
    db_session: AsyncSession | None,
):
    chapter_ids = normalize_batch_analysis_chapter_ids(chapter_ids_input)
    if not chapter_ids:
        return build_empty_batch_analysis_status_response()

    if request is None or db_session is None:
        raise ValueError("request and db_session are required when chapter_ids are provided")

    query_context = await load_batch_analysis_status_query_context(
        db_session,
        chapter_ids=chapter_ids,
    )

    user_id = require_authenticated_user_id(request)
    for project_id in {chapter.project_id for chapter in query_context.chapters}:
        await verify_project_access(project_id, user_id, db_session)

    status_items_result = build_batch_analysis_status_items(
        chapter_ids,
        latest_tasks_by_chapter_id=query_context.latest_tasks_by_chapter_id,
    )
    if status_items_result.changed:
        await db_session.commit()

    return {
        "project_id": query_context.response_project_id,
        "total": len(status_items_result.items),
        "items": status_items_result.items,
    }


async def check_can_generate_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    db_session: AsyncSession,
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    can_generate, error_message, previous_chapters = await check_chapter_generation_prerequisites(
        db_session,
        chapter,
    )
    previous_info = [
        {
            "id": previous_chapter.id,
            "chapter_number": previous_chapter.chapter_number,
            "title": previous_chapter.title,
            "has_content": bool(previous_chapter.content and previous_chapter.content.strip()),
            "word_count": previous_chapter.word_count or 0,
        }
        for previous_chapter in previous_chapters
    ]

    return {
        "can_generate": can_generate,
        "reason": error_message if not can_generate else "",
        "previous_chapters": previous_info,
        "chapter_number": chapter.chapter_number,
    }


async def trigger_chapter_analysis_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    db_session: AsyncSession,
    user_ai_service: AIService,
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    if not chapter.content or chapter.content.strip() == "":
        raise HTTPException(status_code=400, detail="章节不存在或内容为空")

    project = await verify_project_access(chapter.project_id, user_id, db_session)
    manual_analysis = await prepare_manual_chapter_analysis(
        db_session,
        chapter=chapter,
        project=project,
        user_id=user_id,
    )
    if manual_analysis is None:
        raise HTTPException(status_code=409, detail="Chapter or project was deleted before analysis task creation")

    logger.info("Created analysis task: %s, chapter=%s", manual_analysis.task_id, chapter_id)
    await asyncio.sleep(3)

    background_tasks.add_task(
        execute_chapter_analysis_background,
        chapter_id=chapter_id,
        user_id=user_id,
        project_id=project.id,
        task_id=manual_analysis.task_id,
        ai_service=user_ai_service,
        quality_profile=manual_analysis.quality_profile,
        story_packet=manual_analysis.story_packet,
    )

    return {
        "task_id": manual_analysis.task_id,
        "chapter_id": chapter_id,
        "status": "pending",
        "message": "章节分析任务已创建",
    }
