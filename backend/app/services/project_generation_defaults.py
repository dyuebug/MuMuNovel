"""项目级生成默认值解析工具。"""

from typing import Optional

from app.models.project import Project


def _normalize_optional_text(value: Optional[str]) -> Optional[str]:
    if value is None:
        return None

    normalized = value.strip()
    return normalized or None


def resolve_project_generation_defaults(
    project: Project,
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
) -> dict[str, Optional[str]]:
    """合并请求参数与项目级默认偏好。"""
    return {
        "creative_mode": creative_mode or getattr(project, "default_creative_mode", None),
        "story_focus": story_focus or getattr(project, "default_story_focus", None),
        "plot_stage": plot_stage or getattr(project, "default_plot_stage", None),
        "story_creation_brief": (
            _normalize_optional_text(story_creation_brief)
            or _normalize_optional_text(getattr(project, "default_story_creation_brief", None))
        ),
        "quality_preset": quality_preset or getattr(project, "default_quality_preset", None),
        "quality_notes": (
            _normalize_optional_text(quality_notes)
            or _normalize_optional_text(getattr(project, "default_quality_notes", None))
        ),
    }
