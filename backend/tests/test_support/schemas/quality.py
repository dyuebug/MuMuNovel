from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Dict, List, Mapping, Optional, Sequence

from pydantic import BaseModel, ConfigDict, Field

from tests.test_support.schemas.novel_quality_profile_service import (
    resolve_quality_weight_profile,
    resolve_runtime_quality_profile,
)


class QualitySchemaModel(BaseModel):
    model_config = ConfigDict(extra="allow")


class StoryRepairGuidance(QualitySchemaModel):
    summary: str = ""
    repair_targets: List[str] = Field(default_factory=list)
    preserve_strengths: List[str] = Field(default_factory=list)
    focus_areas: List[str] = Field(default_factory=list)
    weakest_metric_key: Optional[str] = None
    weakest_metric_label: Optional[str] = None
    weakest_metric_value: Optional[float] = None
    quality_stage: Optional[str] = None
    quality_stage_label: Optional[str] = None
    continuity_preflight: Optional["StoryContinuityPreflight"] = None
    quality_runtime_pressure: Optional["StoryQualityRuntimePressure"] = None


class StoryQualityGateMetric(QualitySchemaModel):
    key: Optional[str] = None
    label: Optional[str] = None
    value: Optional[float] = None
    threshold: Optional[float] = None
    gap: Optional[float] = None
    focus_area: Optional[str] = None
    repair_target: Optional[str] = None


class StoryQualityMetricFrequency(QualitySchemaModel):
    key: Optional[str] = None
    label: Optional[str] = None
    focus_area: Optional[str] = None
    count: int = 0


class StoryContinuityPreflightWarning(QualitySchemaModel):
    ledger_label: Optional[str] = None
    focus_area: Optional[str] = None
    item: Optional[str] = None


class StoryContinuityPreflight(QualitySchemaModel):
    status: Optional[str] = None
    summary: Optional[str] = None
    warning_count: int = 0
    checked_item_count: int = 0
    missing_item_count: int = 0
    focus_areas: List[str] = Field(default_factory=list)
    repair_targets: List[str] = Field(default_factory=list)
    warnings: List[StoryContinuityPreflightWarning] = Field(default_factory=list)


class StoryQualityRuntimePressure(QualitySchemaModel):
    foreshadow_state_count: int = 0
    character_state_count: int = 0
    relationship_state_count: int = 0
    organization_state_count: int = 0
    career_state_count: int = 0
    foreshadow_state_items: List[str] = Field(default_factory=list)
    character_state_items: List[str] = Field(default_factory=list)
    relationship_state_items: List[str] = Field(default_factory=list)
    organization_state_items: List[str] = Field(default_factory=list)
    career_state_items: List[str] = Field(default_factory=list)




class QualityRuntimeLedgerEntry(QualitySchemaModel):
    name: Optional[str] = None
    state: Optional[str] = None
    status: Optional[str] = None
    pair: Optional[str] = None
    label: Optional[str] = None
    detail: Optional[str] = None


class QualityRuntimePlanEntry(QualitySchemaModel):
    name: Optional[str] = None
    status: Optional[str] = None
    summary: Optional[str] = None
    label: Optional[str] = None
    target_chapter: Optional[int] = None


QualityRuntimeLedgerItem = str | QualityRuntimeLedgerEntry
QualityRuntimePlanItem = str | QualityRuntimePlanEntry

class QualityRuntimeContextSummary(QualitySchemaModel):
    plot_stage: Optional[str] = None
    chapter_count: Optional[int] = None
    current_chapter_number: Optional[int] = None
    target_word_count: Optional[int] = None
    quality_preset: Optional[str] = None
    quality_notes: Optional[str] = None
    creative_mode: Optional[str] = None
    story_focus: Optional[str] = None
    story_creation_brief: Optional[str] = None
    story_long_term_goal: Optional[str] = None
    genre: Optional[str] = None
    genre_profiles: List[str] = Field(default_factory=list)
    style_name: Optional[str] = None
    style_preset_id: Optional[str] = None
    style_profile: Optional[str] = None
    chapter_number_span: List[int] = Field(default_factory=list)
    character_focus: List[str] = Field(default_factory=list)
    foreshadow_payoff_plan: List[QualityRuntimePlanItem] = Field(default_factory=list)
    character_state_ledger: List[QualityRuntimeLedgerItem] = Field(default_factory=list)
    relationship_state_ledger: List[QualityRuntimeLedgerItem] = Field(default_factory=list)
    foreshadow_state_ledger: List[QualityRuntimeLedgerItem] = Field(default_factory=list)
    organization_state_ledger: List[QualityRuntimeLedgerItem] = Field(default_factory=list)
    career_state_ledger: List[QualityRuntimeLedgerItem] = Field(default_factory=list)


