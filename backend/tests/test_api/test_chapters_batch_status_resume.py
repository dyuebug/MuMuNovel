from typing import Any

import pytest

from app.api import chapters as chapters_api
from app.models.batch_generation_snapshot import BatchGenerationSnapshot
from app.models.batch_generation_task import BatchGenerationTask
from app.services import batch_generation_run_wiring_service
from tests.test_api.chapters_test_support import (
    chapters_client,
    chapters_session_factory,
    create_chapter,
    create_project,
    fake_ai_service,
    mock_side_effect_services,
    reset_chapters_runtime_caches,
)

pytestmark = pytest.mark.asyncio

async def test_should_create_batch_generation_task_and_query_status(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-1",
        content="ready",
        status="completed",
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="chapter-2",
        content=None,
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="chapter-3",
        content=None,
    )

    create_response = await chapters_client.post(
        f"/api/chapters/project/{project.id}/batch-generate",
        json={
            "start_chapter_number": 2,
            "count": 2,
            "target_word_count": 500,
            "enable_analysis": False,
            "enable_mcp": False,
            "max_retries": 1,
        },
    )
    assert create_response.status_code == 200
    body = create_response.json()
    batch_id = body["batch_id"]
    assert len(body["chapters_to_generate"]) == 2

    status_response = await chapters_client.get(
        f"/api/chapters/batch-generate/{batch_id}/status"
    )
    assert status_response.status_code == 200
    status_body = status_response.json()
    assert status_body["batch_id"] == batch_id
    assert status_body["status"] == "pending"
    assert status_body["total"] == 2
    assert status_body["completed"] == 0
    assert status_body["stage_code"] == "6.writing"
    assert status_body["execution_mode"] == "interactive"
    assert status_body["checkpoint"]["current_chapter_number"] is None
    assert status_body["latest_quality_metrics"] is None
    assert status_body["quality_metrics_summary"] is None
    assert status_body["active_story_repair_payload"] is None
    assert status_body["terminal_reason"] is None
    assert status_body["terminal_label"] is None
    assert status_body["review_required"] is False
    assert status_body["can_resume"] is False

    async with chapters_session_factory() as session:
        task = await session.get(BatchGenerationTask, batch_id)
        assert task is not None
        assert task.chapter_count == 2

async def test_should_expose_manual_review_terminal_status_for_failed_batch_task(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="chapter-2",
        content=None,
    )

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=1,
            chapter_ids=[chapter.id],
            status="failed",
            total_chapters=1,
            completed_chapters=0,
            current_chapter_id=chapter.id,
            current_chapter_number=2,
            current_retry_count=0,
            max_retries=2,
            failed_chapters=[
                {
                    "chapter_id": chapter.id,
                    "chapter_number": 2,
                    "title": chapter.title,
                    "error": "quality gate blocked; manual review required",
                    "retry_count": 2,
                    "phase": "quality_blocked",
                    "quality_gate_status": "blocked",
                    "quality_gate_decision": "manual_review",
                    "quality_gate_label": "manual review",
                    "quality_gate_failed_metrics": ["Conflict chain"],
                }
            ],
            error_message="chapter 2 needs manual review",
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        task_id = task.id

    status_response = await chapters_client.get(
        f"/api/chapters/batch-generate/{task_id}/status"
    )
    assert status_response.status_code == 200
    status_body = status_response.json()
    assert status_body["batch_id"] == task_id
    assert status_body["status"] == "failed"
    assert status_body["terminal_reason"] == "manual_review"
    assert status_body["terminal_label"] == "manual review"
    assert status_body["review_required"] is True
    assert status_body["can_resume"] is False
    assert status_body["failed_chapters"][0]["quality_gate_decision"] == "manual_review"

    resume_response = await chapters_client.post(
        f"/api/chapters/batch-generate/{task_id}/resume"
    )
    assert resume_response.status_code == 400
    assert resume_response.json()["detail"] == "Manual review blocked tasks cannot be resumed"

