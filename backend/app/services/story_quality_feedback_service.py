"""故事质量修复建议服务 - 将质量指标转换为稳定的结构化修复目标。"""
from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Dict, Mapping, Optional, Sequence


@dataclass(frozen=True)
class RepairMetricRule:
    """质量指标规则。"""

    key: str
    aliases: tuple[str, ...]
    label: str
    focus_area: str
    weak_threshold: float
    preserve_threshold: float
    scale: float
    repair_target: str
    preserve_hint: str


METRIC_RULES: tuple[RepairMetricRule, ...] = (
    RepairMetricRule(
        key="conflict_chain_hit_rate",
        aliases=("conflict_chain_hit_rate", "avg_conflict_chain_hit_rate"),
        label="冲突链推进",
        focus_area="conflict",
        weak_threshold=62.0,
        preserve_threshold=82.0,
        scale=1.0,
        repair_target="补强冲突升级与代价。",
        preserve_hint="保留当前有效的冲突张力。",
    ),
    RepairMetricRule(
        key="rule_grounding_hit_rate",
        aliases=("rule_grounding_hit_rate", "avg_rule_grounding_hit_rate"),
        label="规则落地",
        focus_area="rule_grounding",
        weak_threshold=65.0,
        preserve_threshold=84.0,
        scale=1.0,
        repair_target="把设定限制写进动作和结果。",
        preserve_hint="保留当前的设定因果闭环。",
    ),
    RepairMetricRule(
        key="outline_alignment_rate",
        aliases=("outline_alignment_rate", "avg_outline_alignment_rate"),
        label="大纲贴合",
        focus_area="outline",
        weak_threshold=66.0,
        preserve_threshold=84.0,
        scale=1.0,
        repair_target="回扣本轮大纲任务、变化与收束。",
        preserve_hint="保留主线推进的稳定性。",
    ),
    RepairMetricRule(
        key="dialogue_naturalness_rate",
        aliases=("dialogue_naturalness_rate", "avg_dialogue_naturalness_rate"),
        label="对白自然度",
        focus_area="dialogue",
        weak_threshold=68.0,
        preserve_threshold=82.0,
        scale=1.0,
        repair_target="对白加入潜台词和立场碰撞。",
        preserve_hint="保留人物语气的辨识度。",
    ),
    RepairMetricRule(
        key="opening_hook_rate",
        aliases=("opening_hook_rate", "avg_opening_hook_rate"),
        label="开场钩子",
        focus_area="opening",
        weak_threshold=64.0,
        preserve_threshold=80.0,
        scale=1.0,
        repair_target="开头尽快抛出目标、异常或受阻。",
        preserve_hint="保留当前的开场抓力。",
    ),
    RepairMetricRule(
        key="payoff_chain_rate",
        aliases=("payoff_chain_rate", "avg_payoff_chain_rate"),
        label="回报兑现",
        focus_area="payoff",
        weak_threshold=62.0,
        preserve_threshold=80.0,
        scale=1.0,
        repair_target="回收承诺、伏笔或阶段期待。",
        preserve_hint="保留已有的回收感。",
    ),
    RepairMetricRule(
        key="cliffhanger_rate",
        aliases=("cliffhanger_rate", "avg_cliffhanger_rate"),
        label="章尾牵引",
        focus_area="cliffhanger",
        weak_threshold=64.0,
        preserve_threshold=82.0,
        scale=1.0,
        repair_target="章尾留下未决问题或新失衡。",
        preserve_hint="保留当前的章尾牵引。",
    ),
    RepairMetricRule(
        key="pacing_score",
        aliases=("pacing_score", "avg_pacing_score"),
        label="节奏稳定度",
        focus_area="pacing",
        weak_threshold=6.4,
        preserve_threshold=8.2,
        scale=10.0,
        repair_target="调整推进、停顿和转折节拍。",
        preserve_hint="保留顺畅的节奏起伏。",
    ),
)


def _safe_float(value: Any) -> Optional[float]:
    try:
        if value is None:
            return None
        return float(value)
    except (TypeError, ValueError):
        return None


def _extract_rule_value(metrics: Mapping[str, Any], rule: RepairMetricRule) -> Optional[float]:
    for key in rule.aliases:
        value = _safe_float(metrics.get(key))
        if value is not None:
            return value
    return None


QUALITY_STAGE_LABELS: Dict[str, str] = {
    "opening": "??",
    "development": "??",
    "ending": "??",
}


