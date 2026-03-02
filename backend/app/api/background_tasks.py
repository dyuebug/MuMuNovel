"""Background generation task APIs."""
from __future__ import annotations

import asyncio
import json
import uuid
from types import SimpleNamespace
from typing import Any, Dict, Literal

from fastapi import APIRouter, Depends, HTTPException, Request
from pydantic import BaseModel, Field
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from app.api.common import verify_project_access
from app.api.settings import read_env_defaults
from app.database import get_db, get_engine
from app.logger import get_logger
from app.models.mcp_plugin import MCPPlugin
from app.models.settings import Settings
from app.services.ai_service import AIService, create_user_ai_service_with_mcp
from app.services.background_task_manager import background_task_manager

logger = get_logger(__name__)
router = APIRouter(prefix="/background-tasks", tags=["后台任务"])


TaskType = Literal[
    "careers_generate_system",
    "character_generate",
    "organization_generate",
    "world_regenerate",
    "outline_generate",
    "outline_expand",
    "outline_batch_expand",
    "wizard_world_building",
    "wizard_career_system",
    "wizard_characters",
    "wizard_outline",
]


class BackgroundTaskCreateRequest(BaseModel):
    task_type: TaskType
    project_id: str | None = None
    payload: Dict[str, Any] = Field(default_factory=dict)


def _as_bool(value: Any, default: bool) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        return value.strip().lower() in {"1", "true", "yes", "on"}
    return default


async def _build_user_ai_service(user_id: str, db: AsyncSession) -> AIService:
    result = await db.execute(select(Settings).where(Settings.user_id == user_id))
    user_settings = result.scalar_one_or_none()

    if not user_settings:
        defaults = read_env_defaults()
        user_settings = Settings(user_id=user_id, **defaults)
        db.add(user_settings)
        await db.commit()
        await db.refresh(user_settings)

    mcp_result = await db.execute(select(MCPPlugin).where(MCPPlugin.user_id == user_id))
    plugins = mcp_result.scalars().all()
    enable_mcp = any(plugin.enabled for plugin in plugins) if plugins else False

    backup_urls = None
    if user_settings.api_backup_urls:
        try:
            backup_urls = (
                json.loads(user_settings.api_backup_urls)
                if isinstance(user_settings.api_backup_urls, str)
                else user_settings.api_backup_urls
            )
        except (TypeError, json.JSONDecodeError):
            backup_urls = None

    return create_user_ai_service_with_mcp(
        api_provider=user_settings.api_provider,
        api_key=user_settings.api_key,
        api_base_url=user_settings.api_base_url or "",
        model_name=user_settings.llm_model,
        temperature=user_settings.temperature,
        max_tokens=user_settings.max_tokens,
        user_id=user_id,
        db_session=db,
        system_prompt=user_settings.system_prompt,
        enable_mcp=enable_mcp,
        backup_urls=backup_urls,
        fallback_strategy=user_settings.fallback_strategy,
    )


def _build_fake_request(user_id: str) -> SimpleNamespace:
    state = SimpleNamespace(user_id=user_id, user=SimpleNamespace(user_id=user_id))
    return SimpleNamespace(state=state)


