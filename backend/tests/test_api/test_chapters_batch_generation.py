import json
import asyncio
from datetime import datetime
from typing import Any

import pytest
from sqlalchemy import select

import tests.test_support.database_test_support as app_database
from migrator_app.models.batch_generation_task import BatchGenerationTask
from migrator_app.models.chapter import Chapter
from migrator_app.models import ChapterDraftAttempt, GenerationHistory
from migrator_app.models.project import Project
from tests.test_support import (
    batch_generation_retry_test_adapter as batch_generation_retry_service,
)
from tests.test_support import (
    batch_generation_single_chapter_wiring_test_adapter as batch_generation_single_chapter_entry_service,
)
from tests.test_support.story_packet_test_support import StoryPacket
from tests.test_api.chapters_test_support import (
    REAL_EXECUTE_BATCH_GENERATION_IN_ORDER,
    chapters_client,
    chapters_session_factory,
    create_chapter,
    create_outline,
    create_project,
    fake_ai_service,
    load_single_generation_stream_entry_module,
    mock_side_effect_services,
    parse_sse_data,
    reset_chapters_runtime_caches,
)
from tests.test_support import (
    batch_generation_run_wiring_test_adapter as batch_generation_run_wiring_service,
)

pytestmark = pytest.mark.asyncio

async def test_should_create_single_chapter_background_generation_task_via_generation_route_compat(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="single-background-outline",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="single-background-chapter",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 1200},
    )

    assert response.status_code == 200
    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["status"] == "pending"
    assert body["task_id"]

    async with chapters_session_factory() as session:
        task = await session.get(BatchGenerationTask, body["task_id"])
        assert task is not None
        assert task.chapter_count == 1
        assert task.chapter_ids == [chapter.id]
        assert task.target_word_count == 1200
        assert task.enable_analysis is True


@pytest.mark.parametrize(
    ("quality_metrics", "expected_action", "expected_decision", "expect_provisional_save"),
    [
        (
            {
                "overall_score": 80.5,
                "conflict_chain_hit_rate": 78.0,
                "rule_grounding_hit_rate": 82.0,
                "outline_alignment_rate": 84.0,
                "dialogue_naturalness_rate": 79.0,
                "opening_hook_rate": 83.0,
                "payoff_chain_rate": 77.0,
                "cliffhanger_rate": 81.0,
                "pacing_score": 7.8,
            },
            "retry",
            "auto_repair",
            True,
        ),
        (
            {
                "overall_score": 66.0,
                "conflict_chain_hit_rate": 48.0,
                "rule_grounding_hit_rate": 52.0,
                "outline_alignment_rate": 50.0,
                "dialogue_naturalness_rate": 75.0,
                "opening_hook_rate": 70.0,
                "payoff_chain_rate": 46.0,
                "cliffhanger_rate": 68.0,
                "pacing_score": 6.1,
            },
            "manual_review",
            "manual_review",
            False,
        ),
    ],
)

