from tests.test_support.chapter_candidate_runtime_state_test_support import (
    build_chapter_candidate_runtime_state,
    snapshot_chapter_candidate_runtime_state,
    sync_chapter_candidate_runtime_state,
)


def test_should_build_candidate_runtime_state_defaults():
    state = build_chapter_candidate_runtime_state(max_candidates=2)

    assert state == {
        "candidate_total": 2,
        "candidate_count": 2,
        "candidate_index": 1,
        "current_chars": 0,
        "word_count": 0,
        "chunk_count": 0,
        "generation_path": "single_pass",
        "attempt_kind": "initial_candidate",
        "rerank_used": False,
        "word_budget_repair_used": False,
        "winner_candidate_index": None,
    }


def test_should_sync_and_snapshot_candidate_runtime_state():
    state = {}
    sync_chapter_candidate_runtime_state(
        state,
        candidate_index=3,
        candidate_total=5,
        current_chars=321,
        chunk_count=4,
        generation_path="word_budget_repair",
        attempt_kind="word_budget_repair",
        rerank_used=True,
        word_budget_repair_used=True,
        winner_candidate_index=3,
    )

    snapshot = snapshot_chapter_candidate_runtime_state(state)

    assert snapshot.candidate_index == 3
    assert snapshot.candidate_total == 5
    assert snapshot.candidate_count == 5
    assert snapshot.current_chars == 321
    assert snapshot.word_count == 321
    assert snapshot.chunk_count == 4
    assert snapshot.generation_path == "word_budget_repair"
    assert snapshot.attempt_kind == "word_budget_repair"
    assert snapshot.rerank_used is True
    assert snapshot.word_budget_repair_used is True
    assert snapshot.winner_candidate_index == 3
