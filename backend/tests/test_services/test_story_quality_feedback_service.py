import json

from app.services.story_quality_feedback_service import (
    build_quality_metrics_summary,
    build_story_repair_guidance,
    extract_quality_metrics_from_history_payload,
)


def test_should_build_chapter_repair_guidance_from_low_metrics():
    guidance = build_story_repair_guidance(
        {
            "conflict_chain_hit_rate": 48.0,
            "rule_grounding_hit_rate": 72.0,
            "outline_alignment_rate": 54.0,
            "dialogue_naturalness_rate": 83.0,
            "opening_hook_rate": 61.0,
            "payoff_chain_rate": 75.0,
            "cliffhanger_rate": 69.0,
        },
        scope="chapter",
    )

    assert guidance["weakest_metric_key"] == "conflict_chain_hit_rate"
    assert guidance["weakest_metric_label"] == "冲突链推进"
    assert guidance["focus_areas"][:2] == ["conflict", "outline"]
    assert any("冲突" in item for item in guidance["repair_targets"])
    assert any("语气" in item for item in guidance["preserve_strengths"])
    assert "表面润色" in guidance["summary"]


def test_should_support_batch_summary_and_pacing_guidance():
    guidance = build_story_repair_guidance(
        {
            "avg_overall_score": 76.0,
            "avg_conflict_chain_hit_rate": 70.0,
            "avg_rule_grounding_hit_rate": 84.0,
            "avg_outline_alignment_rate": 82.0,
            "avg_dialogue_naturalness_rate": 79.0,
            "avg_opening_hook_rate": 76.0,
            "avg_payoff_chain_rate": 80.0,
            "avg_cliffhanger_rate": 78.0,
            "avg_pacing_score": 5.9,
        },
        scope="batch",
    )

    assert guidance["weakest_metric_key"] == "pacing_score"
    assert guidance["focus_areas"][0] == "pacing"
    assert guidance["weakest_metric_value"] == 5.9
    assert guidance["summary"].startswith("这一批章节")


def test_should_extract_quality_metrics_from_history_payload_with_guidance():
    generated_content = json.dumps(
        {
            "quality_metrics": {
                "conflict_chain_hit_rate": 55.0,
                "rule_grounding_hit_rate": 74.0,
                "outline_alignment_rate": 63.0,
                "dialogue_naturalness_rate": 82.0,
                "opening_hook_rate": 66.0,
                "payoff_chain_rate": 71.0,
                "cliffhanger_rate": 60.0,
            }
        },
        ensure_ascii=False,
    )

    metrics = extract_quality_metrics_from_history_payload(generated_content, scope="chapter")

    assert metrics is not None
    assert metrics["outline_alignment_rate"] == 63.0
    assert metrics["repair_guidance"]["weakest_metric_key"] == "conflict_chain_hit_rate"
    assert metrics["repair_guidance"]["focus_areas"][0] == "conflict"


def test_should_build_outline_quality_metrics_summary_from_recent_history():
    summary = build_quality_metrics_summary(
        [
            {
                "overall_score": 72.0,
                "conflict_chain_hit_rate": 58.0,
                "rule_grounding_hit_rate": 76.0,
                "outline_alignment_rate": 61.0,
                "dialogue_naturalness_rate": 80.0,
                "opening_hook_rate": 67.0,
                "payoff_chain_rate": 66.0,
                "cliffhanger_rate": 59.0,
            },
            {
                "overall_score": 75.0,
                "conflict_chain_hit_rate": 62.0,
                "rule_grounding_hit_rate": 79.0,
                "outline_alignment_rate": 64.0,
                "dialogue_naturalness_rate": 82.0,
                "opening_hook_rate": 69.0,
                "payoff_chain_rate": 68.0,
                "cliffhanger_rate": 63.0,
            },
        ],
        scope="outline",
    )

    assert summary is not None
    assert summary["chapter_count"] == 2
    assert summary["avg_outline_alignment_rate"] == 62.5
    assert summary["repair_guidance"]["summary"].startswith("最近章节")
    assert summary["repair_guidance"]["focus_areas"]
