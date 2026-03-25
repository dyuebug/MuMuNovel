"""故事质量修复建议服务 - 将质量指标转换为稳定的结构化修复目标。"""
from __future__ import annotations

from dataclasses import dataclass
import json
import re
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
    "opening": "开篇",
    "development": "发展段",
    "ending": "收束段",
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


_CONTINUITY_LEDGER_SPECS: tuple[tuple[str, str, str, str], ...] = (
    ("character_state_ledger", "character_continuity", "Character continuity ledger", "Carry forward the character continuity ledger: {item}"),
    ("relationship_state_ledger", "relationship_continuity", "Relationship continuity ledger", "Express the relationship ledger through dialogue, alignment, or exchange: {item}"),
    ("foreshadow_state_ledger", "foreshadow_continuity", "Foreshadow continuity ledger", "Advance the foreshadow ledger toward payoff: {item}"),
    ("organization_state_ledger", "organization_continuity", "Organization continuity ledger", "Carry forward the organization continuity ledger through command, resource, or territory change: {item}"),
    ("career_state_ledger", "career_continuity", "Career continuity ledger", "Carry forward the career growth ledger through skill use, bottleneck, or cost: {item}"),
)



def _extract_continuity_anchor_candidates(item: Any) -> list[str]:
    text = str(item or "").strip()
    if not text:
        return []
    head = re.split(r"[:：]", text, maxsplit=1)[0].strip() or text
    segments = [
        segment.strip()
        for segment in re.split(r"[、,\/|&＆和与+·•]+", head)
        if segment.strip()
    ]
    if not segments:
        segments = [head]
    tokens: list[str] = []
    seen: set[str] = set()
    cleanup_translation = str.maketrans({
        "?": " ",
        "?": " ",
        "[": " ",
        "]": " ",
        "?": " ",
        "?": " ",
        "(": " ",
        ")": " ",
        "<": " ",
        ">": " ",
        "?": " ",
        "?": " ",
        "?": " ",
        "?": " ",
        '"': " ",
        "'": " ",
        "`": " ",
    })
    for segment in segments[:3]:
        cleaned = segment.translate(cleanup_translation)
        for token in re.findall(r"[A-Za-z0-9_\-]{2,}|[\u4E00-\u9FFF]{2,}", cleaned):
            normalized = token.strip().lower()
            if not normalized or normalized in seen:
                continue
            seen.add(normalized)
            tokens.append(normalized)
    if tokens:
        return tokens[:3]

    fallback = re.sub(r"\s+", "", head).lower()
    return [fallback] if len(fallback) >= 2 else []


def build_story_continuity_preflight(
    content: str,
    runtime_context: Optional[Mapping[str, Any]],
) -> Dict[str, Any]:
    if not isinstance(runtime_context, Mapping):
        return {}

    normalized_content = re.sub(r"\s+", "", str(content or "")).lower()
    if not normalized_content:
        return {}

    warnings: list[Dict[str, Any]] = []
    focus_areas: list[str] = []
    repair_targets: list[str] = []
    checked_item_count = 0
    missing_item_count = 0

    for ledger_key, focus_area, ledger_label, repair_template in _CONTINUITY_LEDGER_SPECS:
        for item in _normalize_runtime_items(runtime_context.get(ledger_key), limit=3):
            checked_item_count += 1
            anchors = _extract_continuity_anchor_candidates(item)
            matched_anchor_count = len({anchor for anchor in anchors if len(anchor) >= 2 and anchor in normalized_content})
            required_match_count = 2 if ledger_key in {"relationship_state_ledger", "career_state_ledger"} and len(anchors) >= 2 else 1
            is_matched = matched_anchor_count >= required_match_count
            if is_matched:
                continue
            missing_item_count += 1
            if focus_area not in focus_areas:
                focus_areas.append(focus_area)
            target = repair_template.format(item=item)
            if target not in repair_targets:
                repair_targets.append(target)
            warnings.append({
                "ledger_key": ledger_key,
                "ledger_label": ledger_label,
                "focus_area": focus_area,
                "item": item,
                "anchors": anchors,
                "matched_anchor_count": matched_anchor_count,
                "required_match_count": required_match_count,
            })
            if len(warnings) >= 4:
                break
        if len(warnings) >= 4:
            break

    if not warnings:
        return {
            "status": "ok",
            "checked_item_count": checked_item_count,
            "warning_count": 0,
            "warnings": [],
            "focus_areas": [],
            "repair_targets": [],
            "summary": "",
        }

    labels = ", ".join(dict.fromkeys(warning["ledger_label"] for warning in warnings))
    summary = f"Current chapter misses explicit handoff for {missing_item_count} continuity ledger items."
    if labels: summary = f"Current chapter misses explicit handoff for {missing_item_count} continuity ledger items. Prioritize {labels}."
    return {
        "status": "warning",
        "checked_item_count": checked_item_count,
        "warning_count": len(warnings),
        "missing_item_count": missing_item_count,
        "warnings": warnings,
        "focus_areas": focus_areas,
        "repair_targets": repair_targets[:4],
        "summary": summary,
    }


