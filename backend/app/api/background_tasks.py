"""Background generation task APIs."""
from __future__ import annotations

import asyncio
import hashlib
import json
import time
import uuid
from datetime import datetime, timezone
from types import SimpleNamespace
from typing import Any, Awaitable, Callable, Dict, Literal, Optional

from fastapi import APIRouter, Depends, HTTPException, Query, Request
from pydantic import BaseModel, Field
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from app.api.common import verify_project_access
from app.api.settings import read_env_defaults
from app.database import get_db, get_session_factory
from app.logger import get_logger
from app.schemas.outline import BatchOutlineExpansionRequest, OutlineExpansionRequest, OutlineGenerateRequest
from app.models.mcp_plugin import MCPPlugin
from app.models.settings import Settings
from app.services.ai_service import AIService, create_user_ai_service_with_mcp
from app.services.background_task_manager import background_task_manager
from app.services.background_task_wizard_executor import run_wizard_background_task

logger = get_logger(__name__)
router = APIRouter(prefix="/background-tasks", tags=["后台任务"])

USER_AI_SERVICE_CONFIG_TTL_SECONDS = 30.0
_user_ai_service_config_cache: Dict[str, tuple[float, Dict[str, Any]]] = {}


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
ExecutionMode = Literal["interactive", "auto"]

TASK_STATUSES = {"pending", "running", "completed", "failed", "cancelled"}
EXECUTION_MODES = {"interactive", "auto"}
TASK_STAGE_DEFAULTS: Dict[str, str] = {
    "wizard_world_building": "0.creative",
    "wizard_characters": "1.outline",
    "wizard_outline": "1.outline",
    "wizard_career_system": "1.outline",
    "world_regenerate": "0.creative",
    "outline_generate": "1.outline",
    "outline_expand": "4.group",
    "outline_batch_expand": "4.group",
    "careers_generate_system": "1.outline",
    "character_generate": "1.outline",
    "organization_generate": "1.outline",
}
DEDUPABLE_TASK_TYPES = {
    "wizard_world_building",
    "wizard_career_system",
    "wizard_characters",
    "wizard_outline",
    "world_regenerate",
    "outline_generate",
    "outline_expand",
    "outline_batch_expand",
}