def _normalize_runtime_stage(value: Any) -> Optional[str]:
    text = str(value or "").strip().lower()
    if not text:
        return None
    alias_map = {
        "opening": "opening",
        "setup": "opening",
        "beginning": "opening",
        "intro": "opening",
        "development": "development",
        "middle": "development",
        "mid": "development",
        "escalation": "development",
        "climax": "ending",
        "ending": "ending",
        "finale": "ending",
        "resolution": "ending",
    }
    return alias_map.get(text, text if text in QUALITY_STAGE_LABELS else None)


def _normalize_runtime_items(values: Any, *, limit: int = 4) -> list[str]:
    if values is None:
        return []
    if isinstance(values, str):
        raw_items = [values]
    elif isinstance(values, Sequence) and not isinstance(values, (str, bytes, bytearray)):
        raw_items = list(values)
    else:
        raw_items = [values]

    items: list[str] = []
    seen: set[str] = set()
    for value in raw_items:
        text = str(value or "").strip()
        if not text or text in seen:
            continue
        seen.add(text)
        items.append(text)
        if len(items) >= limit:
            break
    return items


def _extract_quality_runtime_context(metrics: Mapping[str, Any]) -> Dict[str, Any]:
    if not isinstance(metrics, Mapping):
        return {}
    context = metrics.get("quality_runtime_context")
    return dict(context) if isinstance(context, Mapping) else {}


def _resolve_quality_stage(runtime_context: Mapping[str, Any]) -> Optional[str]:
    if not isinstance(runtime_context, Mapping):
        return None
    stage = _normalize_runtime_stage(runtime_context.get("plot_stage"))
    if stage:
        return stage

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


def _build_runtime_pressure(runtime_context: Mapping[str, Any]) -> Dict[str, Any]:
    character_states = _normalize_runtime_items(runtime_context.get("character_state_ledger"), limit=6)
    relationship_states = _normalize_runtime_items(runtime_context.get("relationship_state_ledger"), limit=6)
    foreshadow_states = _normalize_runtime_items(runtime_context.get("foreshadow_state_ledger"), limit=6)
    return {
        "character_state_count": len(character_states),
        "relationship_state_count": len(relationship_states),
        "foreshadow_state_count": len(foreshadow_states),
        "character_state_items": character_states[:3],
        "relationship_state_items": relationship_states[:3],
        "foreshadow_state_items": foreshadow_states[:3],
    }


def _resolve_metric_threshold_adjustments(runtime_context: Mapping[str, Any]) -> Dict[str, float]:
    stage = _resolve_quality_stage(runtime_context)
    adjustments: Dict[str, float] = {}
    if stage == "opening":
        adjustments.update({
            "opening_hook_rate": 6.0,
            "outline_alignment_rate": 3.0,
            "payoff_chain_rate": -4.0,
            "cliffhanger_rate": 1.0,
        })
    elif stage == "development":
        adjustments.update({
            "conflict_chain_hit_rate": 4.0,
            "dialogue_naturalness_rate": 1.0,
            "opening_hook_rate": -2.0,
            "pacing_score": 0.4,
        })
    elif stage == "ending":
        adjustments.update({
            "payoff_chain_rate": 6.0,
            "cliffhanger_rate": 4.0,
            "conflict_chain_hit_rate": 2.0,
            "opening_hook_rate": -4.0,
            "outline_alignment_rate": 1.0,
            "pacing_score": 0.4,
        })

    pressure = _build_runtime_pressure(runtime_context)
    if pressure["foreshadow_state_count"] >= 3:
        adjustments["payoff_chain_rate"] = adjustments.get("payoff_chain_rate", 0.0) + 2.0
    if pressure["relationship_state_count"] >= 2:
        adjustments["dialogue_naturalness_rate"] = adjustments.get("dialogue_naturalness_rate", 0.0) + 1.0
        adjustments["conflict_chain_hit_rate"] = adjustments.get("conflict_chain_hit_rate", 0.0) + 1.0
    if pressure["character_state_count"] >= 3:
        adjustments["outline_alignment_rate"] = adjustments.get("outline_alignment_rate", 0.0) + 1.0
    return adjustments


