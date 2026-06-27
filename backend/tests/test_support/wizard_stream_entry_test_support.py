from __future__ import annotations

from collections.abc import Mapping
from typing import Any, AsyncGenerator, Callable, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.ai_gateway.ai_service import AIService

WizardStreamOwner = Callable[
    [dict[str, Any], AsyncSession, AIService],
    AsyncGenerator[str, None],
]
WorldRegenerateStreamOwner = Callable[
    [str, dict[str, Any], AsyncSession, AIService],
    AsyncGenerator[str, None],
]


def _normalize_payload_with_user_id(
    data: Mapping[str, Any] | dict[str, Any],
    *,
    request_user_id: Optional[str],
    project_id: Optional[str] = None,
) -> dict[str, Any]:
    payload = dict(data)
    user_id = request_user_id or payload.get("user_id")
    if project_id is not None:
        payload["project_id"] = project_id
    if user_id:
        payload["user_id"] = user_id
    return payload


async def create_wizard_world_building_stream(
    *,
    data: Mapping[str, Any] | dict[str, Any],
    request_user_id: Optional[str],
    db: AsyncSession,
    user_ai_service: AIService,
    world_building_stream_owner: WizardStreamOwner,
):
    payload = _normalize_payload_with_user_id(data, request_user_id=request_user_id)
    return world_building_stream_owner(payload, db, user_ai_service)


async def create_wizard_career_system_stream(
    *,
    data: Mapping[str, Any] | dict[str, Any],
    request_user_id: Optional[str],
    project_id: str,
    db: AsyncSession,
    user_ai_service: AIService,
    career_system_stream_owner: WizardStreamOwner,
):
    payload = _normalize_payload_with_user_id(
        data,
        request_user_id=request_user_id,
        project_id=project_id,
    )
    return career_system_stream_owner(payload, db, user_ai_service)


async def create_wizard_characters_stream(
    *,
    data: Mapping[str, Any] | dict[str, Any],
    request_user_id: Optional[str],
    project_id: str,
    db: AsyncSession,
    user_ai_service: AIService,
    characters_stream_owner: WizardStreamOwner,
):
    payload = _normalize_payload_with_user_id(
        data,
        request_user_id=request_user_id,
        project_id=project_id,
    )
    return characters_stream_owner(payload, db, user_ai_service)


async def create_wizard_outline_stream(
    *,
    data: Mapping[str, Any] | dict[str, Any],
    request_user_id: Optional[str],
    project_id: str,
    db: AsyncSession,
    user_ai_service: AIService,
    outline_stream_owner: WizardStreamOwner,
):
    payload = _normalize_payload_with_user_id(
        data,
        request_user_id=request_user_id,
        project_id=project_id,
    )
    return outline_stream_owner(payload, db, user_ai_service)


async def create_world_building_regenerate_stream(
    *,
    project_id: str,
    data: Mapping[str, Any] | dict[str, Any],
    request_user_id: Optional[str],
    db: AsyncSession,
    user_ai_service: AIService,
    world_building_regenerate_stream_owner: WorldRegenerateStreamOwner,
):
    payload = _normalize_payload_with_user_id(data, request_user_id=request_user_id)
    return world_building_regenerate_stream_owner(
        project_id,
        payload,
        db,
        user_ai_service,
    )