class StoryPacingImbalanceSignal(QualitySchemaModel):
    key: Optional[str] = None
    label: Optional[str] = None
    severity: Optional[str] = None
    summary: Optional[str] = None
    metric: Optional[float] = None


class StoryPacingImbalanceSummary(QualitySchemaModel):
    status: Optional[str] = None
    window_size: Optional[int] = None
    signal_count: int = 0
    recent_progression_density: Optional[float] = None
    recent_payoff_momentum: Optional[float] = None
    recent_payoff_rate: Optional[float] = None
    recent_cliffhanger_pull: Optional[float] = None
    recent_tension_variation: Optional[float] = None
    signals: List[StoryPacingImbalanceSignal] = Field(default_factory=list)
    focus_areas: List[str] = Field(default_factory=list)
    repair_targets: List[str] = Field(default_factory=list)
    summary: Optional[str] = None


class StoryVolumeGoalCompletionSummary(QualitySchemaModel):
    status: Optional[str] = None
    completion_rate: Optional[float] = None
    expected_stage: Optional[str] = None
    expected_stage_label: Optional[str] = None
    current_stage: Optional[str] = None
    current_stage_label: Optional[str] = None
    stage_alignment: Optional[float] = None
    focus_areas: List[str] = Field(default_factory=list)
    repair_targets: List[str] = Field(default_factory=list)
    profile_summary: Optional[str] = None
    profile_focuses: List[str] = Field(default_factory=list)
    style_profile: Optional[str] = None
    genre_profiles: List[str] = Field(default_factory=list)
    quality_preset: Optional[str] = None
    summary: Optional[str] = None


class StoryForeshadowPayoffDelaySummary(QualitySchemaModel):
    status: Optional[str] = None
    delay_index: Optional[float] = None
    plan_count: int = 0
    backlog_count: int = 0
    recent_payoff_rate: Optional[float] = None
    recent_payoff_momentum: Optional[float] = None
    focus_areas: List[str] = Field(default_factory=list)
    repair_targets: List[str] = Field(default_factory=list)
    summary: Optional[str] = None


class StoryRepairEffectivenessFocusAreaStat(QualitySchemaModel):
    focus_area: Optional[str] = None
    label: Optional[str] = None
    metric_key: Optional[str] = None
    evaluated_pairs: int = 0
    successful_pairs: int = 0
    success_rate: Optional[float] = None
    avg_delta: Optional[float] = None


class StoryRepairEffectivenessSummary(QualitySchemaModel):
    status: Optional[str] = None
    success_rate: Optional[float] = None
    evaluated_pairs: int = 0
    successful_pairs: int = 0
    recovered_focus_areas: List[str] = Field(default_factory=list)
    unresolved_focus_areas: List[str] = Field(default_factory=list)
    focus_area_stats: List[StoryRepairEffectivenessFocusAreaStat] = Field(default_factory=list)
    summary: Optional[str] = None


class StoryQualityGateDecision(QualitySchemaModel):
    status: Optional[str] = None
    decision: Optional[str] = None
    label: Optional[str] = None
    summary: Optional[str] = None
    reason: Optional[str] = None
    overall_score: Optional[float] = None
    weak_metric_count: int = 0
    failed_metrics: List[StoryQualityGateMetric] = Field(default_factory=list)
    focus_areas: List[str] = Field(default_factory=list)
    repair_targets: List[str] = Field(default_factory=list)
    allow_save: bool = False
    can_auto_repair: bool = False
    requires_manual_review: bool = False
    weakest_metric_key: Optional[str] = None
    weakest_metric_label: Optional[str] = None
    weakest_metric_value: Optional[float] = None
    recommended_action: Optional[str] = None
    recommended_action_label: Optional[str] = None
    recommended_action_mode: Optional[str] = None
    recommended_focus_area: Optional[str] = None
    continuity_warning_count: int = 0
    continuity_preflight: Optional[StoryContinuityPreflight] = None
    pacing_imbalance: Optional[StoryPacingImbalanceSummary] = None
    manual_review_threshold: Optional[float] = None
    allow_save_threshold: Optional[float] = None
    weak_metric_block_count: Optional[int] = None
    allow_save_weak_metric_count: Optional[int] = None
    normalized_gap_threshold: Optional[float] = None
    quality_stage: Optional[str] = None
    quality_stage_label: Optional[str] = None
    quality_runtime_pressure: Optional[StoryQualityRuntimePressure] = None


