from types import SimpleNamespace

from tests.test_support import chapter_candidate_executor_test_support as wiring_service


def test_should_build_default_candidate_executor_dependencies(monkeypatch):
    captured = {}

    monkeypatch.setattr(
        wiring_service,
        "build_chapter_candidate_generation_dependencies",
        lambda **kwargs: captured.setdefault("generation", kwargs) or SimpleNamespace(kind="generation"),
    )
    monkeypatch.setattr(
        wiring_service,
        "build_chapter_candidate_word_budget_repair_dependencies",
        lambda **kwargs: captured.setdefault("word_budget", kwargs) or SimpleNamespace(kind="word_budget"),
    )
    monkeypatch.setattr(
        wiring_service,
        "build_chapter_candidate_targeted_final_repair_dependencies",
        lambda **kwargs: captured.setdefault("targeted", kwargs) or SimpleNamespace(kind="targeted"),
    )
    monkeypatch.setattr(
        wiring_service,
        "build_chapter_candidate_finalize_dependencies",
        lambda **kwargs: captured.setdefault("finalize", kwargs) or SimpleNamespace(kind="finalize"),
    )

    def fake_executor_builder(**kwargs):
        captured["executor"] = kwargs
        return {"ok": True}

    monkeypatch.setattr(
        wiring_service,
        "build_chapter_candidate_executor_dependencies",
        fake_executor_builder,
    )

    result = wiring_service.build_default_chapter_candidate_executor_dependencies(
        resolve_generation_attempt_labels_fn="resolve",
        sync_generation_runtime_state_fn="sync",
        collect_generation_candidate_output_fn="collect",
        build_generation_candidate_record_fn="record",
    )

    assert result == {"ok": True}
    assert captured["generation"]["resolve_generation_attempt_labels_fn"] == "resolve"
    assert captured["generation"]["collect_generation_candidate_output_fn"] == "collect"
    assert captured["word_budget"]["build_generation_candidate_record_fn"] == "record"
    assert captured["targeted"]["sync_generation_runtime_state_fn"] == "sync"
    assert captured["finalize"]["resolve_generation_attempt_labels_fn"] == "resolve"
    assert captured["executor"]["should_apply_targeted_final_repair_fn"] is wiring_service.should_apply_targeted_final_repair
    assert (
        captured["executor"]["select_targeted_final_repair_seed_candidate_fn"]
        is wiring_service.select_targeted_final_repair_seed_candidate
    )
