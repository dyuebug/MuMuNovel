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


def _collect_metric_items(metrics: Mapping[str, Any]) -> list[dict[str, Any]]:
    if not isinstance(metrics, Mapping):
        return []

    metric_items: list[dict[str, Any]] = []
    for rule in METRIC_RULES:
        raw_value = _extract_rule_value(metrics, rule)
        if raw_value is None:
            continue
        metric_items.append(
            {
                "key": rule.key,
                "label": rule.label,
                "focus_area": rule.focus_area,
                "raw_value": round(raw_value, 1),
                "normalized_value": round(raw_value * rule.scale, 1),
                "weak_threshold": round(rule.weak_threshold * rule.scale, 1),
                "weak_threshold_value": round(rule.weak_threshold, 1),
                "preserve_threshold": round(rule.preserve_threshold * rule.scale, 1),
                "preserve_threshold_value": round(rule.preserve_threshold, 1),
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


def build_quality_metrics_summary(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str = "batch",
) -> Optional[Dict[str, Any]]:
    """根据章节质量历史构建聚合质量摘要。"""
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
    }
    summary["repair_guidance"] = build_story_repair_guidance(summary, scope=scope)
    summary["quality_gate"] = build_quality_gate_decision(summary, scope=scope)
    return summary


def build_story_repair_guidance(
    metrics: Mapping[str, Any],
    *,
    scope: str = "chapter",
) -> Dict[str, Any]:
    """根据质量指标生成结构化修复建议。"""

    if not isinstance(metrics, Mapping):
        return _build_empty_guidance()

    analysis = _split_metric_items(_collect_metric_items(metrics))
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
    if not preserve_strengths and weakest["label"] != "节奏稳定度":
        preserve_strengths = ["保留当前已经有效的角色语气、主线推进和记忆点。"]

    focus_areas = list(dict.fromkeys(item["focus_area"] for item in low_items))
    if not focus_areas:
        focus_areas = [weakest["focus_area"]]

    if low_items:
        labels = " / ".join(item["label"] for item in low_items)
        summary = f"{scope_label}优先修复「{labels}」，先让推进、约束与结果真正落地，再做表面润色。"
    else:
        strongest_label = strength_items[0]["label"] if strength_items else weakest["label"]
        summary = f"{scope_label}质量整体稳定，优先保持「{strongest_label}」的优势，再放大薄弱项的真实推进感。"

    return {
        "summary": summary,
        "repair_targets": repair_targets,
        "preserve_strengths": preserve_strengths,
        "focus_areas": focus_areas,
        "weakest_metric_key": weakest["key"],
        "weakest_metric_label": weakest["label"],
        "weakest_metric_value": weakest["raw_value"],
    }


def build_quality_gate_decision(
    metrics: Mapping[str, Any],
    *,
    scope: str = "chapter",
) -> Dict[str, Any]:
    """根据质量指标生成质量门禁决策。"""

    if not isinstance(metrics, Mapping):
        return _build_empty_quality_gate()

    analysis = _split_metric_items(_collect_metric_items(metrics))
    metric_items = analysis["metric_items"]
    weakest = analysis["weakest"]
    low_items = analysis["low_items"]
    if not metric_items or weakest is None:
        return _build_empty_quality_gate(overall_score=_resolve_overall_score(metrics))

    overall_score = _resolve_overall_score(metrics)
    weak_metric_count = len(low_items)
    normalized_gap = max(weakest["weak_threshold"] - weakest["normalized_value"], 0.0)

    failed_source_items = list(low_items)
    if not failed_source_items and overall_score is not None and overall_score < 82.0:
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

    blocked_reasons: list[str] = []
    if overall_score is not None and overall_score < 70.0:
        blocked_reasons.append(f"整体分 {overall_score:.1f} 低于稳定生成阈值")
    if weak_metric_count >= 3:
        blocked_reasons.append(f"存在 {weak_metric_count} 个薄弱维度")
    if normalized_gap >= 12.0:
        blocked_reasons.append(f"最弱项「{weakest['label']}」明显低于门槛")

    scope_label = _resolve_scope_label(scope)
    if blocked_reasons:
        status = "blocked"
        decision = "manual_review"
        label = "需人工介入"
        reason = "；".join(blocked_reasons)
        summary = f"{scope_label}质量风险较高，建议先人工介入或重写关键桥段，再继续后续生成。"
    elif weak_metric_count > 0 or (overall_score is not None and overall_score < 82.0):
        status = "repairable"
        decision = "auto_repair"
        label = "建议局部修复"
        if weak_metric_count > 0:
            reason = f"存在 {weak_metric_count} 个薄弱维度，适合先按修复建议做局部补强。"
        else:
            reason = "整体分尚未达到稳定保存阈值，建议先做局部增强。"
        summary = f"{scope_label}存在可修复弱项，建议优先局部修复后再继续保存或批量生成。"
    else:
        status = "pass"
        decision = "allow_save"
        label = "允许保存"
        reason = "整体分达标且未发现低于阈值的弱项。"
        summary = f"{scope_label}质量通过，可直接保存并进入下一阶段生成。"

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
    }
