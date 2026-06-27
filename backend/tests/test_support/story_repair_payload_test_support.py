from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any, Dict, Mapping, Optional, Sequence

from tests.test_support.chapter_quality_metrics_query_test_support import (
    build_quality_metrics_summary,
    load_latest_quality_metrics_for_chapter_ids,
    load_recent_previous_chapter_ids,
)
from tests.test_support.schemas.quality import (
    build_quality_gate_decision,
    build_story_repair_guidance,
)
from tests.test_support.task_system.snapshot_runtime_persistence import (
    normalize_runtime_payload as _normalize_runtime_payload,
)


STORY_REPAIR_SOURCE_LABELS: dict[str, str] = {
    "manual_request": "Manual request",
    "current_chapter_quality": "Current chapter quality",
    "recent_history_summary": "Recent history summary",
    "manual_plus_current_chapter_quality": "Manual + current chapter quality",
    "manual_plus_recent_history_summary": "Manual + recent history summary",
}


def _normalize_story_repair_text(value: Optional[str]) -> str:
    return str(value or "").strip()


def _normalize_story_repair_items(
    values: Optional[Sequence[str]],
    *,
    limit: int,
) -> tuple[str, ...]:
    if not values:
        return ()

    normalized: list[str] = []
    seen: set[str] = set()
    for value in values:
        text = _normalize_story_repair_text(value)
        if not text or text in seen:
            continue
        seen.add(text)
        normalized.append(text)
        if len(normalized) >= limit:
            break
    return tuple(normalized)


@dataclass(frozen=True)
class StoryRepairPayload:
    summary: str = ""
    targets: tuple[str, ...] = ()
    strengths: tuple[str, ...] = ()

    def to_prompt_kwargs(self) -> Dict[str, Any]:
        return {
            "story_repair_summary": self.summary or None,
            "story_repair_targets": list(self.targets) or None,
            "story_preserve_strengths": list(self.strengths) or None,
        }


def normalize_story_repair_payload(
    summary: Optional[str] = None,
    targets: Optional[Sequence[str]] = None,
    strengths: Optional[Sequence[str]] = None,
) -> Optional[StoryRepairPayload]:
    normalized_summary = _normalize_story_repair_text(summary)
    normalized_targets = _normalize_story_repair_items(targets, limit=4)
    normalized_strengths = _normalize_story_repair_items(strengths, limit=2)

    if (
        not normalized_summary
        and not normalized_targets
        and not normalized_strengths
    ):
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


def story_repair_payload_to_prompt_kwargs(
    payload: Optional[StoryRepairPayload],
) -> Dict[str, Any]:
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
) -> Dict[str, Any]:
    explicit_payload = normalize_story_repair_payload(
        summary=summary,
        targets=targets,
        strengths=strengths,
    )
    effective_payload = merge_story_repair_payload(explicit_payload, payload)
    return story_repair_payload_to_prompt_kwargs(effective_payload)