def _build_task_dedupe_fingerprint(
    task_type: TaskType,
    project_id: str,
    payload: Dict[str, Any],
) -> Optional[str]:
    if task_type not in DEDUPABLE_TASK_TYPES:
        return None

    normalized_payload = {
        "task_type": task_type,
        "project_id": project_id,
        "payload": payload,
    }
    encoded = json.dumps(normalized_payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"), default=str)
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


class BackgroundTaskCreateRequest(BaseModel):
    task_type: TaskType
    project_id: str | None = None
    payload: Dict[str, Any] = Field(default_factory=dict)
    stage_code: str | None = Field(default=None, description="当前工作流阶段编码，如 1.outline")
    execution_mode: ExecutionMode = Field(default="interactive", description="执行模式：interactive/auto")
    workflow_scope: str | None = Field(default=None, description="可选的执行范围说明")
    checkpoint: Dict[str, Any] | None = Field(default=None, description="可选的阶段检查点快照")


class BackgroundTaskWorkflowStateUpdateRequest(BaseModel):
    stage_code: str | None = None
    execution_mode: ExecutionMode | None = None
    workflow_scope: str | None = None
    checkpoint: Dict[str, Any] | None = None
    message: str | None = None
    progress: int | None = Field(default=None, ge=0, le=100)


def _as_bool(value: Any, default: bool) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        return value.strip().lower() in {"1", "true", "yes", "on"}
    return default


def _extract_workflow_payload(
    payload: Dict[str, Any],
) -> tuple[Dict[str, Any], Optional[str], Optional[str], Optional[Dict[str, Any]]]:
    """Extract workflow-only metadata from payload and return cleaned business payload."""
    clean_payload = dict(payload or {})
    stage_code = clean_payload.pop("__stage_code", None)
    workflow_scope = clean_payload.pop("__workflow_scope", None)
    checkpoint = clean_payload.pop("__checkpoint", None)
    if checkpoint is not None and not isinstance(checkpoint, dict):
        checkpoint = None
    return clean_payload, stage_code, workflow_scope, checkpoint


def _strip_internal_payload_fields(payload: Dict[str, Any], *extra_fields: str) -> Dict[str, Any]:
    clean_payload = dict(payload or {})
    for field_name in ("user_id", *extra_fields):
        clean_payload.pop(field_name, None)
    return clean_payload


async def _build_user_ai_service(user_id: str, db: AsyncSession) -> AIService:
    cache_deadline, cached_config = _user_ai_service_config_cache.get(user_id, (0.0, {}))
    if cache_deadline > time.monotonic() and cached_config:
        return create_user_ai_service_with_mcp(db_session=db, **cached_config)

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

    service_config = {
        "api_provider": user_settings.api_provider,
        "api_key": user_settings.api_key,
        "api_base_url": user_settings.api_base_url or "",
        "model_name": user_settings.llm_model,
        "temperature": user_settings.temperature,
        "max_tokens": user_settings.max_tokens,
        "user_id": user_id,
        "system_prompt": user_settings.system_prompt,
        "enable_mcp": enable_mcp,
        "backup_urls": list(backup_urls) if isinstance(backup_urls, list) else backup_urls,
        "fallback_strategy": user_settings.fallback_strategy,
    }
    _user_ai_service_config_cache[user_id] = (
        time.monotonic() + USER_AI_SERVICE_CONFIG_TTL_SECONDS,
        service_config,
    )

    return create_user_ai_service_with_mcp(db_session=db, **service_config)


def _build_fake_request(user_id: str) -> SimpleNamespace:
    state = SimpleNamespace(user_id=user_id, user=SimpleNamespace(user_id=user_id))
    return SimpleNamespace(state=state)


async def _consume_response_stream(task_id: str, response: Any, error_message: str) -> None:
    stream = getattr(response, "body_iterator", None)
    if stream is None:
        raise RuntimeError(error_message)
    await background_task_manager.consume_sse_stream(task_id, stream)


async def _start_background_task_runner(task_id: str, job: Awaitable[None]) -> None:
    await asyncio.sleep(0)
    runner_task = asyncio.current_task()
    if runner_task is not None:
        await background_task_manager.attach_runner(task_id, runner_task)
        cancelling = getattr(runner_task, "cancelling", None)
        if callable(cancelling) and cancelling():
            close = getattr(job, "close", None)
            if callable(close):
                close()
            return
    await background_task_manager.run_job(task_id, job)


TaskRunner = Callable[[str, str, str, Dict[str, Any], AsyncSession, AIService, Any], Awaitable[None]]


async def _run_careers_generate_system_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    from app.api.careers import generate_career_system

    response = await generate_career_system(
        project_id=project_id,
        main_career_count=int(payload.get("main_career_count", 3)),
        sub_career_count=int(payload.get("sub_career_count", 6)),
        enable_mcp=_as_bool(payload.get("enable_mcp"), True),
        http_request=fake_request,  # type: ignore[arg-type]
        db=db,
        user_ai_service=user_ai_service,
    )
    await _consume_response_stream(task_id, response, "careers stream not available")


async def _run_character_generate_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
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
    await _consume_response_stream(task_id, response, "character stream not available")


async def _run_organization_generate_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
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
    await _consume_response_stream(task_id, response, "organization stream not available")


async def _run_world_regenerate_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    from app.api.wizard_stream import world_building_regenerate_generator

    world_payload = dict(payload)
    world_payload["user_id"] = user_id
    stream = world_building_regenerate_generator(project_id, world_payload, db, user_ai_service)
    await background_task_manager.consume_sse_stream(task_id, stream)


async def _run_outline_generate_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    from app.api.outlines import generate_outline_stream

    outline_payload_raw = _strip_internal_payload_fields(payload)
    outline_request = OutlineGenerateRequest.model_validate({
        **outline_payload_raw,
        "project_id": project_id,
    })
    outline_payload = outline_request.model_dump(exclude_none=False)
    response = await generate_outline_stream(
        data=outline_payload,
        request=fake_request,  # type: ignore[arg-type]
        db=db,
        user_ai_service=user_ai_service,
    )
    await _consume_response_stream(task_id, response, "outline generate stream not available")


async def _run_outline_expand_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    from app.api.outlines import expand_outline_to_chapters_stream

    outline_id = str(payload.get("outline_id", "")).strip()
    if not outline_id:
        raise RuntimeError("outline_id is required for outline_expand")

    expand_payload_raw = _strip_internal_payload_fields(payload, "outline_id")
    expand_request = OutlineExpansionRequest.model_validate(expand_payload_raw)
    expand_payload = expand_request.model_dump(exclude_none=False)
    response = await expand_outline_to_chapters_stream(
        outline_id=outline_id,
        data=expand_payload,
        request=fake_request,  # type: ignore[arg-type]
        db=db,
        user_ai_service=user_ai_service,
    )
    await _consume_response_stream(task_id, response, "outline expand stream not available")


async def _run_outline_batch_expand_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    from app.api.outlines import batch_expand_outlines_stream

    batch_payload_raw = _strip_internal_payload_fields(payload)
    batch_request = BatchOutlineExpansionRequest.model_validate({
        **batch_payload_raw,
        "project_id": project_id,
    })
    batch_payload = batch_request.model_dump(exclude_none=False)
    response = await batch_expand_outlines_stream(
        data=batch_payload,
        request=fake_request,  # type: ignore[arg-type]
        db=db,
        user_ai_service=user_ai_service,
    )
    await _consume_response_stream(task_id, response, "outline batch expand stream not available")


TASK_RUNNERS: Dict[str, TaskRunner] = {
    "careers_generate_system": _run_careers_generate_system_task,
    "character_generate": _run_character_generate_task,
    "organization_generate": _run_organization_generate_task,
    "world_regenerate": _run_world_regenerate_task,
    "outline_generate": _run_outline_generate_task,
    "outline_expand": _run_outline_expand_task,
    "outline_batch_expand": _run_outline_batch_expand_task,
}


async def _run_generation_task(
    task_id: str,
    user_id: str,
    task_type: TaskType,
    project_id: str,
    payload: Dict[str, Any],
) -> None:
    session_factory = await get_session_factory(user_id)

    async with session_factory() as db:
        user_ai_service = await _build_user_ai_service(user_id, db)
        fake_request = _build_fake_request(user_id)

        runner = TASK_RUNNERS.get(task_type)
        if runner is not None:
            await runner(task_id, user_id, project_id, payload, db, user_ai_service, fake_request)
            return

        if await run_wizard_background_task(
            task_id=task_id,
            user_id=user_id,
            task_type=task_type,
            project_id=project_id,
            payload=payload,
            db=db,
            user_ai_service=user_ai_service,
        ):
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

    clean_payload, payload_stage_code, payload_scope, payload_checkpoint = _extract_workflow_payload(data.payload)
    stage_code = (data.stage_code or payload_stage_code or TASK_STAGE_DEFAULTS.get(data.task_type))
    execution_mode = (data.execution_mode or "interactive").lower()
    if execution_mode not in EXECUTION_MODES:
        raise HTTPException(status_code=400, detail="非法执行模式")
    workflow_scope = data.workflow_scope or payload_scope
    checkpoint = data.checkpoint if isinstance(data.checkpoint, dict) else payload_checkpoint

    task_project_id = data.project_id or ""
    dedupe_fingerprint = _build_task_dedupe_fingerprint(data.task_type, task_project_id, clean_payload)
    existing_task = await background_task_manager.find_active_task(
        user_id=user_id,
        task_type=data.task_type,
        project_id=task_project_id,
        payload_fingerprint=dedupe_fingerprint,
    )
    if existing_task is not None:
        logger.info(
            "Reusing existing active background task: user=%s task=%s type=%s",
            user_id,
            existing_task.task_id,
            data.task_type,
        )
        return existing_task.to_dict()

    task_id = str(uuid.uuid4())
    record = await background_task_manager.create_task(
        task_id=task_id,
        task_type=data.task_type,
        user_id=user_id,
        project_id=task_project_id,
        message="后台任务已创建",
        stage_code=stage_code,
        execution_mode=execution_mode,
        workflow_scope=workflow_scope,
        checkpoint=checkpoint,
        payload_fingerprint=dedupe_fingerprint,
    )

    job = _run_generation_task(
        task_id=task_id,
        user_id=user_id,
        task_type=data.task_type,
        project_id=task_project_id,
        payload=clean_payload,
    )
    asyncio.create_task(_start_background_task_runner(task_id, job))

    logger.info(f"Created background task: user={user_id}, task={task_id}, type={data.task_type}")
    return record.to_dict()


@router.get("", summary="查询后台任务列表")
async def list_background_tasks(
    request: Request,
    project_id: str | None = Query(default=None, description="按项目ID过滤"),
    statuses: str | None = Query(default=None, description="按状态过滤，逗号分隔"),
    active_only: bool = Query(default=False, description="仅返回进行中任务"),
    limit: int = Query(default=20, ge=1, le=100, description="返回数量上限"),
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    status_filters: set[str] = set()
    if statuses:
        raw_statuses = [item.strip().lower() for item in statuses.split(",")]
        status_filters.update(item for item in raw_statuses if item)
        invalid = sorted(item for item in status_filters if item not in TASK_STATUSES)
        if invalid:
            raise HTTPException(
                status_code=400,
                detail=f"非法任务状态: {', '.join(invalid)}",
            )
    if active_only:
        status_filters = status_filters.intersection({"pending", "running"}) if status_filters else {"pending", "running"}

    records = await background_task_manager.list_tasks(
        user_id=user_id,
        project_id=project_id,
        statuses=status_filters or None,
        limit=limit,
    )
    items = [record.to_dict() for record in records]
    return {
        "total": len(items),
        "items": items,
    }


@router.get("/{task_id}", summary="查询后台任务状态")
async def get_background_task_status(task_id: str, request: Request):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    record = await background_task_manager.get_task(task_id, user_id)
    if not record:
        now = datetime.now(timezone.utc).isoformat()
        logger.warning(f"Background task missing: user={user_id}, task={task_id}")
        return {
            "task_id": task_id,
            "task_type": None,
            "project_id": None,
            "status": "cancelled",
            "progress": 100,
            "message": "任务不存在",
            "error": "task_missing",
            "stage_code": None,
            "execution_mode": None,
            "workflow_scope": None,
            "checkpoint": None,
            "created_at": None,
            "updated_at": now,
            "started_at": None,
            "completed_at": now,
        }

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


@router.patch("/{task_id}/workflow-state", summary="更新后台任务工作流状态")
async def update_background_task_workflow_state(
    task_id: str,
    data: BackgroundTaskWorkflowStateUpdateRequest,
    request: Request,
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    record = await background_task_manager.update_workflow_state(
        task_id=task_id,
        user_id=user_id,
        stage_code=data.stage_code,
        execution_mode=data.execution_mode,
        workflow_scope=data.workflow_scope,
        checkpoint=data.checkpoint,
        message=data.message,
        progress=data.progress,
    )
    if not record:
        raise HTTPException(status_code=404, detail="任务不存在")
    return record.to_dict()
