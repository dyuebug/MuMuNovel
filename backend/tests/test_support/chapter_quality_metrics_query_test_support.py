from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any, Mapping, Optional, Sequence

from tests.test_support.schemas.novel_quality_profile_service import (
    QUALITY_FOCUS_LABELS,
    resolve_quality_weight_profile,
)
from tests.test_support.schemas.quality import (
    QUALITY_STAGE_LABELS,
    _extract_quality_runtime_context,
    _normalize_runtime_context_items,
    _normalize_runtime_context_item_texts,
    _normalize_runtime_items,
    _resolve_quality_stage,
    _safe_float,
    build_quality_gate_decision,
    build_story_repair_guidance,
)
if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession


def _build_repair_effectiveness_summary(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str,
) -> Dict[str, Any]:
    metric_map: Dict[str, tuple[str, float, float]] = {
        "conflict": ("conflict_chain_hit_rate", 72.0, 3.0),
        "outline": ("outline_alignment_rate", 72.0, 3.0),
        "pacing": ("pacing_score", 7.2, 0.4),
        "payoff": ("payoff_chain_rate", 72.0, 3.0),
        "cliffhanger": ("cliffhanger_rate", 74.0, 3.0),
        "dialogue": ("dialogue_naturalness_rate", 74.0, 3.0),
        "rule_grounding": ("rule_grounding_hit_rate", 72.0, 3.0),
        "opening": ("opening_hook_rate", 72.0, 3.0),
        "foreshadow_continuity": ("payoff_chain_rate", 72.0, 3.0),
        "relationship_continuity": ("dialogue_naturalness_rate", 74.0, 3.0),
        "character_continuity": ("dialogue_naturalness_rate", 74.0, 3.0),
        "organization_continuity": ("rule_grounding_hit_rate", 72.0, 3.0),
        "career_continuity": ("rule_grounding_hit_rate", 72.0, 3.0),
    }

    normalized_history = [
        _normalize_quality_metrics_history_item(item, scope=scope)
        for item in history
        if isinstance(item, Mapping) and item
    ]
    if len(normalized_history) < 2:
        return {}

    evaluated_pairs = 0
    successful_pairs = 0
    focus_area_state: Dict[str, Dict[str, Any]] = {}

    for current_item, next_item in zip(normalized_history, normalized_history[1:]):
        guidance = (
            current_item.get("repair_guidance")
            if isinstance(current_item.get("repair_guidance"), Mapping)
            else build_story_repair_guidance(current_item, scope=scope)
        )
        focus_areas = _normalize_runtime_items(guidance.get("focus_areas"), limit=4)
        pair_evaluations = []
        for focus_area in focus_areas:
            metric_spec = metric_map.get(focus_area)
            if metric_spec is None:
                continue
            metric_key, safe_threshold, improvement_threshold = metric_spec
            current_value = _safe_float(current_item.get(metric_key))
            next_value = _safe_float(next_item.get(metric_key))
            if current_value is None or next_value is None:
                continue
            delta = round(next_value - current_value, 1)
            success = next_value >= current_value + improvement_threshold or (
                current_value < safe_threshold <= next_value
            )
            pair_evaluations.append(
                {
                    "focus_area": focus_area,
                    "metric_key": metric_key,
                    "delta": delta,
                    "success": success,
                }
            )
            state = focus_area_state.setdefault(
                focus_area,
                {
                    "focus_area": focus_area,
                    "label": QUALITY_FOCUS_LABELS.get(focus_area, focus_area),
                    "metric_key": metric_key,
                    "evaluated_pairs": 0,
                    "successful_pairs": 0,
                    "delta_total": 0.0,
                },
            )
            state["evaluated_pairs"] += 1
            state["delta_total"] = round((_safe_float(state.get("delta_total")) or 0.0) + delta, 6)
            if success:
                state["successful_pairs"] += 1

        if not pair_evaluations:
            continue

        evaluated_pairs += 1
        pair_success_count = sum(1 for item in pair_evaluations if item["success"])
        if pair_success_count >= max(1, (len(pair_evaluations) + 1) // 2):
            successful_pairs += 1

    if evaluated_pairs <= 0:
        return {}

    success_rate = round(successful_pairs / evaluated_pairs * 100, 1)
    focus_area_stats = []
    for focus_area, state in focus_area_state.items():
        area_pairs = int(state.get("evaluated_pairs") or 0)
        if area_pairs <= 0:
            continue
        area_successful_pairs = int(state.get("successful_pairs") or 0)
        focus_area_stats.append(
            {
                "focus_area": focus_area,
                "label": state.get("label") or focus_area,
                "metric_key": state.get("metric_key"),
                "evaluated_pairs": area_pairs,
                "successful_pairs": area_successful_pairs,
                "success_rate": round(area_successful_pairs / area_pairs * 100, 1),
                "avg_delta": round((_safe_float(state.get("delta_total")) or 0.0) / area_pairs, 1),
            }
        )

    focus_area_stats.sort(
        key=lambda item: (
            float(item.get("success_rate") or 0.0),
            -int(item.get("evaluated_pairs") or 0),
            str(item.get("label") or ""),
        )
    )

    recovered_focus_areas = [
        str(item.get("label") or "").strip()
        for item in focus_area_stats
        if (item.get("success_rate") or 0.0) >= 60.0 and (item.get("avg_delta") or 0.0) > 0.0
    ][:3]
    unresolved_focus_areas = [
        str(item.get("label") or "").strip()
        for item in focus_area_stats
        if (item.get("success_rate") or 0.0) < 50.0
    ][:3]

    summary_text = f"最近 {evaluated_pairs} 组相邻章节中，修复成效率约 {success_rate:.1f}%。"
    if recovered_focus_areas:
        summary_text += f" 已开始回收：{' / '.join(recovered_focus_areas[:2])}。"
    if unresolved_focus_areas:
        summary_text += f" 仍需盯住：{' / '.join(unresolved_focus_areas[:2])}。"

    status = "stable"
    if success_rate < 40.0:
        status = "warning"
    elif success_rate < 65.0:
        status = "watch"

    return {
        "status": status,
        "success_rate": success_rate,
        "evaluated_pairs": evaluated_pairs,
        "successful_pairs": successful_pairs,
        "recovered_focus_areas": _normalize_runtime_items(recovered_focus_areas, limit=3),
        "unresolved_focus_areas": _normalize_runtime_items(unresolved_focus_areas, limit=3),
        "focus_area_stats": focus_area_stats,
        "summary": summary_text,
    }


def _parse_quality_metrics_from_history(
    generated_content: Optional[str],
) -> Optional[dict[str, Any]]:
    return extract_quality_metrics_from_history_payload(
        generated_content,
        scope="chapter",
    )


def _extract_history_runtime_snapshot(payload: Mapping[str, Any]) -> Dict[str, Any]:
    if not isinstance(payload, Mapping):
        return {}
    runtime_snapshot = payload.get("story_runtime_snapshot")
    if isinstance(runtime_snapshot, Mapping):
        return dict(runtime_snapshot)
    runtime_contract = payload.get("story_runtime_contract")
    from tests.test_support.schemas.generation_payload import (
        extract_story_runtime_snapshot_from_contract,
    )

    extracted_snapshot = (
        extract_story_runtime_snapshot_from_contract(runtime_contract)
        if isinstance(runtime_contract, Mapping)
        else None
    )
    return dict(extracted_snapshot) if isinstance(extracted_snapshot, Mapping) else {}


def extract_quality_metrics_from_history_payload(
    generated_content: Optional[str],
    *,
    scope: str = "chapter",
) -> Optional[dict[str, Any]]:
    """从 generation_history.generated_content 中提取质量指标。"""
    if not generated_content:
        return None

    try:
        payload = json.loads(generated_content)
    except Exception:
        return None

    if not isinstance(payload, dict):
        return None

    metrics = payload.get("quality_metrics")
    if not isinstance(metrics, Mapping):
        return None

    normalized_metrics = dict(metrics)
    runtime_snapshot = _extract_history_runtime_snapshot(payload)
    existing_runtime_context = normalized_metrics.get("quality_runtime_context")
    if runtime_snapshot:
        if isinstance(existing_runtime_context, Mapping):
            merged_runtime_context = dict(runtime_snapshot)
            merged_runtime_context.update(dict(existing_runtime_context))
            normalized_metrics["quality_runtime_context"] = merged_runtime_context
        else:
            normalized_metrics["quality_runtime_context"] = dict(runtime_snapshot)
    if not isinstance(normalized_metrics.get("repair_guidance"), dict):
        normalized_metrics["repair_guidance"] = build_story_repair_guidance(
            normalized_metrics,
            scope=scope,
        )
    if not isinstance(normalized_metrics.get("quality_gate"), dict):
        normalized_metrics["quality_gate"] = build_quality_gate_decision(
            normalized_metrics,
            scope=scope,
        )
    return normalized_metrics


def build_quality_metrics_summary_state(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str = "batch",
) -> Optional[dict[str, Any]]:
    normalized_history = [
        _normalize_quality_metrics_history_item(item, scope=scope)
        for item in history
        if isinstance(item, Mapping) and item
    ]
    if not normalized_history:
        return None

    state: dict[str, Any] = {
        "chapter_count": len(normalized_history),
        "first_overall_score": _coerce_metric_float(normalized_history[0].get("overall_score")),
        "last_overall_score": _coerce_metric_float(normalized_history[-1].get("overall_score")),
        "recent_history": [dict(item) for item in normalized_history[-5:]],
        "pacing_score_total": 0.0,
        "pacing_score_count": 0,
    }
    for metric_key, _avg_key in _SUMMARY_METRIC_FIELDS:
        state[f"{metric_key}_total"] = sum(
            _coerce_metric_float(item.get(metric_key))
            for item in normalized_history
        )

    pacing_values = [
        _coerce_metric_float(item.get("pacing_score"))
        for item in normalized_history
        if item.get("pacing_score") is not None
    ]
    if pacing_values:
        state["pacing_score_total"] = sum(pacing_values)
        state["pacing_score_count"] = len(pacing_values)
    return state


def advance_quality_metrics_summary_state(
    summary_state: Optional[Mapping[str, Any]],
    *,
    appended_event: Mapping[str, Any],
    current_history: Sequence[Mapping[str, Any]],
    dropped_event: Optional[Mapping[str, Any]] = None,
    scope: str = "batch",
) -> Optional[dict[str, Any]]:
    normalized_history = [
        _normalize_quality_metrics_history_item(item, scope=scope)
        for item in current_history
        if isinstance(item, Mapping) and item
    ]
    if not normalized_history:
        return None
    if not isinstance(summary_state, Mapping):
        return build_quality_metrics_summary_state(normalized_history, scope=scope)

    state = dict(summary_state)
    normalized_appended = _normalize_quality_metrics_history_item(appended_event, scope=scope)
    normalized_dropped = (
        _normalize_quality_metrics_history_item(dropped_event, scope=scope)
        if isinstance(dropped_event, Mapping) and dropped_event
        else None
    )

    for metric_key, _avg_key in _SUMMARY_METRIC_FIELDS:
        total_key = f"{metric_key}_total"
        updated_total = _coerce_metric_float(state.get(total_key)) + _coerce_metric_float(
            normalized_appended.get(metric_key)
        )
        if normalized_dropped is not None:
            updated_total -= _coerce_metric_float(normalized_dropped.get(metric_key))
        state[total_key] = round(updated_total, 6)

    pacing_total = _coerce_metric_float(state.get("pacing_score_total"))
    pacing_count = int(state.get("pacing_score_count") or 0)
    if normalized_appended.get("pacing_score") is not None:
        pacing_total += _coerce_metric_float(normalized_appended.get("pacing_score"))
        pacing_count += 1
    if normalized_dropped is not None and normalized_dropped.get("pacing_score") is not None:
        pacing_total -= _coerce_metric_float(normalized_dropped.get("pacing_score"))
        pacing_count = max(0, pacing_count - 1)
    state["pacing_score_total"] = round(pacing_total, 6)
    state["pacing_score_count"] = pacing_count
    state["chapter_count"] = len(normalized_history)
    state["first_overall_score"] = _coerce_metric_float(normalized_history[0].get("overall_score"))
    state["last_overall_score"] = _coerce_metric_float(normalized_history[-1].get("overall_score"))
    state["recent_history"] = [dict(item) for item in normalized_history[-5:]]
    return state


def build_quality_metrics_summary_from_state(
    summary_state: Optional[Mapping[str, Any]],
    *,
    scope: str = "batch",
) -> Optional[dict[str, Any]]:
    if not isinstance(summary_state, Mapping):
        return None

    chapter_count = int(summary_state.get("chapter_count") or 0)
    if chapter_count <= 0:
        return None

    recent_history = [
        _normalize_quality_metrics_history_item(item, scope=scope)
        for item in (summary_state.get("recent_history") or [])
        if isinstance(item, Mapping) and item
    ]
    trend_delta = (
        round(
            _coerce_metric_float(summary_state.get("last_overall_score"))
            - _coerce_metric_float(summary_state.get("first_overall_score")),
            1,
        )
        if chapter_count > 1
        else 0.0
    )
    if trend_delta >= 2.0:
        trend_direction = "rising"
    elif trend_delta <= -2.0:
        trend_direction = "falling"
    else:
        trend_direction = "stable"

    recent_focus_areas: list[str] = []
    seen_focus: set[str] = set()
    for item in recent_history[-3:]:
        guidance = (
            item.get("repair_guidance")
            if isinstance(item.get("repair_guidance"), Mapping)
            else build_story_repair_guidance(item, scope=scope)
        )
        for area in guidance.get("focus_areas") or []:
            if area in seen_focus:
                continue
            seen_focus.add(area)
            recent_focus_areas.append(area)
            if len(recent_focus_areas) >= 4:
                break
        if len(recent_focus_areas) >= 4:
            break

    recent_failed_metric_counts = _collect_recent_failed_metric_counts(recent_history, scope=scope)
    quality_gate_counts, recent_manual_review_count, recent_auto_repair_count = _collect_recent_quality_gate_counts(
        recent_history,
        scope=scope,
    )

    summary: dict[str, Any] = {
        "chapter_count": chapter_count,
        "overall_score_delta": trend_delta,
        "overall_score_trend": trend_direction,
        "recent_focus_areas": recent_focus_areas,
        "recent_failed_metric_counts": recent_failed_metric_counts,
        "quality_gate_counts": quality_gate_counts,
        "recent_manual_review_count": recent_manual_review_count,
        "recent_auto_repair_count": recent_auto_repair_count,
        "avg_pacing_score": (
            round(
                _coerce_metric_float(summary_state.get("pacing_score_total"))
                / int(summary_state.get("pacing_score_count") or 1),
                1,
            )
            if int(summary_state.get("pacing_score_count") or 0) > 0
            else None
        ),
    }
    for metric_key, avg_key in _SUMMARY_METRIC_FIELDS:
        summary[avg_key] = round(
            _coerce_metric_float(summary_state.get(f"{metric_key}_total"))
            / max(chapter_count, 1),
            1,
        )

    runtime_context = _aggregate_quality_runtime_context(recent_history)
    if runtime_context:
        summary["quality_runtime_context"] = runtime_context
    continuity_preflight = _aggregate_continuity_preflight(recent_history)
    if continuity_preflight:
        summary["continuity_preflight"] = continuity_preflight
    pacing_imbalance = _build_pacing_imbalance_summary(recent_history)
    if pacing_imbalance:
        summary["pacing_imbalance"] = pacing_imbalance
    volume_goal_completion = _build_volume_goal_completion_summary(summary)
    if volume_goal_completion:
        summary["volume_goal_completion"] = volume_goal_completion
    foreshadow_payoff_delay = _build_foreshadow_payoff_delay_summary(summary)
    if foreshadow_payoff_delay:
        summary["foreshadow_payoff_delay"] = foreshadow_payoff_delay
    repair_effectiveness = _build_repair_effectiveness_summary(
        recent_history,
        scope=scope,
    )
    if repair_effectiveness:
        summary["repair_effectiveness"] = repair_effectiveness
    summary["repair_guidance"] = build_story_repair_guidance(summary, scope=scope)
    summary["quality_gate"] = build_quality_gate_decision(summary, scope=scope)
    return summary


def build_quality_metrics_summary(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str = "batch",
) -> Optional[dict[str, Any]]:
    summary_state = build_quality_metrics_summary_state(history, scope=scope)
    return build_quality_metrics_summary_from_state(summary_state, scope=scope)


def _collect_recent_failed_metric_counts(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str,
) -> list[dict[str, Any]]:
    counts: dict[str, dict[str, Any]] = {}
    for item in history:
        gate = (
            item.get("quality_gate")
            if isinstance(item.get("quality_gate"), Mapping)
            else build_quality_gate_decision(item, scope=scope)
        )
        for metric in gate.get("failed_metrics") or []:
            key = str(metric.get("key") or metric.get("label") or "").strip()
            if not key:
                continue
            entry = counts.setdefault(
                key,
                {
                    "key": key,
                    "label": metric.get("label") or key,
                    "focus_area": metric.get("focus_area"),
                    "count": 0,
                },
            )
            entry["count"] += 1

    return sorted(
        counts.values(),
        key=lambda item: (
            -int(item.get("count") or 0),
            str(item.get("label") or item.get("key") or ""),
        ),
    )[:6]


def _collect_recent_quality_gate_counts(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str,
) -> tuple[dict[str, int], int, int]:
    gate_counts: dict[str, int] = {"pass": 0, "repairable": 0, "blocked": 0, "unknown": 0}
    manual_review_count = 0
    auto_repair_count = 0

    for item in history:
        gate = (
            item.get("quality_gate")
            if isinstance(item.get("quality_gate"), Mapping)
            else build_quality_gate_decision(item, scope=scope)
        )
        status = str(gate.get("status") or "unknown")
        gate_counts[status] = gate_counts.get(status, 0) + 1

        decision = str(gate.get("decision") or "")
        if decision == "manual_review":
            manual_review_count += 1
        elif decision == "auto_repair":
            auto_repair_count += 1

    return gate_counts, manual_review_count, auto_repair_count


def _aggregate_quality_runtime_context(history: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    contexts = [
        _extract_quality_runtime_context(item)
        for item in history
        if isinstance(item, Mapping) and _extract_quality_runtime_context(item)
    ]
    if not contexts:
        return {}

    contexts.sort(
        key=lambda item: (
            _safe_float(item.get("current_chapter_number")) or 0.0,
            _safe_float(item.get("chapter_count")) or 0.0,
        )
    )
    latest = contexts[-1]
    merged = dict(latest)
    merged["character_focus"] = _normalize_runtime_items(
        [
            entry
            for ctx in contexts[-3:]
            for entry in _normalize_runtime_items(ctx.get("character_focus"), limit=4)
        ],
        limit=4,
    )
    merged["foreshadow_payoff_plan"] = _normalize_runtime_context_items(
        [
            entry
            for ctx in contexts[-3:]
            for entry in _normalize_runtime_context_items(
                ctx.get("foreshadow_payoff_plan"),
                limit=6,
            )
        ],
        limit=6,
    )
    merged["character_state_ledger"] = _normalize_runtime_context_items(
        [
            entry
            for ctx in contexts[-3:]
            for entry in _normalize_runtime_context_items(
                ctx.get("character_state_ledger"),
                limit=4,
            )
        ],
        limit=4,
    )
    merged["relationship_state_ledger"] = _normalize_runtime_context_items(
        [
            entry
            for ctx in contexts[-3:]
            for entry in _normalize_runtime_context_items(
                ctx.get("relationship_state_ledger"),
                limit=4,
            )
        ],
        limit=4,
    )
    merged["foreshadow_state_ledger"] = _normalize_runtime_context_items(
        [
            entry
            for ctx in contexts[-3:]
            for entry in _normalize_runtime_context_items(
                ctx.get("foreshadow_state_ledger"),
                limit=4,
            )
        ],
        limit=4,
    )
    merged["organization_state_ledger"] = _normalize_runtime_context_items(
        [
            entry
            for ctx in contexts[-3:]
            for entry in _normalize_runtime_context_items(
                ctx.get("organization_state_ledger"),
                limit=4,
            )
        ],
        limit=4,
    )
    merged["career_state_ledger"] = _normalize_runtime_context_items(
        [
            entry
            for ctx in contexts[-3:]
            for entry in _normalize_runtime_context_items(
                ctx.get("career_state_ledger"),
                limit=4,
            )
        ],
        limit=4,
    )
    chapter_numbers = [
        int(value)
        for value in (
            _safe_float(ctx.get("current_chapter_number"))
            for ctx in contexts
        )
        if value is not None
    ]
    if chapter_numbers:
        merged["chapter_number_span"] = [min(chapter_numbers), max(chapter_numbers)]
        merged["current_chapter_number"] = max(chapter_numbers)
    chapter_count_values = [
        int(value)
        for value in (_safe_float(ctx.get("chapter_count")) for ctx in contexts)
        if value is not None
    ]
    if chapter_count_values:
        merged["chapter_count"] = max(chapter_count_values)
    stage = _resolve_quality_stage(merged)
    if stage:
        merged["plot_stage"] = stage
    return merged


def _extract_continuity_preflight(metrics: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(metrics, Mapping):
        return {}
    payload = metrics.get("continuity_preflight")
    return dict(payload) if isinstance(payload, Mapping) else {}


def _aggregate_continuity_preflight(history: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    recent_items = [
        _extract_continuity_preflight(item)
        for item in history[-3:]
        if isinstance(item, Mapping) and _extract_continuity_preflight(item)
    ]
    if not recent_items:
        return {}

    warnings: list[dict[str, Any]] = []
    focus_areas: list[str] = []
    repair_targets: list[str] = []
    warning_count = 0
    missing_item_count = 0
    checked_item_count = 0

    for item in recent_items:
        warning_count += int(item.get("warning_count") or 0)
        missing_item_count += int(item.get("missing_item_count") or 0)
        checked_item_count += int(item.get("checked_item_count") or 0)
        for focus_area in item.get("focus_areas") or []:
            if isinstance(focus_area, str) and focus_area and focus_area not in focus_areas:
                focus_areas.append(focus_area)
        for repair_target in item.get("repair_targets") or []:
            if (
                isinstance(repair_target, str)
                and repair_target
                and repair_target not in repair_targets
            ):
                repair_targets.append(repair_target)
        for warning in item.get("warnings") or []:
            if not isinstance(warning, Mapping):
                continue
            normalized_warning = dict(warning)
            if normalized_warning not in warnings:
                warnings.append(normalized_warning)
            if len(warnings) >= 4:
                break
        if len(warnings) >= 4:
            break

    if warning_count <= 0:
        return {}

    labels = ", ".join(
        dict.fromkeys(
            str(warning.get("ledger_label") or "")
            for warning in warnings
            if warning.get("ledger_label")
        )
    )
    summary = f"Recent chapters show {warning_count} continuity handoff gaps."
    if labels:
        summary = (
            f"Recent chapters show {warning_count} continuity handoff gaps. "
            f"Prioritize {labels}."
        )
    return {
        "status": "warning",
        "checked_item_count": checked_item_count,
        "warning_count": warning_count,
        "missing_item_count": missing_item_count,
        "warnings": warnings,
        "focus_areas": focus_areas[:4],
        "repair_targets": repair_targets[:4],
        "summary": summary,
    }


def _average_metric_values(
    values: Sequence[Optional[float]],
    *,
    digits: int = 1,
) -> Optional[float]:
    normalized_values = [float(value) for value in values if value is not None]
    if not normalized_values:
        return None
    return round(sum(normalized_values) / len(normalized_values), digits)


def _extract_recent_metric_average(
    history: Sequence[Mapping[str, Any]],
    metric_keys: Sequence[str],
) -> Optional[float]:
    metric_values: list[float] = []
    for item in history:
        current_values = [_safe_float(item.get(metric_key)) for metric_key in metric_keys]
        normalized_values = [value for value in current_values if value is not None]
        if normalized_values:
            metric_values.append(sum(normalized_values) / len(normalized_values))
    return _average_metric_values(metric_values)


def _build_pacing_imbalance_summary(history: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    recent_history = [dict(item) for item in history[-5:] if isinstance(item, Mapping) and item]
    if len(recent_history) < 2:
        return {}

    recent_progression_density = _extract_recent_metric_average(
        recent_history,
        ("conflict_chain_hit_rate", "outline_alignment_rate", "payoff_chain_rate"),
    )
    recent_payoff_momentum = _extract_recent_metric_average(
        recent_history,
        ("payoff_chain_rate", "cliffhanger_rate"),
    )
    recent_payoff_rate = _average_metric_values(
        [_safe_float(item.get("payoff_chain_rate")) for item in recent_history]
    )
    recent_cliffhanger_pull = _average_metric_values(
        [_safe_float(item.get("cliffhanger_rate")) for item in recent_history]
    )

    tension_variation_samples: list[float] = []
    previous_overall_score: Optional[float] = None
    previous_cliffhanger_rate: Optional[float] = None
    for item in recent_history:
        overall_score = _safe_float(item.get("overall_score"))
        cliffhanger_rate = _safe_float(item.get("cliffhanger_rate"))
        if previous_overall_score is not None and overall_score is not None:
            tension_variation_samples.append(abs(overall_score - previous_overall_score))
        if previous_cliffhanger_rate is not None and cliffhanger_rate is not None:
            tension_variation_samples.append(abs(cliffhanger_rate - previous_cliffhanger_rate))
        if overall_score is not None:
            previous_overall_score = overall_score
        if cliffhanger_rate is not None:
            previous_cliffhanger_rate = cliffhanger_rate
    recent_tension_variation = _average_metric_values(tension_variation_samples)

    if (
        recent_progression_density is None
        and recent_payoff_momentum is None
        and recent_tension_variation is None
    ):
        return {}

    signals: list[dict[str, Any]] = []
    focus_areas: list[str] = []
    repair_targets: list[str] = []
    status = "stable"

    def append_signal(
        *,
        key: str,
        label: str,
        severity: str,
        summary: str,
        metric: Optional[float],
        focus_area_items: Sequence[str],
        repair_target_items: Sequence[str],
    ) -> None:
        nonlocal status
        signals.append(
            {
                "key": key,
                "label": label,
                "severity": severity,
                "summary": summary,
                "metric": round(metric, 1) if isinstance(metric, (int, float)) else metric,
            }
        )
        focus_areas.extend(str(item).strip() for item in focus_area_items if str(item).strip())
        repair_targets.extend(str(item).strip() for item in repair_target_items if str(item).strip())
        if severity == "warning":
            status = "warning"
        elif severity == "watch" and status == "stable":
            status = "watch"

    if (
        recent_progression_density is not None
        and recent_progression_density < 68.0
        and recent_tension_variation is not None
        and recent_tension_variation < 6.5
    ):
        append_signal(
            key="middle_drag",
            label="中段拖滞",
            severity="warning" if recent_progression_density < 64.0 else "watch",
            summary="最近数章推进密度与张力波动都偏低，容易出现连续铺陈但有效事件不足。",
            metric=recent_progression_density,
            focus_area_items=("conflict", "outline", "pacing"),
            repair_target_items=(
                "本章至少推进 1 个主线矛盾，并写出新的代价、反制或局势变化。",
                "把当前章节的大纲任务拆成可见动作，不要只做解释性铺陈。",
            ),
        )

    if (
        recent_cliffhanger_pull is not None
        and recent_cliffhanger_pull >= 80.0
        and recent_payoff_rate is not None
        and recent_payoff_rate < 70.0
    ):
        append_signal(
            key="overstretched_suspense",
            label="悬念透支",
            severity="warning" if recent_payoff_rate < 66.0 else "watch",
            summary="章尾牵引持续偏强，但兑现率偏低，容易形成只吊胃口、不回收承诺的拖尾。",
            metric=recent_payoff_rate,
            focus_area_items=("payoff", "cliffhanger"),
            repair_target_items=(
                "本章必须回收至少 1 个既有伏笔、承诺或情绪账。",
                "新增悬念前，先让已有悬念落地成结果、损失或关系变化。",
            ),
        )

    if recent_payoff_rate is not None and recent_payoff_rate < 66.0:
        append_signal(
            key="payoff_fatigue",
            label="回报疲劳",
            severity="warning" if recent_payoff_rate < 62.0 else "watch",
            summary="最近几章兑现动作持续偏弱，读者获得感和阶段闭环不足。",
            metric=recent_payoff_rate,
            focus_area_items=("payoff", "pacing"),
            repair_target_items=(
                "让本章出现一个阶段性结果、关系改写或资源转移，形成明确小闭环。",
            ),
        )

    if recent_tension_variation is not None and recent_tension_variation > 16.0:
        append_signal(
            key="rhythm_whiplash",
            label="节奏摆荡",
            severity="warning" if recent_tension_variation > 20.0 else "watch",
            summary="最近张力波动过大，容易出现忽强忽弱、节拍断裂的阅读体验。",
            metric=recent_tension_variation,
            focus_area_items=("pacing",),
            repair_target_items=(
                "把本章张力曲线收束为“目标—受阻—反制—余波”，避免无序跳档。",
            ),
        )

    if signals:
        leading_labels = "、".join(
            str(signal.get("label") or signal.get("key") or "")
            for signal in signals[:2]
        )
        summary = f"最近 {len(recent_history)} 章出现{leading_labels}风险，需优先修复推进密度、兑现节拍与张力接力。"
    else:
        summary = "最近数章推进密度、兑现节拍与张力波动整体可控，可继续维持当前节奏并放大优势。"

    return {
        "status": status,
        "window_size": len(recent_history),
        "signal_count": len(signals),
        "recent_progression_density": recent_progression_density,
        "recent_payoff_momentum": recent_payoff_momentum,
        "recent_payoff_rate": recent_payoff_rate,
        "recent_cliffhanger_pull": recent_cliffhanger_pull,
        "recent_tension_variation": recent_tension_variation,
        "signals": signals[:4],
        "focus_areas": _normalize_runtime_items(focus_areas, limit=4),
        "repair_targets": _normalize_runtime_items(repair_targets, limit=4),
        "summary": summary,
    }


_SUMMARY_METRIC_FIELDS: tuple[tuple[str, str], ...] = (
    ("overall_score", "avg_overall_score"),
    ("conflict_chain_hit_rate", "avg_conflict_chain_hit_rate"),
    ("rule_grounding_hit_rate", "avg_rule_grounding_hit_rate"),
    ("outline_alignment_rate", "avg_outline_alignment_rate"),
    ("dialogue_naturalness_rate", "avg_dialogue_naturalness_rate"),
    ("opening_hook_rate", "avg_opening_hook_rate"),
    ("payoff_chain_rate", "avg_payoff_chain_rate"),
    ("cliffhanger_rate", "avg_cliffhanger_rate"),
)


def _coerce_metric_float(value: Any) -> float:
    try:
        if value in (None, ""):
            return 0.0
        return float(value)
    except (TypeError, ValueError):
        return 0.0


def _normalize_quality_metrics_history_item(
    metrics: Mapping[str, Any],
    *,
    scope: str,
) -> dict[str, Any]:
    normalized_metrics = dict(metrics)
    if not isinstance(normalized_metrics.get("repair_guidance"), Mapping):
        normalized_metrics["repair_guidance"] = build_story_repair_guidance(
            normalized_metrics,
            scope=scope,
        )
    if not isinstance(normalized_metrics.get("quality_gate"), Mapping):
        normalized_metrics["quality_gate"] = build_quality_gate_decision(
            normalized_metrics,
            scope=scope,
        )
    return normalized_metrics


def _infer_progress_stage(runtime_context: Mapping[str, Any]) -> Optional[str]:
    if not isinstance(runtime_context, Mapping):
        return None
    current = _safe_float(runtime_context.get("current_chapter_number"))
    total = _safe_float(runtime_context.get("chapter_count"))
    if current is None or total is None or total <= 0:
        return None
    progress = current / total
    if progress <= 0.22:
        return "opening"
    if progress >= 0.78:
        return "ending"
    return "development"


def _resolve_summary_metric_value(
    summary: Mapping[str, Any],
    metric_key: str,
) -> Optional[float]:
    if metric_key == "pacing_score":
        value = _safe_float(summary.get("avg_pacing_score"))
        return round(value * 10, 1) if value is not None else None
    return _safe_float(summary.get(f"avg_{metric_key}"))


def _build_volume_goal_completion_summary(summary: Mapping[str, Any]) -> dict[str, Any]:
    runtime_context = _extract_quality_runtime_context(summary)
    if not runtime_context:
        return {}

    expected_stage = _infer_progress_stage(runtime_context)
    current_stage = _resolve_quality_stage(runtime_context) or expected_stage
    resolved_stage = expected_stage or current_stage
    if not resolved_stage:
        return {}

    if resolved_stage == "opening":
        metric_specs = (
            ("opening_hook_rate", "opening", "开场钩子"),
            ("outline_alignment_rate", "outline", "大纲贴合"),
            ("conflict_chain_hit_rate", "conflict", "冲突链推进"),
        )
        stage_goal = "开篇阶段需要把主目标、异常与初始阻力快速立起来。"
        default_targets = [
            "尽快抛出主线目标或异常，不要用整章解释背景。",
            "让主角在本章就遭遇第一次明确受阻或代价。",
        ]
    elif resolved_stage == "ending":
        metric_specs = (
            ("payoff_chain_rate", "payoff", "回报兑现"),
            ("outline_alignment_rate", "outline", "大纲贴合"),
            ("cliffhanger_rate", "cliffhanger", "章尾牵引"),
            ("conflict_chain_hit_rate", "conflict", "冲突链推进"),
        )
        stage_goal = "收束阶段需要完成阶段兑现、冲突回收与下一步牵引。"
        default_targets = [
            "优先回收已经承诺的结果、伏笔或关系变化，不要继续横向开新坑。",
            "让阶段冲突形成结果、损失或站队变化，并保留下一步牵引。",
        ]
    else:
        metric_specs = (
            ("conflict_chain_hit_rate", "conflict", "冲突链推进"),
            ("outline_alignment_rate", "outline", "大纲贴合"),
            ("pacing_score", "pacing", "节奏稳定度"),
            ("payoff_chain_rate", "payoff", "回报兑现"),
        )
        stage_goal = "发展阶段需要把卷内任务拆成可见动作、反制和局势位移。"
        default_targets = [
            "把当前卷的阶段目标拆成可见动作，不要只做解释性铺陈。",
            "至少推进一条主线矛盾，并让角色因此付出新代价。",
        ]

    weight_profile = resolve_quality_weight_profile(runtime_context, resolved_stage)
    weights = (
        weight_profile.get("weights")
        if isinstance(weight_profile.get("weights"), Mapping)
        else {}
    )

    weighted_total = 0.0
    weight_total = 0.0
    weak_labels: list[str] = []
    focus_areas: list[str] = []
    metric_count = 0
    for metric_key, focus_area, label in metric_specs:
        value = _resolve_summary_metric_value(summary, metric_key)
        if value is None:
            continue
        metric_count += 1
        weight = _safe_float(weights.get(focus_area)) or 1.0
        weighted_total += value * weight
        weight_total += weight
        weak_threshold = 72.0 + max(0.0, (weight - 1.0) * 10.0)
        if value < weak_threshold:
            weak_labels.append(label)
            focus_areas.append(focus_area)

    if metric_count <= 0 or weight_total <= 0.0:
        return {}

    base_completion = round(weighted_total / weight_total, 1)
    stage_alignment = None
    if expected_stage and current_stage:
        stage_sequence = ("opening", "development", "ending")
        expected_index = (
            stage_sequence.index(expected_stage)
            if expected_stage in stage_sequence
            else None
        )
        current_index = (
            stage_sequence.index(current_stage)
            if current_stage in stage_sequence
            else None
        )
        if expected_index is not None and current_index is not None:
            stage_alignment = max(40.0, 100.0 - abs(expected_index - current_index) * 35.0)
    completion_rate = round(
        (
            (base_completion * 0.72)
            + ((stage_alignment if stage_alignment is not None else 85.0) * 0.28)
        ),
        1,
    )

    repair_targets: list[str] = []
    if stage_alignment is not None and expected_stage and current_stage and expected_stage != current_stage:
        repair_targets.append(
            f"按章节进度应进入{QUALITY_STAGE_LABELS.get(expected_stage, expected_stage)}，但当前表现仍偏向{QUALITY_STAGE_LABELS.get(current_stage, current_stage)}，本章要主动拉回阶段任务。"
        )
    repair_targets.extend(default_targets)

    status = "stable"
    if completion_rate < 68.0:
        status = "warning"
    elif completion_rate < 78.0:
        status = "watch"

    expected_label = QUALITY_STAGE_LABELS.get(expected_stage or "", expected_stage or "")
    current_label = QUALITY_STAGE_LABELS.get(current_stage or "", current_stage or "")
    weak_label_text = " / ".join(weak_labels[:3])
    summary_text = f"卷级目标达成率约 {completion_rate:.1f}%，{stage_goal}"
    if stage_alignment is not None and expected_stage and current_stage and expected_stage != current_stage:
        summary_text = (
            f"卷级目标达成率约 {completion_rate:.1f}%，按章节进度应处于{expected_label}，"
            f"但当前质量信号更接近{current_label}，说明阶段任务完成度不足。"
        )
    elif weak_label_text:
        summary_text = f"卷级目标达成率约 {completion_rate:.1f}%，当前主要拖累项为{weak_label_text}。"

    return {
        "status": status,
        "completion_rate": completion_rate,
        "expected_stage": expected_stage or "",
        "expected_stage_label": expected_label,
        "current_stage": current_stage or "",
        "current_stage_label": current_label,
        "stage_alignment": round(stage_alignment, 1) if isinstance(stage_alignment, (int, float)) else None,
        "summary": summary_text,
        "focus_areas": _normalize_runtime_items(focus_areas, limit=4),
        "repair_targets": _normalize_runtime_items(repair_targets, limit=4),
        "profile_summary": str(weight_profile.get("summary") or "").strip(),
        "profile_focuses": _normalize_runtime_items(weight_profile.get("focus_labels"), limit=4),
        "style_profile": str(weight_profile.get("style_profile") or "").strip(),
        "genre_profiles": _normalize_runtime_items(weight_profile.get("genre_profiles"), limit=4),
        "quality_preset": str(weight_profile.get("quality_preset") or "").strip(),
    }


def _build_foreshadow_payoff_delay_summary(summary: Mapping[str, Any]) -> dict[str, Any]:
    runtime_context = _extract_quality_runtime_context(summary)
    foreshadow_payoff_plan = _normalize_runtime_context_item_texts(
        runtime_context.get("foreshadow_payoff_plan"),
        limit=6,
    )
    foreshadow_state_ledger = _normalize_runtime_context_item_texts(
        runtime_context.get("foreshadow_state_ledger"),
        limit=6,
    )
    recent_payoff_rate = _safe_float(summary.get("avg_payoff_chain_rate"))
    pacing_imbalance = (
        summary.get("pacing_imbalance")
        if isinstance(summary.get("pacing_imbalance"), Mapping)
        else {}
    )
    recent_payoff_momentum = _safe_float(pacing_imbalance.get("recent_payoff_momentum"))

    if not foreshadow_payoff_plan and not foreshadow_state_ledger and recent_payoff_rate is None:
        return {}

    outstanding_count = max(len(foreshadow_payoff_plan), len(foreshadow_state_ledger))
    current = _safe_float(runtime_context.get("current_chapter_number"))
    total = _safe_float(runtime_context.get("chapter_count"))
    progress_ratio = (current / total) if current is not None and total not in {None, 0} else None

    backlog_pressure = min(100.0, outstanding_count * 18.0)
    payoff_gap = (
        max(0.0, 78.0 - recent_payoff_rate)
        if recent_payoff_rate is not None
        else (18.0 if outstanding_count > 0 else 0.0)
    )
    momentum_gap = (
        max(0.0, 76.0 - recent_payoff_momentum)
        if recent_payoff_momentum is not None
        else (10.0 if outstanding_count > 1 else 0.0)
    )
    progress_multiplier = 1.0
    if progress_ratio is not None and progress_ratio >= 0.75:
        progress_multiplier = 1.15
    elif progress_ratio is not None and progress_ratio >= 0.55:
        progress_multiplier = 1.05

    delay_index = round(
        min(
            100.0,
            (backlog_pressure * 0.45 + payoff_gap * 0.35 + momentum_gap * 0.20)
            * progress_multiplier,
        ),
        1,
    )

    status = "stable"
    if delay_index >= 55.0 or ((progress_ratio or 0.0) >= 0.7 and outstanding_count >= 3):
        status = "warning"
    elif delay_index >= 35.0 or outstanding_count >= 2:
        status = "watch"

    repair_targets: list[str] = []
    if foreshadow_payoff_plan:
        repair_targets.append(f"优先兑现伏笔计划中的至少 1 条：{' / '.join(foreshadow_payoff_plan[:2])}。")
    if outstanding_count >= 3:
        repair_targets.append("减少新增悬念，把已有伏笔写成结果、损失或信息揭示。")
    if (progress_ratio or 0.0) >= 0.72:
        repair_targets.append("临近收束阶段，未兑现伏笔必须与主线结果绑定，避免尾部堆积。")
    if not repair_targets and recent_payoff_rate is not None and recent_payoff_rate < 72.0:
        repair_targets.append("本章至少回收一个既有伏笔、承诺或情绪账，避免继续透支悬念。")

    backlog_label = " / ".join(foreshadow_state_ledger[:2])
    summary_text = f"伏笔兑现延迟指数 {delay_index:.1f}，当前仍有 {outstanding_count} 项伏笔/承诺需要清偿。"
    if backlog_label:
        summary_text = f"伏笔兑现延迟指数 {delay_index:.1f}，待清偿重点包括 {backlog_label}。"

    focus_areas = ["payoff", "cliffhanger"]
    if (progress_ratio or 0.0) >= 0.72:
        focus_areas.append("outline")

    return {
        "status": status,
        "delay_index": delay_index,
        "plan_count": len(foreshadow_payoff_plan),
        "backlog_count": len(foreshadow_state_ledger),
        "recent_payoff_rate": round(recent_payoff_rate, 1) if isinstance(recent_payoff_rate, (int, float)) else None,
        "recent_payoff_momentum": round(recent_payoff_momentum, 1) if isinstance(recent_payoff_momentum, (int, float)) else None,
        "summary": summary_text,
        "focus_areas": _normalize_runtime_items(focus_areas, limit=4),
        "repair_targets": _normalize_runtime_items(repair_targets, limit=4),
    }


async def load_latest_quality_metric_records_for_chapter_ids(
    db_session: AsyncSession,
    chapter_ids: list[str],
) -> dict[str, dict[str, Any]]:
    from sqlalchemy import select

    from migrator_app.models import GenerationHistory

    normalized_ids = [chapter_id for chapter_id in chapter_ids if chapter_id]
    if not normalized_ids:
        return {}

    result = await db_session.execute(
        select(GenerationHistory)
        .where(GenerationHistory.chapter_id.in_(normalized_ids))
        .order_by(GenerationHistory.created_at.desc())
    )

    records_by_chapter: dict[str, dict[str, Any]] = {}
    for history in result.scalars():
        chapter_id = history.chapter_id
        if not chapter_id or chapter_id in records_by_chapter:
            continue
        metrics = _parse_quality_metrics_from_history(history.generated_content)
        if not metrics:
            continue
        records_by_chapter[chapter_id] = {
            "chapter_id": chapter_id,
            "latest_quality_metrics": metrics,
            "history_id": history.id,
            "generated_at": history.created_at.isoformat() if history.created_at else None,
            "generated_at_dt": history.created_at,
        }
        if len(records_by_chapter) >= len(normalized_ids):
            break

    return records_by_chapter


async def load_latest_quality_metrics_for_chapter_ids(
    db_session: AsyncSession,
    chapter_ids: list[str],
) -> list[dict[str, Any]]:
    records_by_chapter = await load_latest_quality_metric_records_for_chapter_ids(
        db_session,
        chapter_ids,
    )
    return [
        record["latest_quality_metrics"]
        for chapter_id in chapter_ids
        if chapter_id in records_by_chapter
        for record in [records_by_chapter[chapter_id]]
    ]


async def load_recent_previous_chapter_ids(
    db_session: AsyncSession,
    *,
    project_id: str,
    before_chapter_number: int,
    limit: int = 3,
) -> list[str]:
    from sqlalchemy import select

    from migrator_app.models.chapter import Chapter

    result = await db_session.execute(
        select(Chapter.id)
        .where(Chapter.project_id == project_id)
        .where(Chapter.chapter_number < before_chapter_number)
        .order_by(Chapter.chapter_number.desc())
        .limit(limit)
    )
    return list(result.scalars().all())



