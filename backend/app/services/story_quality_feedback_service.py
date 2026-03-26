"""故事质量修复建议服务 - 将质量指标转换为稳定的结构化修复目标。"""
from __future__ import annotations

from dataclasses import dataclass
import json
import re
from typing import Any, Dict, Mapping, Optional, Sequence

from app.services.novel_quality_profile_service import (
    QUALITY_PROFILE_GENRE_LABELS,
    QUALITY_PROFILE_PRESET_LABELS,
    QUALITY_PROFILE_STYLE_LABELS,
    resolve_quality_weight_profile,
    resolve_runtime_quality_profile,
)
from app.services.story_quality_repair_effectiveness_service import (
    build_repair_effectiveness_summary,
)
from app.services.story_runtime_serialization_service import (
    extract_story_runtime_snapshot_from_contract,
)


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


RuntimeContextItem = str | Dict[str, Any]


def _normalize_runtime_context_item_mapping(value: Mapping[str, Any]) -> Optional[Dict[str, Any]]:
    summary = str(
        value.get("summary")
        or value.get("content")
        or value.get("item")
        or value.get("value")
        or ""
    ).strip()
    label = str(value.get("label") or value.get("name") or value.get("title") or "").strip()
    status = str(value.get("status") or "").strip().lower()
    target_chapter = _safe_float(value.get("target_chapter"))

    normalized: Dict[str, Any] = {}
    if summary:
        normalized["summary"] = summary
    if label:
        normalized["label"] = label
    if status:
        normalized["status"] = status
    if target_chapter is not None:
        normalized["target_chapter"] = int(target_chapter)
    return normalized or None


def _stringify_runtime_context_item(value: Any) -> str:
    if isinstance(value, Mapping):
        normalized = _normalize_runtime_context_item_mapping(value)
        if not normalized:
            return ""
        summary = str(normalized.get("summary") or "").strip()
        label = str(normalized.get("label") or "").strip()
        status = str(normalized.get("status") or "").strip()
        target_chapter = normalized.get("target_chapter")

        if label and summary and label != summary:
            text = f"{label}: {summary}"
        else:
            text = summary or label

        meta_parts: list[str] = []
        if status:
            meta_parts.append(status)
        if isinstance(target_chapter, int):
            meta_parts.append(f"chapter {target_chapter}")
        if meta_parts:
            text = f"{text} ({', '.join(meta_parts)})"
        return text.strip()
    return str(value or "").strip()


def _normalize_runtime_context_items(values: Any, *, limit: int = 4) -> list[RuntimeContextItem]:
    if values is None:
        return []
    if isinstance(values, str):
        raw_items = [values]
    elif isinstance(values, Sequence) and not isinstance(values, (str, bytes, bytearray)):
        raw_items = list(values)
    else:
        raw_items = [values]

    items: list[RuntimeContextItem] = []
    seen: set[str] = set()
    for value in raw_items:
        if isinstance(value, Mapping):
            normalized_mapping = _normalize_runtime_context_item_mapping(value)
            if not normalized_mapping:
                continue
            dedupe_key = json.dumps(normalized_mapping, sort_keys=True, ensure_ascii=False)
            normalized_value: RuntimeContextItem = normalized_mapping
        else:
            text = str(value or "").strip()
            if not text:
                continue
            dedupe_key = text
            normalized_value = text

        if dedupe_key in seen:
            continue
        seen.add(dedupe_key)
        items.append(normalized_value)
        if len(items) >= limit:
            break
    return items


