import pytest
from typing import Any
from sqlalchemy import select

from app.api import chapter_regeneration_routes as chapter_regeneration_routes_api
from app.models.batch_generation_task import BatchGenerationTask
from app.models.chapter import Chapter
from app.models.chapter_draft_attempt import ChapterDraftAttempt
from app.models.generation_history import GenerationHistory
from app.models.regeneration_task import RegenerationTask
from app.api import chapter_partial_regeneration_routes as chapter_partial_regeneration_routes_api
from tests.test_api.chapters_test_support import (
    chapters_client,
    chapters_session_factory,
    create_chapter,
    create_project,
    fake_ai_service,
    load_batch_generation_rollback_modules,
    load_single_generation_rollback_modules,
    mock_side_effect_services,
    parse_sse_data,
    reset_chapters_runtime_caches,
)

pytestmark = pytest.mark.asyncio

@pytest.mark.parametrize(
    ("outline_mode", "expected_builder"),
    [("one-to-many", "many"), ("one-to-one", "one")],
)
async def test_should_build_context_with_expected_builder_during_generate_stream(
    chapters_client,
    chapters_session_factory,
    fake_ai_service,
    mock_user,
    monkeypatch,
    outline_mode,
    expected_builder,
):
    project = await create_project(
        chapters_session_factory,
        user_id=mock_user.user_id,
        outline_mode=outline_mode,
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待生成章节",
    )

    calls = {"many": 0, "one": 0}

    class FakeContext:
        chapter_outline = "夜巡篇大纲"
        continuation_point = None
        previous_chapter_summary = ""
        chapter_characters = (
            "角色状态\n- 角色A\n"
            "人物动态\n- 角色A承担巡查任务\n"
            "关系动态\n- 角色A/角色B暂时同盟"
        )
        chapter_careers = "角色A：巡夜人"
        foreshadow_reminders = (
            "伏笔提醒\n- 神秘怀表\n"
            "回收线索\n- 留意阁楼钥匙的来源"
        )
        relevant_memories = ""
        recent_chapters_context = ""
        context_stats = {}

    class FakeOneToManyBuilder:
        def __init__(self, *args, **kwargs):
            pass

        async def build(self, **kwargs):
            calls["many"] += 1
            return FakeContext()

    class FakeOneToOneBuilder:
        def __init__(self, *args, **kwargs):
            pass

        async def build(self, **kwargs):
            calls["one"] += 1
            return FakeContext()

    async def fake_get_template(*args, **kwargs):
        return "模板"

    def fake_format_prompt(template, **kwargs):
        return "mock-generate-prompt"

    def fake_build_runtime_system_prompt(*args, **kwargs):
        return "mock-runtime-system-prompt"

    def fake_compute_story_quality_metrics(**kwargs):
        return {
            "overall_score": 92.0,
            "conflict_chain_hit_rate": 90.0,
            "rule_grounding_hit_rate": 91.0,
            "outline_alignment_rate": 93.0,
            "dialogue_naturalness_rate": 92.0,
            "opening_hook_rate": 90.0,
            "payoff_chain_rate": 91.0,
            "cliffhanger_rate": 90.0,
            "pacing_score": 8.8,
        }

    def fake_resolve_quality_gate_execution_plan(*args, **kwargs):
        return {
            "action": "continue",
            "message": "ok",
            "quality_gate": {
                "decision": "allow_save",
                "status": "pass",
                "failed_metrics": [],
            },
        }

    _, chapter_generation_route_wiring_service = load_single_generation_rollback_modules()
    monkeypatch.setattr(chapter_generation_route_wiring_service, "get_template", fake_get_template)
    monkeypatch.setattr(chapter_generation_route_wiring_service, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(chapter_generation_route_wiring_service, "OneToManyContextBuilder", FakeOneToManyBuilder)
    monkeypatch.setattr(chapter_generation_route_wiring_service, "OneToOneContextBuilder", FakeOneToOneBuilder)
    monkeypatch.setattr(chapter_generation_route_wiring_service, "build_chapter_runtime_system_prompt", fake_build_runtime_system_prompt)
    monkeypatch.setattr(chapter_generation_route_wiring_service, "compute_story_quality_metrics", fake_compute_story_quality_metrics)
    monkeypatch.setattr(chapter_generation_route_wiring_service, "resolve_quality_gate_execution_plan", fake_resolve_quality_gate_execution_plan)

    fake_ai_service.calls.clear()
    fake_ai_service.chunks = ["段落甲", "段落乙"]

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-stream",
        json={"target_word_count": 500},
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    assert any(event.get("type") == "chunk" for event in events)
    assert any(event.get("type") == "result" for event in events)

    assert calls[expected_builder] == 1
    unexpected_builder = "one" if expected_builder == "many" else "many"
    assert calls[unexpected_builder] == 0

    assert fake_ai_service.calls
    last_call = fake_ai_service.calls[-1]
    assert last_call["prompt"].startswith("mock-generate-prompt")
    assert last_call["max_tokens"] > 0

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        assert saved_chapter is not None
        assert saved_chapter.status == "completed"
        assert saved_chapter.content == "段落甲段落乙"

async def test_should_stream_partial_regenerate_with_mock_ai_response(
    chapters_client,
    chapters_session_factory,
    fake_ai_service,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待局部重写章节",
        content="ABCDEFG",
        status="completed",
    )

    fake_ai_service.calls.clear()
    fake_ai_service.chunks = ["重", "写"]

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/partial-regenerate-stream",
        json={
            "selected_text": "BCD",
            "start_position": 1,
            "end_position": 4,
            "user_instructions": "增强表现力",
            "context_chars": 120,
            "length_mode": "similar",
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    result_event = next(event for event in events if event.get("type") == "result")
    assert result_event["data"]["new_text"] == "重写"
    assert result_event["data"]["original_word_count"] == 3

    assert fake_ai_service.calls
    assert fake_ai_service.calls[-1]["max_tokens"] == 500

async def test_should_stream_partial_regenerate_with_web_research_grounding(
    chapters_client,
    chapters_session_factory,
    fake_ai_service,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="partial-regenerate-with-research",
        content="ABCDEFG",
        status="completed",
    )

    captured_research: dict[str, Any] = {}

    async def fake_collect_for_chapter(**kwargs):
        captured_research.update(kwargs)
        return {
            "assets": [
                {
                    "title": "Harbor Guild Rules",
                    "snippet": "Guild protocol and tariff etiquette.",
                    "usage_hint": "Improve rule grounding and action consequences.",
                }
            ]
        }

    from app.services import partial_regeneration_service as partial_regeneration_service_module

    monkeypatch.setattr(
        partial_regeneration_service_module.chapter_web_research_service,
        "collect_for_chapter",
        fake_collect_for_chapter,
    )

    fake_ai_service.calls.clear()
    fake_ai_service.chunks = ["Re", "write"]

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/partial-regenerate-stream",
        json={
            "selected_text": "BCD",
            "start_position": 1,
            "end_position": 4,
            "user_instructions": "Improve the action texture.",
            "context_chars": 120,
            "length_mode": "similar",
            "enable_web_research": True,
            "web_research_query": "late qing harbor guild rules",
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    result_event = next(event for event in events if event.get("type") == "result")
    assert result_event["data"]["new_text"] == "Rewrite"

    assert captured_research["enable_web_research"] is True
    assert captured_research["web_research_query"] == "late qing harbor guild rules"
    assert fake_ai_service.calls
    assert "[Web Research References]" in fake_ai_service.calls[-1]["prompt"]
    assert "Harbor Guild Rules" in fake_ai_service.calls[-1]["prompt"]


async def test_should_sanitize_partial_regenerate_text_before_apply(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="\u5c40\u90e8\u6539\u5199\u6e05\u6d17\u6d4b\u8bd5",
        content="ABCDEFG",
        status="completed",
    )

    new_text = (
        "\u4e0b\u4e00\u79d2\uff0c\u95e8\u5916\u6709\u4eba\u6572\u4e86\u4e24\u4e0b\u73bb\u7483\u3002\n"
        "\u4e0b\u4e00\u79d2\uff0c\u6536\u94f6\u53f0\u4e0b\u7684\u706f\u706d\u4e86\u3002"
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/apply-partial-regenerate",
        json={
            "new_text": new_text,
            "start_position": 1,
            "end_position": 4,
        },
    )
    assert response.status_code == 200

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        assert saved_chapter is not None
        assert saved_chapter.content == (
            "A"
            "\u4e0b\u4e00\u79d2\uff0c\u95e8\u5916\u6709\u4eba\u6572\u4e86\u4e24\u4e0b\u73bb\u7483\u3002\n"
            "\u6536\u94f6\u53f0\u4e0b\u7684\u706f\u706d\u4e86\u3002"
            "EFG"
        )

async def test_should_delegate_apply_partial_regenerate_route_to_compat_service(
    chapters_client,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_apply_partial_regenerate_with_default_route_wiring(**kwargs):
        captured.update(kwargs)
        return {
            "success": True,
            "chapter_id": kwargs["chapter_id"],
            "word_count": 11,
            "old_word_count": 7,
            "message": "ok",
        }

    monkeypatch.setattr(
        chapter_partial_regeneration_routes_api,
        "apply_partial_regenerate_with_default_route_wiring",
        fake_apply_partial_regenerate_with_default_route_wiring,
    )

    response = await chapters_client.post(
        '/api/chapters/delegated-partial/apply-partial-regenerate',
        json={"new_text": "XYZ", "start_position": 1, "end_position": 4},
    )

    assert response.status_code == 200
    assert response.json()["chapter_id"] == "delegated-partial"
    assert captured["chapter_id"] == "delegated-partial"
    assert captured["request"] is not None
    assert captured["db_session"] is not None
    assert captured["apply_request"]["new_text"] == "XYZ"


async def test_should_return_400_when_partial_regenerate_position_invalid(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="非法位置测试章节",
        content="ABCDEFG",
        status="completed",
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/partial-regenerate-stream",
        json={
            "selected_text": "BCD",
            "start_position": 4,
            "end_position": 2,
            "user_instructions": "增强表现力",
            "context_chars": 120,
            "length_mode": "similar",
        },
    )
    assert response.status_code == 400

async def test_should_stream_batch_generation_events_via_route_compat(
    chapters_client,
    monkeypatch,
):
    (
        chapter_batch_generation_routes_api,
        _batch_generation_route_wiring_service,
    ) = load_batch_generation_rollback_modules()

    async def fake_validate_access(db_session, *, batch_id, user_id):
        assert batch_id == "batch-1"
        assert user_id is not None
        return object()

    async def fake_build_stream(db_session, *, batch_id):
        from app.utils.sse_response import SSEResponse
        yield await SSEResponse.send_progress("connected", 0, "processing")
        yield await SSEResponse.send_done()

    monkeypatch.setattr(
        chapter_batch_generation_routes_api,
        "validate_batch_generation_stream_access",
        fake_validate_access,
    )
    monkeypatch.setattr(
        chapter_batch_generation_routes_api,
        "build_batch_generation_event_stream",
        fake_build_stream,
    )

    response = await chapters_client.get("/api/chapters/batch-generate/batch-1/stream")
    assert response.status_code == 200
    events = parse_sse_data(response.text)
    assert any(event.get("type") == "progress" for event in events)
    assert any(event.get("type") == "done" for event in events)


async def test_should_reject_stream_subscription_from_other_user(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="订阅权限测试章节",
        content=None,
    )

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=[chapter.id],
            status="running",
            total_chapters=1,
            completed_chapters=0,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        task_id = task.id

    response = await chapters_client.get(
        f"/api/chapters/batch-generate/{task_id}/stream",
        headers={"x-test-user-id": "other-user"},
    )
    assert response.status_code == 403

async def test_should_regenerate_chapter_stream_and_persist_regeneration_task(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待重写章节",
        content="这是旧内容",
        status="completed",
    )

    class FakeRegenerator:
        def __init__(self, ai_service):
            self.ai_service = ai_service

        async def regenerate_with_feedback(self, **kwargs):
            yield {"type": "progress", "progress": 35, "message": "准备中"}
            yield {"type": "chunk", "content": "新"}
            yield {"type": "chunk", "content": "内容"}

        def calculate_content_diff(self, original_content, new_content):
            return {"similarity": 12.5, "difference": 87.5}

    monkeypatch.setattr(
        chapter_regeneration_routes_api,
        "REGENERATOR_FACTORY",
        FakeRegenerator,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "优化节奏",
            "target_word_count": 500,
            "focus_areas": ["pacing"],
            "auto_apply": False,
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    result_event = next(event for event in events if event.get("type") == "result")
    task_id = result_event["data"]["task_id"]
    assert result_event["data"]["word_count"] > 0
    assert "diff_stats" in result_event["data"]

    async with chapters_session_factory() as session:
        task = await session.get(RegenerationTask, task_id)
        assert task is not None
        assert task.status == "completed"
        assert task.regenerated_content == "新内容"

async def test_should_get_and_apply_candidate_draft(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="candidate apply chapter",
        content="old text",
        status="completed",
    )
    candidate_content = (
        "Candidate draft restores the alliance fracture after the dock control change, "
        "and the hidden key oath now surfaces with a visible cost."
    )

    async with chapters_session_factory() as session:
        draft_attempt = ChapterDraftAttempt(
            project_id=project.id,
            chapter_id=chapter.id,
            source="chapter",
            attempt_state="manual_review",
            quality_gate_action="manual_review",
            quality_gate_decision="manual_review",
            word_count=len(candidate_content),
            summary_preview="candidate summary",
            content_preview=candidate_content[:4000],
            quality_metrics={
                "overall_score": 80.1,
                "quality_gate": {
                    "status": "blocked",
                    "decision": "manual_review",
                    "failed_metrics": [],
                },
                "quality_runtime_context": {
                    "relationship_state_ledger": [
                        "Alliance fracture: Lin and Su are still at odds",
                        "Watchtower alarm: the crew expects the alarm signal tonight",
                    ],
                    "organization_state_ledger": [
                        "Dock control change: Su now controls the docks",
                    ],
                    "foreshadow_state_ledger": [
                        "Hidden key oath: the price of the hidden key still hangs over Lin",
                    ],
                    "foreshadow_payoff_plan": [
                        "Hidden key payoff: reveal the price of the hidden key oath",
                        "Royal seal payoff: identify who now holds the royal seal",
                    ],
                },
                "continuity_preflight": {
                    "status": "warning",
                    "summary": "Need to keep the alliance fracture and dock control change in play.",
                    "repair_targets": [
                        "Carry forward the alliance fracture in action.",
                        "Mention the dock control change in a consequential beat.",
                    ],
                    "warnings": [
                        {
                            "item": "Watchtower alarm",
                            "focus_area": "relationship_continuity",
                        }
                    ],
                },
                "foreshadow_payoff_delay": {
                    "status": "warning",
                    "summary": "Key payoff still needs to land on the page.",
                    "repair_targets": [
                        "Reveal the price of the hidden key oath.",
                        "Clarify who now holds the royal seal.",
                    ],
                },
            },
            repair_payload={
                "summary": "Candidate summary",
                "repair_targets": ["Improve the transition"],
                "candidate_full_content": candidate_content,
                "content_complete": True,
            },
        )
        session.add(draft_attempt)
        await session.commit()
        await session.refresh(draft_attempt)
        attempt_id = draft_attempt.id

    draft_response = await chapters_client.get(
        f"/api/chapters/{chapter.id}/analysis/candidate-draft"
    )
    assert draft_response.status_code == 200
    draft = draft_response.json()["candidate_draft"]
    assert draft["attempt_id"] == attempt_id
    assert draft["content"] == candidate_content
    assert draft["can_apply"] is True
    assert any("Alliance fracture" in item for item in draft["quality_highlights"]["continuity"]["matched_items"])
    assert any("Watchtower alarm" in item for item in draft["quality_highlights"]["continuity"]["missing_items"])
    assert draft["quality_highlights"]["continuity"]["matched_evidence"]
    assert any("dock control change" in evidence["snippet"] for evidence in draft["quality_highlights"]["continuity"]["matched_evidence"])
    assert any("Royal seal" in item for item in draft["quality_highlights"]["foreshadow"]["missing_items"])
    assert draft["quality_highlights"]["foreshadow"]["matched_evidence"]
    assert draft["apply_risk"]["status"] == "warning"
    assert any("Watchtower alarm" in item for item in draft["apply_risk"]["items"])
    assert any("Royal seal" in item for item in draft["apply_risk"]["items"])

    apply_response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/analysis/candidate-draft/apply",
        json={"attempt_id": attempt_id},
    )
    assert apply_response.status_code == 200
    apply_body = apply_response.json()
    assert apply_body["success"] is True
    assert apply_body["draft_attempt_id"] == attempt_id
    assert apply_body["word_count"] == len(candidate_content)

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        assert saved_chapter is not None
        assert saved_chapter.content == candidate_content

        history_result = await session.execute(
            select(GenerationHistory)
            .where(GenerationHistory.chapter_id == chapter.id)
            .order_by(GenerationHistory.created_at.desc())
        )
        histories = history_result.scalars().all()
        assert any(history.model == "chapter_candidate_apply_v1" for history in histories)

async def test_should_reject_preview_only_candidate_draft_apply(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="preview only chapter",
        content="old text",
        status="completed",
    )

    async with chapters_session_factory() as session:
        draft_attempt = ChapterDraftAttempt(
            project_id=project.id,
            chapter_id=chapter.id,
            source="chapter",
            attempt_state="manual_review",
            quality_gate_action="manual_review",
            quality_gate_decision="manual_review",
            word_count=120,
            summary_preview="preview",
            content_preview="preview only draft",
            quality_metrics={"overall_score": 74.0},
            repair_payload={"summary": "preview-only draft"},
        )
        session.add(draft_attempt)
        await session.commit()
        await session.refresh(draft_attempt)
        attempt_id = draft_attempt.id

    apply_response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/analysis/candidate-draft/apply",
        json={"attempt_id": attempt_id},
    )
    assert apply_response.status_code == 409
    assert "预览" in apply_response.json()["detail"]
