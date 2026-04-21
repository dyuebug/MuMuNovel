from __future__ import annotations

from datetime import datetime
from typing import Any, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.services.chapter_draft_apply_service import build_draft_detail_response_payload
from app.services.chapter_draft_state_service import (
    AUTO_REVISION_MISSING_DETAIL,
    AUTO_REVISION_NOT_FOUND_DETAIL,
    CANDIDATE_MISSING_DETAIL,
    CANDIDATE_NOT_FOUND_DETAIL,
    load_candidate_draft_attempt_or_raise,
    load_reviser_history_or_raise,
)
from app.services.chapter_generation_history_service import (
    _build_candidate_draft_payload,
    build_auto_revision_draft_payload,
)


async def load_auto_revision_draft_detail_payload(
    *,
    db: AsyncSession,
    chapter_id: str,
    chapter_updated_at: Optional[datetime],
    history_id: Optional[str] = None,
) -> dict[str, Any]:
    reviser_history, reviser_result = await load_reviser_history_or_raise(
        db=db,
        chapter_id=chapter_id,
        history_id=history_id,
        missing_detail=AUTO_REVISION_MISSING_DETAIL,
        not_found_detail=AUTO_REVISION_NOT_FOUND_DETAIL,
    )
    auto_revision_draft = build_auto_revision_draft_payload(
        reviser_result=reviser_result,
        history_id=reviser_history.id,
        created_at=reviser_history.created_at,
        chapter_updated_at=chapter_updated_at,
        include_full_text=True,
    )
    return build_draft_detail_response_payload(
        chapter_id=chapter_id,
        payload_key='auto_revision_draft',
        payload=auto_revision_draft,
    )


async def load_candidate_draft_detail_payload(
    *,
    db: AsyncSession,
    chapter_id: str,
    chapter_updated_at: Optional[datetime],
    attempt_id: Optional[str] = None,
) -> dict[str, Any]:
    draft_attempt = await load_candidate_draft_attempt_or_raise(
        db=db,
        chapter_id=chapter_id,
        attempt_id=attempt_id,
        missing_detail=CANDIDATE_MISSING_DETAIL,
        not_found_detail=CANDIDATE_NOT_FOUND_DETAIL,
    )
    return build_draft_detail_response_payload(
        chapter_id=chapter_id,
        payload_key='candidate_draft',
        payload=_build_candidate_draft_payload(
            draft_attempt=draft_attempt,
            chapter_updated_at=chapter_updated_at,
            include_full_text=True,
        ),
    )
