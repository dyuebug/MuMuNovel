from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Optional, Sequence

from app.services.story_quality_feedback_service import build_story_repair_guidance


@dataclass(frozen=True)
class StoryRepairPayload:
    summary: str = ""
    targets: tuple[str, ...] = ()
    strengths: tuple[str, ...] = ()

    def to_prompt_kwargs(self) -> dict[str, Any]:
        return {
            "story_repair_summary": self.summary or None,
            "story_repair_targets": list(self.targets) or None,
            "story_preserve_strengths": list(self.strengths) or None,
        }


def _normalize_text(value: Optional[str]) -> str:
    return str(value or "").strip()


def _normalize_items(values: Optional[Sequence[str]], *, limit: int) -> tuple[str, ...]:
    if not values:
        return ()

    normalized: list[str] = []
    seen: set[str] = set()
    for value in values:
        text = _normalize_text(value)
        if not text or text in seen:
            continue
        seen.add(text)
        normalized.append(text)
        if len(normalized) >= limit:
            break
    return tuple(normalized)


def normalize_story_repair_payload(
    summary: Optional[str] = None,
    targets: Optional[Sequence[str]] = None,
    strengths: Optional[Sequence[str]] = None,
) -> Optional[StoryRepairPayload]:
    normalized_summary = _normalize_text(summary)
    normalized_targets = _normalize_items(targets, limit=3)
    normalized_strengths = _normalize_items(strengths, limit=2)

    if not normalized_summary and not normalized_targets and not normalized_strengths:
        return None

    return StoryRepairPayload(
        summary=normalized_summary,
        targets=normalized_targets,
        strengths=normalized_strengths,
    )


def merge_story_repair_payload(
    primary: Optional[StoryRepairPayload],
    fallback: Optional[StoryRepairPayload],
) -> Optional[StoryRepairPayload]:
    if primary is None:
        return fallback
    if fallback is None:
        return primary

    return normalize_story_repair_payload(
        summary=primary.summary or fallback.summary,
        targets=primary.targets or fallback.targets,
        strengths=primary.strengths or fallback.strengths,
    )


def build_story_repair_payload_from_guidance(
    guidance: Optional[Mapping[str, Any]],
) -> Optional[StoryRepairPayload]:
    if not isinstance(guidance, Mapping):
        return None

    return normalize_story_repair_payload(
        summary=guidance.get("summary"),
        targets=guidance.get("repair_targets"),
        strengths=guidance.get("preserve_strengths"),
    )


def build_story_repair_payload_from_metrics(
    metrics: Optional[Mapping[str, Any]],
    *,
    scope: str = "chapter",
    prefer_embedded_guidance: bool = True,
) -> Optional[StoryRepairPayload]:
    if not isinstance(metrics, Mapping):
        return None

    guidance = metrics.get("repair_guidance") if prefer_embedded_guidance else None
    if not isinstance(guidance, Mapping):
        guidance = build_story_repair_guidance(metrics, scope=scope)

    return build_story_repair_payload_from_guidance(guidance)