def _resolve_gate_thresholds(runtime_context: Mapping[str, Any]) -> Dict[str, Any]:
    stage = _resolve_quality_stage(runtime_context)
    thresholds: Dict[str, Any] = {
        "stage": stage,
        "stage_label": QUALITY_STAGE_LABELS.get(stage, ""),
        "manual_review_score": 70.0,
        "allow_save_score": 82.0,
        "normalized_gap": 12.0,
        "weak_metric_block_count": 3,
    }
    if stage == "opening":
        thresholds.update({
            "manual_review_score": 68.0,
            "allow_save_score": 80.0,
            "normalized_gap": 13.0,
        })
    elif stage == "ending":
        thresholds.update({
            "manual_review_score": 72.0,
            "allow_save_score": 84.0,
            "normalized_gap": 10.0,
        })

    pressure = _build_runtime_pressure(runtime_context)
    if stage == "ending" and pressure["foreshadow_state_count"] >= 3:
        thresholds["allow_save_score"] = max(float(thresholds["allow_save_score"]), 85.0)
        thresholds["normalized_gap"] = min(float(thresholds["normalized_gap"]), 9.0)
    return thresholds


def _collect_metric_items(
    metrics: Mapping[str, Any],
    *,
    runtime_context: Optional[Mapping[str, Any]] = None,
) -> list[dict[str, Any]]:
    if not isinstance(metrics, Mapping):
        return []

    threshold_adjustments = _resolve_metric_threshold_adjustments(runtime_context or {})
    metric_items: list[dict[str, Any]] = []
    for rule in METRIC_RULES:
        raw_value = _extract_rule_value(metrics, rule)
        if raw_value is None:
            continue
        weak_threshold_value = round(max(rule.weak_threshold + threshold_adjustments.get(rule.key, 0.0), 0.0), 1)
        preserve_threshold_value = round(max(rule.preserve_threshold + threshold_adjustments.get(rule.key, 0.0) * 0.6, 0.0), 1)
        metric_items.append(
            {
                "key": rule.key,
                "label": rule.label,
                "focus_area": rule.focus_area,
                "raw_value": round(raw_value, 1),
                "normalized_value": round(raw_value * rule.scale, 1),
                "weak_threshold": round(weak_threshold_value * rule.scale, 1),
                "weak_threshold_value": weak_threshold_value,
                "preserve_threshold": round(preserve_threshold_value * rule.scale, 1),
                "preserve_threshold_value": preserve_threshold_value,
                "repair_target": rule.repair_target,
                "preserve_hint": rule.preserve_hint,
            }
        )

    metric_items.sort(key=lambda item: item["normalized_value"])
    return metric_items


def _split_metric_items(metric_items: Sequence[Mapping[str, Any]]) -> Dict[str, Any]:
    normalized_items = [dict(item) for item in metric_items if isinstance(item, Mapping)]
    if not normalized_items:
        return {
            "metric_items": [],
            "weakest": None,
            "low_items": [],
            "strength_items": [],
        }

    normalized_items.sort(key=lambda item: item["normalized_value"])
    weakest = normalized_items[0]
    low_items = [item for item in normalized_items if item["normalized_value"] < item["weak_threshold"]][:3]
    strength_items = [
        item
        for item in sorted(normalized_items, key=lambda item: item["normalized_value"], reverse=True)
        if item["normalized_value"] >= item["preserve_threshold"]
    ][:2]
    return {
        "metric_items": normalized_items,
        "weakest": weakest,
        "low_items": low_items,
        "strength_items": strength_items,
    }


def _resolve_scope_label(scope: str) -> str:
    scope_label_map = {
        "chapter": "当前章节",
        "batch": "这一批章节",
        "outline": "最近章节",
    }
    return scope_label_map.get(scope, "当前章节")


def _resolve_overall_score(metrics: Mapping[str, Any]) -> Optional[float]:
    for key in ("overall_score", "avg_overall_score"):
        value = _safe_float(metrics.get(key))
        if value is not None:
            return round(value, 1)
    return None


def _build_empty_guidance() -> Dict[str, Any]:
    return {
        "summary": "",
        "repair_targets": [],
        "preserve_strengths": [],
        "focus_areas": [],
        "weakest_metric_key": None,
        "weakest_metric_label": None,
        "weakest_metric_value": None,
    }


def _build_empty_quality_gate(*, overall_score: Optional[float] = None) -> Dict[str, Any]:
    return {
        "status": "unknown",
        "decision": "unknown",
        "label": "待补充诊断",
        "summary": "缺少有效质量指标，暂无法给出门禁决策。",
        "reason": "缺少可用的质量指标。",
        "overall_score": overall_score,
        "weak_metric_count": 0,
        "failed_metrics": [],
        "focus_areas": [],
        "repair_targets": [],
        "allow_save": False,
        "can_auto_repair": False,
        "requires_manual_review": False,
        "weakest_metric_key": None,
        "weakest_metric_label": None,
        "weakest_metric_value": None,
    }


