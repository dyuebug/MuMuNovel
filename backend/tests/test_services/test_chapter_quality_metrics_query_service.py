import json

from tests.test_support.chapter_quality_metrics_query_test_support import (
    build_quality_metrics_summary,
    extract_quality_metrics_from_history_payload,
)


def test_should_extract_quality_metrics_from_history_payload_with_runtime_snapshot():
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
            },
            "story_runtime_snapshot": {
                "character_state_ledger": [
                    {"label": "主角", "summary": "情绪收紧"}
                ]
            },
        },
        ensure_ascii=False,
    )

    metrics = extract_quality_metrics_from_history_payload(
        generated_content,
        scope="chapter",
    )

    assert metrics is not None
    assert metrics["outline_alignment_rate"] == 63.0
    assert metrics["repair_guidance"]["weakest_metric_key"] == "conflict_chain_hit_rate"
    assert metrics["repair_guidance"]["focus_areas"][0] == "conflict"
    assert metrics["quality_gate"]["status"] == "blocked"
    assert metrics["quality_gate"]["failed_metrics"][0]["label"] == "冲突链推进"
    assert (
        metrics["quality_runtime_context"]["character_state_ledger"][0]["label"]
        == "主角"
    )


def test_should_return_none_for_invalid_history_payload():
    assert extract_quality_metrics_from_history_payload(None, scope="chapter") is None
    assert (
        extract_quality_metrics_from_history_payload("not-json", scope="chapter")
        is None
    )
    assert (
        extract_quality_metrics_from_history_payload(
            json.dumps({"quality_metrics": []}, ensure_ascii=False),
            scope="chapter",
        )
        is None
    )


def test_should_build_batch_quality_metrics_summary_through_query_owner():
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
        scope="batch",
    )

    assert summary is not None
    assert summary["chapter_count"] == 2
    assert summary["avg_outline_alignment_rate"] == 62.5
    assert summary["repair_guidance"]["focus_areas"]
    assert summary["quality_gate"]["status"] == "blocked"
