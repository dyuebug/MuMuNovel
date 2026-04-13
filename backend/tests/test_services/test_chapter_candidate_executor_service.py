from types import SimpleNamespace

from app.services.chapter_candidate_executor_service import (
    _resolve_followup_targeted_repair_seed_candidate,
    _select_post_finalize_targeted_repair_seed_candidate,
)


def _build_executor_dependencies(*, should_followup):
    return SimpleNamespace(
        targeted_final_repair_dependencies=SimpleNamespace(
            should_apply_followup_targeted_final_repair_fn=should_followup,
        ),
        select_targeted_final_repair_seed_candidate_fn=lambda selected_candidate, candidates: {"candidate_index": 99, "source": "selected"},
    )


def test_should_select_post_finalize_targeted_repair_seed_from_current_winner_when_followup_applies():
    selected_candidate = {"candidate_index": 2, "attempt_kind": "targeted_quality_repair"}
    dependencies = _build_executor_dependencies(should_followup=lambda candidate: True)

    result = _select_post_finalize_targeted_repair_seed_candidate(
        selected_candidate=selected_candidate,
        candidates=[selected_candidate],
        deferred_followup_targeted_repair_seed_candidate=None,
        dependencies=dependencies,
    )

    assert result is selected_candidate


def test_should_prefer_deferred_post_finalize_targeted_repair_seed_before_selecting_new_one():
    selected_candidate = {"candidate_index": 2, "attempt_kind": "initial_candidate"}
    deferred_seed = {"candidate_index": 5, "attempt_kind": "targeted_quality_repair"}
    dependencies = _build_executor_dependencies(should_followup=lambda candidate: False)

    result = _select_post_finalize_targeted_repair_seed_candidate(
        selected_candidate=selected_candidate,
        candidates=[selected_candidate],
        deferred_followup_targeted_repair_seed_candidate=deferred_seed,
        dependencies=dependencies,
    )

    assert result is deferred_seed


def test_should_skip_new_seed_selection_when_winner_is_already_targeted_quality_repair():
    selected_candidate = {"candidate_index": 2, "attempt_kind": "targeted_quality_repair"}
    dependencies = _build_executor_dependencies(should_followup=lambda candidate: False)

    result = _select_post_finalize_targeted_repair_seed_candidate(
        selected_candidate=selected_candidate,
        candidates=[selected_candidate],
        deferred_followup_targeted_repair_seed_candidate=None,
        dependencies=dependencies,
    )

    assert result is None


def test_should_resolve_followup_targeted_repair_seed_candidate_from_final_state():
    selected_candidate = {"candidate_index": 3, "attempt_kind": "targeted_quality_repair"}
    final_state = SimpleNamespace(selected_candidate=selected_candidate)
    dependencies = _build_executor_dependencies(should_followup=lambda candidate: candidate.get("candidate_index") == 3)

    result = _resolve_followup_targeted_repair_seed_candidate(
        final_state=final_state,
        dependencies=dependencies,
    )

    assert result is selected_candidate
