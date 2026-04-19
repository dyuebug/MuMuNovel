"""章节分析 API。"""

from __future__ import annotations

from fastapi import APIRouter, Depends, HTTPException, Query, Request
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.database import get_db
from app.models.generation_history import GenerationHistory
from app.models.memory import PlotAnalysis, StoryMemory
from app.services.chapter_analysis_response_service import build_chapter_analysis_payload
from app.services.chapter_generation_history_service import _load_latest_candidate_draft_attempt

router = APIRouter(prefix="/chapters", tags=["章节管理"])


@router.get("/{chapter_id}/analysis", summary="获取章节分析")
async def get_chapter_analysis(
    chapter_id: str,
    request: Request,
    include_full_draft: bool = Query(False, description="是否包含完整草稿"),
    db: AsyncSession = Depends(get_db),
):
    """获取指定章节的分析结果。"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    analysis_result = await db.execute(
        select(PlotAnalysis)
        .where(PlotAnalysis.chapter_id == chapter_id)
        .order_by(PlotAnalysis.created_at.desc())
        .limit(1)
    )
    analysis = analysis_result.scalar_one_or_none()
    if not analysis:
        raise HTTPException(status_code=404, detail="未找到章节分析结果")

    memories_result = await db.execute(
        select(StoryMemory)
        .where(StoryMemory.chapter_id == chapter_id)
        .order_by(StoryMemory.importance_score.desc())
    )
    memories = memories_result.scalars().all()

    history_result = await db.execute(
        select(GenerationHistory)
        .where(GenerationHistory.chapter_id == chapter_id)
        .order_by(GenerationHistory.created_at.desc())
        .limit(30)
    )
    histories = history_result.scalars().all()

    candidate_attempt = await _load_latest_candidate_draft_attempt(db, chapter_id)
    return build_chapter_analysis_payload(
        chapter=chapter,
        analysis=analysis,
        memories=memories,
        histories=histories,
        candidate_attempt=candidate_attempt,
        include_full_draft=include_full_draft,
    )
