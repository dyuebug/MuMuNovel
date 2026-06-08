import asyncio
from typing import Any

from types import SimpleNamespace

import pytest

from app.api import chapters as chapters_api
from app.api import chapter_analysis_task_routes as chapter_analysis_task_routes_api
from app.services.chapter_quality_context_service import StoryPacket
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

async def test_should_return_empty_batch_analysis_status_when_chapter_ids_missing(
):
    response = await chapter_analysis_task_routes_api.get_batch_analysis_task_status(
        SimpleNamespace(chapter_ids=None),
        request=None,  # type: ignore[arg-type]
        db=None,  # type: ignore[arg-type]
    )
    assert response == {
        "project_id": "",
        "total": 0,
        "items": {},
    }

async def test_should_trigger_manual_analysis_task_creation(
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
        title="手动分析章节",
        content="正文存在，可分析。",
        status="completed",
    )

    calls: list[dict[str, Any]] = []

    async def fake_analyze_chapter_background(**kwargs):
        calls.append(kwargs)
        return True

    async def fake_sleep(_seconds):
        return None

    monkeypatch.setattr(
        chapter_analysis_task_routes_api,
        "execute_chapter_analysis_background",
        fake_analyze_chapter_background,
    )
    monkeypatch.setattr(asyncio, "sleep", fake_sleep)

    response = await chapters_client.post(f"/api/chapters/{chapter.id}/analyze")
    assert response.status_code == 200
    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["status"] == "pending"
    assert body["task_id"]

    assert calls
    assert calls[0]["chapter_id"] == chapter.id
    assert calls[0]["project_id"] == project.id
    assert calls[0]["task_id"] == body["task_id"]
    assert isinstance(calls[0]["story_packet"], StoryPacket)
    assert calls[0]["story_packet"].source == "manual-analysis-request"

async def test_should_delegate_trigger_analysis_route_to_compat_service(
    chapters_client,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_trigger_chapter_analysis_with_default_route_wiring(**kwargs):
        captured.update(kwargs)
        return {
            "task_id": "delegated-task",
            "chapter_id": kwargs["chapter_id"],
            "status": "pending",
            "message": "ok",
        }

    monkeypatch.setattr(
        chapter_analysis_task_routes_api,
        "trigger_chapter_analysis_with_default_route_wiring",
        fake_trigger_chapter_analysis_with_default_route_wiring,
    )

    response = await chapters_client.post('/api/chapters/delegated-analysis/analyze')

    assert response.status_code == 200
    assert response.json()["chapter_id"] == "delegated-analysis"
    assert captured["chapter_id"] == "delegated-analysis"
    assert captured["request"] is not None
    assert captured["background_tasks"] is not None
    assert captured["db_session"] is not None
    assert captured["user_ai_service"] is not None


async def test_should_delegate_analysis_status_route_to_compat_service(
    chapters_client,
    monkeypatch,
):
    captured: dict[str, object] = {}

    async def fake_get_analysis_task_status_with_default_route_wiring(**kwargs):
        captured.update(kwargs)
        return {
            "chapter_id": kwargs["chapter_id"],
            "has_task": False,
            "status": "none",
            "task_id": None,
        }

    monkeypatch.setattr(
        chapter_analysis_task_routes_api,
        "get_analysis_task_status_with_default_route_wiring",
        fake_get_analysis_task_status_with_default_route_wiring,
    )

    response = await chapters_client.get('/api/chapters/delegated-status/analysis/status')

    assert response.status_code == 200
    assert response.json()["chapter_id"] == "delegated-status"
    assert captured["chapter_id"] == "delegated-status"
    assert captured["request"] is not None
    assert captured["db_session"] is not None


async def test_should_delegate_can_generate_route_to_compat_service(
    chapters_client,
    monkeypatch,
):
    captured: dict[str, object] = {}

    async def fake_check_can_generate_with_default_route_wiring(**kwargs):
        captured.update(kwargs)
        return {
            "can_generate": True,
            "reason": "",
            "previous_chapters": [],
            "chapter_number": 9,
        }

    monkeypatch.setattr(
        chapter_analysis_task_routes_api,
        "check_can_generate_with_default_route_wiring",
        fake_check_can_generate_with_default_route_wiring,
    )

    response = await chapters_client.get('/api/chapters/delegated-generate/can-generate')

    assert response.status_code == 200
    assert response.json()["can_generate"] is True
    assert response.json()["chapter_number"] == 9
    assert captured["chapter_id"] == "delegated-generate"
    assert captured["request"] is not None
    assert captured["db_session"] is not None


async def test_should_delegate_batch_analysis_status_route_to_compat_service(
    chapters_client,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_get_batch_analysis_task_status_with_default_route_wiring(**kwargs):
        captured.update(kwargs)
        return {
            "project_id": "project-1",
            "total": 1,
            "items": {"chapter-1": {"status": "running"}},
        }

    monkeypatch.setattr(
        chapter_analysis_task_routes_api,
        "get_batch_analysis_task_status_with_default_route_wiring",
        fake_get_batch_analysis_task_status_with_default_route_wiring,
    )

    response = await chapters_client.post(
        '/api/chapters/analysis/status/batch',
        json={"chapter_ids": ["chapter-1"]},
    )

    assert response.status_code == 200
    assert response.json()["project_id"] == "project-1"
    assert response.json()["total"] == 1
    assert captured["chapter_ids_input"] == ["chapter-1"]
    assert captured["request"] is not None
    assert captured["db_session"] is not None


async def test_should_return_404_when_trigger_manual_analysis_for_foreign_project(
    chapters_client,
    chapters_session_factory,
):
    foreign_project = await create_project(
        chapters_session_factory,
        user_id="another-user",
        title="他人分析项目",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=foreign_project.id,
        chapter_number=1,
        title="他人章节",
        content="正文存在，可分析。",
        status="completed",
    )

    response = await chapters_client.post(f"/api/chapters/{chapter.id}/analyze")
    assert response.status_code == 404
