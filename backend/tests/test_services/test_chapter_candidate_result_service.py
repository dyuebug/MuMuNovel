from tests.test_support.chapter_candidate_result_test_support import (
    normalize_selected_candidate_result,
)


def test_should_normalize_selected_candidate_result_with_quality_gate_snapshot_merge():
    result = normalize_selected_candidate_result(
        selected_candidate={
            "candidate_index": 2,
            "candidate_count": 3,
            "full_content": "hello",
            "word_count": 5,
            "candidate_chunks": ["he", "llo"],
            "quality_metrics": {"score": 88},
            "quality_gate_plan": {"action": "retry", "quality_gate": {"decision": "manual_review"}},
        },
        story_runtime_contract={"contract": True},
        attach_story_runtime_contract_fn=lambda payload, contract: {**dict(payload or {}), "story_runtime_contract": contract},
        include_quality_gate_snapshot_in_metrics=True,
    )

    assert result.candidate_index == 2
    assert result.candidate_count == 3
    assert result.full_content == "hello"
    assert result.candidate_word_count == 5
    assert result.candidate_chunks == ["he", "llo"]
    assert result.quality_gate_action == "retry"
    assert result.quality_gate_snapshot == {"decision": "manual_review"}
    assert result.quality_metrics["quality_gate"] == {"decision": "manual_review"}
    assert result.quality_metrics["story_runtime_contract"] == {"contract": True}


def test_should_normalize_selected_candidate_result_without_snapshot_merge():
    result = normalize_selected_candidate_result(
        selected_candidate={
            "full_content": "hello",
            "quality_metrics": {"score": 88},
            "quality_gate_plan": {"action": "continue", "quality_gate": {"decision": "allow_save"}},
        },
        story_runtime_contract=None,
        attach_story_runtime_contract_fn=lambda payload, contract: dict(payload or {}),
    )

    assert result.quality_gate_action == "continue"
    assert result.quality_gate_snapshot == {"decision": "allow_save"}
    assert "quality_gate" not in result.quality_metrics