async def test_should_expose_runtime_workflow_phase_in_batch_status(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(
        chapters_session_factory,
        user_id=mock_user.user_id,
        default_creative_mode="hook",
        default_story_focus="advance_plot",
    )

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=["chapter-1"],
            status="running",
            total_chapters=1,
            completed_chapters=0,
            current_chapter_id="chapter-1",
            current_chapter_number=1,
            current_retry_count=0,
            max_retries=3,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)

    async with chapters_api.task_workflow_lock:
        chapters_api.task_workflow_state_cache.pop(task.id, None)

    await chapters_api.publish_task_stream_event(
        task.id,
        {
            "type": "analysis_started",
            "chapter_id": "chapter-1",
            "chapter_number": 1,
            "message": "analysis started",
            "progress": 85,
            "phase": "parsing",
            "candidate_index": 2,
            "candidate_count": 2,
            "word_count": 1320,
            "generation_path": "rerank_retry",
            "attempt_kind": "rerank_candidate",
            "rerank_used": True,
            "word_budget_repair_used": False,
            "winner_candidate_index": 2,
        },
    )

    response = await chapters_client.get(f"/api/chapters/batch-generate/{task.id}/status")
    assert response.status_code == 200
    body = response.json()
    assert body["stage_code"] == "6.writing.parsing"
    assert body["checkpoint"]["progress_phase"] == "parsing"
    assert body["checkpoint"]["last_event"] == "analysis_started"
    assert body["checkpoint"]["current_chapter_number"] == 1
    assert body["checkpoint"]["candidate_index"] == 2
    assert body["checkpoint"]["candidate_count"] == 2
    assert body["checkpoint"]["word_count"] == 1320
    assert body["checkpoint"]["generation_path"] == "rerank_retry"
    assert body["checkpoint"]["attempt_kind"] == "rerank_candidate"
    assert body["checkpoint"]["rerank_used"] is True
    assert body["checkpoint"]["word_budget_repair_used"] is False
    assert body["checkpoint"]["winner_candidate_index"] == 2

async def test_should_resume_failed_batch_task_from_current_chapter(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-1",
        content="ready",
        status="completed",
    )
    chapter_2 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="chapter-2",
        content=None,
    )
    chapter_3 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="chapter-3",
        content=None,
    )

    async with chapters_session_factory() as session:
        source_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=2,
            chapter_ids=[chapter_2.id, chapter_3.id],
            status="failed",
            total_chapters=2,
            completed_chapters=0,
            current_chapter_id=chapter_2.id,
            current_chapter_number=2,
            current_retry_count=1,
            max_retries=2,
            error_message="mock failed",
        )
        session.add(source_task)
        await session.commit()
        await session.refresh(source_task)
        source_task_id = source_task.id

    response = await chapters_client.post(f"/api/chapters/batch-generate/{source_task_id}/resume")
    assert response.status_code == 200
    body = response.json()
    assert body["resumed_from_batch_id"] == source_task_id
    assert body["status"] == "pending"
    assert body["stage_code"] == "6.writing.loading"
    resumed_task_id = body["batch_id"]
    assert resumed_task_id != source_task_id

    async with chapters_session_factory() as session:
        resumed_task = await session.get(BatchGenerationTask, resumed_task_id)
        assert resumed_task is not None
        assert resumed_task.status == "pending"
        assert resumed_task.start_chapter_number == 2
        assert resumed_task.chapter_ids == [chapter_2.id, chapter_3.id]
        assert resumed_task.total_chapters == 2
        assert resumed_task.completed_chapters == 0

    async with chapters_api.task_workflow_lock:
        runtime = dict(chapters_api.task_workflow_state_cache.get(resumed_task_id) or {})
    assert runtime.get("phase") == "loading"
    assert runtime.get("resume_from_batch_id") == source_task_id

