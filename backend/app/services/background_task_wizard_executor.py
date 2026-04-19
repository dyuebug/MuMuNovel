"""Wizard 后台任务执行器。"""
from __future__ import annotations

from typing import Any, Awaitable, Callable, Dict

from sqlalchemy.ext.asyncio import AsyncSession

from app.services.ai_service import AIService
from app.services.background_task_manager import background_task_manager

WizardTaskRunner = Callable[[str, str, Dict[str, Any], AsyncSession, AIService], Awaitable[Any]]


async def _run_wizard_world_building_task(
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> Any:
    from app.api.wizard_stream import world_building_generator

    world_payload = dict(payload)
    world_payload["user_id"] = user_id
    return world_building_generator(world_payload, db, user_ai_service)


async def _run_wizard_career_system_task(
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> Any:
    from app.api.wizard_stream import career_system_generator

    career_payload = dict(payload)
    career_payload["project_id"] = project_id
    career_payload["user_id"] = user_id
    return career_system_generator(career_payload, db, user_ai_service)


async def _run_wizard_characters_task(
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> Any:
    from app.api.wizard_stream import characters_generator

    characters_payload = dict(payload)
    characters_payload["project_id"] = project_id
    characters_payload["user_id"] = user_id
    return characters_generator(characters_payload, db, user_ai_service)


async def _run_wizard_outline_task(
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> Any:
    from app.api.wizard_stream import outline_generator

    outline_payload = dict(payload)
    outline_payload["project_id"] = project_id
    outline_payload["user_id"] = user_id
    return outline_generator(outline_payload, db, user_ai_service)


WIZARD_TASK_RUNNERS: Dict[str, WizardTaskRunner] = {
    "wizard_world_building": _run_wizard_world_building_task,
    "wizard_career_system": _run_wizard_career_system_task,
    "wizard_characters": _run_wizard_characters_task,
    "wizard_outline": _run_wizard_outline_task,
}


async def run_wizard_background_task(
    task_id: str,
    user_id: str,
    task_type: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> bool:
    """执行 wizard 后台任务并返回是否成功。"""
    runner = WIZARD_TASK_RUNNERS.get(task_type)
    if runner is None:
        return False

    stream = await runner(user_id, project_id, payload, db, user_ai_service)
    await background_task_manager.consume_sse_stream(task_id, stream)
    return True
