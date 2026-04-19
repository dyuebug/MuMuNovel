"""章节草稿相关 API。"""

from __future__ import annotations

from datetime import datetime
from typing import Any, Dict, Optional

from fastapi import APIRouter, Depends, HTTPException, Query, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.database import get_db
from app.logger import get_logger
from app.services.chapter_draft_apply_service import (
    apply_draft_content_with_history,
    build_draft_apply_response_payload as _build_draft_apply_response_payload,
    build_draft_detail_response_payload as _build_draft_detail_response_payload,
    create_candidate_apply_history_entry_factory,
    create_reviser_apply_history_entry_factory,
    ensure_draft_not_stale_or_raise,
    require_draft_loaded_or_raise,
    resolve_draft_apply_request_options,
    sanitize_draft_content_or_raise,
)
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
)
from app.services.chapter_generation_history_service import (
    _build_candidate_draft_payload,
    _extract_candidate_draft_full_content,
    _load_latest_candidate_draft_attempt,
    build_auto_revision_draft_payload,
    is_reviser_draft_stale,
    load_latest_reviser_history,
    require_candidate_draft_full_content as _require_candidate_draft_full_content,
)

router = APIRouter(prefix="/chapters", tags=["章节管理"])
logger = get_logger(__name__)


def _require_candidate_draft_full_content(draft_attempt) -> str:
    candidate_content_raw, has_full_content = _extract_candidate_draft_full_content(draft_attempt)
    if not has_full_content or not candidate_content_raw.strip():
        raise HTTPException(status_code=409, detail="该候选草稿仅保存了预览，无法直接恢复正文")
    return candidate_content_raw


@router.get("/{chapter_id}/analysis/auto-revision-draft", summary="获取自动修订草稿详情")
async def get_auto_revision_draft(
    chapter_id: str,
    request: Request,
    history_id: Optional[str] = Query(None, description="指定修订草稿历史ID"),
    db: AsyncSession = Depends(get_db),
):
    """获取自动修订草稿详情。"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    reviser_loaded = await load_latest_reviser_history(
        db=db,
        chapter_id=chapter_id,
        history_id=history_id,
    )
    if not reviser_loaded:
        raise HTTPException(status_code=404, detail="该章节暂无自动修订草稿")

    reviser_history, reviser_result = reviser_loaded
    auto_revision_draft = build_auto_revision_draft_payload(
        reviser_result=reviser_result,
        history_id=reviser_history.id,
        created_at=reviser_history.created_at,
        chapter_updated_at=chapter.updated_at,
        include_full_text=True,
    )
    return _build_draft_detail_response_payload(
        chapter_id=chapter_id,
        payload_key="auto_revision_draft",
        payload=auto_revision_draft,
    )


@router.post("/{chapter_id}/analysis/auto-revision-draft/apply", summary="应用自动修订草稿")
async def apply_auto_revision_draft(
    chapter_id: str,
    request: Request,
    apply_request: Optional[Dict[str, Any]] = None,
    db: AsyncSession = Depends(get_db),
):
    """应用自动修订草稿到章节正文。"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    request_options = resolve_draft_apply_request_options(
        apply_request,
        draft_id_field="history_id",
    )
    history_id = request_options.draft_id
    allow_stale = request_options.allow_stale

    reviser_loaded = await load_latest_reviser_history(
        db=db,
        chapter_id=chapter_id,
        history_id=history_id,
    )
    if not reviser_loaded:
        if history_id:
            raise HTTPException(status_code=404, detail="指定的自动修订草稿不存在或不可用")
        raise HTTPException(status_code=404, detail="该章节暂无可应用的自动修订草稿")

    reviser_history, reviser_result = reviser_loaded
    revised_text = sanitize_draft_content_or_raise(
        reviser_result.get("revised_text"),
        empty_detail="自动修订草稿内容为空，无法应用",
        meta_detail="自动修订草稿包含流程化元文本，无法应用",
        sanitize_text_fn=sanitize_generated_narrative_text,
        contains_meta_fn=contains_chapter_workflow_meta_text,
    )

    stale = ensure_draft_not_stale_or_raise(
        chapter_updated_at=chapter.updated_at,
        draft_created_at=reviser_history.created_at,
        allow_stale=allow_stale,
        stale_detail="自动修订草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true",
        is_stale_fn=is_reviser_draft_stale,
    )

    apply_result = await apply_draft_content_with_history(
        db,
        chapter=chapter,
        content=revised_text,
        history_entry_factory=create_reviser_apply_history_entry_factory(
            chapter=chapter,
            chapter_id=chapter_id,
            source_history_id=reviser_history.id,
            source_created_at=reviser_history.created_at,
            reviser_result=reviser_result,
            stale_applied=stale,
            allow_stale=allow_stale,
        ),
    )
    logger.info(
        "已应用自动修订草稿: chapter_id=%s, old=%s, new=%s, stale=%s",
        chapter_id,
        apply_result.old_word_count,
        apply_result.new_word_count,
        stale,
    )
    return _build_draft_apply_response_payload(
        chapter_id=chapter_id,
        old_word_count=apply_result.old_word_count,
        new_word_count=apply_result.new_word_count,
        stale_applied=stale,
        message="自动修订草稿已应用",
        draft_id_field="draft_history_id",
        draft_id=reviser_history.id,
        draft_created_at=reviser_history.created_at,
    )


