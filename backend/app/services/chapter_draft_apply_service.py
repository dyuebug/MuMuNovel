from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any, Callable, Mapping, Optional, TypeVar

from fastapi import HTTPException
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.models.generation_history import GenerationHistory
from app.services.chapter_content_apply_service import ChapterContentApplyResult, apply_chapter_content_update
from app.services.chapter_generation_history_service import (
    build_generation_history_payload,
    build_reviser_apply_history_payload,
)


@dataclass(frozen=True)
class DraftApplyRequestOptions:
    draft_id: Optional[str]
    allow_stale: bool


DraftItemT = TypeVar('DraftItemT')


def build_draft_apply_response_payload(
    *,
    chapter_id: str,
    old_word_count: int,
    new_word_count: int,
    stale_applied: bool,
    message: str,
    draft_id_field: str,
    draft_id: str,
    draft_created_at: Optional[datetime],
) -> dict[str, Any]:
    return {
        "success": True,
        "chapter_id": chapter_id,
        "word_count": new_word_count,
        "old_word_count": old_word_count,
        draft_id_field: draft_id,
        "draft_created_at": draft_created_at.isoformat() if draft_created_at else None,
        "stale_applied": stale_applied,
        "message": message,
    }


def build_draft_detail_response_payload(
    *,
    chapter_id: str,
    payload_key: str,
    payload: Any,
) -> dict[str, Any]:
    return {
        "chapter_id": chapter_id,
        payload_key: payload,
    }


def resolve_draft_apply_request_options(
    payload: Optional[Mapping[str, Any]],
    *,
    draft_id_field: str,
) -> DraftApplyRequestOptions:
    data = payload or {}
    draft_id_raw = data.get(draft_id_field)
    draft_id = str(draft_id_raw).strip() if draft_id_raw is not None else ''
    draft_id = draft_id or None

    allow_stale_raw = data.get('allow_stale', False)
    if isinstance(allow_stale_raw, bool):
        allow_stale = allow_stale_raw
    elif isinstance(allow_stale_raw, str):
        allow_stale = allow_stale_raw.strip().lower() in {'1', 'true', 'yes', 'on'}
    else:
        allow_stale = bool(allow_stale_raw)

    return DraftApplyRequestOptions(draft_id=draft_id, allow_stale=allow_stale)


def require_draft_loaded_or_raise(
    draft_item: Optional[DraftItemT],
    *,
    draft_id: Optional[str],
    missing_detail: str,
    not_found_detail: str,
) -> DraftItemT:
    if draft_item is not None:
        return draft_item
    if draft_id:
        raise HTTPException(status_code=404, detail=not_found_detail)
    raise HTTPException(status_code=404, detail=missing_detail)


def sanitize_draft_content_or_raise(
    raw_content: Any,
    *,
    empty_detail: str,
    meta_detail: str,
    sanitize_text_fn: Callable[[str], tuple[str, int]],
    contains_meta_fn: Callable[[str], bool],
) -> str:
    content_text = str(raw_content or '').strip()
    sanitized_content, _ = sanitize_text_fn(content_text)
    if not sanitized_content.strip():
        raise HTTPException(status_code=400, detail=empty_detail)
    if contains_meta_fn(sanitized_content):
        raise HTTPException(status_code=400, detail=meta_detail)
    return sanitized_content


def ensure_draft_not_stale_or_raise(
    *,
    chapter_updated_at: Optional[datetime],
    draft_created_at: Optional[datetime],
    allow_stale: bool,
    stale_detail: str,
    is_stale_fn: Callable[[Optional[datetime], Optional[datetime]], bool],
) -> bool:
    stale = is_stale_fn(chapter_updated_at, draft_created_at)
    if stale and not allow_stale:
        raise HTTPException(status_code=409, detail=stale_detail)
    return stale


def create_reviser_apply_history_entry_factory(
    *,
    chapter: Chapter,
    chapter_id: str,
    source_history_id: str,
    source_created_at: Optional[datetime],
    reviser_result: Mapping[str, Any],
    stale_applied: bool,
    allow_stale: bool,
) -> Callable[[int, int], GenerationHistory]:
    title = chapter.title or ''
    chapter_number = chapter.chapter_number
    critical_count = int(reviser_result.get('critical_count') or 0)
    major_count = int(reviser_result.get('major_count') or 0)
    priority_issue_count = int(
        reviser_result.get('priority_issue_count')
        or (critical_count + major_count)
    )
    applied_critical_count = int(reviser_result.get('applied_critical_count') or 0)
    applied_issue_count = int(
        reviser_result.get('applied_issue_count')
        or reviser_result.get('applied_critical_count')
        or 0
    )

    def build_apply_history(old_word_count: int, new_word_count: int) -> GenerationHistory:
        return GenerationHistory(
            project_id=chapter.project_id,
            chapter_id=chapter_id,
            prompt=f"自动修订应用: 第{chapter_number}章 {title}",
            generated_content=build_reviser_apply_history_payload(
                source_history_id=source_history_id,
                source_created_at=source_created_at,
                critical_count=critical_count,
                major_count=major_count,
                priority_issue_count=priority_issue_count,
                applied_critical_count=applied_critical_count,
                applied_issue_count=applied_issue_count,
                old_word_count=old_word_count,
                new_word_count=new_word_count,
                stale_applied=stale_applied,
                allow_stale=allow_stale,
            ),
            model='chapter_text_reviser_apply_v1',
        )

    return build_apply_history


def create_candidate_apply_history_entry_factory(
    *,
    chapter: Chapter,
    chapter_id: str,
    candidate_content: str,
    quality_metrics: Optional[dict[str, Any]],
) -> Callable[[int, int], GenerationHistory]:
    title = chapter.title or ''
    chapter_number = chapter.chapter_number
    normalized_quality_metrics = dict(quality_metrics or {}) if isinstance(quality_metrics, dict) else {}

    def build_apply_history(old_word_count: int, new_word_count: int) -> GenerationHistory:
        return GenerationHistory(
            project_id=chapter.project_id,
            chapter_id=chapter_id,
            prompt=f"apply candidate draft: chapter {chapter_number} {title}",
            generated_content=build_generation_history_payload(
                candidate_content,
                normalized_quality_metrics,
                content_applied=True,
                attempt_state='applied_from_candidate',
            ),
            model='chapter_candidate_apply_v1',
        )

    return build_apply_history


async def apply_draft_content_with_history(
    db: AsyncSession,
    *,
    chapter: Chapter,
    content: str,
    history_entry_factory: Callable[[int, int], Any],
) -> ChapterContentApplyResult:
    old_word_count = chapter.word_count or len(chapter.content or '')
    new_word_count = len(content)
    history_entry = history_entry_factory(old_word_count, new_word_count)
    return await apply_chapter_content_update(
        db,
        chapter=chapter,
        content=content,
        history_entry=history_entry,
    )
