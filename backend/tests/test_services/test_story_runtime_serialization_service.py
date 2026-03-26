from app.services.story_runtime_serialization_service import (
    attach_story_runtime_contract,
    attach_story_runtime_result_payload,
    extract_story_runtime_snapshot_from_contract,
)


def test_should_extract_runtime_snapshot_from_contract():
    contract = {
        "guidance": {
            "plot_stage": "development",
            "story_focus": "alliance under pressure",
            "quality_preset": "cinematic",
        },
        "blueprint": {
            "long_term_goal": "recover the hidden key",
            "chapter_count": 12,
            "current_chapter_number": 6,
            "target_word_count": 2400,
            "character_focus_names": ["Lin", "Su"],
            "organization_state_ledger": ["ShadowGuild: control tightened around the docks"],
            "career_state_ledger": ["Lin/Strategist: stage 3 with supply-chain pressure"],
        },
    }

    snapshot = extract_story_runtime_snapshot_from_contract(contract)

    assert snapshot is not None
    assert snapshot["plot_stage"] == "development"
    assert snapshot["story_focus"] == "alliance under pressure"
    assert snapshot["chapter_count"] == 12
    assert snapshot["current_chapter_number"] == 6
    assert snapshot["character_focus"] == ["Lin", "Su"]


def test_should_attach_story_runtime_contract_to_metrics_and_result_payload():
    contract = {
        "guidance": {"plot_stage": "ending"},
        "blueprint": {"current_chapter_number": 10, "chapter_count": 12},
    }

    metrics = attach_story_runtime_contract({"overall_score": 88.0}, contract)
    result_payload = attach_story_runtime_result_payload({"word_count": 2200}, contract)

    assert metrics["story_runtime_contract"] == contract
    assert metrics["quality_runtime_context"]["plot_stage"] == "ending"
    assert metrics["quality_runtime_context"]["current_chapter_number"] == 10
    assert result_payload["word_count"] == 2200
    assert result_payload["story_runtime_contract"] == contract
