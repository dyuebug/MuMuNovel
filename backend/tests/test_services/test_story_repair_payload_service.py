from app.services.story_repair_payload_service import (
    build_story_repair_runtime_state,
    normalize_story_repair_payload,
    resolve_story_repair_prompt_kwargs,
    story_repair_payload_to_prompt_kwargs,
)


def test_should_convert_story_repair_payload_to_prompt_kwargs():
    payload = normalize_story_repair_payload(
        summary="补强冲突兑现",
        targets=["强化代价递进"],
        strengths=["保留对白节奏"],
    )

    assert story_repair_payload_to_prompt_kwargs(payload) == {
        "story_repair_summary": "补强冲突兑现",
        "story_repair_targets": ["强化代价递进"],
        "story_preserve_strengths": ["保留对白节奏"],
    }


def test_should_build_story_repair_runtime_state_with_merged_sources():
    explicit_payload = normalize_story_repair_payload(
        summary="先补足代价链路",
        targets=["强化代价递进"],
        strengths=["保留对白节奏"],
    )
    derived_payload = normalize_story_repair_payload(
        summary="聚焦伏笔回收",
        targets=["兑现前文伏笔"],
        strengths=["保留动作节奏"],
    )

    state = build_story_repair_runtime_state(
        explicit_payload=explicit_payload,
        derived_payload=derived_payload,
        scope="chapter",
        derived_source="current_chapter_quality",
        guidance={
            "summary": "当前章节需要补强冲突与伏笔兑现",
            "focus_areas": ["conflict", "payoff"],
            "weakest_metric_label": "兑现链",
            "weakest_metric_value": 58.0,
        },
        quality_gate={
            "status": "repairable",
            "decision": "auto_repair",
            "label": "可修复",
            "summary": "关键指标存在短板",
            "failed_metrics": [
                {"label": "冲突链"},
                {"label": "兑现链"},
            ],
        },
    )

    assert state["payload"] is not None
    assert state["payload"].summary == "先补足代价链路"
    assert state["payload"].targets == ("强化代价递进",)
    assert state["active_story_repair_payload"]["source"] == "manual_plus_current_chapter_quality"
    assert state["active_story_repair_payload"]["source_label"] == "Manual + current chapter quality"
    assert state["active_story_repair_payload"]["quality_gate_decision"] == "auto_repair"
    assert state["active_story_repair_payload"]["quality_gate_failed_metrics"] == ["冲突链", "兑现链"]



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
