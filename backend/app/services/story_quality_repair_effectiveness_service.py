from __future__ import annotations

from typing import Any, Callable, Dict, Mapping, Optional, Sequence

from app.services.novel_quality_profile_service import QUALITY_FOCUS_LABELS

RepairGuidanceBuilder = Callable[..., Mapping[str, Any]]
HistoryNormalizer = Callable[..., Dict[str, Any]]
RuntimeItemsNormalizer = Callable[..., list[str]]
SafeFloatResolver = Callable[[Any], Optional[float]]

REPAIR_EFFECTIVENESS_METRIC_MAP: Dict[str, tuple[str, float, float]] = {
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


def _default_safe_float(value: Any) -> Optional[float]:
    try:
        if value is None:
            return None
        return float(value)
    except (TypeError, ValueError):
        return None



def _evaluate_repair_focus_improvement(
    current_item: Mapping[str, Any],
    next_item: Mapping[str, Any],
    focus_area: str,
    *,
    safe_float: SafeFloatResolver,
) -> Optional[Dict[str, Any]]:
    metric_spec = REPAIR_EFFECTIVENESS_METRIC_MAP.get(focus_area)
    if metric_spec is None:
        return None

    metric_key, safe_threshold, improvement_threshold = metric_spec
    current_value = safe_float(current_item.get(metric_key))
    next_value = safe_float(next_item.get(metric_key))
    if current_value is None or next_value is None:
        return None

    delta = round(next_value - current_value, 1)
    success = next_value >= current_value + improvement_threshold or (
        current_value < safe_threshold <= next_value
    )
    return {
        "focus_area": focus_area,
        "metric_key": metric_key,
        "delta": delta,
        "success": success,
    }



def build_repair_effectiveness_summary(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str,
    history_normalizer: HistoryNormalizer,
    repair_guidance_builder: RepairGuidanceBuilder,
    runtime_items_normalizer: RuntimeItemsNormalizer,
    safe_float: SafeFloatResolver = _default_safe_float,
) -> Dict[str, Any]:
    normalized_history = [
        history_normalizer(item, scope=scope)
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
            else repair_guidance_builder(current_item, scope=scope)
        )
        focus_areas = runtime_items_normalizer(guidance.get("focus_areas"), limit=4)
        pair_evaluations = []
        for focus_area in focus_areas:
            evaluation = _evaluate_repair_focus_improvement(
                current_item,
                next_item,
                focus_area,
                safe_float=safe_float,
            )
            if evaluation is None:
                continue
            pair_evaluations.append(evaluation)
            state = focus_area_state.setdefault(
                focus_area,
                {
                    "focus_area": focus_area,
                    "label": QUALITY_FOCUS_LABELS.get(focus_area, focus_area),
                    "metric_key": evaluation["metric_key"],
                    "evaluated_pairs": 0,
                    "successful_pairs": 0,
                    "delta_total": 0.0,
                },
            )
            state["evaluated_pairs"] += 1
            state["delta_total"] = round((safe_float(state.get("delta_total")) or 0.0) + evaluation["delta"], 6)
            if evaluation["success"]:
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
                "label": state.get("label") or QUALITY_FOCUS_LABELS.get(focus_area, focus_area),
                "metric_key": state.get("metric_key"),
                "evaluated_pairs": area_pairs,
                "successful_pairs": area_successful_pairs,
                "success_rate": round(area_successful_pairs / area_pairs * 100, 1),
                "avg_delta": round((safe_float(state.get("delta_total")) or 0.0) / area_pairs, 1),
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
        "recovered_focus_areas": runtime_items_normalizer(recovered_focus_areas, limit=3),
        "unresolved_focus_areas": runtime_items_normalizer(unresolved_focus_areas, limit=3),
        "focus_area_stats": focus_area_stats,
        "summary": summary_text,
    }
