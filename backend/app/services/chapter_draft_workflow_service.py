from __future__ import annotations

from typing import TYPE_CHECKING, Any, Mapping, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active chapter-draft apply workflow and history-persist "
    "contract; this Python service is kept only as frozen rollback/source-map "
    "material behind repointed draft route shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_draft_routes.rs; "
    "backend-rs/src/services/chapter_draft_history_service.rs; "
    "backend-rs/src/services/chapter_draft_source_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_chapter_draft_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.logger import get_logger
from app.services.chapter_draft_apply_service import (
    apply_draft_content_with_history,
    build_draft_apply_response_payload,
    create_candidate_apply_history_entry_factory,
    create_reviser_apply_history_entry_factory,
    ensure_draft_not_stale_or_raise,
    resolve_draft_apply_request_options,
    sanitize_draft_content_or_raise,
)
from app.services.chapter_draft_state_service import (
    AUTO_REVISION_APPLIED_MESSAGE,
    AUTO_REVISION_APPLY_MISSING_DETAIL,
    AUTO_REVISION_EMPTY_DETAIL,
    AUTO_REVISION_META_DETAIL,
    AUTO_REVISION_NOT_FOUND_DETAIL,
    AUTO_REVISION_STALE_DETAIL,
    CANDIDATE_APPLIED_MESSAGE,
    CANDIDATE_APPLY_MISSING_DETAIL,
    CANDIDATE_EMPTY_DETAIL,
    CANDIDATE_META_DETAIL,
    CANDIDATE_NOT_FOUND_DETAIL,
    CANDIDATE_STALE_DETAIL,
    load_candidate_draft_attempt_or_raise,
    load_reviser_history_or_raise,
    require_candidate_draft_full_content_or_raise,
)
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
)
from app.services.chapter_generation.history_service import is_reviser_draft_stale

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.chapter import Chapter

logger = get_logger(__name__)


async def apply_auto_revision_draft_payload(
    *,
    db: AsyncSession,
    chapter: Chapter,
    chapter_id: str,
    apply_request: Optional[Mapping[str, Any]] = None,
) -> dict[str, Any]:
    request_options = resolve_draft_apply_request_options(
        apply_request,
        draft_id_field='history_id',
    )
    history_id = request_options.draft_id
    allow_stale = request_options.allow_stale

    reviser_history, reviser_result = await load_reviser_history_or_raise(
        db=db,
        chapter_id=chapter_id,
        history_id=history_id,
        missing_detail=AUTO_REVISION_APPLY_MISSING_DETAIL,
        not_found_detail=AUTO_REVISION_NOT_FOUND_DETAIL,
    )
    revised_text = sanitize_draft_content_or_raise(
        reviser_result.get('revised_text'),
        empty_detail=AUTO_REVISION_EMPTY_DETAIL,
        meta_detail=AUTO_REVISION_META_DETAIL,
        sanitize_text_fn=sanitize_generated_narrative_text,
        contains_meta_fn=contains_chapter_workflow_meta_text,
    )

    stale = ensure_draft_not_stale_or_raise(
        chapter_updated_at=chapter.updated_at,
        draft_created_at=reviser_history.created_at,
        allow_stale=allow_stale,
        stale_detail=AUTO_REVISION_STALE_DETAIL,
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
        'Applied auto revision draft: chapter_id=%s, old=%s, new=%s, stale=%s',
        chapter_id,
        apply_result.old_word_count,
        apply_result.new_word_count,
        stale,
    )
    return build_draft_apply_response_payload(
        chapter_id=chapter_id,
        old_word_count=apply_result.old_word_count,
        new_word_count=apply_result.new_word_count,
        stale_applied=stale,
        message=AUTO_REVISION_APPLIED_MESSAGE,
        draft_id_field='draft_history_id',
        draft_id=reviser_history.id,
        draft_created_at=reviser_history.created_at,
    )


async def apply_candidate_draft_payload(
    *,
    db: AsyncSession,
    chapter: Chapter,
    chapter_id: str,
    apply_request: Optional[Mapping[str, Any]] = None,
) -> dict[str, Any]:
    request_options = resolve_draft_apply_request_options(
        apply_request,
        draft_id_field='attempt_id',
    )
    attempt_id = request_options.draft_id
    allow_stale = request_options.allow_stale

    draft_attempt = await load_candidate_draft_attempt_or_raise(
        db=db,
        chapter_id=chapter_id,
        attempt_id=attempt_id,
        missing_detail=CANDIDATE_APPLY_MISSING_DETAIL,
        not_found_detail=CANDIDATE_NOT_FOUND_DETAIL,
    )
    candidate_content_raw = require_candidate_draft_full_content_or_raise(draft_attempt)
    candidate_content = sanitize_draft_content_or_raise(
        candidate_content_raw,
        empty_detail=CANDIDATE_EMPTY_DETAIL,
        meta_detail=CANDIDATE_META_DETAIL,
        sanitize_text_fn=sanitize_generated_narrative_text,
        contains_meta_fn=contains_chapter_workflow_meta_text,
    )

    stale = ensure_draft_not_stale_or_raise(
        chapter_updated_at=chapter.updated_at,
        draft_created_at=draft_attempt.created_at,
        allow_stale=allow_stale,
        stale_detail=CANDIDATE_STALE_DETAIL,
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
        'Applied candidate draft: chapter_id=%s, old=%s, new=%s, stale=%s',
        chapter_id,
        apply_result.old_word_count,
        apply_result.new_word_count,
        stale,
    )
    return build_draft_apply_response_payload(
        chapter_id=chapter_id,
        old_word_count=apply_result.old_word_count,
        new_word_count=apply_result.new_word_count,
        stale_applied=stale,
        message=CANDIDATE_APPLIED_MESSAGE,
        draft_id_field='draft_attempt_id',
        draft_id=draft_attempt.id,
        draft_created_at=draft_attempt.created_at,
    )
