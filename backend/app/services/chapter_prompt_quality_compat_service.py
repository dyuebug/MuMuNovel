"""Compatibility shim for chapter prompt/quality seams.

Deprecated: import from app.services.compat.chapter_prompt_quality_compat_service instead.
"""

from __future__ import annotations

from typing import Any, Dict, Optional

from app.models.project import Project
from app.services.compat import chapter_prompt_quality_compat_service as _impl

_compute_story_quality_metrics_service = _impl._compute_story_quality_metrics_service
_detect_style_profile_service = _impl._detect_style_profile_service
_resolve_generation_temperature_service = _impl._resolve_generation_temperature_service
_build_chapter_runtime_system_prompt_service = _impl._build_chapter_runtime_system_prompt_service


def compute_story_quality_metrics(
    content: str,
    chapter_outline: Optional[str],
    world_rules: Optional[str],
    quality_runtime_context: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    return _compute_story_quality_metrics_service(
        content=content,
        chapter_outline=chapter_outline,
        world_rules=world_rules,
        quality_runtime_context=quality_runtime_context,
    )


def detect_style_profile(
    style_name: Optional[str],
    style_preset_id: Optional[str],
    style_content: Optional[str] = None,
) -> str:
    return _detect_style_profile_service(
        style_name=style_name,
        style_preset_id=style_preset_id,
        style_content=style_content,
    )


def resolve_generation_temperature(style_profile: str) -> float:
    return _resolve_generation_temperature_service(style_profile)


def build_chapter_runtime_system_prompt(
    project: Project,
    style_content: str,
    chapter_outline: Optional[str],
    previous_summary: Optional[str] = None,
    style_name: Optional[str] = None,
    style_preset_id: Optional[str] = None,
    target_word_count: Optional[int] = None,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
    web_research_grounding_block: Optional[str] = None,
) -> str:
    return _build_chapter_runtime_system_prompt_service(
        project=project,
        style_content=style_content,
        chapter_outline=chapter_outline,
        previous_summary=previous_summary,
        style_name=style_name,
        style_preset_id=style_preset_id,
        target_word_count=target_word_count,
        story_runtime_contract=story_runtime_contract,
        web_research_grounding_block=web_research_grounding_block,
    )
