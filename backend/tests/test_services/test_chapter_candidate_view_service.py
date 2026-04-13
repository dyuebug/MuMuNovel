from app.services.chapter_candidate_view_service import snapshot_chapter_candidate


def test_should_snapshot_candidate_defaults_and_normalize_fields():
    view = snapshot_chapter_candidate({
        "candidate_index": 2,
        "candidate_count": 3,
        "winner_candidate_index": 2,
        "word_count": 456,
        "generation_path": " rerank_retry ",
        "attempt_kind": " rerank_candidate ",
        "rerank_used": True,
        "word_budget_repair_used": False,
        "full_content": "content",
        "candidate_chunks": ["a", 2],
        "quality_metrics": {"score": 88},
        "quality_gate_plan": {"action": "continue"},
    })

    assert view.candidate_index == 2
    assert view.candidate_count == 3
    assert view.winner_candidate_index == 2
    assert view.word_count == 456
    assert view.generation_path == "rerank_retry"
    assert view.attempt_kind == "rerank_candidate"
    assert view.rerank_used is True
    assert view.word_budget_repair_used is False
    assert view.full_content == "content"
    assert view.candidate_chunks == ["a", "2"]
    assert view.quality_metrics == {"score": 88}
    assert view.quality_gate_plan == {"action": "continue"}


def test_should_snapshot_candidate_with_fallbacks():
    view = snapshot_chapter_candidate({"full_content": "hello"})

    assert view.candidate_index == 1
    assert view.candidate_count == 1
    assert view.winner_candidate_index == 1
    assert view.word_count == 5
    assert view.generation_path == ""
    assert view.attempt_kind == ""
    assert view.candidate_chunks == []
    assert view.quality_metrics == {}
    assert view.quality_gate_plan == {}
