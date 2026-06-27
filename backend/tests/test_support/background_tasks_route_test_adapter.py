from __future__ import annotations

import asyncio
import hashlib
from functools import lru_cache
import json
import time
import uuid
from datetime import datetime, timezone
from types import SimpleNamespace
from typing import TYPE_CHECKING, Any, Awaitable, Callable, Dict, Literal, Optional

from fastapi import APIRouter, Depends, HTTPException, Query, Request
from pydantic import BaseModel, Field
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from tests.test_support.ai_dependencies_test_support import read_env_defaults
from tests.test_support.api_common_test_support import verify_project_access
from tests.test_support import outlines_route_test_adapter as outlines_api
from tests.test_support.database_test_support import get_db, get_session_factory
from tests.test_support.retired_runtime_test_support import get_logger
from tests.test_support.outline_schema_test_support import (
    BatchOutlineExpansionRequest,
    OutlineExpansionRequest,
    OutlineGenerateRequest,
)
from tests.test_support.ai_gateway.ai_service import AIService, create_user_ai_service_with_mcp
from tests.test_support.background_task_manager_test_support import background_task_manager
from tests.test_support.character_organization_stream_entry_test_support import (
    create_character_generate_stream,
    create_organization_generate_stream,
)
from tests.test_support.outline_stream_entry_test_support import (
    create_outline_batch_expand_stream,
    create_outline_expand_stream,
    create_outline_generate_stream,
)
from tests.test_support.outlines_route_test_adapter import new_outline_generator as outline_generator
from tests.test_support.wizard_generation_test_support import (
    career_system_generator,
    characters_generator,
    world_building_generator,
    world_building_regenerate_generator,
)
from tests.test_support.wizard_stream_entry_test_support import (
    create_wizard_career_system_stream,
    create_wizard_characters_stream,
    create_wizard_outline_stream,
    create_wizard_world_building_stream,
    create_world_building_regenerate_stream,
)
from tests.test_support.career_generation_test_support import create_career_system_stream

if TYPE_CHECKING:
    from migrator_app.models import MCPPlugin, Settings

logger = get_logger(__name__)
router = APIRouter(prefix="/background-tasks", tags=["background-tasks"])

USER_AI_SERVICE_CONFIG_TTL_SECONDS = 30.0
_user_ai_service_config_cache: Dict[str, tuple[float, Dict[str, Any]]] = {}


@lru_cache(maxsize=1)
def _background_task_models() -> tuple[type[Any], type[Any]]:
    import migrator_app.models as models_module

    return models_module.MCPPlugin, models_module.Settings

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
    encoded = json.dumps(
        normalized_payload,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        default=str,
    )
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


class BackgroundTaskCreateRequest(BaseModel):
    task_type: TaskType
    project_id: str | None = None
    payload: Dict[str, Any] = Field(default_factory=dict)
    stage_code: str | None = Field(default=None, description="Workflow stage code, for example 1.outline")
    execution_mode: ExecutionMode = Field(default="interactive", description="Execution mode: interactive or auto")
    workflow_scope: str | None = Field(default=None, description="Workflow scope")
    checkpoint: Dict[str, Any] | None = Field(default=None, description="Workflow checkpoint payload")


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
    MCPPlugin, Settings = _background_task_models()

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
WizardTaskRunner = Callable[[str, str, Dict[str, Any], AsyncSession, AIService], Awaitable[Any]]


async def _run_careers_generate_system_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    _ = fake_request
    await verify_project_access(project_id, user_id, db)
    stream = create_career_system_stream(
        project_id=project_id,
        main_career_count=int(payload.get("main_career_count", 3)),
        sub_career_count=int(payload.get("sub_career_count", 6)),
        enable_mcp=_as_bool(payload.get("enable_mcp"), True),
        db=db,
        user_ai_service=user_ai_service,
    )
    await background_task_manager.consume_sse_stream(task_id, stream)