def _extract_quality_runtime_context(metrics: Mapping[str, Any]) -> Dict[str, Any]:
    if not isinstance(metrics, Mapping):
        return {}
    context = metrics.get("quality_runtime_context")
    return dict(context) if isinstance(context, Mapping) else {}


def _extract_history_runtime_snapshot(payload: Mapping[str, Any]) -> Dict[str, Any]:
    if not isinstance(payload, Mapping):
        return {}
    runtime_snapshot = payload.get("story_runtime_snapshot")
    return dict(runtime_snapshot) if isinstance(runtime_snapshot, Mapping) else {}


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
    organization_states = _normalize_runtime_items(runtime_context.get("organization_state_ledger"), limit=6)
    career_states = _normalize_runtime_items(runtime_context.get("career_state_ledger"), limit=6)
    return {
        "character_state_count": len(character_states),
        "relationship_state_count": len(relationship_states),
        "foreshadow_state_count": len(foreshadow_states),
        "organization_state_count": len(organization_states),
        "career_state_count": len(career_states),
        "character_state_items": character_states[:3],
        "relationship_state_items": relationship_states[:3],
        "foreshadow_state_items": foreshadow_states[:3],
        "organization_state_items": organization_states[:3],
        "career_state_items": career_states[:3],
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
    if pressure["organization_state_count"] >= 2:
        adjustments["rule_grounding_hit_rate"] = adjustments.get("rule_grounding_hit_rate", 0.0) + 1.0
        adjustments["conflict_chain_hit_rate"] = adjustments.get("conflict_chain_hit_rate", 0.0) + 1.0
    if pressure["career_state_count"] >= 2:
        adjustments["outline_alignment_rate"] = adjustments.get("outline_alignment_rate", 0.0) + 1.0
        adjustments["payoff_chain_rate"] = adjustments.get("payoff_chain_rate", 0.0) + 1.0
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


def _resolve_quality_gate_recommended_action(
    *,
    focus_areas: Sequence[str],
    weakest: Optional[Mapping[str, Any]],
    continuity_preflight: Mapping[str, Any],
) -> Dict[str, Optional[str]]:
    ordered_areas: list[str] = []
    seen: set[str] = set()

    def add_area(value: Any) -> None:
        area = str(value or "").strip()
        if not area or area in seen:
            return
        seen.add(area)
        ordered_areas.append(area)

    for area in continuity_preflight.get("focus_areas") or []:
        add_area(area)
    for area in focus_areas or []:
        add_area(area)
    if weakest is not None:
        add_area(weakest.get("focus_area"))

    action_rules: Dict[str, tuple[str, str, str]] = {
        "opening": ("rewrite_opening", "重写开场钩子", "rewrite"),
        "dialogue": ("strengthen_dialogue", "增强对白张力", "dialogue"),
        "relationship_continuity": ("strengthen_dialogue", "增强对白张力", "dialogue"),
        "payoff": ("patch_payoff", "补强回报兑现", "payoff"),
        "foreshadow_continuity": ("patch_payoff", "补强回报兑现", "payoff"),
        "cliffhanger": ("patch_payoff", "补强回报兑现", "payoff"),
        "outline": ("bridge_scene", "补桥关键场景", "bridge"),
        "conflict": ("bridge_scene", "补桥关键场景", "bridge"),
        "pacing": ("bridge_scene", "补桥关键场景", "bridge"),
        "character_continuity": ("bridge_scene", "补桥关键场景", "bridge"),
        "organization_continuity": ("bridge_scene", "补桥关键场景", "bridge"),
        "career_continuity": ("bridge_scene", "补桥关键场景", "bridge"),
        "rule_grounding": ("grounding_pass", "强化设定落地", "grounding"),
    }

    for area in ordered_areas:
        matched = action_rules.get(area)
        if not matched:
            continue
        action, label, mode = matched
        return {
            "recommended_action": action,
            "recommended_action_label": label,
            "recommended_action_mode": mode,
            "recommended_focus_area": area,
        }

    return {
        "recommended_action": None,
        "recommended_action_label": None,
        "recommended_action_mode": None,
        "recommended_focus_area": None,
    }


def _collect_recent_failed_metric_counts(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str,
) -> list[Dict[str, Any]]:
    counts: Dict[str, Dict[str, Any]] = {}
    for item in history:
        gate = item.get("quality_gate") if isinstance(item.get("quality_gate"), Mapping) else build_quality_gate_decision(item, scope=scope)
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
        key=lambda item: (-int(item.get("count") or 0), str(item.get("label") or item.get("key") or "")),
    )[:6]


