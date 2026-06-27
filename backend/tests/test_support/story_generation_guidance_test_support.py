from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Mapping, Optional


STORY_GUIDANCE_FIELD_NAMES: tuple[str, ...] = (
    "creative_mode",
    "story_focus",
    "plot_stage",
    "story_creation_brief",
    "quality_preset",
    "quality_notes",
)


def _normalize_optional_text(value: Optional[str]) -> Optional[str]:
    if value is None:
        return None
    normalized = str(value).strip()
    return normalized or None


def normalize_story_guidance_values(
    values: Mapping[str, Any],
) -> Dict[str, Optional[str]]:
    return {
        field_name: _normalize_optional_text(values.get(field_name))
        for field_name in STORY_GUIDANCE_FIELD_NAMES
    }


def _resolve_optional_text(*values: Optional[str]) -> Optional[str]:
    for value in values:
        normalized = _normalize_optional_text(value)
        if normalized is not None:
            return normalized
    return None


def read_story_guidance_value(
    source: Optional[Any],
    field_name: str,
) -> Optional[str]:
    if source is None:
        return None
    if isinstance(source, Mapping):
        return source.get(field_name)
    return getattr(source, field_name, None)


def extract_story_guidance_overrides(
    source: Optional[Any] = None,
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
) -> Dict[str, Optional[str]]:
    raw_overrides = {
        "creative_mode": creative_mode,
        "story_focus": story_focus,
        "plot_stage": plot_stage,
        "story_creation_brief": story_creation_brief,
        "quality_preset": quality_preset,
        "quality_notes": quality_notes,
    }
    return {
        field_name: value if value is not None else read_story_guidance_value(source, field_name)
        for field_name, value in raw_overrides.items()
    }


def resolve_project_generation_defaults(
    project: Any,
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
) -> Dict[str, Optional[str]]:
    return {
        "creative_mode": _resolve_optional_text(
            creative_mode,
            getattr(project, "default_creative_mode", None),
        ),
        "story_focus": _resolve_optional_text(
            story_focus,
            getattr(project, "default_story_focus", None),
        ),
        "plot_stage": _resolve_optional_text(
            plot_stage,
            getattr(project, "default_plot_stage", None),
        ),
        "story_creation_brief": _resolve_optional_text(
            story_creation_brief,
            getattr(project, "default_story_creation_brief", None),
        ),
        "quality_preset": _resolve_optional_text(
            quality_preset,
            getattr(project, "default_quality_preset", None),
        ),
        "quality_notes": _resolve_optional_text(
            quality_notes,
            getattr(project, "default_quality_notes", None),
        ),
    }


@dataclass(frozen=True)
class StoryGenerationGuidance:
    creative_mode: Optional[str] = None
    story_focus: Optional[str] = None
    plot_stage: Optional[str] = None
    story_creation_brief: Optional[str] = None
    quality_preset: Optional[str] = None
    quality_notes: Optional[str] = None

    @classmethod
    def from_generation_kwargs(
        cls,
        values: Optional[Mapping[str, Any]] = None,
    ) -> "StoryGenerationGuidance":
        normalized_values = normalize_story_guidance_values(values or {})
        return cls(**normalized_values)

    def to_generation_kwargs(self) -> Dict[str, Optional[str]]:
        return {
            "creative_mode": self.creative_mode,
            "story_focus": self.story_focus,
            "plot_stage": self.plot_stage,
            "story_creation_brief": self.story_creation_brief,
            "quality_preset": self.quality_preset,
            "quality_notes": self.quality_notes,
        }

    def to_runtime_contract(self) -> Dict[str, Optional[str]]:
        return self.to_generation_kwargs()

    def to_prompt_fields(self) -> Dict[str, Any]:
        return {
            key: value or ""
            for key, value in self.to_generation_kwargs().items()
        }


def resolve_story_generation_guidance(
    project: Optional[Any],
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
) -> StoryGenerationGuidance:
    normalized_overrides = normalize_story_guidance_values(
        {
            "creative_mode": creative_mode,
            "story_focus": story_focus,
            "plot_stage": plot_stage,
            "story_creation_brief": story_creation_brief,
            "quality_preset": quality_preset,
            "quality_notes": quality_notes,
        }
    )
    if project is None:
        return StoryGenerationGuidance(**normalized_overrides)

    resolved = resolve_project_generation_defaults(project, **normalized_overrides)
    return StoryGenerationGuidance(**resolved)


__all__ = [
    "STORY_GUIDANCE_FIELD_NAMES",
    "StoryGenerationGuidance",
    "extract_story_guidance_overrides",
    "normalize_story_guidance_values",
    "read_story_guidance_value",
    "resolve_project_generation_defaults",
    "resolve_story_generation_guidance",
]
