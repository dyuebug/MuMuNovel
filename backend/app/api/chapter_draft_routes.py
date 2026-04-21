"""Chapter draft workflow API routes."""

from __future__ import annotations

from typing import Any, Dict, Optional

from fastapi import APIRouter, Depends, Query, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.database import get_db
from app.services.chapter_draft_query_service import (
    load_auto_revision_draft_detail_payload,
    load_candidate_draft_detail_payload,
)
from app.services.chapter_draft_workflow_service import (
    apply_auto_revision_draft_payload,
    apply_candidate_draft_payload,
)

router = APIRouter(prefix="/chapters", tags=["章节管理"])


@router.get("/{chapter_id}/analysis/auto-revision-draft", summary="Get auto revision draft detail")
async def get_auto_revision_draft(
    chapter_id: str,
    request: Request,
    history_id: Optional[str] = Query(None, description="Specify revision draft history ID"),
    db: AsyncSession = Depends(get_db),
):
    """Return the latest or specified auto-revision draft payload."""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    return await load_auto_revision_draft_detail_payload(
        db=db,
        chapter_id=chapter_id,
        chapter_updated_at=chapter.updated_at,
        history_id=history_id,
    )


@router.post("/{chapter_id}/analysis/auto-revision-draft/apply", summary="Apply auto revision draft")
async def apply_auto_revision_draft(
    chapter_id: str,
    request: Request,
    apply_request: Optional[Dict[str, Any]] = None,
    db: AsyncSession = Depends(get_db),
):
    """Apply an auto-revision draft back to chapter content."""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    return await apply_auto_revision_draft_payload(
        db=db,
        chapter=chapter,
        chapter_id=chapter_id,
        apply_request=apply_request,
    )


@router.get("/{chapter_id}/analysis/candidate-draft", summary="Get candidate draft detail")
async def get_candidate_draft(
    chapter_id: str,
    request: Request,
    attempt_id: Optional[str] = Query(None, description="Specify candidate draft attempt ID"),
    db: AsyncSession = Depends(get_db),
):
    """Return the latest or specified candidate draft payload."""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    return await load_candidate_draft_detail_payload(
        db=db,
        chapter_id=chapter_id,
        chapter_updated_at=chapter.updated_at,
        attempt_id=attempt_id,
    )


@router.post("/{chapter_id}/analysis/candidate-draft/apply", summary="Apply candidate draft")
async def apply_candidate_draft(
    chapter_id: str,
    request: Request,
    apply_request: Optional[Dict[str, Any]] = None,
    db: AsyncSession = Depends(get_db),
):
    """Apply a candidate draft back to chapter content."""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    return await apply_candidate_draft_payload(
        db=db,
        chapter=chapter,
        chapter_id=chapter_id,
        apply_request=apply_request,
    )