def extract_quality_metrics_from_history_payload(
    generated_content: Optional[str],
    *,
    scope: str = "chapter",
) -> Optional[Dict[str, Any]]:
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
    if not isinstance(normalized_metrics.get("repair_guidance"), dict):
        normalized_metrics["repair_guidance"] = build_story_repair_guidance(normalized_metrics, scope=scope)
    if not isinstance(normalized_metrics.get("quality_gate"), dict):
        normalized_metrics["quality_gate"] = build_quality_gate_decision(normalized_metrics, scope=scope)
    return normalized_metrics


def _aggregate_quality_runtime_context(history: Sequence[Mapping[str, Any]]) -> Dict[str, Any]:
    contexts = [
        _extract_quality_runtime_context(item)
        for item in history
        if isinstance(item, Mapping) and _extract_quality_runtime_context(item)
    ]
    if not contexts:
        return {}

    contexts.sort(key=lambda item: (_safe_float(item.get("current_chapter_number")) or 0.0, _safe_float(item.get("chapter_count")) or 0.0))
    latest = contexts[-1]
    merged = dict(latest)
    merged["character_state_ledger"] = _normalize_runtime_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_items(ctx.get("character_state_ledger"), limit=4)],
        limit=4,
    )
    merged["relationship_state_ledger"] = _normalize_runtime_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_items(ctx.get("relationship_state_ledger"), limit=4)],
        limit=4,
    )
    merged["foreshadow_state_ledger"] = _normalize_runtime_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_items(ctx.get("foreshadow_state_ledger"), limit=4)],
        limit=4,
    )
    chapter_numbers = [int(value) for value in (_safe_float(ctx.get("current_chapter_number")) for ctx in contexts) if value is not None]
    if chapter_numbers:
        merged["chapter_number_span"] = [min(chapter_numbers), max(chapter_numbers)]
        merged["current_chapter_number"] = max(chapter_numbers)
    chapter_count_values = [int(value) for value in (_safe_float(ctx.get("chapter_count")) for ctx in contexts) if value is not None]
    if chapter_count_values:
        merged["chapter_count"] = max(chapter_count_values)
    stage = _resolve_quality_stage(merged)
    if stage:
        merged["plot_stage"] = stage
    return merged


def build_quality_metrics_summary(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str = "batch",
) -> Optional[Dict[str, Any]]:
    """?????????????????"""
    normalized_history = [dict(item) for item in history if isinstance(item, Mapping) and item]
    if not normalized_history:
        return None

    overall_list = [item.get("overall_score", 0.0) for item in normalized_history]
    conflict_list = [item.get("conflict_chain_hit_rate", 0.0) for item in normalized_history]
    rule_list = [item.get("rule_grounding_hit_rate", 0.0) for item in normalized_history]
    outline_alignment_list = [item.get("outline_alignment_rate", 0.0) for item in normalized_history]
    dialogue_list = [item.get("dialogue_naturalness_rate", 0.0) for item in normalized_history]
    opening_list = [item.get("opening_hook_rate", 0.0) for item in normalized_history]
    payoff_list = [item.get("payoff_chain_rate", 0.0) for item in normalized_history]
    cliffhanger_list = [item.get("cliffhanger_rate", 0.0) for item in normalized_history]
    pacing_values = [item.get("pacing_score") for item in normalized_history if item.get("pacing_score") is not None]
    runtime_context = _aggregate_quality_runtime_context(normalized_history)
    trend_delta = round(float(overall_list[-1]) - float(overall_list[0]), 1) if len(overall_list) > 1 else 0.0
    if trend_delta >= 2.0:
        trend_direction = "rising"
    elif trend_delta <= -2.0:
        trend_direction = "falling"
    else:
        trend_direction = "stable"

    recent_focus_areas: list[str] = []
    seen_focus: set[str] = set()
    for item in normalized_history[-3:]:
        guidance = item.get("repair_guidance") if isinstance(item.get("repair_guidance"), Mapping) else build_story_repair_guidance(item, scope=scope)
        for area in guidance.get("focus_areas") or []:
            if area in seen_focus:
                continue
            seen_focus.add(area)
            recent_focus_areas.append(area)
            if len(recent_focus_areas) >= 4:
                break
        if len(recent_focus_areas) >= 4:
            break

    summary = {
        "avg_overall_score": round(sum(overall_list) / max(len(overall_list), 1), 1),
        "avg_conflict_chain_hit_rate": round(sum(conflict_list) / max(len(conflict_list), 1), 1),
        "avg_rule_grounding_hit_rate": round(sum(rule_list) / max(len(rule_list), 1), 1),
        "avg_outline_alignment_rate": round(sum(outline_alignment_list) / max(len(outline_alignment_list), 1), 1),
        "avg_dialogue_naturalness_rate": round(sum(dialogue_list) / max(len(dialogue_list), 1), 1),
        "avg_opening_hook_rate": round(sum(opening_list) / max(len(opening_list), 1), 1),
        "avg_payoff_chain_rate": round(sum(payoff_list) / max(len(payoff_list), 1), 1),
        "avg_cliffhanger_rate": round(sum(cliffhanger_list) / max(len(cliffhanger_list), 1), 1),
        "avg_pacing_score": round(sum(pacing_values) / len(pacing_values), 1) if pacing_values else None,
        "chapter_count": len(normalized_history),
        "overall_score_delta": trend_delta,
        "overall_score_trend": trend_direction,
        "recent_focus_areas": recent_focus_areas,
    }
    if runtime_context:
        summary["quality_runtime_context"] = runtime_context
    summary["repair_guidance"] = build_story_repair_guidance(summary, scope=scope)
    summary["quality_gate"] = build_quality_gate_decision(summary, scope=scope)
    return summary


