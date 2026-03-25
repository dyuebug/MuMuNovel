from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
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


def _normalize_guidance_items(values: Any, *, limit: int = 4) -> list[str]:
    if not isinstance(values, list):
        return []

    items: list[str] = []
    seen: set[str] = set()
    for value in values:
        text = str(value or "").strip()
        if not text or text in seen:
            continue
        seen.add(text)
        items.append(text)
        if len(items) >= limit:
            break
    return items


def normalize_story_repair_payload(
    summary: Optional[str] = None,
    targets: Optional[Sequence[str]] = None,
    strengths: Optional[Sequence[str]] = None,
) -> Optional[StoryRepairPayload]:
    normalized_summary = _normalize_text(summary)
    normalized_targets = _normalize_items(targets, limit=4)
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


def story_repair_payload_to_prompt_kwargs(payload: Optional[StoryRepairPayload]) -> dict[str, Any]:
    if payload is None:
        return {
            "story_repair_summary": None,
            "story_repair_targets": None,
            "story_preserve_strengths": None,
        }
    return payload.to_prompt_kwargs()


def resolve_story_repair_prompt_kwargs(
    payload: Optional[StoryRepairPayload],
    *,
    summary: Optional[str] = None,
    targets: Optional[Sequence[str]] = None,
    strengths: Optional[Sequence[str]] = None,
) -> dict[str, Any]:
    explicit_payload = normalize_story_repair_payload(
        summary=summary,
        targets=targets,
        strengths=strengths,
    )
    effective_payload = merge_story_repair_payload(explicit_payload, payload)
    return story_repair_payload_to_prompt_kwargs(effective_payload)


STORY_REPAIR_SOURCE_LABELS: dict[str, str] = {
    "manual_request": "Manual request",
    "current_chapter_quality": "Current chapter quality",
    "recent_history_summary": "Recent history summary",
    "manual_plus_current_chapter_quality": "Manual + current chapter quality",
    "manual_plus_recent_history_summary": "Manual + recent history summary",
}


def resolve_story_repair_runtime_source(
    *,
    explicit_payload: Optional[StoryRepairPayload],
    derived_payload: Optional[StoryRepairPayload],
    derived_source: Optional[str],
) -> Optional[str]:
    if explicit_payload and derived_payload:
        if derived_source == "current_chapter_quality":
            return "manual_plus_current_chapter_quality"
        if derived_source == "recent_history_summary":
            return "manual_plus_recent_history_summary"
    if explicit_payload:
        return "manual_request"
    if derived_payload:
        return derived_source
    return None


def build_story_repair_runtime_snapshot(
    payload: Optional[StoryRepairPayload],
    *,
    scope: str,
    source: Optional[str],
    guidance: Optional[Mapping[str, Any]] = None,
    quality_gate: Optional[Mapping[str, Any]] = None,
) -> Optional[dict[str, Any]]:
    if payload is None or not source:
        return None

    guidance_payload = dict(guidance) if isinstance(guidance, Mapping) else {}
    quality_gate_payload = dict(quality_gate) if isinstance(quality_gate, Mapping) else {}
    summary = payload.summary or _normalize_text(guidance_payload.get("summary")) or None
    weakest_metric_key = guidance_payload.get("weakest_metric_key")
    weakest_metric_label = guidance_payload.get("weakest_metric_label")
    weakest_metric_value = guidance_payload.get("weakest_metric_value")
    failed_metric_labels = [
        item.get("label")
        for item in (quality_gate_payload.get("failed_metrics") or [])
        if isinstance(item, Mapping) and isinstance(item.get("label"), str) and item.get("label")
    ]

    return {
        "summary": summary,
        "repair_targets": list(payload.targets),
        "preserve_strengths": list(payload.strengths),
        "focus_areas": _normalize_guidance_items(guidance_payload.get("focus_areas"), limit=4),
        "weakest_metric_key": weakest_metric_key if isinstance(weakest_metric_key, str) and weakest_metric_key else None,
        "weakest_metric_label": weakest_metric_label if isinstance(weakest_metric_label, str) and weakest_metric_label else None,
        "weakest_metric_value": weakest_metric_value if isinstance(weakest_metric_value, (int, float)) else None,
        "quality_gate": dict(quality_gate_payload) if quality_gate_payload else None,
        "quality_gate_status": quality_gate_payload.get("status") if isinstance(quality_gate_payload.get("status"), str) else None,
        "quality_gate_decision": quality_gate_payload.get("decision") if isinstance(quality_gate_payload.get("decision"), str) else None,
        "quality_gate_label": quality_gate_payload.get("label") if isinstance(quality_gate_payload.get("label"), str) else None,
        "quality_gate_summary": quality_gate_payload.get("summary") if isinstance(quality_gate_payload.get("summary"), str) else None,
        "quality_gate_failed_metrics": failed_metric_labels,
        "source": source,
        "source_label": STORY_REPAIR_SOURCE_LABELS.get(source, source),
        "scope": scope,
        "updated_at": datetime.now().isoformat(),
    }


def build_story_repair_runtime_state(
    *,
    explicit_payload: Optional[StoryRepairPayload],
    derived_payload: Optional[StoryRepairPayload],
    scope: str,
    derived_source: Optional[str],
    guidance: Optional[Mapping[str, Any]] = None,
    quality_gate: Optional[Mapping[str, Any]] = None,
) -> dict[str, Any]:
    payload = merge_story_repair_payload(explicit_payload, derived_payload)
    source = resolve_story_repair_runtime_source(
        explicit_payload=explicit_payload,
        derived_payload=derived_payload,
        derived_source=derived_source,
    )
    return {
        "payload": payload,
        "active_story_repair_payload": build_story_repair_runtime_snapshot(
            payload,
            scope=scope,
            source=source,
            guidance=guidance,
            quality_gate=quality_gate,
        ),
    }