def _collect_recent_quality_gate_counts(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str,
) -> tuple[Dict[str, int], int, int]:
    gate_counts: Dict[str, int] = {"pass": 0, "repairable": 0, "blocked": 0, "unknown": 0}
    manual_review_count = 0
    auto_repair_count = 0

    for item in history:
        gate = item.get("quality_gate") if isinstance(item.get("quality_gate"), Mapping) else build_quality_gate_decision(item, scope=scope)
        status = str(gate.get("status") or "unknown")
        gate_counts[status] = gate_counts.get(status, 0) + 1

        decision = str(gate.get("decision") or "")
        if decision == "manual_review":
            manual_review_count += 1
        elif decision == "auto_repair":
            auto_repair_count += 1

    return gate_counts, manual_review_count, auto_repair_count


def _build_empty_quality_gate(*, overall_score: Optional[float] = None) -> Dict[str, Any]:
    return {
        "status": "unknown",
        "decision": "unknown",
        "label": "待评估",
        "summary": "尚未生成质量闸门结果。",
        "reason": "缺少质量指标",
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
        "recommended_action": None,
        "recommended_action_label": None,
        "recommended_action_mode": None,
        "recommended_focus_area": None,
        "continuity_warning_count": 0,
        "continuity_preflight": None,
        "manual_review_threshold": None,
        "allow_save_threshold": None,
        "quality_runtime_pressure": None,
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
    merged["organization_state_ledger"] = _normalize_runtime_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_items(ctx.get("organization_state_ledger"), limit=4)],
        limit=4,
    )
    merged["career_state_ledger"] = _normalize_runtime_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_items(ctx.get("career_state_ledger"), limit=4)],
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


def _extract_continuity_preflight(metrics: Mapping[str, Any]) -> Dict[str, Any]:
    if not isinstance(metrics, Mapping):
        return {}
    payload = metrics.get("continuity_preflight")
    return dict(payload) if isinstance(payload, Mapping) else {}


def _aggregate_continuity_preflight(history: Sequence[Mapping[str, Any]]) -> Dict[str, Any]:
    recent_items = [
        _extract_continuity_preflight(item)
        for item in history[-3:]
        if isinstance(item, Mapping) and _extract_continuity_preflight(item)
    ]
    if not recent_items:
        return {}

    warnings: list[Dict[str, Any]] = []
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
            if isinstance(repair_target, str) and repair_target and repair_target not in repair_targets:
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

    labels = ", ".join(dict.fromkeys(str(warning.get("ledger_label") or "") for warning in warnings if warning.get("ledger_label")))
    summary = f"Recent chapters show {warning_count} continuity handoff gaps."
    if labels:
        summary = f"Recent chapters show {warning_count} continuity handoff gaps. Prioritize {labels}."
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



def _average_metric_values(values: Sequence[Optional[float]], *, digits: int = 1) -> Optional[float]:
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


