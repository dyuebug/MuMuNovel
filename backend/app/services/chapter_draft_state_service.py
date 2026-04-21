from __future__ import annotations

from fastapi import HTTPException
from sqlalchemy.ext.asyncio import AsyncSession

from app.services.chapter_draft_apply_service import require_draft_loaded_or_raise
from app.services.chapter_generation_history_service import (
    _extract_candidate_draft_full_content,
    _load_latest_candidate_draft_attempt,
    load_latest_reviser_history,
)

PREVIEW_ONLY_DETAIL = "\u8be5\u5019\u9009\u8349\u7a3f\u4ec5\u4fdd\u5b58\u4e86\u9884\u89c8\uff0c\u65e0\u6cd5\u76f4\u63a5\u6062\u590d\u6b63\u6587"
AUTO_REVISION_MISSING_DETAIL = "\u8be5\u7ae0\u8282\u6682\u65e0\u81ea\u52a8\u4fee\u8ba2\u8349\u7a3f"
AUTO_REVISION_NOT_FOUND_DETAIL = "\u6307\u5b9a\u7684\u81ea\u52a8\u4fee\u8ba2\u8349\u7a3f\u4e0d\u5b58\u5728\u6216\u4e0d\u53ef\u7528"
AUTO_REVISION_APPLY_MISSING_DETAIL = "\u8be5\u7ae0\u8282\u6682\u65e0\u53ef\u5e94\u7528\u7684\u81ea\u52a8\u4fee\u8ba2\u8349\u7a3f"
AUTO_REVISION_EMPTY_DETAIL = "\u81ea\u52a8\u4fee\u8ba2\u8349\u7a3f\u5185\u5bb9\u4e3a\u7a7a\uff0c\u65e0\u6cd5\u5e94\u7528"
AUTO_REVISION_META_DETAIL = "\u81ea\u52a8\u4fee\u8ba2\u8349\u7a3f\u5305\u542b\u6d41\u7a0b\u5316\u5143\u6587\u672c\uff0c\u65e0\u6cd5\u5e94\u7528"
AUTO_REVISION_STALE_DETAIL = "\u81ea\u52a8\u4fee\u8ba2\u8349\u7a3f\u5df2\u8fc7\u671f\uff0c\u8bf7\u83b7\u53d6\u6700\u65b0\u8349\u7a3f\u6216\u5728\u8bf7\u6c42\u4e2d\u8bbe\u7f6e allow_stale=true"
AUTO_REVISION_APPLIED_MESSAGE = "\u81ea\u52a8\u4fee\u8ba2\u8349\u7a3f\u5df2\u5e94\u7528\u5230\u7ae0\u8282\u6b63\u6587"
CANDIDATE_MISSING_DETAIL = "\u8be5\u7ae0\u8282\u6682\u65e0\u5019\u9009\u8349\u7a3f"
CANDIDATE_NOT_FOUND_DETAIL = "\u6307\u5b9a\u7684\u5019\u9009\u8349\u7a3f\u4e0d\u5b58\u5728\u6216\u4e0d\u53ef\u7528"
CANDIDATE_APPLY_MISSING_DETAIL = "\u8be5\u7ae0\u8282\u6682\u65e0\u53ef\u5e94\u7528\u7684\u5019\u9009\u8349\u7a3f"
CANDIDATE_EMPTY_DETAIL = "\u5019\u9009\u8349\u7a3f\u5185\u5bb9\u4e3a\u7a7a\uff0c\u65e0\u6cd5\u5e94\u7528"
CANDIDATE_META_DETAIL = "\u5019\u9009\u8349\u7a3f\u5305\u542b\u6d41\u7a0b\u5316\u5143\u6587\u672c\uff0c\u65e0\u6cd5\u5e94\u7528"
CANDIDATE_STALE_DETAIL = "\u5019\u9009\u8349\u7a3f\u5df2\u8fc7\u671f\uff0c\u8bf7\u83b7\u53d6\u6700\u65b0\u8349\u7a3f\u6216\u5728\u8bf7\u6c42\u4e2d\u8bbe\u7f6e allow_stale=true"
CANDIDATE_APPLIED_MESSAGE = "\u5019\u9009\u8349\u7a3f\u5df2\u6062\u590d\u5230\u7ae0\u8282\u6b63\u6587"


async def load_reviser_history_or_raise(
    *,
    db: AsyncSession,
    chapter_id: str,
    history_id: str | None,
    missing_detail: str,
    not_found_detail: str,
):
    reviser_loaded = await load_latest_reviser_history(
        db=db,
        chapter_id=chapter_id,
        history_id=history_id,
    )
    return require_draft_loaded_or_raise(
        reviser_loaded,
        draft_id=history_id,
        missing_detail=missing_detail,
        not_found_detail=not_found_detail,
    )


async def load_candidate_draft_attempt_or_raise(
    *,
    db: AsyncSession,
    chapter_id: str,
    attempt_id: str | None,
    missing_detail: str,
    not_found_detail: str,
):
    draft_attempt = await _load_latest_candidate_draft_attempt(
        db=db,
        chapter_id=chapter_id,
        attempt_id=attempt_id,
    )
    return require_draft_loaded_or_raise(
        draft_attempt,
        draft_id=attempt_id,
        missing_detail=missing_detail,
        not_found_detail=not_found_detail,
    )


def require_candidate_draft_full_content_or_raise(draft_attempt) -> str:
    candidate_content_raw, has_full_content = _extract_candidate_draft_full_content(draft_attempt)
    if not has_full_content or not candidate_content_raw.strip():
        raise HTTPException(status_code=409, detail=PREVIEW_ONLY_DETAIL)
    return candidate_content_raw