class StoryQualityMetricsPayload(QualitySchemaModel):
    overall_score: Optional[float] = None
    conflict_chain_hit_rate: Optional[float] = None
    rule_grounding_hit_rate: Optional[float] = None
    outline_alignment_rate: Optional[float] = None
    dialogue_naturalness_rate: Optional[float] = None
    opening_hook_rate: Optional[float] = None
    payoff_chain_rate: Optional[float] = None
    cliffhanger_rate: Optional[float] = None
    pacing_score: Optional[float] = None
    repair_guidance: Optional[StoryRepairGuidance] = None
    quality_gate: Optional[StoryQualityGateDecision] = None
    quality_runtime_context: Optional[QualityRuntimeContextSummary] = None
    continuity_preflight: Optional[StoryContinuityPreflight] = None
    pacing_imbalance: Optional[StoryPacingImbalanceSummary] = None
    volume_goal_completion: Optional[StoryVolumeGoalCompletionSummary] = None
    foreshadow_payoff_delay: Optional[StoryForeshadowPayoffDelaySummary] = None
    repair_effectiveness: Optional[StoryRepairEffectivenessSummary] = None
    story_runtime_contract: Optional[Dict[str, Any]] = None


class ChapterLatestQualityMetrics(StoryQualityMetricsPayload):
    chapter_id: Optional[str] = None
    history_id: Optional[str] = None
    generated_at: Optional[str] = None


class ChapterQualityMetricsSummary(QualitySchemaModel):
    avg_overall_score: Optional[float] = None
    avg_conflict_chain_hit_rate: Optional[float] = None
    avg_rule_grounding_hit_rate: Optional[float] = None
    avg_outline_alignment_rate: Optional[float] = None
    avg_dialogue_naturalness_rate: Optional[float] = None
    avg_opening_hook_rate: Optional[float] = None
    avg_payoff_chain_rate: Optional[float] = None
    avg_cliffhanger_rate: Optional[float] = None
    avg_pacing_score: Optional[float] = None
    chapter_count: int = 0
    total_chapters: Optional[int] = None
    analyzed_chapters: Optional[int] = None
    last_generated_at: Optional[str] = None
    overall_score_delta: Optional[float] = None
    overall_score_trend: Optional[str] = None
    recent_focus_areas: List[str] = Field(default_factory=list)
    recent_failed_metric_counts: List[StoryQualityMetricFrequency] = Field(default_factory=list)
    quality_gate_counts: Dict[str, int] = Field(default_factory=dict)
    recent_manual_review_count: int = 0
    recent_auto_repair_count: int = 0
    quality_runtime_context: Optional[QualityRuntimeContextSummary] = None
    continuity_preflight: Optional[StoryContinuityPreflight] = None
    pacing_imbalance: Optional[StoryPacingImbalanceSummary] = None
    volume_goal_completion: Optional[StoryVolumeGoalCompletionSummary] = None
    foreshadow_payoff_delay: Optional[StoryForeshadowPayoffDelaySummary] = None
    repair_effectiveness: Optional[StoryRepairEffectivenessSummary] = None
    repair_guidance: Optional[StoryRepairGuidance] = None
    quality_gate: Optional[StoryQualityGateDecision] = None


class ActiveStoryRepairPayload(StoryRepairGuidance):
    source: Optional[str] = None
    source_label: Optional[str] = None
    scope: Optional[str] = None
    quality_gate: Optional[StoryQualityGateDecision] = None
    quality_gate_status: Optional[str] = None
    quality_gate_decision: Optional[str] = None
    quality_gate_label: Optional[str] = None
    quality_gate_summary: Optional[str] = None
    quality_gate_failed_metrics: List[str] = Field(default_factory=list)
    updated_at: Optional[str] = None


class ProjectChapterQualityTrendItemPayload(QualitySchemaModel):
    chapter_id: str
    chapter_number: int
    title: str
    status: Optional[str] = None
    history_id: Optional[str] = None
    generated_at: Optional[str] = None
    latest_quality_metrics: Optional[ChapterLatestQualityMetrics] = None


def _validate_optional(model_cls, payload: Optional[Mapping[str, Any]]):
    if not isinstance(payload, Mapping):
        return None
    return model_cls.model_validate(dict(payload))


