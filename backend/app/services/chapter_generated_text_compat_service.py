"""Compatibility helpers for generated chapter text sanitation seams."""
from __future__ import annotations

from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text as _contains_chapter_workflow_meta_text_service,
    is_likely_chapter_meta_line as _is_likely_chapter_meta_line_service,
    lightly_polish_template_phrases as _lightly_polish_template_phrases_service,
    sanitize_generated_narrative_text as _sanitize_generated_narrative_text_service,
    trim_text_to_sentence_boundary as _trim_text_to_sentence_boundary_service,
)


def trim_text_to_sentence_boundary(text: str, *, hard_limit: int, lookback_chars: int = 220) -> str:
    return _trim_text_to_sentence_boundary_service(
        text,
        hard_limit=hard_limit,
        lookback_chars=lookback_chars,
    )


def is_likely_chapter_meta_line(line: str) -> bool:
    return _is_likely_chapter_meta_line_service(line)


def contains_chapter_workflow_meta_text(text: str) -> bool:
    return _contains_chapter_workflow_meta_text_service(text)


def lightly_polish_template_phrases(text: str) -> str:
    return _lightly_polish_template_phrases_service(text)


def sanitize_generated_narrative_text(text: str) -> tuple[str, int]:
    return _sanitize_generated_narrative_text_service(text)
