from tests.test_support.chapter_candidate_finalize_test_support import (
    collect_word_budget_repair_candidates,
    is_targeted_quality_repair_candidate,
    is_word_budget_repair_candidate,
)


def test_should_identify_word_budget_repair_candidate_from_attempt_kind_or_generation_path():
    assert is_word_budget_repair_candidate({"attempt_kind": "word_budget_repair"}) is True
    assert is_word_budget_repair_candidate({"generation_path": "word_budget_repair"}) is True
    assert is_word_budget_repair_candidate({"attempt_kind": "initial_candidate"}) is False


def test_should_identify_targeted_quality_repair_candidate_from_attempt_kind_or_generation_path():
    assert is_targeted_quality_repair_candidate({"attempt_kind": "targeted_quality_repair"}) is True
    assert is_targeted_quality_repair_candidate({"generation_path": "targeted_quality_repair"}) is True
    assert is_targeted_quality_repair_candidate({"generation_path": "rerank_retry"}) is False


def test_should_collect_word_budget_repair_candidates_only():
    candidates = collect_word_budget_repair_candidates([
        {"candidate_index": 1, "attempt_kind": "initial_candidate"},
        {"candidate_index": 2, "attempt_kind": "word_budget_repair"},
        {"candidate_index": 3, "generation_path": "word_budget_repair"},
        None,
    ])

    assert [item["candidate_index"] for item in candidates] == [2, 3]
