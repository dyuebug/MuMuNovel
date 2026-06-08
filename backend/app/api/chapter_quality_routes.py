from __future__ import annotations

from fastapi import APIRouter, Depends, Query, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import require_authenticated_user_id
from app.api.common import verify_project_access
from app.database import get_db
from app.schemas.chapter import ProjectChapterQualityTrendResponse
from app.services.project_quality_trend_query_service import (
    load_project_quality_trend_query_context,
)
from app.services import project_quality_trend_service
from app.services.project_quality_trend_service import (
    build_project_quality_trend_response_payload,
)


router = APIRouter(prefix="/chapters", tags=["章节管理"])


@router.get(
    "/project/{project_id}/quality-trend",
    response_model=ProjectChapterQualityTrendResponse,
    summary="获取项目章节质量趋势",
)
async def get_project_chapter_quality_trend(
    project_id: str,
    request: Request,
    limit: int = Query(12, ge=1, le=50, description="Number of recent chapters to return"),
    db: AsyncSession = Depends(get_db),
):
    """获取项目最近章节的质量趋势数据。"""
    user_id = require_authenticated_user_id(request)
    await verify_project_access(project_id, user_id, db)

    query_context = await load_project_quality_trend_query_context(
        db,
        project_id=project_id,
    )
    return await build_project_quality_trend_response_payload(
        project_id=project_id,
        chapters=list(query_context.chapters),
        records_by_chapter=dict(query_context.records_by_chapter),
        limit=limit,
        resolve_snapshot_fn=project_quality_trend_service.get_project_quality_trend_snapshot_with_default_wiring,
    )