def _normalize_runtime_context_item_texts(values: Any, *, limit: int = 4) -> list[str]:
    return [
        text
        for text in (
            _stringify_runtime_context_item(item)
            for item in _normalize_runtime_context_items(values, limit=limit)
        )
        if text
    ]


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
        "【": " ",
        "】": " ",
        "[": " ",
        "]": " ",
        "（": " ",
        "）": " ",
        "(": " ",
        ")": " ",
        "<": " ",
        ">": " ",
        "《": " ",
        "》": " ",
        "“": " ",
        "”": " ",
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
        for item in _normalize_runtime_context_items(runtime_context.get(ledger_key), limit=3):
            item_text = _stringify_runtime_context_item(item)
            if not item_text:
                continue
            checked_item_count += 1
            anchors = _extract_continuity_anchor_candidates(item_text)
            matched_anchor_count = len({anchor for anchor in anchors if len(anchor) >= 2 and anchor in normalized_content})
            required_match_count = 2 if ledger_key in {"relationship_state_ledger", "career_state_ledger"} and len(anchors) >= 2 else 1
            is_matched = matched_anchor_count >= required_match_count
            if is_matched:
                continue
            missing_item_count += 1
            if focus_area not in focus_areas:
                focus_areas.append(focus_area)
            target = repair_template.format(item=item_text)
            if target not in repair_targets:
                repair_targets.append(target)
            warnings.append({
                "ledger_key": ledger_key,
                "ledger_label": ledger_label,
                "focus_area": focus_area,
                "item": item_text,
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
    if isinstance(runtime_snapshot, Mapping):
        return dict(runtime_snapshot)
    runtime_contract = payload.get("story_runtime_contract")
    extracted_snapshot = (
        extract_story_runtime_snapshot_from_contract(runtime_contract)
        if isinstance(runtime_contract, Mapping)
        else None
    )
    return dict(extracted_snapshot) if isinstance(extracted_snapshot, Mapping) else {}


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
    character_state_entries = _normalize_runtime_context_items(runtime_context.get("character_state_ledger"), limit=6)
    relationship_state_entries = _normalize_runtime_context_items(runtime_context.get("relationship_state_ledger"), limit=6)
    foreshadow_state_entries = _normalize_runtime_context_items(runtime_context.get("foreshadow_state_ledger"), limit=6)
    organization_state_entries = _normalize_runtime_context_items(runtime_context.get("organization_state_ledger"), limit=6)
    career_state_entries = _normalize_runtime_context_items(runtime_context.get("career_state_ledger"), limit=6)
    return {
        "character_state_count": len(character_state_entries),
        "relationship_state_count": len(relationship_state_entries),
        "foreshadow_state_count": len(foreshadow_state_entries),
        "organization_state_count": len(organization_state_entries),
        "career_state_count": len(career_state_entries),
        "character_state_items": [_stringify_runtime_context_item(item) for item in character_state_entries[:3]],
        "relationship_state_items": [_stringify_runtime_context_item(item) for item in relationship_state_entries[:3]],
        "foreshadow_state_items": [_stringify_runtime_context_item(item) for item in foreshadow_state_entries[:3]],
        "organization_state_items": [_stringify_runtime_context_item(item) for item in organization_state_entries[:3]],
        "career_state_items": [_stringify_runtime_context_item(item) for item in career_state_entries[:3]],
    }


def _resolve_adaptive_quality_gate_profile(runtime_context: Mapping[str, Any]) -> Dict[str, Any]:
    resolved_stage = _resolve_quality_stage(runtime_context)
    runtime_profile = resolve_runtime_quality_profile(runtime_context or {})
    weight_profile = resolve_quality_weight_profile(runtime_context or {}, resolved_stage)
    return {
        "resolved_stage": resolved_stage,
        "quality_preset": str(runtime_profile.get("quality_preset") or "").strip(),
        "style_profile": str(runtime_profile.get("style_profile") or "").strip(),
        "genre_profiles": _normalize_runtime_items(runtime_profile.get("genre_profiles"), limit=4),
        "focus_areas": _normalize_runtime_items(weight_profile.get("focus_areas"), limit=4),
        "weight_profile": weight_profile,
    }


def _resolve_metric_threshold_adjustments(runtime_context: Mapping[str, Any]) -> Dict[str, float]:
    stage = _resolve_quality_stage(runtime_context)
    adaptive_profile = _resolve_adaptive_quality_gate_profile(runtime_context)
    quality_preset = str(adaptive_profile.get("quality_preset") or "").strip()
    style_profile = str(adaptive_profile.get("style_profile") or "").strip()
    genre_profiles = set(adaptive_profile.get("genre_profiles") or [])
    focus_areas = list(adaptive_profile.get("focus_areas") or [])
    creative_mode = str(runtime_context.get("creative_mode") or "").strip()
    story_focus = str(runtime_context.get("story_focus") or "").strip()

    adjustments: Dict[str, float] = {}

    def add_adjustment(key: str, delta: float) -> None:
        adjustments[key] = adjustments.get(key, 0.0) + delta

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

    if quality_preset == "emotion_drama":
        add_adjustment("dialogue_naturalness_rate", 2.0)
        add_adjustment("payoff_chain_rate", 2.0)
        add_adjustment("conflict_chain_hit_rate", 1.0)
    elif quality_preset == "clean_prose":
        add_adjustment("pacing_score", 0.5)
        add_adjustment("dialogue_naturalness_rate", 1.0)
        add_adjustment("rule_grounding_hit_rate", 0.5)
    elif quality_preset == "plot_drive":
        add_adjustment("conflict_chain_hit_rate", 2.0)
        add_adjustment("cliffhanger_rate", 1.0)
        add_adjustment("outline_alignment_rate", 1.0)
    elif quality_preset == "immersive":
        add_adjustment("dialogue_naturalness_rate", 1.0)
        add_adjustment("rule_grounding_hit_rate", 1.0)
        add_adjustment("pacing_score", 0.2)

    if style_profile == "urban_finance":
        add_adjustment("rule_grounding_hit_rate", 3.0)
        add_adjustment("outline_alignment_rate", 1.0)
        add_adjustment("dialogue_naturalness_rate", 1.0)
    elif style_profile == "tech_xianxia":
        add_adjustment("rule_grounding_hit_rate", 4.0)
        add_adjustment("payoff_chain_rate", 1.0)
        add_adjustment("outline_alignment_rate", 1.0)
    elif style_profile == "low_ai_life":
        add_adjustment("dialogue_naturalness_rate", 2.0)
        add_adjustment("payoff_chain_rate", 1.0)
        add_adjustment("cliffhanger_rate", -1.0)
    elif style_profile == "low_ai_serial":
        add_adjustment("conflict_chain_hit_rate", 1.0)
        add_adjustment("payoff_chain_rate", 1.0)
        add_adjustment("cliffhanger_rate", 1.0)

    if "romance_slice_of_life" in genre_profiles:
        add_adjustment("dialogue_naturalness_rate", 2.0)
        add_adjustment("payoff_chain_rate", 1.0)
        add_adjustment("cliffhanger_rate", -1.0)
    if "suspense_mystery" in genre_profiles:
        add_adjustment("conflict_chain_hit_rate", 1.0)
        add_adjustment("cliffhanger_rate", 2.0)
        add_adjustment("outline_alignment_rate", 1.0)
    if "xianxia_fantasy" in genre_profiles:
        add_adjustment("rule_grounding_hit_rate", 2.0)
        add_adjustment("payoff_chain_rate", 1.0)
    if "science_fiction_tech" in genre_profiles:
        add_adjustment("rule_grounding_hit_rate", 2.0)
        add_adjustment("outline_alignment_rate", 1.0)
    if "history_power" in genre_profiles:
        add_adjustment("rule_grounding_hit_rate", 2.0)
        add_adjustment("outline_alignment_rate", 1.0)
        add_adjustment("conflict_chain_hit_rate", 1.0)

    if creative_mode in {"hook", "suspense"}:
        add_adjustment("opening_hook_rate", 1.0)
        add_adjustment("cliffhanger_rate", 1.0)
    elif creative_mode == "emotion":
        add_adjustment("dialogue_naturalness_rate", 1.0)
        add_adjustment("payoff_chain_rate", 1.0)
    elif creative_mode == "relationship":
        add_adjustment("dialogue_naturalness_rate", 2.0)
        add_adjustment("conflict_chain_hit_rate", 1.0)
    elif creative_mode == "payoff":
        add_adjustment("payoff_chain_rate", 2.0)

    if story_focus == "advance_plot":
        add_adjustment("conflict_chain_hit_rate", 1.0)
        add_adjustment("outline_alignment_rate", 1.0)
    elif story_focus == "deepen_character":
        add_adjustment("dialogue_naturalness_rate", 1.0)
        add_adjustment("payoff_chain_rate", 1.0)
    elif story_focus == "escalate_conflict":
        add_adjustment("conflict_chain_hit_rate", 2.0)
        add_adjustment("cliffhanger_rate", 1.0)
    elif story_focus == "reveal_mystery":
        add_adjustment("outline_alignment_rate", 1.0)
        add_adjustment("cliffhanger_rate", 1.0)
    elif story_focus == "relationship_shift":
        add_adjustment("dialogue_naturalness_rate", 2.0)
        add_adjustment("conflict_chain_hit_rate", 1.0)
    elif story_focus == "foreshadow_payoff":
        add_adjustment("payoff_chain_rate", 2.0)

    focus_metric_map = {
        "opening": ("opening_hook_rate", 1.0),
        "conflict": ("conflict_chain_hit_rate", 1.0),
        "outline": ("outline_alignment_rate", 1.0),
        "dialogue": ("dialogue_naturalness_rate", 1.0),
        "payoff": ("payoff_chain_rate", 1.0),
        "cliffhanger": ("cliffhanger_rate", 1.0),
        "rule_grounding": ("rule_grounding_hit_rate", 1.0),
        "pacing": ("pacing_score", 0.2),
    }
    for focus_area in focus_areas:
        mapped = focus_metric_map.get(str(focus_area or "").strip())
        if not mapped:
            continue
        metric_key, delta = mapped
        add_adjustment(metric_key, delta)

    pressure = _build_runtime_pressure(runtime_context)
    if pressure["foreshadow_state_count"] >= 3:
        add_adjustment("payoff_chain_rate", 2.0)
    if pressure["relationship_state_count"] >= 2:
        add_adjustment("dialogue_naturalness_rate", 1.0)
        add_adjustment("conflict_chain_hit_rate", 1.0)
    if pressure["character_state_count"] >= 3:
        add_adjustment("outline_alignment_rate", 1.0)
    if pressure["organization_state_count"] >= 2:
        add_adjustment("rule_grounding_hit_rate", 1.0)
        add_adjustment("conflict_chain_hit_rate", 1.0)
    if pressure["career_state_count"] >= 2:
        add_adjustment("outline_alignment_rate", 1.0)
        add_adjustment("payoff_chain_rate", 1.0)
    return adjustments


def _resolve_gate_thresholds(runtime_context: Mapping[str, Any]) -> Dict[str, Any]:
    stage = _resolve_quality_stage(runtime_context)
    adaptive_profile = _resolve_adaptive_quality_gate_profile(runtime_context)
    quality_preset = str(adaptive_profile.get("quality_preset") or "").strip()
    style_profile = str(adaptive_profile.get("style_profile") or "").strip()
    genre_profiles = set(adaptive_profile.get("genre_profiles") or [])
    focus_areas = set(adaptive_profile.get("focus_areas") or [])
    creative_mode = str(runtime_context.get("creative_mode") or "").strip()
    story_focus = str(runtime_context.get("story_focus") or "").strip()

    thresholds: Dict[str, Any] = {
        "stage": stage,
        "stage_label": QUALITY_STAGE_LABELS.get(stage, ""),
        "manual_review_score": 70.0,
        "allow_save_score": 82.0,
        "normalized_gap": 12.0,
        "weak_metric_block_count": 3,
        "allow_save_weak_metric_count": 0,
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

    if quality_preset == "emotion_drama":
        thresholds["manual_review_score"] = min(float(thresholds["manual_review_score"]), 69.0)
        thresholds["allow_save_score"] = min(float(thresholds["allow_save_score"]), 81.0)
        thresholds["normalized_gap"] = max(float(thresholds["normalized_gap"]), 12.5)
    elif quality_preset == "clean_prose":
        thresholds["manual_review_score"] = max(float(thresholds["manual_review_score"]), 71.0)
        thresholds["allow_save_score"] = max(float(thresholds["allow_save_score"]), 83.0)
        thresholds["normalized_gap"] = min(float(thresholds["normalized_gap"]), 11.0)
    elif quality_preset == "plot_drive":
        thresholds["allow_save_score"] = max(float(thresholds["allow_save_score"]), 82.5)
    elif quality_preset == "immersive":
        thresholds["manual_review_score"] = max(float(thresholds["manual_review_score"]), 70.5)

    if story_focus == "relationship_shift" or creative_mode in {"relationship", "emotion"}:
        thresholds["weak_metric_block_count"] = max(int(thresholds["weak_metric_block_count"]), 4)
        thresholds["allow_save_weak_metric_count"] = max(int(thresholds["allow_save_weak_metric_count"]), 2)
        thresholds["normalized_gap"] = max(float(thresholds["normalized_gap"]), 12.5)

    if story_focus == "foreshadow_payoff" or creative_mode == "payoff":
        thresholds["allow_save_score"] = max(float(thresholds["allow_save_score"]), 83.0)
        thresholds["normalized_gap"] = min(float(thresholds["normalized_gap"]), 11.0)

    grounding_profiles = {"urban_finance", "tech_xianxia"}
    if style_profile in grounding_profiles or genre_profiles.intersection({"science_fiction_tech", "history_power", "xianxia_fantasy"}):
        thresholds["manual_review_score"] = max(float(thresholds["manual_review_score"]), 71.0)
        thresholds["allow_save_score"] = max(float(thresholds["allow_save_score"]), 83.0)
        thresholds["normalized_gap"] = min(float(thresholds["normalized_gap"]), 11.0)

    if "rule_grounding" in focus_areas:
        thresholds["allow_save_score"] = max(float(thresholds["allow_save_score"]), 82.5)

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
    merged["character_focus"] = _normalize_runtime_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_items(ctx.get("character_focus"), limit=4)],
        limit=4,
    )
    merged["foreshadow_payoff_plan"] = _normalize_runtime_context_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_context_items(ctx.get("foreshadow_payoff_plan"), limit=6)],
        limit=6,
    )
    merged["character_state_ledger"] = _normalize_runtime_context_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_context_items(ctx.get("character_state_ledger"), limit=4)],
        limit=4,
    )
    merged["relationship_state_ledger"] = _normalize_runtime_context_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_context_items(ctx.get("relationship_state_ledger"), limit=4)],
        limit=4,
    )
    merged["foreshadow_state_ledger"] = _normalize_runtime_context_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_context_items(ctx.get("foreshadow_state_ledger"), limit=4)],
        limit=4,
    )
    merged["organization_state_ledger"] = _normalize_runtime_context_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_context_items(ctx.get("organization_state_ledger"), limit=4)],
        limit=4,
    )
    merged["career_state_ledger"] = _normalize_runtime_context_items(
        [entry for ctx in contexts[-3:] for entry in _normalize_runtime_context_items(ctx.get("career_state_ledger"), limit=4)],
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



def _resolve_summary_metric_value(summary: Mapping[str, Any], metric_key: str) -> Optional[float]:
    if metric_key == "pacing_score":
        value = _safe_float(summary.get("avg_pacing_score"))
        return round(value * 10, 1) if value is not None else None
    return _safe_float(summary.get(f"avg_{metric_key}"))



def _build_volume_goal_completion_summary(summary: Mapping[str, Any]) -> Dict[str, Any]:
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
    weights = weight_profile.get("weights") if isinstance(weight_profile.get("weights"), Mapping) else {}

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
        expected_index = stage_sequence.index(expected_stage) if expected_stage in stage_sequence else None
        current_index = stage_sequence.index(current_stage) if current_stage in stage_sequence else None
        if expected_index is not None and current_index is not None:
            stage_alignment = max(40.0, 100.0 - abs(expected_index - current_index) * 35.0)
    completion_rate = round(((base_completion * 0.72) + ((stage_alignment if stage_alignment is not None else 85.0) * 0.28)), 1)

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



def _build_foreshadow_payoff_delay_summary(summary: Mapping[str, Any]) -> Dict[str, Any]:
    runtime_context = _extract_quality_runtime_context(summary)
    foreshadow_payoff_plan = _normalize_runtime_context_item_texts(runtime_context.get("foreshadow_payoff_plan"), limit=6)
    foreshadow_state_ledger = _normalize_runtime_context_item_texts(runtime_context.get("foreshadow_state_ledger"), limit=6)
    recent_payoff_rate = _safe_float(summary.get("avg_payoff_chain_rate"))
    pacing_imbalance = summary.get("pacing_imbalance") if isinstance(summary.get("pacing_imbalance"), Mapping) else {}
    recent_payoff_momentum = _safe_float(pacing_imbalance.get("recent_payoff_momentum"))

    if not foreshadow_payoff_plan and not foreshadow_state_ledger and recent_payoff_rate is None:
        return {}

    outstanding_count = max(len(foreshadow_payoff_plan), len(foreshadow_state_ledger))
    current = _safe_float(runtime_context.get("current_chapter_number"))
    total = _safe_float(runtime_context.get("chapter_count"))
    progress_ratio = (current / total) if current is not None and total not in {None, 0} else None

    backlog_pressure = min(100.0, outstanding_count * 18.0)
    payoff_gap = max(0.0, 78.0 - recent_payoff_rate) if recent_payoff_rate is not None else (18.0 if outstanding_count > 0 else 0.0)
    momentum_gap = max(0.0, 76.0 - recent_payoff_momentum) if recent_payoff_momentum is not None else (10.0 if outstanding_count > 1 else 0.0)
    progress_multiplier = 1.0
    if progress_ratio is not None and progress_ratio >= 0.75:
        progress_multiplier = 1.15
    elif progress_ratio is not None and progress_ratio >= 0.55:
        progress_multiplier = 1.05

    delay_index = round(min(100.0, (backlog_pressure * 0.45 + payoff_gap * 0.35 + momentum_gap * 0.20) * progress_multiplier), 1)

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

    backlog_label = ' / '.join(foreshadow_state_ledger[:2])
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



def _evaluate_repair_focus_improvement(
    current_item: Mapping[str, Any],
    next_item: Mapping[str, Any],
    focus_area: str,
) -> Optional[Dict[str, Any]]:
    metric_spec = REPAIR_EFFECTIVENESS_METRIC_MAP.get(focus_area)
    if metric_spec is None:
        return None

    metric_key, safe_threshold, improvement_threshold = metric_spec
    current_value = _safe_float(current_item.get(metric_key))
    next_value = _safe_float(next_item.get(metric_key))
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



def _build_repair_effectiveness_summary(
    history: Sequence[Mapping[str, Any]],
    *,
    scope: str,
) -> Dict[str, Any]:
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
            evaluation = _evaluate_repair_focus_improvement(current_item, next_item, focus_area)
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
            state["delta_total"] = round(_safe_float(state.get("delta_total")) + evaluation["delta"], 6)
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
                "avg_delta": round(_safe_float(state.get("delta_total")) / area_pairs, 1),
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
    volume_goal_completion = _build_volume_goal_completion_summary(summary)
    if volume_goal_completion:
        summary["volume_goal_completion"] = volume_goal_completion
    foreshadow_payoff_delay = _build_foreshadow_payoff_delay_summary(summary)
    if foreshadow_payoff_delay:
        summary["foreshadow_payoff_delay"] = foreshadow_payoff_delay
    repair_effectiveness = build_repair_effectiveness_summary(
        recent_history,
        scope=scope,
        history_normalizer=_normalize_quality_metrics_history_item,
        repair_guidance_builder=build_story_repair_guidance,
        runtime_items_normalizer=_normalize_runtime_items,
        safe_float=_safe_float,
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
) -> Optional[Dict[str, Any]]:
    """汇总最近章节质量指标，生成可直接用于提示和看板展示的摘要。"""
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
    pacing_imbalance = metrics.get("pacing_imbalance") if isinstance(metrics.get("pacing_imbalance"), Mapping) else {}
    volume_goal_completion = metrics.get("volume_goal_completion") if isinstance(metrics.get("volume_goal_completion"), Mapping) else {}
    foreshadow_payoff_delay = metrics.get("foreshadow_payoff_delay") if isinstance(metrics.get("foreshadow_payoff_delay"), Mapping) else {}
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

    priority_signal_summaries: list[str] = []
    for signal_payload in (volume_goal_completion, foreshadow_payoff_delay, pacing_imbalance):
        signal_status = str(signal_payload.get("status") or "").strip().lower()
        if signal_status not in {"watch", "warning"}:
            continue
        signal_summary = str(signal_payload.get("summary") or "").strip()
        if signal_summary:
            priority_signal_summaries.append(signal_summary)
        signal_targets = [
            str(target).strip()
            for target in (signal_payload.get("repair_targets") or [])
            if str(target).strip()
        ]
        signal_focus_areas = [
            str(area).strip()
            for area in (signal_payload.get("focus_areas") or [])
            if str(area).strip()
        ]
        if signal_targets:
            repair_targets = signal_targets + repair_targets
        if signal_focus_areas:
            focus_areas = signal_focus_areas + focus_areas

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

    if priority_signal_summaries:
        signal_summary = next((item for item in priority_signal_summaries if item not in summary), "")
        if signal_summary:
            summary = f"{summary} {signal_summary}"

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
    elif weak_metric_count > int(thresholds.get("allow_save_weak_metric_count") or 0) or (
        overall_score is not None and overall_score < float(thresholds["allow_save_score"])
    ):
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
        "weak_metric_block_count": thresholds["weak_metric_block_count"],
        "allow_save_weak_metric_count": thresholds.get("allow_save_weak_metric_count"),
        "normalized_gap_threshold": thresholds["normalized_gap"],
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