def _build_pacing_imbalance_summary(history: Sequence[Mapping[str, Any]]) -> Dict[str, Any]:
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

    signals: list[Dict[str, Any]] = []
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
        leading_labels = "、".join(str(signal.get("label") or signal.get("key") or "") for signal in signals[:2])
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
) -> Dict[str, Any]:
    normalized_metrics = dict(metrics)
    if not isinstance(normalized_metrics.get("repair_guidance"), Mapping):
        normalized_metrics["repair_guidance"] = build_story_repair_guidance(normalized_metrics, scope=scope)
    if not isinstance(normalized_metrics.get("quality_gate"), Mapping):
        normalized_metrics["quality_gate"] = build_quality_gate_decision(normalized_metrics, scope=scope)
    return normalized_metrics



def build_quality_metrics_summary_state(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str = "batch",
) -> Optional[Dict[str, Any]]:
    normalized_history = [
        _normalize_quality_metrics_history_item(item, scope=scope)
        for item in history
        if isinstance(item, Mapping) and item
    ]
    if not normalized_history:
        return None

    state: Dict[str, Any] = {
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
) -> Optional[Dict[str, Any]]:
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
) -> Optional[Dict[str, Any]]:
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

    summary: Dict[str, Any] = {
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
    summary["repair_guidance"] = build_story_repair_guidance(summary, scope=scope)
    summary["quality_gate"] = build_quality_gate_decision(summary, scope=scope)
    return summary



def build_quality_metrics_summary(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str = "batch",
) -> Optional[Dict[str, Any]]:
    """??????????????????????"""
    summary_state = build_quality_metrics_summary_state(history, scope=scope)
    return build_quality_metrics_summary_from_state(summary_state, scope=scope)


def build_story_repair_guidance(
    metrics: Mapping[str, Any],
    *,
    scope: str = "chapter",
) -> Dict[str, Any]:
    """根据质量指标生成修复指引与重点补强方向。"""

    if not isinstance(metrics, Mapping):
        return _build_empty_guidance()

    runtime_context = _extract_quality_runtime_context(metrics)
    continuity_preflight = _extract_continuity_preflight(metrics)
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
    if not preserve_strengths and weakest["label"] != "综合质量":
        preserve_strengths = ["保留当前已成立的章节优势与角色辨识度。"]

    focus_areas = list(dict.fromkeys(item["focus_area"] for item in low_items))
    if not focus_areas:
        focus_areas = [weakest["focus_area"]]

    continuity_targets = [
        str(target).strip()
        for target in (continuity_preflight.get("repair_targets") or [])
        if str(target).strip()
    ]
    continuity_focus_areas = [
        str(area).strip()
        for area in (continuity_preflight.get("focus_areas") or [])
        if str(area).strip()
    ]
    if continuity_targets:
        repair_targets = continuity_targets + repair_targets
    if continuity_focus_areas:
        focus_areas = continuity_focus_areas + focus_areas

    pressure = _build_runtime_pressure(runtime_context)
    if "payoff" in focus_areas and pressure["foreshadow_state_items"]:
        repair_targets.insert(0, f"优先回应伏笔账本：{' / '.join(pressure['foreshadow_state_items'][:2])}。")
    if any(area in focus_areas for area in ("conflict", "outline", "pacing", "character_continuity")) and pressure["character_state_items"]:
        repair_targets.append(f"把角色当前状态落实进动作与代价：{' / '.join(pressure['character_state_items'][:2])}。")
    if any(area in focus_areas for area in ("dialogue", "conflict", "relationship_continuity")) and pressure["relationship_state_items"]:
        repair_targets.append(f"把关系变化落实进对白或站队：{' / '.join(pressure['relationship_state_items'][:2])}。")
    repair_targets = list(dict.fromkeys(repair_targets))[:4]
    focus_areas = list(dict.fromkeys(focus_areas))[:4]

    continuity_summary = str(continuity_preflight.get("summary") or "").strip()
    if low_items:
        labels = " / ".join(item["label"] for item in low_items)
        if stage_label:
            summary = f"{scope_label}在{stage_label}阶段主要短板集中在{labels}，建议按优先级修补。"
        else:
            summary = f"{scope_label}当前主要短板集中在{labels}，建议按优先级修补。"
        if continuity_summary:
            summary = f"{summary} {continuity_summary}"
    elif continuity_summary:
        summary = continuity_summary
    else:
        strongest_label = strength_items[0]["label"] if strength_items else weakest["label"]
        if stage_label:
            summary = f"{scope_label}在{stage_label}阶段整体稳定，可继续保持{strongest_label}上的优势。"
        else:
            summary = f"{scope_label}整体稳定，可继续保持{strongest_label}上的优势。"

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
    """根据质量指标生成质量闸门结论与推荐动作。"""

    if not isinstance(metrics, Mapping):
        return _build_empty_quality_gate()

    runtime_context = _extract_quality_runtime_context(metrics)
    continuity_preflight = _extract_continuity_preflight(metrics)
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
    recommended_action = _resolve_quality_gate_recommended_action(
        focus_areas=focus_areas,
        weakest=weakest,
        continuity_preflight=continuity_preflight,
    )

    blocked_reasons: list[str] = []
    if overall_score is not None and overall_score < float(thresholds["manual_review_score"]):
        blocked_reasons.append(f"总分 {overall_score:.1f} 低于人工复核线")
    if weak_metric_count >= int(thresholds["weak_metric_block_count"]):
        blocked_reasons.append(f"存在 {weak_metric_count} 个弱项指标")
    if normalized_gap >= float(thresholds["normalized_gap"]):
        blocked_reasons.append(f"最弱项{weakest['label']}缺口过大")
    if stage == "ending" and pressure["foreshadow_state_count"] >= 3 and weakest["focus_area"] == "payoff":
        blocked_reasons.append("收束段伏笔压力过高，需人工复核兑现节奏")

    scope_label = _resolve_scope_label(scope)
    if blocked_reasons:
        status = "blocked"
        decision = "manual_review"
        label = "需复核"
        reason = "；".join(blocked_reasons)
        if stage_label:
            summary = f"{scope_label}在{stage_label}阶段暂不建议直接保存，建议先人工复核再决定是否重写。"
        else:
            summary = f"{scope_label}暂不建议直接保存，建议先人工复核再决定是否重写。"
    elif weak_metric_count > 0 or (overall_score is not None and overall_score < float(thresholds["allow_save_score"])):
        status = "repairable"
        decision = "auto_repair"
        label = "可修复"
        if weak_metric_count > 0:
            reason = f"存在 {weak_metric_count} 个待修复弱项"
        else:
            reason = "综合分未达直接保存阈值"
        if stage_label:
            summary = f"{scope_label}在{stage_label}阶段仍有明显短板，建议先按修复指引补强后再保存。"
        else:
            summary = f"{scope_label}仍有明显短板，建议先按修复指引补强后再保存。"
    else:
        status = "pass"
        decision = "allow_save"
        label = "可保存"
        reason = "质量指标达到保存要求"
        if stage_label:
            summary = f"{scope_label}在{stage_label}阶段通过质量闸门，可继续保存或进入下一步。"
        else:
            summary = f"{scope_label}已通过质量闸门，可继续保存或进入下一步。"

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
        "continuity_warning_count": int(continuity_preflight.get("warning_count") or 0),
        "continuity_preflight": continuity_preflight or None,
        "manual_review_threshold": thresholds["manual_review_score"],
        "allow_save_threshold": thresholds["allow_save_score"],
        "quality_runtime_pressure": pressure,
        **recommended_action,
    }


def test_should_detect_organization_and_career_continuity_gaps():
    runtime_context = {
        "organization_state_ledger": ["ShadowGuild: power=72; location=North Dock"],
        "career_state_ledger": ["Lin/Strategist: stage 3; promotion blocked by council"],
    }

    preflight = build_story_continuity_preflight(
        "Lin argued with Su in the archive, but the guild structure and strategist bottleneck were never mentioned.",
        runtime_context,
    )

    assert preflight["status"] == "warning"
    assert preflight["warning_count"] == 2
    assert "organization_continuity" in preflight["focus_areas"]
    assert "career_continuity" in preflight["focus_areas"]
