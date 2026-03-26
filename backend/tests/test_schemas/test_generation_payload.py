from __future__ import annotations

from app.schemas.generation_payload import (
    build_chapter_generation_quality_history_payload,
    build_chapter_generation_stream_result_payload,
    build_chapter_regeneration_stream_result_payload,
)


SAMPLE_RUNTIME_CONTRACT = {
    "guidance": {
        "creative_mode": "balanced",
        "story_focus": "conflict",
        "plot_stage": "opening",
        "story_creation_brief": "???????????",
        "quality_preset": "cinematic",
        "quality_notes": "???????????",
    },
    "blueprint": {
        "long_term_goal": "????????????",
        "chapter_count": 24,
        "current_chapter_number": 3,
        "target_word_count": 3200,
        "character_focus_names": ["??", "??"],
        "foreshadow_payoff_plan": ["????", "????"],
        "character_state_ledger": [{"name": "??", "state": "??????"}],
        "relationship_state_ledger": [{"pair": "??-??", "state": "????"}],
        "foreshadow_state_ledger": [{"name": "????", "status": "??"}],
        "organization_state_ledger": [{"name": "????", "state": "????"}],
        "career_state_ledger": [{"name": "???", "state": "?????"}],
    },
}


SAMPLE_QUALITY_METRICS = {
    "overall_score": 82.5,
    "conflict_chain_hit_rate": 84.0,
    "rule_grounding_hit_rate": 81.0,
    "outline_alignment_rate": 79.0,
    "dialogue_naturalness_rate": 86.0,
    "opening_hook_rate": 83.0,
    "payoff_chain_rate": 78.0,
    "cliffhanger_rate": 80.0,
}


def test_should_build_chapter_generation_quality_history_payload_with_runtime_contract():
    payload = build_chapter_generation_quality_history_payload(
        content="A" * 600,
        metrics=SAMPLE_QUALITY_METRICS,
        content_applied=False,
        story_runtime_contract=SAMPLE_RUNTIME_CONTRACT,
    )

    assert payload.log_type == "chapter_generation_quality_v1"
    assert len(payload.preview) == 500
    assert payload.content_applied is False
    assert payload.attempt_state == "candidate"
    assert payload.story_runtime_contract == SAMPLE_RUNTIME_CONTRACT
    assert payload.quality_metrics["story_runtime_contract"] == SAMPLE_RUNTIME_CONTRACT
    assert payload.story_runtime_snapshot is not None
    assert payload.story_runtime_snapshot["plot_stage"] == "opening"
    assert payload.story_runtime_snapshot["chapter_count"] == 24
    assert payload.story_runtime_snapshot["current_chapter_number"] == 3
    assert payload.story_runtime_snapshot["character_focus"] == ["??", "??"]
    assert payload.quality_metrics["quality_runtime_context"] == payload.story_runtime_snapshot
    assert isinstance(payload.quality_metrics.get("repair_guidance"), dict)
    assert payload.quality_metrics["repair_guidance"]["focus_areas"]
    assert isinstance(payload.quality_metrics.get("quality_gate"), dict)
    assert payload.quality_metrics["quality_gate"]["status"] in {"pass", "repairable", "blocked"}
    assert payload.generated_at


def test_should_build_generation_stream_result_payload_without_none_fields():
    payload = build_chapter_generation_stream_result_payload(
        word_count=2888,
        analysis_task_id=None,
        quality_metrics=None,
        quality_gate_action=None,
        quality_gate_message=None,
        content_applied=True,
        chapter_status="completed",
        saved_word_count=2860,
        hard_gate_blocked=False,
        story_runtime_contract=SAMPLE_RUNTIME_CONTRACT,
    )

    assert payload["word_count"] == 2888
    assert payload["content_applied"] is True
    assert payload["chapter_status"] == "completed"
    assert payload["saved_word_count"] == 2860
    assert payload["hard_gate_blocked"] is False
    assert payload["story_runtime_contract"] == SAMPLE_RUNTIME_CONTRACT
    assert "analysis_task_id" not in payload
    assert "quality_metrics" not in payload
    assert "quality_gate_action" not in payload
    assert "quality_gate_message" not in payload


def test_should_build_regeneration_stream_result_payload_without_none_fields():
    payload = build_chapter_regeneration_stream_result_payload(
        task_id="regen-task-001",
        word_count=3012,
        version_number=4,
        auto_applied=True,
        diff_stats=None,
        story_runtime_contract=SAMPLE_RUNTIME_CONTRACT,
    )

    assert payload == {
        "task_id": "regen-task-001",
        "word_count": 3012,
        "version_number": 4,
        "auto_applied": True,
        "diff_stats": {},
        "story_runtime_contract": SAMPLE_RUNTIME_CONTRACT,
    }