@router.get("/{chapter_id}/analysis/candidate-draft", summary="获取候选草稿详情")
async def get_candidate_draft(
    chapter_id: str,
    request: Request,
    attempt_id: Optional[str] = Query(None, description="指定候选草稿ID"),
    db: AsyncSession = Depends(get_db),
):
    """获取候选草稿详情。"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    draft_attempt = await _load_latest_candidate_draft_attempt(
        db=db,
        chapter_id=chapter_id,
        attempt_id=attempt_id,
    )
    if not draft_attempt:
        if attempt_id:
            raise HTTPException(status_code=404, detail="指定的候选草稿不存在或不可用")
        raise HTTPException(status_code=404, detail="该章节暂无候选草稿")

    return _build_draft_detail_response_payload(
        chapter_id=chapter_id,
        payload_key="candidate_draft",
        payload=_build_candidate_draft_payload(
            draft_attempt=draft_attempt,
            chapter_updated_at=chapter.updated_at,
            include_full_text=True,
        ),
    )


@router.post("/{chapter_id}/analysis/candidate-draft/apply", summary="应用候选草稿")
async def apply_candidate_draft(
    chapter_id: str,
    request: Request,
    apply_request: Optional[Dict[str, Any]] = None,
    db: AsyncSession = Depends(get_db),
):
    """应用候选草稿到章节正文。"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    request_options = resolve_draft_apply_request_options(
        apply_request,
        draft_id_field="attempt_id",
    )
    attempt_id = request_options.draft_id
    allow_stale = request_options.allow_stale

    draft_attempt = await _load_latest_candidate_draft_attempt(
        db=db,
        chapter_id=chapter_id,
        attempt_id=attempt_id,
    )
    if not draft_attempt:
        if attempt_id:
            raise HTTPException(status_code=404, detail="指定的候选草稿不存在或不可用")
        raise HTTPException(status_code=404, detail="该章节暂无可应用的候选草稿")

    candidate_content_raw = _require_candidate_draft_full_content(draft_attempt)
    candidate_content = sanitize_draft_content_or_raise(
        candidate_content_raw,
        empty_detail="候选草稿内容为空，无法应用",
        meta_detail="候选草稿包含流程化元文本，无法应用",
        sanitize_text_fn=sanitize_generated_narrative_text,
        contains_meta_fn=contains_chapter_workflow_meta_text,
    )

    stale = ensure_draft_not_stale_or_raise(
        chapter_updated_at=chapter.updated_at,
        draft_created_at=draft_attempt.created_at,
        allow_stale=allow_stale,
        stale_detail="候选草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true",
        is_stale_fn=is_reviser_draft_stale,
    )

    apply_result = await apply_draft_content_with_history(
        db,
        chapter=chapter,
        content=candidate_content,
        history_entry_factory=create_candidate_apply_history_entry_factory(
            chapter=chapter,
            chapter_id=chapter_id,
            candidate_content=candidate_content,
            quality_metrics=draft_attempt.quality_metrics,
        ),
    )
    logger.info(
        "Applied candidate draft: chapter_id=%s, old=%s, new=%s, stale=%s",
        chapter_id,
        apply_result.old_word_count,
        apply_result.new_word_count,
        stale,
    )
    return _build_draft_apply_response_payload(
        chapter_id=chapter_id,
        old_word_count=apply_result.old_word_count,
        new_word_count=apply_result.new_word_count,
        stale_applied=stale,
        message="候选草稿已恢复到章节正文",
        draft_id_field="draft_attempt_id",
        draft_id=draft_attempt.id,
        draft_created_at=draft_attempt.created_at,
    )
