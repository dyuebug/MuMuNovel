from __future__ import annotations

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession
from fastapi import Request

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.models.memory import PlotAnalysis, StoryMemory
from app.services.chapter_annotation_service import build_chapter_annotations_payload


async def get_chapter_annotations_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
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

    memories_result = await db_session.execute(
        select(StoryMemory)
        .where(StoryMemory.chapter_id == chapter_id)
        .order_by(StoryMemory.importance_score.desc())
    )
    memories = memories_result.scalars().all()

    return build_chapter_annotations_payload(
        chapter=chapter,
        analysis=analysis,
        memories=memories,
    )
