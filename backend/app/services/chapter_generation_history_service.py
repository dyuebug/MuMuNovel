"""Compatibility shim.

Deprecated: import from app.services.chapter_generation.history_service instead.
"""

from app.services.chapter_generation import history_service as _impl

# re-export public API
build_generation_history_payload = _impl.build_generation_history_payload
parse_reviser_result_from_history = _impl.parse_reviser_result_from_history
build_auto_revision_draft_payload = _impl.build_auto_revision_draft_payload
build_reviser_apply_history_payload = _impl.build_reviser_apply_history_payload
is_reviser_draft_stale = _impl.is_reviser_draft_stale
load_latest_reviser_history = _impl.load_latest_reviser_history

# re-export private helpers imported by API/services
_build_candidate_draft_payload = _impl._build_candidate_draft_payload
_build_candidate_draft_quality_highlights = _impl._build_candidate_draft_quality_highlights
_extract_candidate_draft_full_content = _impl._extract_candidate_draft_full_content
_load_latest_candidate_draft_attempt = _impl._load_latest_candidate_draft_attempt

__all__ = [
    "build_generation_history_payload",
    "parse_reviser_result_from_history",
    "build_auto_revision_draft_payload",
    "build_reviser_apply_history_payload",
    "is_reviser_draft_stale",
    "load_latest_reviser_history",
    "_build_candidate_draft_payload",
    "_build_candidate_draft_quality_highlights",
    "_extract_candidate_draft_full_content",
    "_load_latest_candidate_draft_attempt",
]
