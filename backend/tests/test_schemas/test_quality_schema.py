from __future__ import annotations

from app.schemas.chapter import BatchGenerateStatusResponse
from app.schemas.quality import (
    ActiveStoryRepairPayload,
    ChapterLatestQualityMetrics,
    ChapterQualityMetricsSummary,
    QualityRuntimeLedgerEntry,
    QualityRuntimePlanEntry,
    StoryQualityMetricsPayload,
    normalize_story_quality_metrics_payload,
)


SAMPLE_GUIDANCE = {
    "summary": "需要补强兑现链路",
    "repair_targets": ["补足伏笔回收", "强化冲突升级"],
    "preserve_strengths": ["保留对白节奏"],
    "focus_areas": ["conflict", "payoff"],
    "weakest_metric_key": "payoff_chain_rate",
    "weakest_metric_label": "兑现链",
    "weakest_metric_value": 61.0,
}

SAMPLE_GATE = {
    "status": "repairable",
    "decision": "auto_repair",
    "label": "可修复",
    "summary": "两项关键指标低于阈值，需要补写桥接与回收",
    "reason": "共 2 项指标偏低",
    "overall_score": 78.5,
    "weak_metric_count": 2,
    "failed_metrics": [
        {
            "key": "payoff_chain_rate",
            "label": "兑现链",
            "value": 61.0,
            "threshold": 62.0,
            "gap": 1.0,
            "focus_area": "payoff",
            "repair_target": "补足回收",
        }
    ],
    "focus_areas": ["conflict", "payoff"],
    "repair_targets": ["补足回收", "强化冲突"],
    "can_auto_repair": True,
    "recommended_action": "bridge_scene",
    "recommended_action_label": "补桥接场景",
    "quality_runtime_pressure": {
        "foreshadow_state_count": 2,
        "character_state_count": 1,
        "foreshadow_state_items": ["旧怀表"],
        "character_state_items": ["林秋开始怀疑"],
    },
}

SAMPLE_METRICS = {
    "overall_score": 78.5,
    "conflict_chain_hit_rate": 76.0,
    "rule_grounding_hit_rate": 83.0,
    "outline_alignment_rate": 80.0,
    "dialogue_naturalness_rate": 84.0,
    "opening_hook_rate": 82.0,
    "payoff_chain_rate": 61.0,
    "cliffhanger_rate": 79.0,
    "quality_runtime_context": {
        "plot_stage": "development",
        "chapter_count": 18,
        "current_chapter_number": 7,
        "character_focus": ["林秋", "沈砚"],
    },
    "repair_guidance": SAMPLE_GUIDANCE,
    "quality_gate": SAMPLE_GATE,
    "story_runtime_contract": {
        "guidance": {"plot_stage": "development", "quality_preset": "immersive"},
        "blueprint": {"chapter_count": 18, "current_chapter_number": 7},
    },
    "extra_signal": {"enabled": True},
}


def test_should_normalize_story_quality_metrics_payload_into_nested_models():
    payload = normalize_story_quality_metrics_payload(SAMPLE_METRICS)

    assert isinstance(payload, StoryQualityMetricsPayload)
    assert payload.overall_score == 78.5
    assert payload.quality_runtime_context is not None
    assert payload.quality_runtime_context.plot_stage == "development"
    assert payload.repair_guidance is not None
    assert payload.repair_guidance.focus_areas == ["conflict", "payoff"]
    assert payload.quality_gate is not None
    assert payload.quality_gate.status == "repairable"
    assert payload.quality_gate.failed_metrics[0].key == "payoff_chain_rate"
    assert payload.quality_gate.quality_runtime_pressure is not None
    assert payload.quality_gate.quality_runtime_pressure.foreshadow_state_items == ["旧怀表"]
    dumped = payload.model_dump(exclude_none=True)
    assert dumped["story_runtime_contract"]["blueprint"]["chapter_count"] == 18
    assert dumped["extra_signal"]["enabled"] is True


def test_should_parse_batch_generate_status_response_with_typed_quality_payloads():
    response = BatchGenerateStatusResponse(
        batch_id="batch-001",
        status="running",
        total=5,
        completed=2,
        latest_quality_metrics=SAMPLE_METRICS,
        quality_metrics_summary={
            "chapter_count": 3,
            "avg_overall_score": 80.2,
            "recent_focus_areas": ["conflict", "payoff"],
            "repair_guidance": SAMPLE_GUIDANCE,
            "quality_gate": SAMPLE_GATE,
            "quality_gate_counts": {"pass": 1, "repairable": 2},
        },
        active_story_repair_payload={
            **SAMPLE_GUIDANCE,
            "source": "current_chapter_quality",
            "quality_gate": SAMPLE_GATE,
            "quality_gate_status": "repairable",
        },
    )

    assert isinstance(response.latest_quality_metrics, ChapterLatestQualityMetrics)
    assert response.latest_quality_metrics.quality_gate is not None
    assert response.latest_quality_metrics.quality_gate.status == "repairable"
    assert isinstance(response.quality_metrics_summary, ChapterQualityMetricsSummary)
    assert response.quality_metrics_summary.repair_guidance is not None
    assert response.quality_metrics_summary.repair_guidance.summary.startswith("需要")
    assert isinstance(response.active_story_repair_payload, ActiveStoryRepairPayload)
    assert response.active_story_repair_payload.quality_gate is not None
    assert response.active_story_repair_payload.quality_gate.decision == "auto_repair"
