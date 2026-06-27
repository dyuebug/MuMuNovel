import tests.test_support.story_repair_payload_test_support as story_repair_payload_service


def test_should_keep_manual_review_for_batch_when_quality_gate_requests_manual_review(monkeypatch):
    monkeypatch.setattr(
        story_repair_payload_service,
        "resolve_quality_gate_from_metrics",
        lambda *args, **kwargs: {
            "decision": "manual_review",
            "label": "当前章节质量",
            "summary": "建议人工复核后再决定是否重写。",
            "reason": "存在 3 个弱项指标",
            "recommended_action_label": "补桥关键场景",
        },
    )

    plan = story_repair_payload_service.resolve_quality_gate_execution_plan(
        {"overall_score": 55},
        retry_count=0,
        max_retries=2,
        current_story_repair_payload=None,
        scope="batch",
    )

    assert plan["action"] == "manual_review"
    assert plan["quality_gate"]["decision"] == "manual_review"
    assert "补桥关键场景" in plan["message"]


def test_should_keep_manual_review_for_chapter_scope(monkeypatch):
    monkeypatch.setattr(
        story_repair_payload_service,
        "resolve_quality_gate_from_metrics",
        lambda *args, **kwargs: {
            "decision": "manual_review",
            "label": "当前章节质量",
            "summary": "建议人工复核后再决定是否重写。",
        },
    )

    plan = story_repair_payload_service.resolve_quality_gate_execution_plan(
        {"overall_score": 55},
        retry_count=0,
        max_retries=2,
        current_story_repair_payload=None,
        scope="chapter",
    )

    assert plan["action"] == "manual_review"
