from app.services.chapter_candidate_finalize_service import (
    ChapterCandidateFinalizeRequest,
    build_chapter_candidate_finalize_dependencies,
    finalize_selected_candidate_result,
    resolve_final_candidate_state,
)


def _build_finalize_dependencies(sync_calls: list[dict]):
    return build_chapter_candidate_finalize_dependencies(
        resolve_generation_attempt_labels_fn=lambda candidate_index, is_word_budget_repair=False: (
            "word_budget_repair" if is_word_budget_repair else "rerank_retry",
            "word_budget_repair" if is_word_budget_repair else "rerank_candidate",
        ),
        build_candidate_selection_metadata_fn=lambda quality_metrics, **kwargs: {
            "candidate_index": kwargs["candidate_index"],
            "candidate_count": kwargs["candidate_count"],
            "generation_path": kwargs["generation_path"],
            "attempt_kind": kwargs["attempt_kind"],
            "rerank_used": kwargs["rerank_used"],
            "word_budget_repair_used": kwargs["word_budget_repair_used"],
            "winner_candidate_index": kwargs["winner_candidate_index"],
        },
        attach_candidate_selection_metadata_fn=lambda quality_metrics, *, selection_metadata: {
            **dict(quality_metrics or {}),
            "candidate_selection": dict(selection_metadata),
        },
        normalize_candidate_quality_gate_plan_fn=lambda plan, **kwargs: dict(plan or {}),
        build_candidate_pool_summary_fn=lambda candidates, **kwargs: [
            {"candidate_index": item.get("candidate_index"), "is_winner": item.get("candidate_index") == kwargs["winner_candidate_index"]}
            for item in candidates
        ],
        sync_generation_runtime_state_fn=lambda runtime_state, **kwargs: sync_calls.append(dict(kwargs)),
        select_best_generation_candidate_fn=lambda candidates: dict(candidates[-1]),
        should_prefer_word_budget_repair_candidate_fn=lambda selected, repair: True,
    )


def test_should_resolve_final_candidate_state_with_word_budget_repair_metadata():
    sync_calls = []
    dependencies = _build_finalize_dependencies(sync_calls)
    request = ChapterCandidateFinalizeRequest(target_word_count=1200, source="chapter")
    selected_candidate = {
        "candidate_index": 2,
        "attempt_kind": "word_budget_repair",
        "generation_path": "word_budget_repair",
        "word_count": 1260,
        "quality_metrics": {"overall_score": 88},
        "quality_gate_plan": {"action": "continue", "quality_gate": {"decision": "allow_save"}},
        "candidate_chunks": ["chunk-a"],
    }
    candidates = [
        {"candidate_index": 1, "attempt_kind": "initial_candidate", "generation_path": "single_pass"},
        dict(selected_candidate),
    ]

    state = resolve_final_candidate_state(
        request=request,
        selected_candidate=selected_candidate,
        candidates=candidates,
        quality_gate_plan_builder=lambda metrics, attempt_offset: {"action": "continue", "quality_gate": {"decision": "allow_save"}},
        dependencies=dependencies,
    )

    assert state.winner_candidate_index == 2
    assert state.final_attempt_kind == "word_budget_repair"
    assert state.final_generation_path == "word_budget_repair"
    assert state.word_budget_repair_used is True
    assert state.rerank_used is False
    assert state.selected_candidate["winner_candidate_index"] == 2
    assert state.final_quality_metrics["candidate_selection"]["generation_path"] == "word_budget_repair"


def test_should_finalize_selected_candidate_result_and_sync_runtime_state():
    sync_calls = []
    dependencies = _build_finalize_dependencies(sync_calls)
    request = ChapterCandidateFinalizeRequest(target_word_count=1200, source="chapter", runtime_state={})
    selected_candidate = {
        "candidate_index": 2,
        "candidate_count": 2,
        "winner_candidate_index": 2,
        "word_count": 1260,
        "generation_path": "word_budget_repair",
        "attempt_kind": "word_budget_repair",
        "rerank_used": False,
        "word_budget_repair_used": True,
        "candidate_chunks": ["chunk-a", "chunk-b"],
        "quality_metrics": {"candidate_selection": {"repair_seed_candidate_index": 1}},
        "quality_gate_plan": {"action": "continue", "quality_gate": {"decision": "allow_save"}},
    }
    state = resolve_final_candidate_state(
        request=request,
        selected_candidate=selected_candidate,
        candidates=[
            {"candidate_index": 1},
            dict(selected_candidate),
        ],
        quality_gate_plan_builder=lambda metrics, attempt_offset: {"action": "continue", "quality_gate": {"decision": "allow_save"}},
        dependencies=dependencies,
    )

    result = finalize_selected_candidate_result(
        request=request,
        state=state,
        dependencies=dependencies,
    )

    assert result["candidate_count"] == 2
    assert result["rerank_pool_size"] == 2
    assert result["quality_metrics"]["candidate_pool_summary"][1]["is_winner"] is True
    assert sync_calls[-1]["winner_candidate_index"] == 2
    assert sync_calls[-1]["current_chars"] == 1260
    assert sync_calls[-1]["chunk_count"] == 2
