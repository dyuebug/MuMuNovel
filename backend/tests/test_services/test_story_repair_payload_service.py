from app.services.story_repair_payload_service import (
    build_story_repair_runtime_state,
    normalize_story_repair_payload,
    resolve_story_repair_prompt_kwargs,
    story_repair_payload_to_prompt_kwargs,
)


def test_should_convert_story_repair_payload_to_prompt_kwargs():
    payload = normalize_story_repair_payload(
        summary="??????",
        targets=["?????????????"],
        strengths=["??????????"],
    )

    assert story_repair_payload_to_prompt_kwargs(payload) == {
        "story_repair_summary": "??????",
        "story_repair_targets": ["?????????????"],
        "story_preserve_strengths": ["??????????"],
    }


def test_should_build_story_repair_runtime_state_with_merged_sources():
    explicit_payload = normalize_story_repair_payload(
        summary="????????????",
        targets=["?????????????"],
        strengths=["??????????"],
    )
    derived_payload = normalize_story_repair_payload(
        summary="???????????",
        targets=["?????????????"],
        strengths=["???????"],
    )

    state = build_story_repair_runtime_state(
        explicit_payload=explicit_payload,
        derived_payload=derived_payload,
        scope="chapter",
        derived_source="current_chapter_quality",
        guidance={
            "summary": "??????????????????????",
            "focus_areas": ["conflict", "payoff"],
            "weakest_metric_label": "????",
            "weakest_metric_value": 58.0,
        },
        quality_gate={
            "status": "repairable",
            "decision": "auto_repair",
            "label": "????",
            "summary": "????????????",
            "failed_metrics": [
                {"label": "?????"},
                {"label": "????"},
            ],
        },
    )

    assert state["payload"] is not None
    assert state["payload"].summary == "????????????"
    assert state["payload"].targets == ("?????????????",)
    assert state["active_story_repair_payload"]["source"] == "manual_plus_current_chapter_quality"
    assert state["active_story_repair_payload"]["source_label"] == "Manual + current chapter quality"
    assert state["active_story_repair_payload"]["quality_gate_decision"] == "auto_repair"
    assert state["active_story_repair_payload"]["quality_gate_failed_metrics"] == ["?????", "????"]



def test_should_merge_explicit_story_repair_prompt_kwargs_with_payload_fallback():
    payload = normalize_story_repair_payload(
        summary="补强冲突折返与伏笔兑现",
        targets=["升级代价", "兑现伏笔"],
        strengths=["保留对白辨识度"],
    )

    kwargs = resolve_story_repair_prompt_kwargs(
        payload,
        summary="优先把代价写实",
        strengths=["保留动作节奏"],
    )

    assert kwargs["story_repair_summary"] == "优先把代价写实"
    assert kwargs["story_repair_targets"] == ["升级代价", "兑现伏笔"]
    assert kwargs["story_preserve_strengths"] == ["保留动作节奏"]
