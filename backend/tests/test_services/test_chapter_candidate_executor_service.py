from types import SimpleNamespace

import pytest

from tests.test_support import chapter_candidate_executor_test_support as executor_service

from tests.test_support.chapter_candidate_executor_test_support import (
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


@pytest.mark.asyncio
async def test_should_delegate_generate_best_ranked_candidate_with_cached_dependencies(monkeypatch):
    captured = {}

    def fake_dependencies(**kwargs):
        captured["dependency_kwargs"] = kwargs
        return {"deps": True}

    async def fake_workflow(**kwargs):
        captured["workflow_kwargs"] = kwargs
        return {"winner": "ok"}

    monkeypatch.setattr(executor_service, "get_chapter_candidate_executor_dependencies", fake_dependencies)
    monkeypatch.setattr(executor_service, "generate_best_ranked_candidate_workflow", fake_workflow)

    result = await executor_service.generate_best_ranked_candidate(
        ai_service="ai",
        base_generate_kwargs={"prompt": "hello"},
        target_word_count=1200,
        source="chapter",
        generation_label="label",
        quality_evaluator="quality",
        quality_gate_plan_builder="gate",
        max_candidates=3,
        runtime_state={"candidate_total": 3},
        resolve_generation_attempt_labels_fn="resolve",
        sync_generation_runtime_state_fn="sync",
        collect_generation_candidate_output_fn="collect",
        build_generation_candidate_record_fn="record",
    )

    assert result == {"winner": "ok"}
    assert captured["dependency_kwargs"] == {
        "resolve_generation_attempt_labels_fn": "resolve",
        "sync_generation_runtime_state_fn": "sync",
        "collect_generation_candidate_output_fn": "collect",
        "build_generation_candidate_record_fn": "record",
    }
    assert captured["workflow_kwargs"]["dependencies"] == {"deps": True}
    assert captured["workflow_kwargs"]["ai_service"] == "ai"
    assert captured["workflow_kwargs"]["base_generate_kwargs"] == {"prompt": "hello"}


@pytest.mark.asyncio
async def test_should_delegate_generate_best_ranked_candidate_with_default_wiring(monkeypatch):
    captured = {}

    async def fake_generate_best_ranked_candidate(**kwargs):
        captured.update(kwargs)
        return {"winner": "default"}

    monkeypatch.setattr(
        executor_service,
        "generate_best_ranked_candidate",
        fake_generate_best_ranked_candidate,
    )

    result = await executor_service.generate_best_ranked_candidate_with_default_wiring(
        ai_service="ai",
        base_generate_kwargs={"prompt": "hello"},
        target_word_count=888,
        source="chapter",
        generation_label="route",
        quality_evaluator="quality",
        quality_gate_plan_builder="gate",
        max_candidates=4,
        runtime_state={"candidate_total": 4},
    )

    assert result == {"winner": "default"}
    assert captured["ai_service"] == "ai"
    assert captured["base_generate_kwargs"] == {"prompt": "hello"}
    assert captured["target_word_count"] == 888
    assert captured["source"] == "chapter"
    assert captured["generation_label"] == "route"
    assert captured["quality_evaluator"] == "quality"
    assert captured["quality_gate_plan_builder"] == "gate"
    assert captured["max_candidates"] == 4
    assert captured["runtime_state"] == {"candidate_total": 4}
    assert (
        captured["resolve_generation_attempt_labels_fn"]
        == executor_service.resolve_default_generation_attempt_labels
    )
    assert (
        captured["sync_generation_runtime_state_fn"]
        == executor_service.sync_default_generation_runtime_state
    )
    assert (
        captured["collect_generation_candidate_output_fn"]
        == executor_service.collect_default_generation_candidate_output
    )
    assert (
        captured["build_generation_candidate_record_fn"]
        == executor_service.build_default_generation_candidate_record_with_default_logging
    )


def test_should_cache_candidate_executor_dependencies(monkeypatch):
    calls = []

    def fake_builder(**kwargs):
        calls.append(kwargs)
        return {"deps": len(calls)}

    executor_service.get_chapter_candidate_executor_dependencies.cache_clear()
    monkeypatch.setattr(
        executor_service,
        "build_default_chapter_candidate_executor_dependencies",
        fake_builder,
    )

    first = executor_service.get_chapter_candidate_executor_dependencies(
        resolve_generation_attempt_labels_fn="resolve",
        sync_generation_runtime_state_fn="sync",
        collect_generation_candidate_output_fn="collect",
        build_generation_candidate_record_fn="record",
    )
    second = executor_service.get_chapter_candidate_executor_dependencies(
        resolve_generation_attempt_labels_fn="resolve",
        sync_generation_runtime_state_fn="sync",
        collect_generation_candidate_output_fn="collect",
        build_generation_candidate_record_fn="record",
    )

    assert first == second == {"deps": 1}
    assert len(calls) == 1
