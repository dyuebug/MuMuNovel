from __future__ import annotations

from fastapi import HTTPException, Request
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.models.generation_history import GenerationHistory
from app.models.memory import PlotAnalysis, StoryMemory
from app.services.chapter_analysis_response_service import build_chapter_analysis_payload
from app.services.chapter_generation_history_service import _load_latest_candidate_draft_attempt


async def get_chapter_analysis_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    include_full_draft: bool,
    db_session: AsyncSession,
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    analysis_result = await db_session.execute(
        select(PlotAnalysis)
        .where(PlotAnalysis.chapter_id == chapter_id)
        .order_by(PlotAnalysis.created_at.desc())
        .limit(1)
    )
    analysis = analysis_result.scalar_one_or_none()
    if analysis is None:
        raise HTTPException(status_code=404, detail="未找到章节分析结果")

    memories_result = await db_session.execute(
        select(StoryMemory)
        .where(StoryMemory.chapter_id == chapter_id)
        .order_by(StoryMemory.importance_score.desc())
    )
    memories = memories_result.scalars().all()

    history_result = await db_session.execute(
        select(GenerationHistory)
        .where(GenerationHistory.chapter_id == chapter_id)
        .order_by(GenerationHistory.created_at.desc())
        .limit(30)
    )
    histories = history_result.scalars().all()

    candidate_attempt = await _load_latest_candidate_draft_attempt(db_session, chapter_id)
    return build_chapter_analysis_payload(
        chapter=chapter,
        analysis=analysis,
        memories=memories,
        histories=histories,
        candidate_attempt=candidate_attempt,
        include_full_draft=include_full_draft,
    )
