from app.services.story_repair_payload_service import (
    build_story_repair_payload_from_metrics,
    merge_story_repair_payload,
    normalize_story_repair_payload,
)


def test_should_merge_partial_story_repair_payload_with_fallback():
    primary = normalize_story_repair_payload(summary="??????")
    fallback = normalize_story_repair_payload(
        targets=["??????", "?????"],
        strengths=["???????"],
    )

    merged = merge_story_repair_payload(primary, fallback)

    assert merged is not None
    assert merged.summary == "??????"
    assert list(merged.targets) == ["??????", "?????"]
    assert list(merged.strengths) == ["???????"]


def test_should_build_story_repair_payload_from_metrics():
    payload = build_story_repair_payload_from_metrics(
        {
            "overall_score": 72.0,
            "conflict_chain_hit_rate": 60.0,
            "rule_grounding_hit_rate": 81.0,
            "outline_alignment_rate": 58.0,
            "dialogue_naturalness_rate": 78.0,
            "opening_hook_rate": 76.0,
            "payoff_chain_rate": 56.0,
            "cliffhanger_rate": 82.0,
            "pacing_score": 6.9,
        },
        scope="chapter",
    )

    assert payload is not None
    assert payload.summary
    assert list(payload.targets)
    assert list(payload.strengths)