async def test_should_schedule_followup_analysis_when_generate_stream_hits_quality_gate(
    chapters_client,
    chapters_session_factory,
    fake_ai_service,
    mock_user,
    monkeypatch,
    quality_metrics,
    expected_action,
    expected_decision,
    expect_provisional_save,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待生成章节",
    )

    calls: list[dict[str, Any]] = []
    quality_metric_calls: list[dict[str, Any]] = []

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
            return FakeContext()

    async def fake_get_template(*args, **kwargs):
        return "模板"

    def fake_format_prompt(template, **kwargs):
        return "mock-generate-prompt"

    def fake_build_runtime_system_prompt(*args, **kwargs):
        return "mock-runtime-system-prompt"

    def fake_compute_story_quality_metrics(**kwargs):
        quality_metric_calls.append(kwargs)
        return dict(quality_metrics)

    async def fake_analyze_chapter_background(**kwargs):
        calls.append(kwargs)
        return True

    async def fake_sleep(_seconds):
        return None

    stream_entry_service = load_single_generation_stream_entry_module()
    monkeypatch.setattr(stream_entry_service, "get_template", fake_get_template)
    monkeypatch.setattr(stream_entry_service, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(stream_entry_service, "OneToManyContextBuilder", FakeOneToManyBuilder)
    monkeypatch.setattr(stream_entry_service, "build_chapter_runtime_system_prompt", fake_build_runtime_system_prompt)
    monkeypatch.setattr(stream_entry_service, "compute_story_quality_metrics", fake_compute_story_quality_metrics)
    monkeypatch.setattr(stream_entry_service, "execute_chapter_analysis_background", fake_analyze_chapter_background)
    
    fake_ai_service.calls.clear()
    fake_ai_service.chunks = ["继续", "创作"]
    expected_generated_content = "".join(fake_ai_service.chunks)

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-stream",
        json={"target_word_count": 500, "enable_analysis": False},
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    result_event = next(event for event in events if event.get("type") == "result")
    result_data = result_event["data"]

    assert result_data["analysis_task_id"]
    assert result_data["quality_gate_action"] == expected_action
    assert result_data["quality_metrics"]["quality_gate"]["decision"] == expected_decision
    assert result_data["quality_metrics"]["quality_gate"]["status"]
    candidate_draft = result_data["candidate_draft"]
    assert candidate_draft["attempt_state"] == expected_action
    assert candidate_draft["quality_gate_action"] == expected_action
    assert candidate_draft["quality_gate_decision"] == expected_decision
    assert candidate_draft["word_count"] == len(expected_generated_content)
    assert candidate_draft["can_apply"] is True
    assert candidate_draft["has_full_content"] is True
    assert candidate_draft.get("content") is None

    assert calls
    assert calls[0]["chapter_id"] == chapter.id
    assert calls[0]["project_id"] == project.id
    assert calls[0]["task_id"] == result_data["analysis_task_id"]
    assert isinstance(calls[0]["story_packet"], StoryPacket)
    assert calls[0]["chapter_content_override"] == "继续创作"
    assert calls[0]["chapter_word_count_override"] == len("继续创作")

    assert quality_metric_calls
    runtime_context = quality_metric_calls[0]["quality_runtime_context"]
    assert runtime_context["current_chapter_number"] == 1
    assert runtime_context["target_word_count"] == 500
    assert "角色A承担巡查任务" in runtime_context["character_state_ledger"]
    assert "角色A/角色B暂时同盟" in runtime_context["relationship_state_ledger"]
    assert "留意阁楼钥匙的来源" in runtime_context["foreshadow_state_ledger"]

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        saved_project = await session.get(Project, project.id)
        assert saved_chapter is not None
        assert saved_chapter.status == "draft"
        assert saved_project is not None
        if expect_provisional_save:
            assert saved_chapter.content == expected_generated_content
            assert saved_chapter.word_count == len(expected_generated_content)
            assert saved_project.current_words == len(expected_generated_content)
            assert result_data["saved_word_count"] == len(expected_generated_content)
            assert result_data["content_applied"] is False
        else:
            assert saved_chapter.content is None
            assert saved_chapter.word_count == 0
            assert saved_project.current_words == 0

        history_result = await session.execute(
            select(GenerationHistory).where(GenerationHistory.chapter_id == chapter.id)
        )
        histories = history_result.scalars().all()
        draft_attempt_result = await session.execute(
            select(ChapterDraftAttempt).where(ChapterDraftAttempt.chapter_id == chapter.id)
        )
        draft_attempts = draft_attempt_result.scalars().all()
        assert len(histories) == 1
        history_payload = json.loads(histories[0].generated_content)
        assert history_payload["content_applied"] is False
        assert history_payload["attempt_state"] == expected_action
        assert len(draft_attempts) == 1
        assert draft_attempts[0].source == "chapter"
        assert draft_attempts[0].attempt_state == expected_action
        assert draft_attempts[0].quality_gate_decision == expected_decision
        assert candidate_draft["attempt_id"] == draft_attempts[0].id
        assert candidate_draft["content_preview"]

async def test_execute_batch_generation_should_apply_candidate_only_after_quality_gate_passes(
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
        title="batch-pass",
    )

    async with chapters_session_factory() as session:
        engine = session.bind
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=[chapter.id],
            status="pending",
            total_chapters=1,
            completed_chapters=0,
            target_word_count=600,
            enable_analysis=False,
            max_retries=1,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        batch_id = task.id

    async def fake_get_engine(_user_id):
        return engine

    async def fake_check_prerequisites(*args, **kwargs):
        return True, None, None

    async def fake_generate_single_chapter_for_batch(**kwargs):
        assert kwargs["base_quality_profile"]["resolved_style_id"] is None
        content = "batch-candidate-pass"
        return {
            "full_content": content,
            "word_count": len(content),
            "summary_preview": content,
            "quality_metrics": {
                "overall_score": 88.0,
                "conflict_chain_hit_rate": 82.0,
                "rule_grounding_hit_rate": 84.0,
                "outline_alignment_rate": 86.0,
                "dialogue_naturalness_rate": 80.0,
                "opening_hook_rate": 87.0,
                "payoff_chain_rate": 81.0,
                "cliffhanger_rate": 85.0,
                "pacing_score": 8.0,
            },
        }

    async def fake_resolve_generation_story_repair_state_for_batch(*args, **kwargs):
        return {"payload": None, "active_story_repair_payload": None}

    async def fake_sync_task_story_repair_state(*args, **kwargs):
        return {"payload": None, "active_story_repair_payload": None}

    async def fake_publish_task_stream_event(*args, **kwargs):
        return None

    async def fake_record_task_quality_metrics(*args, **kwargs):
        return None

    async def fake_sleep(_seconds):
        return None

    monkeypatch.setattr(app_database, "get_engine", fake_get_engine)
    monkeypatch.setattr(
        batch_generation_retry_service,
        "check_chapter_generation_prerequisites",
        fake_check_prerequisites,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "generate_single_chapter_for_batch",
        fake_generate_single_chapter_for_batch,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "resolve_story_repair_state_for_batch",
        fake_resolve_generation_story_repair_state_for_batch,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "sync_task_story_repair_state",
        fake_sync_task_story_repair_state,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "publish_task_stream_event_service",
        fake_publish_task_stream_event,
    )
    monkeypatch.setattr(
        batch_generation_retry_service,
        "record_task_quality_metrics",
        fake_record_task_quality_metrics,
    )
    
    await REAL_EXECUTE_BATCH_GENERATION_IN_ORDER(
        batch_id=batch_id,
        user_id=mock_user.user_id,
        ai_service=fake_ai_service,
        base_quality_profile={
            "resolved_style_id": None,
            "style_content": "",
            "style_name": "",
            "style_preset_id": "",
        },
    )

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        saved_project = await session.get(Project, project.id)
        saved_task = await session.get(BatchGenerationTask, batch_id)
        history_result = await session.execute(
            select(GenerationHistory).where(GenerationHistory.chapter_id == chapter.id)
        )
        histories = history_result.scalars().all()

        assert saved_chapter is not None
        assert saved_chapter.content == "batch-candidate-pass"
        assert saved_chapter.status == "completed"
        assert saved_chapter.word_count == len("batch-candidate-pass")
        assert saved_project is not None
        assert saved_project.current_words == len("batch-candidate-pass")
        assert saved_task is not None
        assert saved_task.status == "completed"
        assert saved_task.completed_chapters == 1
        assert len(histories) == 1
        assert "batch-candidate-pass" in histories[0].generated_content

async def test_execute_batch_generation_should_forward_web_research_options_to_single_chapter_generation(
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
        title="batch-web-research-forwarding",
    )

    async with chapters_session_factory() as session:
        engine = session.bind
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=[chapter.id],
            status="pending",
            total_chapters=1,
            completed_chapters=0,
            target_word_count=600,
            enable_analysis=False,
            max_retries=1,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        batch_id = task.id

    generation_calls: list[dict[str, Any]] = []

    async def fake_get_engine(_user_id):
        return engine

    async def fake_check_prerequisites(*args, **kwargs):
        return True, None, None

    async def fake_generate_single_chapter_for_batch(**kwargs):
        generation_calls.append(kwargs)
        content = "batch-candidate-pass"
        return {
            "full_content": content,
            "word_count": len(content),
            "summary_preview": content,
            "quality_metrics": {
                "overall_score": 88.0,
                "conflict_chain_hit_rate": 82.0,
                "rule_grounding_hit_rate": 84.0,
                "outline_alignment_rate": 86.0,
                "dialogue_naturalness_rate": 80.0,
                "opening_hook_rate": 87.0,
                "payoff_chain_rate": 81.0,
                "cliffhanger_rate": 85.0,
                "pacing_score": 8.0,
            },
        }

    async def fake_resolve_generation_story_repair_state_for_batch(*args, **kwargs):
        return {"payload": None, "active_story_repair_payload": None}

    async def fake_sync_task_story_repair_state(*args, **kwargs):
        return {"payload": None, "active_story_repair_payload": None}

    async def fake_publish_task_stream_event(*args, **kwargs):
        return None

    async def fake_record_task_quality_metrics(*args, **kwargs):
        return None

    async def fake_sleep(_seconds):
        return None

    monkeypatch.setattr(app_database, "get_engine", fake_get_engine)
    monkeypatch.setattr(
        batch_generation_retry_service,
        "check_chapter_generation_prerequisites",
        fake_check_prerequisites,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "generate_single_chapter_for_batch",
        fake_generate_single_chapter_for_batch,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "resolve_story_repair_state_for_batch",
        fake_resolve_generation_story_repair_state_for_batch,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "sync_task_story_repair_state",
        fake_sync_task_story_repair_state,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "publish_task_stream_event_service",
        fake_publish_task_stream_event,
    )
    monkeypatch.setattr(
        batch_generation_retry_service,
        "record_task_quality_metrics",
        fake_record_task_quality_metrics,
    )
    
    await REAL_EXECUTE_BATCH_GENERATION_IN_ORDER(
        batch_id=batch_id,
        user_id=mock_user.user_id,
        ai_service=fake_ai_service,
        enable_web_research=True,
        web_research_query="night market customs",
        base_quality_profile={
            "resolved_style_id": None,
            "style_content": "",
            "style_name": "",
            "style_preset_id": "",
        },
    )

    assert generation_calls
    assert generation_calls[0]["enable_web_research"] is True
    assert generation_calls[0]["web_research_query"] == "night market customs"


async def test_execute_batch_generation_should_stop_promptly_when_task_cancelled_during_chapter_generation(
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
        title="batch-cancel-during-generation",
    )

    async with chapters_session_factory() as session:
        engine = session.bind
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=[chapter.id],
            status="pending",
            total_chapters=1,
            completed_chapters=0,
            target_word_count=600,
            enable_analysis=False,
            max_retries=1,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        batch_id = task.id

    generation_started = asyncio.Event()
    generation_cancelled = asyncio.Event()
    published_events: list[dict[str, Any]] = []

    async def fake_get_engine(_user_id):
        return engine

    async def fake_check_prerequisites(*args, **kwargs):
        return True, None, None

    async def fake_generate_single_chapter_for_batch(**kwargs):
        generation_started.set()
        try:
            await asyncio.Future()
        except asyncio.CancelledError:
            generation_cancelled.set()
            raise

    async def fake_resolve_generation_story_repair_state_for_batch(*args, **kwargs):
        return {"payload": None, "active_story_repair_payload": None}

    async def fake_sync_task_story_repair_state(*args, **kwargs):
        return {"payload": None, "active_story_repair_payload": None}

    async def fake_publish_task_stream_event(_task_id, event, db_session=None):
        published_events.append(dict(event))
        return None

    async def fake_record_task_quality_metrics(*args, **kwargs):
        return None

    monkeypatch.setattr(app_database, "get_engine", fake_get_engine)
    monkeypatch.setattr(
        batch_generation_retry_service,
        "check_chapter_generation_prerequisites",
        fake_check_prerequisites,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "generate_single_chapter_for_batch",
        fake_generate_single_chapter_for_batch,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "resolve_story_repair_state_for_batch",
        fake_resolve_generation_story_repair_state_for_batch,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "sync_task_story_repair_state",
        fake_sync_task_story_repair_state,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "publish_task_stream_event_service",
        fake_publish_task_stream_event,
    )
    monkeypatch.setattr(
        batch_generation_retry_service,
        "record_task_quality_metrics",
        fake_record_task_quality_metrics,
    )
    monkeypatch.setattr(batch_generation_run_wiring_service, "CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS", 0.01)

    execution_task = asyncio.create_task(
        REAL_EXECUTE_BATCH_GENERATION_IN_ORDER(
            batch_id=batch_id,
            user_id=mock_user.user_id,
            ai_service=fake_ai_service,
            base_quality_profile={
                "resolved_style_id": None,
                "style_content": "",
                "style_name": "",
                "style_preset_id": "",
            },
        )
    )

    await asyncio.wait_for(generation_started.wait(), timeout=1)

    async with chapters_session_factory() as session:
        saved_task = await session.get(BatchGenerationTask, batch_id)
        assert saved_task is not None
        saved_task.status = "cancelled"
        saved_task.completed_at = datetime.now()
        await session.commit()

    await asyncio.wait_for(execution_task, timeout=1)

    assert generation_cancelled.is_set()
    assert any(event.get("phase") == "cancelled" for event in published_events)

    async with chapters_session_factory() as session:
        saved_task = await session.get(BatchGenerationTask, batch_id)
        saved_chapter = await session.get(Chapter, chapter.id)
        history_result = await session.execute(
            select(GenerationHistory).where(GenerationHistory.chapter_id == chapter.id)
        )
        histories = history_result.scalars().all()

        assert saved_task is not None
        assert saved_task.status == "cancelled"
        assert saved_chapter is not None
        assert saved_chapter.content is None
        assert histories == []

async def test_execute_batch_generation_should_keep_candidate_out_of_chapter_and_history_when_quality_gate_blocks(
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
        title="batch-blocked",
    )

    async with chapters_session_factory() as session:
        engine = session.bind
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=[chapter.id],
            status="pending",
            total_chapters=1,
            completed_chapters=0,
            target_word_count=600,
            enable_analysis=False,
            max_retries=1,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        batch_id = task.id

    analysis_calls: list[dict[str, Any]] = []

    async def fake_get_engine(_user_id):
        return engine

    async def fake_check_prerequisites(*args, **kwargs):
        return True, None, None

    async def fake_generate_single_chapter_for_batch(**kwargs):
        assert kwargs["base_quality_profile"]["resolved_style_id"] is None
        content = "batch-candidate-blocked"
        return {
            "full_content": content,
            "word_count": len(content),
            "summary_preview": content,
            "quality_metrics": {
                "overall_score": 66.0,
                "conflict_chain_hit_rate": 48.0,
                "rule_grounding_hit_rate": 52.0,
                "outline_alignment_rate": 50.0,
                "dialogue_naturalness_rate": 75.0,
                "opening_hook_rate": 70.0,
                "payoff_chain_rate": 46.0,
                "cliffhanger_rate": 68.0,
                "pacing_score": 6.1,
            },
        }

    async def fake_resolve_generation_story_repair_state_for_batch(*args, **kwargs):
        return {"payload": None, "active_story_repair_payload": None}

    async def fake_sync_task_story_repair_state(*args, **kwargs):
        return {"payload": None, "active_story_repair_payload": None}

    async def fake_publish_task_stream_event(*args, **kwargs):
        return None

    async def fake_record_task_quality_metrics(*args, **kwargs):
        return None

    async def fake_analyze_chapter_background(**kwargs):
        analysis_calls.append(kwargs)
        return True

    async def fake_sleep(_seconds):
        return None

    monkeypatch.setattr(app_database, "get_engine", fake_get_engine)
    monkeypatch.setattr(
        batch_generation_retry_service,
        "check_chapter_generation_prerequisites",
        fake_check_prerequisites,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "generate_single_chapter_for_batch",
        fake_generate_single_chapter_for_batch,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "resolve_story_repair_state_for_batch",
        fake_resolve_generation_story_repair_state_for_batch,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "sync_task_story_repair_state",
        fake_sync_task_story_repair_state,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "publish_task_stream_event_service",
        fake_publish_task_stream_event,
    )
    monkeypatch.setattr(
        batch_generation_retry_service,
        "record_task_quality_metrics",
        fake_record_task_quality_metrics,
    )

    # chapters_test_support.mock_side_effect_services 会默认覆盖分析后台 seam；
    # 当前 batch runtime 已消费 run_wiring_service 本地 seam，这里继续覆盖该 owner 以保持 analysis_calls 可观测。
    monkeypatch.setattr(batch_generation_run_wiring_service, "analyze_chapter_background", fake_analyze_chapter_background)
    stream_entry_service = load_single_generation_stream_entry_module()
    monkeypatch.setattr(stream_entry_service, "execute_chapter_analysis_background", fake_analyze_chapter_background)

    # 断言批量流程确实会触发当前 wiring owner 的 analysis seam；避免未来入口漂移导致误报。
    assert batch_generation_run_wiring_service.analyze_chapter_background is not None

    await REAL_EXECUTE_BATCH_GENERATION_IN_ORDER(
        batch_id=batch_id,
        user_id=mock_user.user_id,
        ai_service=fake_ai_service,
        base_quality_profile={
            "resolved_style_id": None,
            "style_content": "",
            "style_name": "",
            "style_preset_id": "",
        },
    )

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        saved_project = await session.get(Project, project.id)
        saved_task = await session.get(BatchGenerationTask, batch_id)
        history_result = await session.execute(
            select(GenerationHistory).where(GenerationHistory.chapter_id == chapter.id)
        )
        histories = history_result.scalars().all()
        draft_attempt_result = await session.execute(
            select(ChapterDraftAttempt).where(ChapterDraftAttempt.chapter_id == chapter.id)
        )
        draft_attempts = draft_attempt_result.scalars().all()

        assert saved_chapter is not None
        assert saved_chapter.content == "batch-candidate-blocked"
        assert saved_chapter.status == "completed"
        assert saved_chapter.word_count == len("batch-candidate-blocked")
        assert saved_project is not None
        assert saved_project.current_words == len("batch-candidate-blocked")
        assert saved_task is not None
        assert saved_task.status == "completed"
        assert saved_task.completed_chapters == 1
        assert saved_task.failed_chapters == []
        assert len(histories) == 1
        assert draft_attempts == []

    assert analysis_calls
    assert analysis_calls[0]["chapter_id"] == chapter.id
    assert analysis_calls[0]["project_id"] == project.id
    assert analysis_calls[0]["task_id"]
    assert isinstance(analysis_calls[0]["story_packet"], StoryPacket)
    assert analysis_calls[0]["story_packet"].source == "batch-generation-runtime"
    assert analysis_calls[0]["chapter_content_override"] == "batch-candidate-blocked"
    assert analysis_calls[0]["chapter_word_count_override"] == len("batch-candidate-blocked")
