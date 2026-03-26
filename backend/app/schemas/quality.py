from __future__ import annotations

from typing import Any, Dict, List, Mapping, Optional

from pydantic import BaseModel, ConfigDict, Field


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


QualityRuntimeLedgerEntry.model_rebuild()
QualityRuntimePlanEntry.model_rebuild()
StoryRepairGuidance.model_rebuild()
StoryQualityGateDecision.model_rebuild()
StoryQualityMetricsPayload.model_rebuild()
ChapterLatestQualityMetrics.model_rebuild()
ChapterQualityMetricsSummary.model_rebuild()
ActiveStoryRepairPayload.model_rebuild()