def normalize_story_repair_guidance(payload: Optional[Mapping[str, Any]]) -> Optional[StoryRepairGuidance]:
    return _validate_optional(StoryRepairGuidance, payload)


def normalize_story_quality_gate_decision(payload: Optional[Mapping[str, Any]]) -> Optional[StoryQualityGateDecision]:
    return _validate_optional(StoryQualityGateDecision, payload)


def normalize_story_quality_metrics_payload(payload: Optional[Mapping[str, Any]]) -> Optional[StoryQualityMetricsPayload]:
    return _validate_optional(StoryQualityMetricsPayload, payload)


def normalize_chapter_latest_quality_metrics(payload: Optional[Mapping[str, Any]]) -> Optional[ChapterLatestQualityMetrics]:
    return _validate_optional(ChapterLatestQualityMetrics, payload)


def normalize_chapter_quality_metrics_summary(payload: Optional[Mapping[str, Any]]) -> Optional[ChapterQualityMetricsSummary]:
    return _validate_optional(ChapterQualityMetricsSummary, payload)


def normalize_active_story_repair_payload(payload: Optional[Mapping[str, Any]]) -> Optional[ActiveStoryRepairPayload]:
    return _validate_optional(ActiveStoryRepairPayload, payload)


@dataclass(frozen=True)
class RepairMetricRule:
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


QUALITY_STAGE_LABELS: Dict[str, str] = {
    "opening": "开篇",
    "development": "发展段",
    "ending": "收束段",
}


def _safe_float(value: Any) -> Optional[float]:
    try:
        if value is None:
            return None
        return float(value)
    except (TypeError, ValueError):
        return None


def _extract_rule_value(
    metrics: Mapping[str, Any],
    rule: RepairMetricRule,
) -> Optional[float]:
    for key in rule.aliases:
        value = _safe_float(metrics.get(key))
        if value is not None:
            return value
    return None


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


def _normalize_runtime_context_item_mapping(
    value: Mapping[str, Any],
) -> Optional[Dict[str, Any]]:
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
        "allow_save_weak_metric_count": 1,
    }

    if stage == "opening":
        thresholds["manual_review_score"] = 68.0
        thresholds["allow_save_score"] = 80.0
        thresholds["normalized_gap"] = 10.0
        thresholds["weak_metric_block_count"] = 3
    elif stage == "development":
        thresholds["manual_review_score"] = 70.0
        thresholds["allow_save_score"] = 82.0
        thresholds["normalized_gap"] = 12.0
        thresholds["weak_metric_block_count"] = 3
    elif stage == "ending":
        thresholds["manual_review_score"] = 72.0
        thresholds["allow_save_score"] = 84.0
        thresholds["normalized_gap"] = 10.0
        thresholds["weak_metric_block_count"] = 2

    if quality_preset == "plot_drive":
        thresholds["allow_save_score"] -= 1.0
    elif quality_preset == "clean_prose":
        thresholds["manual_review_score"] += 1.0
        thresholds["allow_save_score"] += 1.0
        thresholds["normalized_gap"] -= 1.0
    elif quality_preset == "emotion_drama":
        thresholds["manual_review_score"] -= 1.0
        thresholds["allow_save_score"] -= 1.0
    elif quality_preset == "immersive":
        thresholds["allow_save_score"] += 0.5

    if style_profile == "urban_finance":
        thresholds["allow_save_score"] += 0.5
        thresholds["manual_review_score"] += 0.5
    elif style_profile == "tech_xianxia":
        thresholds["allow_save_score"] += 1.0
        thresholds["normalized_gap"] -= 0.5
    elif style_profile == "low_ai_life":
        thresholds["manual_review_score"] -= 1.0
    elif style_profile == "low_ai_serial":
        thresholds["allow_save_score"] -= 0.5

    if "suspense_mystery" in genre_profiles:
        thresholds["allow_save_score"] += 0.5
    if "history_power" in genre_profiles:
        thresholds["manual_review_score"] += 0.5
    if "romance_slice_of_life" in genre_profiles:
        thresholds["manual_review_score"] -= 0.5

    if "payoff" in focus_areas and stage == "ending":
        thresholds["allow_save_score"] += 0.5
        thresholds["weak_metric_block_count"] = min(int(thresholds["weak_metric_block_count"]), 2)
    if "cliffhanger" in focus_areas and creative_mode in {"hook", "suspense"}:
        thresholds["allow_save_score"] -= 0.5
    if "dialogue" in focus_areas and story_focus in {"deepen_character", "relationship_shift"}:
        thresholds["manual_review_score"] -= 0.5
    if quality_preset == "emotion_drama" and story_focus == "relationship_shift":
        thresholds["weak_metric_block_count"] = max(int(thresholds["weak_metric_block_count"]), 4)
        thresholds["allow_save_weak_metric_count"] = max(
            int(thresholds["allow_save_weak_metric_count"]),
            2,
        )

    thresholds["allow_save_score"] = max(float(thresholds["manual_review_score"]) + 6.0, float(thresholds["allow_save_score"]))
    thresholds["normalized_gap"] = max(float(thresholds["normalized_gap"]), 6.0)
    thresholds["weak_metric_block_count"] = max(int(thresholds["weak_metric_block_count"]), 2)
    thresholds["allow_save_weak_metric_count"] = max(int(thresholds["allow_save_weak_metric_count"]), 0)

    pressure = _build_runtime_pressure(runtime_context)
    if pressure["foreshadow_state_count"] >= 3 and stage == "ending":
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
        weak_threshold_value = round(
            max(rule.weak_threshold + threshold_adjustments.get(rule.key, 0.0), 0.0),
            1,
        )
        preserve_threshold_value = round(
            max(rule.preserve_threshold + threshold_adjustments.get(rule.key, 0.0) * 0.6, 0.0),
            1,
        )
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


