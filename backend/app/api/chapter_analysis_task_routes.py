"""章节分析任务相关 API。"""

from __future__ import annotations

from typing import List

from fastapi import APIRouter, BackgroundTasks, Depends, Request
from pydantic import BaseModel
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.settings import get_user_ai_service
from app.database import get_db
from app.services.ai_service import AIService
from app.services.chapter_analysis_task_route_compat_service import (
    check_can_generate_with_default_route_wiring,
    get_analysis_task_status_with_default_route_wiring,
    get_batch_analysis_task_status_with_default_route_wiring,
    trigger_chapter_analysis_with_default_route_wiring,
)

router = APIRouter(prefix="/chapters", tags=["章节管理"])


class BatchAnalysisStatusRequest(BaseModel):
    chapter_ids: List[str]


@router.get("/{chapter_id}/analysis/status", summary="查询章节分析任务状态")
async def get_analysis_task_status(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """查询指定章节的最新分析任务状态。"""
    return await get_analysis_task_status_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        db_session=db,
    )


@router.post("/analysis/status/batch", summary="批量查询章节分析任务状态")
async def get_batch_analysis_task_status(
    data: BatchAnalysisStatusRequest,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    return await get_batch_analysis_task_status_with_default_route_wiring(
        chapter_ids_input=data.chapter_ids,
        request=request,
        db_session=db,
    )


@router.get("/{chapter_id}/can-generate", summary="检查章节是否可以生成")
async def check_can_generate(
    chapter_id: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """检查章节当前是否满足生成前置条件。"""
    return await check_can_generate_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        db_session=db,
    )


@router.post("/{chapter_id}/analyze", summary="手动触发章节分析")
async def trigger_chapter_analysis(
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """手动触发章节分析，并异步创建后台分析任务。"""
    return await trigger_chapter_analysis_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        db_session=db,
        user_ai_service=user_ai_service,
    )