def build_story_repair_guidance(
    metrics: Mapping[str, Any],
    *,
    scope: str = "chapter",
) -> Dict[str, Any]:
    """????????????????"""

    if not isinstance(metrics, Mapping):
        return _build_empty_guidance()

    runtime_context = _extract_quality_runtime_context(metrics)
    stage = _resolve_quality_stage(runtime_context)
    stage_label = QUALITY_STAGE_LABELS.get(stage, "")
    analysis = _split_metric_items(_collect_metric_items(metrics, runtime_context=runtime_context))
    metric_items = analysis["metric_items"]
    weakest = analysis["weakest"]
    low_items = analysis["low_items"]
    strength_items = analysis["strength_items"]
    if not metric_items or weakest is None:
        return _build_empty_guidance()

    scope_label = _resolve_scope_label(scope)
    repair_targets = list(dict.fromkeys(item["repair_target"] for item in low_items))
    if not repair_targets:
        repair_targets = [weakest["repair_target"]]

    preserve_strengths = list(dict.fromkeys(item["preserve_hint"] for item in strength_items))
    if not preserve_strengths and weakest["label"] != "?????":
        preserve_strengths = ["???????????????????????"]

    focus_areas = list(dict.fromkeys(item["focus_area"] for item in low_items))
    if not focus_areas:
        focus_areas = [weakest["focus_area"]]

    pressure = _build_runtime_pressure(runtime_context)
    if "payoff" in focus_areas and pressure["foreshadow_state_items"]:
        repair_targets.insert(0, f"?????????{' / '.join(pressure['foreshadow_state_items'][:2])}?")
    if any(area in focus_areas for area in ("conflict", "outline", "pacing")) and pressure["character_state_items"]:
        repair_targets.append(f"??????????????{' / '.join(pressure['character_state_items'][:2])}?")
    if any(area in focus_areas for area in ("dialogue", "conflict")) and pressure["relationship_state_items"]:
        repair_targets.append(f"?????????????{' / '.join(pressure['relationship_state_items'][:2])}?")
    repair_targets = list(dict.fromkeys(repair_targets))[:4]

    if low_items:
        labels = " / ".join(item["label"] for item in low_items)
        if stage_label:
            summary = f"{scope_label}????{stage_label}????????{labels}????????????????????????"
        else:
            summary = f"{scope_label}?????{labels}????????????????????????"
    else:
        strongest_label = strength_items[0]["label"] if strength_items else weakest["label"]
        if stage_label:
            summary = f"{scope_label}????{stage_label}???????????????{strongest_label}??????????????????"
        else:
            summary = f"{scope_label}????????????{strongest_label}??????????????????"

    return {
        "summary": summary,
        "repair_targets": repair_targets,
        "preserve_strengths": preserve_strengths,
        "focus_areas": focus_areas,
        "weakest_metric_key": weakest["key"],
        "weakest_metric_label": weakest["label"],
        "weakest_metric_value": weakest["raw_value"],
        "quality_stage": stage or "",
        "quality_stage_label": stage_label,
        "quality_runtime_pressure": pressure,
    }


