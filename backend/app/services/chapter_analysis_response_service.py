from __future__ import annotations

import json
from datetime import datetime
from typing import TYPE_CHECKING, Any, Dict, List, Optional, Sequence

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active chapter-analysis response projection and draft view "
    "payload contract; this Python service is kept only as frozen "
    "rollback/source-map material behind repointed route shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_analysis_runtime_service/query_owner.rs; "
    "backend-rs/src/services/chapter_draft_view_payload_service.rs; "
    "backend-rs/src/services/chapter_draft_history_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_chapter_analysis_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.services.chapter_generation.history_service import (
    build_auto_revision_draft_payload,
    _build_candidate_draft_payload as build_candidate_draft_payload,
    parse_reviser_result_from_history,
)

if TYPE_CHECKING:
    from app.models.chapter import Chapter
    from app.models.chapter_draft_attempt import ChapterDraftAttempt
    from app.models.generation_history import GenerationHistory
    from app.models.memory import PlotAnalysis, StoryMemory


def parse_checker_result_from_history(generated_content: Optional[str]) -> Optional[Dict[str, Any]]:
    if not generated_content:
        return None
    try:
        payload = json.loads(generated_content)
        if not isinstance(payload, dict):
            return None
        if payload.get('log_type') != 'chapter_text_checker_v1':
            return None
        checker_result = payload.get('checker_result')
        if isinstance(checker_result, dict):
            return checker_result
    except Exception:
        return None
    return None


def _serialize_memories(memories: Sequence[StoryMemory]) -> List[Dict[str, Any]]:
    return [
        {
            'id': memory.id,
            'type': memory.memory_type,
            'title': memory.title,
            'content': memory.content,
            'importance': memory.importance_score,
            'tags': memory.tags,
            'is_foreshadow': memory.is_foreshadow,
            'position': memory.chapter_position,
            'related_characters': memory.related_characters,
        }
        for memory in memories
    ]


def build_chapter_analysis_payload(
    *,
    chapter: Chapter,
    analysis: PlotAnalysis,
    memories: Sequence[StoryMemory],
    histories: Sequence[GenerationHistory],
    candidate_attempt: Optional[ChapterDraftAttempt],
    include_full_draft: bool = False,
) -> Dict[str, Any]:
    from app.services.story_quality_feedback_service import (
        extract_quality_metrics_from_history_payload,
    )
    from app.services.story_repair_payload_service import (
        build_batch_quality_metrics_summary,
    )

    latest_checker_result: Optional[Dict[str, Any]] = None
    latest_reviser_result: Optional[Dict[str, Any]] = None
    checker_created_at: Optional[str] = None
    latest_reviser_created_at: Optional[datetime] = None
    latest_reviser_history_id: Optional[str] = None

    for history in histories:
        if latest_checker_result is None:
            parsed_checker = parse_checker_result_from_history(history.generated_content)
            if parsed_checker:
                latest_checker_result = parsed_checker
                checker_created_at = history.created_at.isoformat() if history.created_at else None
        if latest_reviser_result is None:
            parsed_reviser = parse_reviser_result_from_history(history.generated_content)
            if parsed_reviser:
                latest_reviser_result = parsed_reviser
                latest_reviser_created_at = history.created_at
                latest_reviser_history_id = history.id
        if latest_checker_result is not None and latest_reviser_result is not None:
            break

    auto_revision_draft = None
    if latest_reviser_result:
        auto_revision_draft = build_auto_revision_draft_payload(
            reviser_result=latest_reviser_result,
            history_id=latest_reviser_history_id,
            created_at=latest_reviser_created_at,
            chapter_updated_at=chapter.updated_at,
            include_full_text=include_full_draft,
        )

    quality_metrics_history = [
        metrics
        for metrics in (
            extract_quality_metrics_from_history_payload(history.generated_content)
            for history in histories
        )
        if metrics
    ]
    latest_quality_metrics = quality_metrics_history[0] if quality_metrics_history else None
    quality_metrics_summary = build_batch_quality_metrics_summary(quality_metrics_history)

    candidate_draft = (
        build_candidate_draft_payload(
            draft_attempt=candidate_attempt,
            chapter_updated_at=chapter.updated_at,
            include_full_text=include_full_draft,
        )
        if candidate_attempt is not None
        else None
    )

    return {
        'chapter_id': chapter.id,
        'analysis': analysis.to_dict(),
        'memories': _serialize_memories(memories),
        'checker_result': latest_checker_result,
        'checker_created_at': checker_created_at,
        'auto_revision_draft': auto_revision_draft,
        'candidate_draft': candidate_draft,
        'quality_metrics': latest_quality_metrics,
        'quality_metrics_summary': quality_metrics_summary,
        'created_at': analysis.created_at.isoformat() if analysis.created_at else None,
    }