def build_story_repair_guidance(
    metrics: Mapping[str, Any],
    *,
    scope: str = "chapter",
) -> Dict[str, Any]:
    if not isinstance(metrics, Mapping):
        return _build_empty_guidance()

    continuity_preflight = (
        dict(metrics.get("continuity_preflight"))
        if isinstance(metrics.get("continuity_preflight"), Mapping)
        else {}
    )
    runtime_context = _extract_quality_runtime_context(metrics)
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
    if not isinstance(metrics, Mapping):
        return _build_empty_quality_gate()

    continuity_preflight = (
        dict(metrics.get("continuity_preflight"))
        if isinstance(metrics.get("continuity_preflight"), Mapping)
        else {}
    )
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
    recommended_action = _resolve_quality_gate_recommended_action(
        focus_areas=focus_areas,
        weakest=weakest,
        continuity_preflight=continuity_preflight,
    )

    candidate_selection = metrics.get("candidate_selection") if isinstance(metrics.get("candidate_selection"), Mapping) else {}
    severe_word_budget_pressure = False
    severe_word_budget_reason = ""
    if isinstance(candidate_selection, Mapping):
        target_word_count = int(candidate_selection.get("target_word_count") or 0)
        current_word_count = int(candidate_selection.get("word_count") or 0)
        if target_word_count > 0 and current_word_count > 0:
            target_lower_bound = max(200, min(target_word_count - 120, int(target_word_count * 0.9)))
            target_upper_bound = max(target_lower_bound + 80, min(target_word_count + 150, int(target_word_count * 1.15)))
            severe_upper_bound = max(target_upper_bound + 120, int(target_upper_bound * 1.1))
            severe_lower_bound = max(200, min(target_lower_bound - 120, int(target_lower_bound * 0.9)))
            severe_word_budget_pressure = (
                current_word_count > severe_upper_bound
                or (0 < current_word_count < severe_lower_bound)
            )
            if severe_word_budget_pressure:
                severe_word_budget_reason = (
                    f"字数严重偏离目标窗口（当前 {current_word_count}，目标 {target_word_count}，理想范围 {target_lower_bound}-{target_upper_bound}）"
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
    elif severe_word_budget_pressure or weak_metric_count > int(thresholds.get("allow_save_weak_metric_count") or 0) or (
        overall_score is not None and overall_score < float(thresholds["allow_save_score"])
    ):
        status = "repairable"
        decision = "auto_repair"
        label = "可修复"
        if severe_word_budget_pressure:
            reason = severe_word_budget_reason
        elif weak_metric_count > 0:
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


QualityRuntimeLedgerEntry.model_rebuild()
QualityRuntimePlanEntry.model_rebuild()
StoryRepairGuidance.model_rebuild()
StoryQualityGateDecision.model_rebuild()
StoryQualityMetricsPayload.model_rebuild()
ChapterLatestQualityMetrics.model_rebuild()
ChapterQualityMetricsSummary.model_rebuild()
ActiveStoryRepairPayload.model_rebuild()