def _normalize_story_repair_test_text(value: Optional[str]) -> str:
    return str(value or "").strip()


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
    quality_gate_payload = (
        dict(quality_gate) if isinstance(quality_gate, Mapping) else {}
    )
    summary = (
        payload.summary
        or _normalize_story_repair_test_text(guidance_payload.get("summary"))
        or None
    )
    weakest_metric_key = guidance_payload.get("weakest_metric_key")
    weakest_metric_label = guidance_payload.get("weakest_metric_label")
    weakest_metric_value = guidance_payload.get("weakest_metric_value")
    failed_metric_labels = [
        item.get("label")
        for item in (quality_gate_payload.get("failed_metrics") or [])
        if isinstance(item, Mapping)
        and isinstance(item.get("label"), str)
        and item.get("label")
    ]

    return {
        "summary": summary,
        "repair_targets": list(payload.targets),
        "preserve_strengths": list(payload.strengths),
        "focus_areas": _normalize_guidance_items(
            guidance_payload.get("focus_areas"), limit=4
        ),
        "weakest_metric_key": (
            weakest_metric_key
            if isinstance(weakest_metric_key, str) and weakest_metric_key
            else None
        ),
        "weakest_metric_label": (
            weakest_metric_label
            if isinstance(weakest_metric_label, str) and weakest_metric_label
            else None
        ),
        "weakest_metric_value": (
            weakest_metric_value
            if isinstance(weakest_metric_value, (int, float))
            else None
        ),
        "quality_gate": dict(quality_gate_payload) if quality_gate_payload else None,
        "quality_gate_status": (
            quality_gate_payload.get("status")
            if isinstance(quality_gate_payload.get("status"), str)
            else None
        ),
        "quality_gate_decision": (
            quality_gate_payload.get("decision")
            if isinstance(quality_gate_payload.get("decision"), str)
            else None
        ),
        "quality_gate_label": (
            quality_gate_payload.get("label")
            if isinstance(quality_gate_payload.get("label"), str)
            else None
        ),
        "quality_gate_summary": (
            quality_gate_payload.get("summary")
            if isinstance(quality_gate_payload.get("summary"), str)
            else None
        ),
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


def extract_quality_history_context(
    metrics_summary: Optional[Mapping[str, Any]],
) -> Optional[dict[str, Any]]:
    if not isinstance(metrics_summary, dict):
        return None
    runtime_context = metrics_summary.get("quality_runtime_context")
    if not isinstance(runtime_context, dict) or not runtime_context:
        return None
    return dict(runtime_context)


def attach_story_repair_quality_history(
    state: Optional[dict[str, Any]],
    metrics_summary: Optional[Mapping[str, Any]],
) -> dict[str, Any]:
    normalized_state = dict(state or {})
    normalized_state["quality_metrics_summary"] = (
        _normalize_runtime_payload(metrics_summary)
        if isinstance(metrics_summary, dict) and metrics_summary
        else None
    )
    normalized_state["quality_history_context"] = extract_quality_history_context(
        metrics_summary
    )
    return normalized_state


def resolve_quality_gate_from_metrics(
    metrics: Optional[Mapping[str, Any]],
    *,
    scope: str = "chapter",
    prefer_embedded_quality_gate: bool = True,
) -> Optional[dict[str, Any]]:
    if not isinstance(metrics, dict):
        return None

    quality_gate = metrics.get("quality_gate") if prefer_embedded_quality_gate else None
    if not isinstance(quality_gate, dict):
        derived_quality_gate = build_quality_gate_decision(metrics, scope=scope)
        quality_gate = (
            derived_quality_gate if isinstance(derived_quality_gate, dict) else None
        )

    return dict(quality_gate) if isinstance(quality_gate, dict) else None


def resolve_story_repair_guidance_from_metrics(
    metrics: Optional[Mapping[str, Any]],
    *,
    scope: str = "chapter",
) -> Optional[dict[str, Any]]:
    if not isinstance(metrics, dict):
        return None

    guidance = metrics.get("repair_guidance")
    if not isinstance(guidance, dict):
        derived_guidance = build_story_repair_guidance(metrics, scope=scope)
        guidance = derived_guidance if isinstance(derived_guidance, dict) else None

    return dict(guidance) if isinstance(guidance, dict) else None


def build_batch_quality_metrics_summary(
    history: list[dict[str, Any]],
) -> Optional[dict[str, Any]]:
    return build_quality_metrics_summary(history, scope="batch")


def restore_story_repair_payload_from_active_snapshot(
    active_story_repair_payload: Optional[Mapping[str, Any]],
) -> Optional[StoryRepairPayload]:
    if not isinstance(active_story_repair_payload, dict):
        return None
    return normalize_story_repair_payload(
        summary=active_story_repair_payload.get("summary"),
        targets=active_story_repair_payload.get("repair_targets"),
        strengths=active_story_repair_payload.get("preserve_strengths"),
    )


async def resolve_generation_story_repair_state_for_batch(*args, **kwargs):
    db_session = args[0] if args else kwargs.pop("db_session")
    explicit_payload = merge_story_repair_payload(
        normalize_story_repair_payload(
            kwargs.get("story_repair_summary"),
            kwargs.get("story_repair_targets"),
            kwargs.get("story_preserve_strengths"),
        ),
        restore_story_repair_payload_from_active_snapshot(
            kwargs.get("active_story_repair_payload")
        ),
    )

    previous_chapter_ids = await load_recent_previous_chapter_ids(
        db_session,
        project_id=kwargs["project_id"],
        before_chapter_number=kwargs["before_chapter_number"],
        limit=3,
    )
    previous_metrics = await load_latest_quality_metrics_for_chapter_ids(
        db_session, previous_chapter_ids
    )
    if not previous_metrics:
        return attach_story_repair_quality_history(
            build_story_repair_runtime_state(
                explicit_payload=explicit_payload,
                derived_payload=None,
                scope="batch",
                derived_source=None,
            ),
            None,
        )

    summary_metrics = build_batch_quality_metrics_summary(previous_metrics)
    derived_payload = (
        build_story_repair_payload_from_metrics(
            summary_metrics,
            scope="batch",
            prefer_embedded_guidance=False,
        )
        if summary_metrics
        else None
    )
    derived_guidance = (
        resolve_story_repair_guidance_from_metrics(
            summary_metrics,
            scope="batch",
        )
        if summary_metrics
        else None
    )
    derived_quality_gate = (
        resolve_quality_gate_from_metrics(
            summary_metrics,
            scope="batch",
            prefer_embedded_quality_gate=False,
        )
        if summary_metrics
        else None
    )
    return attach_story_repair_quality_history(
        build_story_repair_runtime_state(
            explicit_payload=explicit_payload,
            derived_payload=derived_payload,
            scope="batch",
            derived_source="recent_history_summary",
            guidance=derived_guidance,
            quality_gate=derived_quality_gate,
        ),
        summary_metrics,
    )


async def resolve_generation_story_repair_state_for_chapter(*args, **kwargs):
    db_session = args[0] if args else kwargs.pop("db_session")
    chapter = kwargs["chapter"]
    explicit_payload = normalize_story_repair_payload(
        kwargs.get("story_repair_summary"),
        kwargs.get("story_repair_targets"),
        kwargs.get("story_preserve_strengths"),
    )

    current_metrics = await load_latest_quality_metrics_for_chapter_ids(
        db_session, [chapter.id]
    )
    if current_metrics:
        derived_payload = build_story_repair_payload_from_metrics(
            current_metrics[0], scope="chapter"
        )
        derived_guidance = resolve_story_repair_guidance_from_metrics(
            current_metrics[0], scope="chapter"
        )
        derived_quality_gate = resolve_quality_gate_from_metrics(
            current_metrics[0], scope="chapter"
        )
        current_summary_metrics = build_batch_quality_metrics_summary(current_metrics)
        return attach_story_repair_quality_history(
            build_story_repair_runtime_state(
                explicit_payload=explicit_payload,
                derived_payload=derived_payload,
                scope="chapter",
                derived_source="current_chapter_quality",
                guidance=derived_guidance,
                quality_gate=derived_quality_gate,
            ),
            current_summary_metrics,
        )

    previous_chapter_ids = await load_recent_previous_chapter_ids(
        db_session,
        project_id=chapter.project_id,
        before_chapter_number=chapter.chapter_number,
        limit=3,
    )
    previous_metrics = await load_latest_quality_metrics_for_chapter_ids(
        db_session, previous_chapter_ids
    )
    if not previous_metrics:
        return attach_story_repair_quality_history(
            build_story_repair_runtime_state(
                explicit_payload=explicit_payload,
                derived_payload=None,
                scope="chapter",
                derived_source=None,
            ),
            None,
        )

    summary_metrics = build_batch_quality_metrics_summary(previous_metrics)
    derived_payload = (
        build_story_repair_payload_from_metrics(
            summary_metrics,
            scope="chapter",
            prefer_embedded_guidance=False,
        )
        if summary_metrics
        else None
    )
    derived_guidance = (
        resolve_story_repair_guidance_from_metrics(
            summary_metrics,
            scope="chapter",
        )
        if summary_metrics
        else None
    )
    derived_quality_gate = (
        resolve_quality_gate_from_metrics(
            summary_metrics,
            scope="chapter",
            prefer_embedded_quality_gate=False,
        )
        if summary_metrics
        else None
    )
    return attach_story_repair_quality_history(
        build_story_repair_runtime_state(
            explicit_payload=explicit_payload,
            derived_payload=derived_payload,
            scope="chapter",
            derived_source="recent_history_summary",
            guidance=derived_guidance,
            quality_gate=derived_quality_gate,
        ),
        summary_metrics,
    )


def resolve_quality_gate_execution_plan(
    quality_metrics: Optional[dict[str, Any]],
    *,
    retry_count: int,
    max_retries: int,
    current_story_repair_payload: Optional[StoryRepairPayload],
    scope: str,
) -> dict[str, Any]:
    quality_gate = resolve_quality_gate_from_metrics(
        quality_metrics,
        scope=scope,
        prefer_embedded_quality_gate=False,
    )
    if not quality_gate:
        return {
            "action": "continue",
            "message": None,
            "repair_payload": current_story_repair_payload,
            "active_story_repair_payload": None,
            "quality_gate": None,
        }

    derived_payload = (
        build_story_repair_payload_from_metrics(quality_metrics, scope=scope)
        if quality_metrics
        else None
    )
    derived_guidance = (
        resolve_story_repair_guidance_from_metrics(quality_metrics, scope=scope)
        if quality_metrics
        else None
    )
    repair_state = build_story_repair_runtime_state(
        explicit_payload=current_story_repair_payload,
        derived_payload=derived_payload,
        scope=scope,
        derived_source="current_chapter_quality",
        guidance=derived_guidance,
        quality_gate=quality_gate,
    )
    repair_payload = repair_state["payload"]
    active_story_repair_payload = repair_state["active_story_repair_payload"]

    decision = str(quality_gate.get("decision") or "").strip()
    label = str(quality_gate.get("label") or "Quality gate").strip()
    summary = str(quality_gate.get("summary") or "").strip()
    reason = str(quality_gate.get("reason") or "").strip()
    weakest_metric_label = str(quality_gate.get("weakest_metric_label") or "").strip()
    weakest_metric_value = quality_gate.get("weakest_metric_value")

    weakest_metric_hint = (
        f"Weakest metric: {weakest_metric_label}" if weakest_metric_label else ""
    )
    if weakest_metric_hint and isinstance(weakest_metric_value, (int, float)):
        weakest_metric_hint = f"{weakest_metric_hint} ({weakest_metric_value:.1f})"

    recommended_action_label = str(
        quality_gate.get("recommended_action_label") or ""
    ).strip()
    recommended_action_key = str(
        quality_gate.get("recommended_action") or ""
    ).strip()
    recommended_action_hint = ""
    if recommended_action_label:
        recommended_action_hint = (
            f"Recommended repair action: {recommended_action_label}"
        )
    elif recommended_action_key:
        recommended_action_hint = (
            f"Recommended repair action: {recommended_action_key}"
        )

    if decision == "manual_review":
        message = f"{label}: {summary or 'Manual review is required for this chapter.'}"
        if reason:
            message = f"{message} Reason: {reason}"
        if recommended_action_hint:
            message = f"{message} {recommended_action_hint}"
        return {
            "action": "manual_review",
            "message": message,
            "repair_payload": repair_payload,
            "active_story_repair_payload": active_story_repair_payload,
            "quality_gate": quality_gate,
        }

    if decision == "auto_repair":
        if retry_count < max_retries:
            message = (
                f"{label}: "
                f"{summary or 'Repairable weaknesses detected; retrying with stronger repair guidance.'}"
            )
            if weakest_metric_hint:
                message = f"{message} {weakest_metric_hint}"
            if recommended_action_hint:
                message = f"{message} {recommended_action_hint}"
            return {
                "action": "retry",
                "message": message,
                "repair_payload": repair_payload,
                "active_story_repair_payload": active_story_repair_payload,
                "quality_gate": quality_gate,
            }

        message = (
            f"{label}: "
            f"{summary or 'Repairable weaknesses remain, but retry budget is exhausted.'}"
        )
        if weakest_metric_hint:
            message = f"{message} {weakest_metric_hint}"
        if recommended_action_hint:
            message = f"{message} {recommended_action_hint}"
        return {
            "action": "manual_review",
            "message": message,
            "repair_payload": repair_payload,
            "active_story_repair_payload": active_story_repair_payload,
            "quality_gate": quality_gate,
        }

    return {
        "action": "continue",
        "message": None,
        "repair_payload": repair_payload,
        "active_story_repair_payload": active_story_repair_payload,
        "quality_gate": quality_gate,
    }