def build_quality_gate_decision(
    metrics: Mapping[str, Any],
    *,
    scope: str = "chapter",
) -> Dict[str, Any]:
    """???????????????"""

    if not isinstance(metrics, Mapping):
        return _build_empty_quality_gate()

    runtime_context = _extract_quality_runtime_context(metrics)
    thresholds = _resolve_gate_thresholds(runtime_context)
    stage = thresholds.get("stage")
    stage_label = thresholds.get("stage_label") or ""
    analysis = _split_metric_items(_collect_metric_items(metrics, runtime_context=runtime_context))
    metric_items = analysis["metric_items"]
    weakest = analysis["weakest"]
    low_items = analysis["low_items"]
    if not metric_items or weakest is None:
        return _build_empty_quality_gate(overall_score=_resolve_overall_score(metrics))

    overall_score = _resolve_overall_score(metrics)
    weak_metric_count = len(low_items)
    normalized_gap = max(weakest["weak_threshold"] - weakest["normalized_value"], 0.0)

    failed_source_items = list(low_items)
    if not failed_source_items and overall_score is not None and overall_score < float(thresholds["allow_save_score"]):
        failed_source_items = [weakest]

    failed_metrics = [
        {
            "key": item["key"],
            "label": item["label"],
            "value": item["raw_value"],
            "threshold": item["weak_threshold_value"],
            "gap": round(max(item["weak_threshold_value"] - item["raw_value"], 0.0), 1),
            "focus_area": item["focus_area"],
            "repair_target": item["repair_target"],
        }
        for item in failed_source_items
    ]
    focus_areas = list(dict.fromkeys(item["focus_area"] for item in failed_source_items))
    repair_targets = list(dict.fromkeys(item["repair_target"] for item in failed_source_items))
    pressure = _build_runtime_pressure(runtime_context)

    blocked_reasons: list[str] = []
    if overall_score is not None and overall_score < float(thresholds["manual_review_score"]):
        blocked_reasons.append(f"??? {overall_score:.1f} ????????")
    if weak_metric_count >= int(thresholds["weak_metric_block_count"]):
        blocked_reasons.append(f"?? {weak_metric_count} ?????")
    if normalized_gap >= float(thresholds["normalized_gap"]):
        blocked_reasons.append(f"????{weakest['label']}???????")
    if stage == "ending" and pressure["foreshadow_state_count"] >= 3 and weakest["focus_area"] == "payoff":
        blocked_reasons.append("????????????????????")

    scope_label = _resolve_scope_label(scope)
    if blocked_reasons:
        status = "blocked"
        decision = "manual_review"
        label = "?????"
        reason = "?".join(blocked_reasons)
        if stage_label:
            summary = f"{scope_label}?{stage_label}??????????????????????????????????"
        else:
            summary = f"{scope_label}??????????????????????????????"
    elif weak_metric_count > 0 or (overall_score is not None and overall_score < float(thresholds["allow_save_score"])):
        status = "repairable"
        decision = "auto_repair"
        label = "??????"
        if weak_metric_count > 0:
            reason = f"?? {weak_metric_count} ????????????????????"
        else:
            reason = "???????????????????????"
        if stage_label:
            summary = f"{scope_label}?{stage_label}??????????????????????????????"
        else:
            summary = f"{scope_label}????????????????????????????"
    else:
        status = "pass"
        decision = "allow_save"
        label = "????"
        reason = "?????????????????"
        if stage_label:
            summary = f"{scope_label}?{stage_label}??????????????????????"
        else:
            summary = f"{scope_label}????????????????????"

    return {
        "status": status,
        "decision": decision,
        "label": label,
        "summary": summary,
        "reason": reason,
        "overall_score": overall_score,
        "weak_metric_count": weak_metric_count,
        "failed_metrics": failed_metrics,
        "focus_areas": focus_areas,
        "repair_targets": repair_targets,
        "allow_save": status == "pass",
        "can_auto_repair": status == "repairable",
        "requires_manual_review": status == "blocked",
        "weakest_metric_key": weakest["key"],
        "weakest_metric_label": weakest["label"],
        "weakest_metric_value": weakest["raw_value"],
        "quality_stage": stage or "",
        "quality_stage_label": stage_label,
        "manual_review_threshold": thresholds["manual_review_score"],
        "allow_save_threshold": thresholds["allow_save_score"],
        "quality_runtime_pressure": pressure,
    }
