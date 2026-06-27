from __future__ import annotations

from fastapi import APIRouter, Depends, Query, Request
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.chapter_route_helpers_test_support import (
    require_authenticated_user_id,
)
from tests.test_support.api_common_test_support import verify_project_access
from tests.test_support.database_test_support import get_db
from tests.test_support.chapter_schema_test_support import ProjectChapterQualityTrendResponse
from tests.test_support import project_quality_trend_test_support as project_quality_trend_service
from tests.test_support.project_quality_trend_test_support import (
    build_project_quality_trend_response_payload,
    load_project_quality_trend_query_context,
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