async def _run_character_generate_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    stream = await create_character_generate_stream(
        data={
            "project_id": project_id,
            "name": payload.get("name"),
            "role_type": payload.get("role_type", "supporting"),
            "background": payload.get("background"),
            "requirements": payload.get("requirements"),
            "enable_mcp": _as_bool(payload.get("enable_mcp"), True),
        },
        request_user_id=getattr(fake_request.state, "user_id", None),
        db=db,
        user_ai_service=user_ai_service,
    )
    await background_task_manager.consume_sse_stream(task_id, stream)


async def _run_organization_generate_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    stream = await create_organization_generate_stream(
        data={
            "project_id": project_id,
            "name": payload.get("name"),
            "organization_type": payload.get("organization_type"),
            "background": payload.get("background"),
            "requirements": payload.get("requirements"),
            "enable_mcp": _as_bool(payload.get("enable_mcp"), True),
        },
        request_user_id=getattr(fake_request.state, "user_id", None),
        db=db,
        user_ai_service=user_ai_service,
    )
    await background_task_manager.consume_sse_stream(task_id, stream)


async def _run_world_regenerate_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    stream = await create_world_building_regenerate_stream(
        project_id=project_id,
        data=payload,
        request_user_id=getattr(fake_request.state, "user_id", None),
        db=db,
        user_ai_service=user_ai_service,
        world_building_regenerate_stream_owner=world_building_regenerate_generator,
    )
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
    outline_payload_raw = _strip_internal_payload_fields(payload)
    outline_request = OutlineGenerateRequest.model_validate({
        **outline_payload_raw,
        "project_id": project_id,
    })
    outline_payload = outline_request.model_dump(exclude_none=False)
    stream = await create_outline_generate_stream(
        data=outline_payload,
        request_user_id=getattr(fake_request.state, "user_id", None),
        db=db,
        user_ai_service=user_ai_service,
        verify_project_access_owner=outlines_api.verify_project_access,
        dump_model_like_payload=outlines_api._dump_model_like_payload,
        prime_stream_generator=outlines_api._prime_stream_generator,
        new_outline_stream_owner=outlines_api.new_outline_generator,
        continue_outline_stream_owner=outlines_api.continue_outline_generator,
    )
    await background_task_manager.consume_sse_stream(task_id, stream)


async def _run_outline_expand_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    outline_id = str(payload.get("outline_id", "")).strip()
    if not outline_id:
        raise RuntimeError("outline_id is required for outline_expand")

    expand_payload_raw = _strip_internal_payload_fields(payload, "outline_id")
    expand_request = OutlineExpansionRequest.model_validate(expand_payload_raw)
    expand_payload = expand_request.model_dump(exclude_none=False)
    stream = await create_outline_expand_stream(
        outline_id=outline_id,
        data=expand_payload,
        request_user_id=getattr(fake_request.state, "user_id", None),
        db=db,
        user_ai_service=user_ai_service,
        verify_project_access_owner=outlines_api.verify_project_access,
        dump_model_like_payload=outlines_api._dump_model_like_payload,
        prime_stream_generator=outlines_api._prime_stream_generator,
        expand_outline_stream_owner=outlines_api.expand_outline_generator,
    )
    await background_task_manager.consume_sse_stream(task_id, stream)


async def _run_outline_batch_expand_task(
    task_id: str,
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
    fake_request: Any,
) -> None:
    batch_payload_raw = _strip_internal_payload_fields(payload)
    batch_request = BatchOutlineExpansionRequest.model_validate({
        **batch_payload_raw,
        "project_id": project_id,
    })
    batch_payload = batch_request.model_dump(exclude_none=False)
    stream = await create_outline_batch_expand_stream(
        data=batch_payload,
        request_user_id=getattr(fake_request.state, "user_id", None),
        db=db,
        user_ai_service=user_ai_service,
        verify_project_access_owner=outlines_api.verify_project_access,
        dump_model_like_payload=outlines_api._dump_model_like_payload,
        prime_stream_generator=outlines_api._prime_stream_generator,
        batch_expand_outline_stream_owner=outlines_api.batch_expand_outlines_generator,
    )
    await background_task_manager.consume_sse_stream(task_id, stream)