async def _run_generation_task(
    task_id: str,
    user_id: str,
    task_type: TaskType,
    project_id: str,
    payload: Dict[str, Any],
) -> None:
    engine = await get_engine(user_id)
    session_factory = async_sessionmaker(engine, class_=AsyncSession, expire_on_commit=False)

    async with session_factory() as db:
        user_ai_service = await _build_user_ai_service(user_id, db)
        fake_request = _build_fake_request(user_id)

        if task_type == "careers_generate_system":
            from app.api.careers import generate_career_system

            response = await generate_career_system(
                project_id=project_id,
                main_career_count=int(payload.get("main_career_count", 3)),
                sub_career_count=int(payload.get("sub_career_count", 6)),
                enable_mcp=_as_bool(payload.get("enable_mcp"), False),
                http_request=fake_request,  # type: ignore[arg-type]
                db=db,
                user_ai_service=user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("careers stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "character_generate":
            from app.api.characters import CharacterGenerateRequest, generate_character_stream

            character_request = CharacterGenerateRequest(
                project_id=project_id,
                name=payload.get("name"),
                role_type=payload.get("role_type", "supporting"),
                background=payload.get("background"),
                requirements=payload.get("requirements"),
                enable_mcp=_as_bool(payload.get("enable_mcp"), True),
            )
            response = await generate_character_stream(
                character_request,
                fake_request,  # type: ignore[arg-type]
                db,
                user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("character stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "organization_generate":
            from app.api.organizations import OrganizationGenerateRequest, generate_organization_stream

            organization_request = OrganizationGenerateRequest(
                project_id=project_id,
                name=payload.get("name"),
                organization_type=payload.get("organization_type"),
                background=payload.get("background"),
                requirements=payload.get("requirements"),
                enable_mcp=_as_bool(payload.get("enable_mcp"), True),
            )
            response = await generate_organization_stream(
                organization_request,
                fake_request,  # type: ignore[arg-type]
                db,
                user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("organization stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "world_regenerate":
            from app.api.wizard_stream import world_building_regenerate_generator

            world_payload = dict(payload)
            world_payload["user_id"] = user_id
            stream = world_building_regenerate_generator(project_id, world_payload, db, user_ai_service)
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "outline_generate":
            from app.api.outlines import generate_outline_stream

            outline_payload = dict(payload)
            outline_payload["project_id"] = project_id
            response = await generate_outline_stream(
                data=outline_payload,
                request=fake_request,  # type: ignore[arg-type]
                db=db,
                user_ai_service=user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("outline generate stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "outline_expand":
            from app.api.outlines import expand_outline_to_chapters_stream

            outline_id = str(payload.get("outline_id", "")).strip()
            if not outline_id:
                raise RuntimeError("outline_id is required for outline_expand")

            expand_payload = {k: v for k, v in payload.items() if k != "outline_id"}
            response = await expand_outline_to_chapters_stream(
                outline_id=outline_id,
                data=expand_payload,
                request=fake_request,  # type: ignore[arg-type]
                db=db,
                user_ai_service=user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("outline expand stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "outline_batch_expand":
            from app.api.outlines import batch_expand_outlines_stream

            batch_payload = dict(payload)
            batch_payload["project_id"] = project_id
            response = await batch_expand_outlines_stream(
                data=batch_payload,
                request=fake_request,  # type: ignore[arg-type]
                db=db,
                user_ai_service=user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("outline batch expand stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "wizard_world_building":
            from app.api.wizard_stream import generate_world_building_stream

            world_payload = dict(payload)
            response = await generate_world_building_stream(
                request=fake_request,  # type: ignore[arg-type]
                data=world_payload,
                db=db,
                user_ai_service=user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("wizard world-building stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "wizard_career_system":
            from app.api.wizard_stream import generate_career_system_stream

            career_payload = dict(payload)
            career_payload["project_id"] = project_id
            response = await generate_career_system_stream(
                request=fake_request,  # type: ignore[arg-type]
                data=career_payload,
                db=db,
                user_ai_service=user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("wizard career-system stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "wizard_characters":
            from app.api.wizard_stream import generate_characters_stream

            characters_payload = dict(payload)
            characters_payload["project_id"] = project_id
            response = await generate_characters_stream(
                request=fake_request,  # type: ignore[arg-type]
                data=characters_payload,
                db=db,
                user_ai_service=user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("wizard characters stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        if task_type == "wizard_outline":
            from app.api.wizard_stream import generate_outline_stream

            outline_payload = dict(payload)
            outline_payload["project_id"] = project_id
            outline_payload["user_id"] = user_id
            response = await generate_outline_stream(
                data=outline_payload,
                db=db,
                user_ai_service=user_ai_service,
            )
            stream = getattr(response, "body_iterator", None)
            if stream is None:
                raise RuntimeError("wizard outline stream not available")
            await background_task_manager.consume_sse_stream(task_id, stream)
            return

        raise RuntimeError(f"unsupported task type: {task_type}")


@router.post("", summary="创建后台生成任务")
async def create_background_task(
    data: BackgroundTaskCreateRequest,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    if data.task_type != "wizard_world_building":
        if not data.project_id:
            raise HTTPException(status_code=400, detail="project_id is required for this task type")
        await verify_project_access(data.project_id, user_id, db)

    task_id = str(uuid.uuid4())
    task_project_id = data.project_id or ""
    record = await background_task_manager.create_task(
        task_id=task_id,
        task_type=data.task_type,
        user_id=user_id,
        project_id=task_project_id,
        message="后台任务已创建",
    )

    job = _run_generation_task(
        task_id=task_id,
        user_id=user_id,
        task_type=data.task_type,
        project_id=task_project_id,
        payload=data.payload,
    )
    runner_task = asyncio.create_task(background_task_manager.run_job(task_id, job))
    await background_task_manager.attach_runner(task_id, runner_task)

    logger.info(f"Created background task: user={user_id}, task={task_id}, type={data.task_type}")
    return record.to_dict()


@router.get("/{task_id}", summary="查询后台任务状态")
async def get_background_task_status(task_id: str, request: Request):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    record = await background_task_manager.get_task(task_id, user_id)
    if not record:
        raise HTTPException(status_code=404, detail="任务不存在")

    return record.to_dict()


@router.post("/{task_id}/cancel", summary="取消后台任务")
async def cancel_background_task(task_id: str, request: Request):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    record = await background_task_manager.cancel_task(task_id, user_id)
    if not record:
        raise HTTPException(status_code=404, detail="任务不存在")

    return record.to_dict()
