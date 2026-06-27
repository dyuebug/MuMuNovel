from tests.test_support.chapter_candidate_executor_test_support import (
    resolve_generation_attempt_labels,
)


def test_should_resolve_generation_attempt_labels_for_initial_candidate():
    assert resolve_generation_attempt_labels(1) == ("single_pass", "initial_candidate")


def test_should_resolve_generation_attempt_labels_for_rerank_candidate():
    assert resolve_generation_attempt_labels(2) == ("rerank_retry", "rerank_candidate")


def test_should_resolve_generation_attempt_labels_for_word_budget_repair():
    assert resolve_generation_attempt_labels(1, is_word_budget_repair=True) == (
        "word_budget_repair",
        "word_budget_repair",
    )
