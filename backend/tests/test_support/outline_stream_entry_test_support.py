from __future__ import annotations

from collections.abc import Mapping
from typing import Any, AsyncGenerator, Awaitable, Callable, Optional

from fastapi import HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.retired_runtime_test_support import get_logger
from migrator_app.models.outline import Outline
from tests.test_support.outline_schema_test_support import (
    BatchOutlineExpansionRequest,
    OutlineExpansionRequest,
    OutlineGenerateRequest,
)
from tests.test_support.ai_gateway.ai_service import AIService

logger = get_logger(__name__)

DumpModelLikePayload = Callable[[Any], dict[str, Any]]
PrimeStreamGenerator = Callable[
    [AsyncGenerator[str, None]],
    Awaitable[AsyncGenerator[str, None]],
]
NewOutlineStreamOwner = Callable[
    [dict[str, Any] | OutlineGenerateRequest, AsyncSession, AIService],
    AsyncGenerator[str, None],
]
ContinueOutlineStreamOwner = Callable[
    [dict[str, Any], AsyncSession, AIService, str],
    AsyncGenerator[str, None],
]
ExpandOutlineStreamOwner = Callable[
    [str, dict[str, Any], AsyncSession, AIService],
    AsyncGenerator[str, None],
]
BatchExpandOutlineStreamOwner = Callable[
    [dict[str, Any], AsyncSession, AIService],
    AsyncGenerator[str, None],
]
VerifyProjectAccess = Callable[
    [str | None, Optional[str], AsyncSession],
    Awaitable[Any],
]


async def create_outline_generate_stream(
    *,
    data: dict[str, Any] | OutlineGenerateRequest,
    request_user_id: Optional[str],
    db: AsyncSession,
    user_ai_service: AIService,
    verify_project_access_owner: VerifyProjectAccess,
    dump_model_like_payload: DumpModelLikePayload,
    prime_stream_generator: PrimeStreamGenerator,
    new_outline_stream_owner: NewOutlineStreamOwner,
    continue_outline_stream_owner: ContinueOutlineStreamOwner,
):
    normalized_data = data
    internal_user_id = None
    if isinstance(data, Mapping):
        raw_payload = dict(data)
        internal_user_id = raw_payload.pop("user_id", None)
        normalized_data = OutlineGenerateRequest.model_validate(raw_payload)
    payload = dump_model_like_payload(normalized_data)

    user_id = request_user_id or internal_user_id
    await verify_project_access_owner(payload.get("project_id"), user_id, db)
    if user_id:
        payload["user_id"] = user_id

    mode = payload.get("mode", "auto")
    existing_result = await db.execute(
        select(Outline)
        .where(Outline.project_id == payload.get("project_id"))
        .order_by(Outline.order_index)
    )
    existing_outlines = existing_result.scalars().all()

    if mode == "auto":
        mode = "continue" if existing_outlines else "new"
        logger.info("自动选择模式：%s", "续写" if existing_outlines else "新建")

    if mode == "new":
        return await prime_stream_generator(
            new_outline_stream_owner(payload, db, user_ai_service)
        )
    if mode == "continue":
        if not existing_outlines:
            raise HTTPException(status_code=400, detail="没有可用的现有大纲，无法继续生成")
        return await prime_stream_generator(
            continue_outline_stream_owner(
                payload,
                db,
                user_ai_service,
                request_user_id or "system",
            )
        )

    raise HTTPException(status_code=400, detail=f"不支持的模式: {mode}")


async def create_outline_expand_stream(
    *,
    outline_id: str,
    data: dict[str, Any] | OutlineExpansionRequest,
    request_user_id: Optional[str],
    db: AsyncSession,
    user_ai_service: AIService,
    verify_project_access_owner: VerifyProjectAccess,
    dump_model_like_payload: DumpModelLikePayload,
    prime_stream_generator: PrimeStreamGenerator,
    expand_outline_stream_owner: ExpandOutlineStreamOwner,
):
    normalized_data = data
    if isinstance(data, Mapping):
        normalized_data = OutlineExpansionRequest.model_validate(dict(data))
    payload = dump_model_like_payload(normalized_data)

    result = await db.execute(select(Outline).where(Outline.id == outline_id))
    outline = result.scalar_one_or_none()
    if not outline:
        raise HTTPException(status_code=404, detail="Outline not found")

    await verify_project_access_owner(outline.project_id, request_user_id, db)
    return await prime_stream_generator(
        expand_outline_stream_owner(outline_id, payload, db, user_ai_service)
    )


async def create_outline_batch_expand_stream(
    *,
    data: dict[str, Any] | BatchOutlineExpansionRequest,
    request_user_id: Optional[str],
    db: AsyncSession,
    user_ai_service: AIService,
    verify_project_access_owner: VerifyProjectAccess,
    dump_model_like_payload: DumpModelLikePayload,
    prime_stream_generator: PrimeStreamGenerator,
    batch_expand_outline_stream_owner: BatchExpandOutlineStreamOwner,
):
    normalized_data = data
    if isinstance(data, Mapping):
        normalized_data = BatchOutlineExpansionRequest.model_validate(dict(data))
    payload = dump_model_like_payload(normalized_data)

    await verify_project_access_owner(payload.get("project_id"), request_user_id, db)
    return await prime_stream_generator(
        batch_expand_outline_stream_owner(payload, db, user_ai_service)
    )