async def test_should_resume_failed_batch_task_with_persisted_story_repair_payload(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-1",
        content="ready",
        status="completed",
    )
    chapter_2 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="chapter-2",
        content=None,
    )
    chapter_3 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="chapter-3",
        content=None,
    )

    active_story_repair_payload = {
        "summary": "Recover the main conflict payoff",
        "repair_targets": ["restore mainline payoff", "raise climax cost"],
        "preserve_strengths": ["keep character voice"],
        "quality_gate_decision": "auto_repair",
        "quality_gate_label": "auto repair",
        "source": "manual_plus_recent_history_summary",
        "scope": "batch",
    }

    async with chapters_session_factory() as session:
        source_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=2,
            chapter_ids=[chapter_2.id, chapter_3.id],
            status="failed",
            total_chapters=2,
            completed_chapters=0,
            current_chapter_id=chapter_2.id,
            current_chapter_number=2,
            current_retry_count=1,
            max_retries=2,
            error_message="quality gate blocked",
        )
        session.add(source_task)
        await session.flush()
        session.add(
            BatchGenerationSnapshot(
                batch_task_id=source_task.id,
                workflow_runtime_state={
                    "active_story_repair_payload": dict(active_story_repair_payload),
                },
            )
        )
        await session.commit()
        source_task_id = source_task.id

    async with chapters_api.task_workflow_lock:
        chapters_api.task_workflow_state_cache.pop(source_task_id, None)

    response = await chapters_client.post(f"/api/chapters/batch-generate/{source_task_id}/resume")
    assert response.status_code == 200
    body = response.json()
    resumed_task_id = body["batch_id"]

    assert captured["story_repair_summary"] == active_story_repair_payload["summary"]
    assert captured["story_repair_targets"] == active_story_repair_payload["repair_targets"]
    assert captured["story_preserve_strengths"] == active_story_repair_payload["preserve_strengths"]
    captured_payload = captured["story_repair_payload"]
    assert isinstance(captured_payload, chapters_api.StoryRepairPayload)
    assert captured_payload.summary == active_story_repair_payload["summary"]
    assert list(captured_payload.targets) == active_story_repair_payload["repair_targets"]
    assert list(captured_payload.strengths) == active_story_repair_payload["preserve_strengths"]

    async with chapters_api.task_workflow_lock:
        runtime = dict(chapters_api.task_workflow_state_cache.get(resumed_task_id) or {})
    assert runtime["active_story_repair_payload"]["summary"] == active_story_repair_payload["summary"]
    assert runtime["active_story_repair_payload"]["repair_targets"] == active_story_repair_payload["repair_targets"]
    assert runtime["active_story_repair_payload"]["preserve_strengths"] == active_story_repair_payload["preserve_strengths"]

async def test_should_resume_cancelled_task_from_completed_checkpoint_when_current_missing(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-1",
        content="ready-1",
        status="completed",
    )
    chapter_2 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="chapter-2",
        content="ready-2",
        status="completed",
    )
    chapter_3 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="chapter-3",
        content=None,
    )
    chapter_4 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=4,
        title="chapter-4",
        content=None,
    )

    async with chapters_session_factory() as session:
        source_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=3,
            chapter_ids=[chapter_2.id, chapter_3.id, chapter_4.id],
            status="cancelled",
            total_chapters=3,
            completed_chapters=1,
            current_chapter_id="missing-chapter-id",
            current_chapter_number=3,
            current_retry_count=0,
            max_retries=3,
        )
        session.add(source_task)
        await session.commit()
        await session.refresh(source_task)
        source_task_id = source_task.id

    response = await chapters_client.post(f"/api/chapters/batch-generate/{source_task_id}/resume")
    assert response.status_code == 200
    body = response.json()
    resumed_task_id = body["batch_id"]
    assert body["resumed_from_batch_id"] == source_task_id
    assert body["task_type"] == "chapters_batch_generate"
    assert body["checkpoint"]["current_chapter_number"] == 3

    async with chapters_session_factory() as session:
        resumed_task = await session.get(BatchGenerationTask, resumed_task_id)
        assert resumed_task is not None
        assert resumed_task.chapter_ids == [chapter_3.id, chapter_4.id]
        assert resumed_task.start_chapter_number == 3
        assert resumed_task.total_chapters == 2

async def test_should_reject_resume_when_batch_task_not_terminal(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-1",
        content="ready",
        status="completed",
    )
    chapter_2 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="chapter-2",
        content=None,
    )

    async with chapters_session_factory() as session:
        source_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=1,
            chapter_ids=[chapter_2.id],
            status="running",
            total_chapters=1,
            completed_chapters=0,
        )
        session.add(source_task)
        await session.commit()
        await session.refresh(source_task)
        source_task_id = source_task.id

    response = await chapters_client.post(f"/api/chapters/batch-generate/{source_task_id}/resume")
    assert response.status_code == 400
    assert "Only failed or cancelled tasks can be resumed" in response.json()["detail"]