async def _run_wizard_world_building_task(
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> Any:
    return await create_wizard_world_building_stream(
        data=payload,
        request_user_id=user_id,
        db=db,
        user_ai_service=user_ai_service,
        world_building_stream_owner=world_building_generator,
    )


async def _run_wizard_career_system_task(
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> Any:
    return await create_wizard_career_system_stream(
        data=payload,
        request_user_id=user_id,
        project_id=project_id,
        db=db,
        user_ai_service=user_ai_service,
        career_system_stream_owner=career_system_generator,
    )


async def _run_wizard_characters_task(
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> Any:
    return await create_wizard_characters_stream(
        data=payload,
        request_user_id=user_id,
        project_id=project_id,
        db=db,
        user_ai_service=user_ai_service,
        characters_stream_owner=characters_generator,
    )


async def _run_wizard_outline_task(
    user_id: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> Any:
    return await create_wizard_outline_stream(
        data=payload,
        request_user_id=user_id,
        project_id=project_id,
        db=db,
        user_ai_service=user_ai_service,
        outline_stream_owner=outline_generator,
    )


TASK_RUNNERS: Dict[str, TaskRunner] = {
    "careers_generate_system": _run_careers_generate_system_task,
    "character_generate": _run_character_generate_task,
    "organization_generate": _run_organization_generate_task,
    "world_regenerate": _run_world_regenerate_task,
    "outline_generate": _run_outline_generate_task,
    "outline_expand": _run_outline_expand_task,
    "outline_batch_expand": _run_outline_batch_expand_task,
}

WIZARD_TASK_RUNNERS: Dict[str, WizardTaskRunner] = {
    "wizard_world_building": _run_wizard_world_building_task,
    "wizard_career_system": _run_wizard_career_system_task,
    "wizard_characters": _run_wizard_characters_task,
    "wizard_outline": _run_wizard_outline_task,
}


async def _run_wizard_background_task(
    task_id: str,
    user_id: str,
    task_type: str,
    project_id: str,
    payload: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> bool:
    runner = WIZARD_TASK_RUNNERS.get(task_type)
    if runner is None:
        return False

    stream = await runner(user_id, project_id, payload, db, user_ai_service)
    await background_task_manager.consume_sse_stream(task_id, stream)
    return True


async def _run_generation_task(
    task_id: str,
    user_id: str,
    task_type: str,
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

        if await _run_wizard_background_task(
            task_id=task_id,
            user_id=user_id,
            task_type=task_type,
            project_id=project_id,
            payload=payload,
            db=db,
            user_ai_service=user_ai_service,
        ):
            return

        raise RuntimeError(f"Unsupported background task type: {task_type}")


def _serialize_task_record(record: Any) -> Dict[str, Any]:
    return {
        "task_id": record.task_id,
        "task_type": record.task_type,
        "project_id": getattr(record, "project_id", None),
        "status": getattr(record, "status", None),
        "progress": getattr(record, "progress", None),
        "message": getattr(record, "message", None),
        "created_at": getattr(record, "created_at", None),
        "updated_at": getattr(record, "updated_at", None),
        "execution_mode": getattr(record, "execution_mode", None),
        "stage_code": getattr(record, "stage_code", None),
        "workflow_scope": getattr(record, "workflow_scope", None),
        "checkpoint": getattr(record, "checkpoint", None),
        "error": getattr(record, "error", None),
    }


def _build_missing_task_payload(task_id: str) -> Dict[str, Any]:
    return {
        "task_id": task_id,
        "task_type": None,
        "project_id": None,
        "status": "cancelled",
        "progress": 100,
        "message": "任务不存在",
        "created_at": None,
        "updated_at": None,
        "execution_mode": None,
        "stage_code": None,
        "workflow_scope": None,
        "checkpoint": None,
        "error": "task_missing",
    }


@router.get("", summary="List background tasks")
async def list_background_tasks(
    request: Request,
    project_id: Optional[str] = Query(default=None),
    status: Optional[str] = Query(default=None),
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    if status is not None and status not in TASK_STATUSES:
        raise HTTPException(status_code=400, detail="invalid status")

    tasks = await background_task_manager.list_tasks(
        user_id=user_id,
        project_id=project_id,
        status=status,
    )
    return [_serialize_task_record(task) for task in tasks]


@router.get("/{task_id}", summary="Get background task status")
async def get_background_task_status(task_id: str, request: Request):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    task = await background_task_manager.get_task(task_id, user_id)
    if task is None:
        return _build_missing_task_payload(task_id)
    return _serialize_task_record(task)


@router.post("/{task_id}/cancel", summary="Cancel background task")
async def cancel_background_task(task_id: str, request: Request):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    task = await background_task_manager.cancel_task(task_id, user_id)
    if task is None:
        return _build_missing_task_payload(task_id)
    return _serialize_task_record(task)


@router.patch("/{task_id}/workflow-state", summary="Update workflow state")
async def update_background_task_workflow_state(
    task_id: str,
    body: BackgroundTaskWorkflowStateUpdateRequest,
    request: Request,
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    if body.execution_mode is not None and body.execution_mode not in EXECUTION_MODES:
        raise HTTPException(status_code=400, detail="invalid execution_mode")

    task = await background_task_manager.update_task_workflow_state(
        task_id=task_id,
        user_id=user_id,
        stage_code=body.stage_code,
        execution_mode=body.execution_mode,
        workflow_scope=body.workflow_scope,
        checkpoint=body.checkpoint,
        message=body.message,
        progress=body.progress,
    )
    if task is None:
        return _build_missing_task_payload(task_id)
    return _serialize_task_record(task)


@router.post("", summary="Create background task")
async def create_background_task(
    data: BackgroundTaskCreateRequest,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    project_id = (data.project_id or "").strip()
    payload, payload_stage_code, payload_workflow_scope, payload_checkpoint = (
        _extract_workflow_payload(data.payload)
    )
    stage_code = data.stage_code or payload_stage_code or TASK_STAGE_DEFAULTS.get(data.task_type)
    workflow_scope = data.workflow_scope or payload_workflow_scope
    checkpoint = data.checkpoint if data.checkpoint is not None else payload_checkpoint

    if data.task_type != "wizard_world_building":
        if not project_id:
            raise HTTPException(status_code=400, detail="project_id is required")
        await verify_project_access(project_id, user_id, db)

    payload_fingerprint = _build_task_dedupe_fingerprint(data.task_type, project_id, payload)
    existing_task = await background_task_manager.find_active_task(
        user_id=user_id,
        task_type=data.task_type,
        project_id=project_id,
        payload_fingerprint=payload_fingerprint,
    )
    if existing_task is not None:
        return _serialize_task_record(existing_task)

    task_id = str(uuid.uuid4())
    created_task = await background_task_manager.create_task(
        task_id=task_id,
        user_id=user_id,
        task_type=data.task_type,
        project_id=project_id,
        message="后台任务已创建",
        progress=0,
        status="pending",
        created_at=datetime.now(timezone.utc),
        updated_at=datetime.now(timezone.utc),
        stage_code=stage_code,
        execution_mode=data.execution_mode,
        workflow_scope=workflow_scope,
        checkpoint=checkpoint,
        payload_fingerprint=payload_fingerprint,
    )

    async def job():
        await _run_generation_task(
            task_id=task_id,
            user_id=user_id,
            task_type=data.task_type,
            project_id=project_id,
            payload=payload,
        )

    asyncio.create_task(_start_background_task_runner(task_id, job()))
    return _serialize_task_record(created_task)