async def test_should_list_active_batch_generation_tasks_for_current_user(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    other_project = await create_project(chapters_session_factory, user_id="other-user", title="鍏朵粬椤圭洰")

    async with chapters_session_factory() as session:
        user_single_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=["chapter-1"],
            status="pending",
            total_chapters=1,
            completed_chapters=0,
        )
        user_batch_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=3,
            chapter_ids=["chapter-2", "chapter-3", "chapter-4"],
            status="running",
            total_chapters=3,
            completed_chapters=1,
        )
        user_completed_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=5,
            chapter_count=2,
            chapter_ids=["chapter-5", "chapter-6"],
            status="completed",
            total_chapters=2,
            completed_chapters=2,
        )
        other_user_task = BatchGenerationTask(
            project_id=other_project.id,
            user_id="other-user",
            start_chapter_number=1,
            chapter_count=2,
            chapter_ids=["x-1", "x-2"],
            status="running",
            total_chapters=2,
            completed_chapters=0,
        )
        session.add_all([user_single_task, user_batch_task, user_completed_task, other_user_task])
        await session.commit()
        await session.refresh(user_single_task)
        await session.refresh(user_batch_task)

    response = await chapters_client.get("/api/chapters/batch-generate/active-tasks?limit=10")
    assert response.status_code == 200
    body = response.json()
    assert body["total"] == 2

    items_by_id = {item["batch_id"]: item for item in body["items"]}
    assert set(items_by_id.keys()) == {user_single_task.id, user_batch_task.id}
    assert items_by_id[user_single_task.id]["task_type"] == "chapter_single_generate"
    assert items_by_id[user_batch_task.id]["task_type"] == "chapters_batch_generate"
    assert items_by_id[user_batch_task.id]["stage_code"] == "6.writing.loading"
    assert items_by_id[user_batch_task.id]["execution_mode"] == "interactive"
    assert items_by_id[user_batch_task.id]["checkpoint"]["current_chapter_number"] is None
    assert items_by_id[user_batch_task.id]["active_story_repair_payload"] is None
    assert items_by_id[user_batch_task.id]["project_id"] == project.id

async def test_should_require_login_when_listing_active_batch_generation_tasks(
    chapters_client,
):
    response = await chapters_client.get(
        "/api/chapters/batch-generate/active-tasks",
        headers={"x-test-user-id": "__none__"},
    )
    assert response.status_code == 401

async def test_should_return_404_when_batch_status_task_missing(chapters_client):
    response = await chapters_client.get("/api/chapters/batch-generate/missing/status")
    assert response.status_code == 404


async def test_should_return_404_when_cancelling_missing_batch_task(chapters_client):
    response = await chapters_client.post("/api/chapters/batch-generate/missing/cancel")
    assert response.status_code == 404
    assert response.json()["detail"] == "Batch generation task not found"


async def test_should_return_400_when_cancelling_terminal_batch_task(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=["chapter-1"],
            status="failed",
            total_chapters=1,
            completed_chapters=0,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        task_id = task.id

    response = await chapters_client.post(
        f"/api/chapters/batch-generate/{task_id}/cancel"
    )
    assert response.status_code == 400
    assert response.json()["detail"] == "Cannot cancel task in status failed"


async def test_should_cancel_running_batch_generation_task(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=2,
            chapter_ids=["chapter-2", "chapter-3"],
            status="running",
            total_chapters=2,
            completed_chapters=1,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        task_id = task.id

    response = await chapters_client.post(
        f"/api/chapters/batch-generate/{task_id}/cancel"
    )
    assert response.status_code == 200
    body = response.json()
    assert body["message"] == "Batch generation cancelled"
    assert body["batch_id"] == task_id
    assert body["completed_chapters"] == 1
    assert body["total_chapters"] == 2

    async with chapters_session_factory() as session:
        saved_task = await session.get(BatchGenerationTask, task_id)
        assert saved_task is not None
        assert saved_task.status == "cancelled"
        assert saved_task.completed_at is not None
